use crate::choke;
use crate::config::SimConfig;
use crate::diagnostics::{self, FrameDiagnostics};
use crate::field;
use crate::foam;
use crate::light::{propagate_all_with_stats, AttractionWave};
use crate::metrics::FrameMetrics;
use crate::parcel::Parcel;
use std::collections::{HashMap, HashSet};

/// # Simulation Loop — All-Pairs Field Interactions
///
/// One tick of the STE universe.  No bonds, no graph.  Every parcel
/// interacts with every other parcel through the ever-present field.
/// At n=200 this is 20,000 pairs per tick — trivial for WASM.
///
/// Order matters:
///
/// 1. Estimate concentration from spatial density
/// 2. Estimate vorticity from velocity curl
/// 3. STE diffusion (heat conduction)
/// 4. All-pairs attraction forces
/// 5. Viscous velocity damping
/// 6. Detect & advance chokes
/// 7. Foam: bubble detachment + annihilation
/// 8. Propagate attraction waves (light)
/// 9. Integrate positions (CFL-clamped)
/// 10. Record metrics

/// The complete simulation state.
pub struct Sim {
    pub parcels: Vec<Parcel>,
    pub waves: Vec<AttractionWave>,
    pub config: SimConfig,
    pub tick: u64,
    pub next_wave_id: u64,
    pub last_diagnostics: Option<FrameDiagnostics>,
    pub compound_pair_dwell_ticks: HashMap<(u64, u64), u64>,
    pub chirality_prev_sign: i8,
    pub chirality_zero_crossings: u64,
    pub chirality_lock_ticks: u64,
}

impl Sim {
    /// Create a new simulation from initial parcels.
    pub fn new(parcels: Vec<Parcel>, config: SimConfig) -> Self {
        Self {
            parcels,
            waves: Vec::new(),
            config,
            tick: 0,
            next_wave_id: 1,
            last_diagnostics: None,
            compound_pair_dwell_ticks: HashMap::new(),
            chirality_prev_sign: 0,
            chirality_zero_crossings: 0,
            chirality_lock_ticks: 0,
        }
    }

    /// Advance the simulation by one timestep.
    pub fn step(&mut self) -> FrameMetrics {
        let n = self.parcels.len();

        // ── 1. Estimate concentration from spatial density ──
        field::estimate_concentrations(&mut self.parcels, &self.config);

        // ── 2. Estimate vorticity from velocity curl ──
        field::estimate_vorticities(&mut self.parcels, &self.config);

        // ── 3. STE diffusion (heat conduction through the field) ──
        field::apply_diffusion(&mut self.parcels, &self.config);

        // ── 4. All-pairs attraction forces ──
        field::apply_forces(&mut self.parcels, &self.config);

        // ── 5. Viscous velocity damping ──
        field::apply_viscosity(&mut self.parcels, &self.config);

        // ── 6. Detect & advance chokes ──
        // Pre-compute local averages: each parcel's neighbourhood
        // concentration and vorticity.  Depression and rotation are
        // RELATIVE to neighbours — no absolute thresholds.
        let local_avg_conc: Vec<f64> = (0..n).map(|i| {
            let mut sum = 0.0;
            let mut count = 0usize;
            for j in 0..n {
                if j == i { continue; }
                let d = self.parcels[i].dist_to(&self.parcels[j]);
                if d < self.config.shell_interaction_range * 5.0 {
                    sum += self.parcels[j].concentration;
                    count += 1;
                }
            }
            if count > 0 { sum / count as f64 } else { self.parcels[i].concentration }
        }).collect();
        let local_avg_vort: Vec<f64> = (0..n).map(|i| {
            let mut sum = 0.0;
            let mut count = 0usize;
            for j in 0..n {
                if j == i { continue; }
                let d = self.parcels[i].dist_to(&self.parcels[j]);
                if d < self.config.shell_interaction_range * 5.0 {
                    sum += self.parcels[j].vorticity;
                    count += 1;
                }
            }
            if count > 0 { sum / count as f64 } else { self.parcels[i].vorticity }
        }).collect();
        let local_avg_vel: Vec<[f64; 3]> = (0..n).map(|i| {
            let mut sx = 0.0;
            let mut sy = 0.0;
            let mut sz = 0.0;
            let mut count = 0usize;
            for j in 0..n {
                if j == i { continue; }
                let d = self.parcels[i].dist_to(&self.parcels[j]);
                if d < self.config.shell_interaction_range * 5.0 {
                    sx += self.parcels[j].vx;
                    sy += self.parcels[j].vy;
                    sz += self.parcels[j].vz;
                    count += 1;
                }
            }
            if count > 0 {
                [sx / count as f64, sy / count as f64, sz / count as f64]
            } else {
                [self.parcels[i].vx, self.parcels[i].vy, self.parcels[i].vz]
            }
        }).collect();

        for i in 0..n {
            if choke::should_form_choke(
                &self.parcels[i],
                local_avg_conc[i],
                local_avg_vort[i],
                local_avg_vel[i],
            ) {
                choke::form_choke(&mut self.parcels[i]);
            }
        }
        for i in 0..n {
            if self.parcels[i].is_choked() {
                // Ambient should represent the exterior shell environment,
                // not the local core concentration itself.
                let ambient = local_avg_conc[i];
                let radius = choke::estimate_radius(
                    i,
                    &self.parcels,
                );
                choke::advance_choke(
                    &mut self.parcels[i],
                    ambient,
                    radius,
                    &self.config,
                );
            }
        }

        // ── 7a. Foam: bubble detachment ──
        let detachments = foam::find_detachments(&self.parcels, &self.config);
        let foam_spawn_count = detachments.len();
        for (parent_idx, ste, spin) in detachments {
            let new_id = self.parcels.len() as u64 + 1000;

            let daughter = foam::spawn_bubble(
                &mut self.parcels[parent_idx],
                ste,
                spin,
                new_id,
            );

            self.parcels.push(daughter);
        }

        // ── 7b. Foam: annihilation ──
        let annihilations = foam::find_annihilations(&self.parcels, &self.config);
        let foam_annihilation_count = annihilations.len();
        let mut annihilation_waves: Vec<(usize, f64)> = Vec::new();
        let mut annihilation_energy_this_tick = 0.0_f64;
        for (a, b) in annihilations {
            let result = foam::annihilate(&mut self.parcels, a, b, &self.config);
            annihilation_energy_this_tick += result.energy.max(0.0);

            if matches!(result.class,
                foam::AnnihilationClass::Light
                | foam::AnnihilationClass::Gamma
                | foam::AnnihilationClass::Cavitation
            ) {
                let wave_energy = result.energy * match result.class {
                    foam::AnnihilationClass::Light => 0.4,
                    foam::AnnihilationClass::Gamma => 0.7,
                    foam::AnnihilationClass::Cavitation => 0.9,
                    _ => 0.0,
                };
                annihilation_waves.push((a, wave_energy * 0.5));
                annihilation_waves.push((b, wave_energy * 0.5));
                for &r in &result.recipients {
                    annihilation_waves.push((r, wave_energy * 0.1));
                }
            }
        }
        for (source, energy) in annihilation_waves {
            if source < self.parcels.len() && energy > 0.001 {
                let freq = energy.sqrt();
                self.emit_wave(source, energy, freq);
            }
        }

        // ── 8. Propagate attraction waves (light) ──
        let wave_stats = propagate_all_with_stats(
            &mut self.waves,
            &mut self.parcels,
            &self.config,
        );

        // ── 9. Integrate positions (CFL-clamped) ──
        field::integrate_positions(&mut self.parcels, &self.config);

        // ── 10. Record metrics ──
        let total_ste: f64 = self.parcels.iter().map(|p| p.ste_amount).sum();
        let choke_count = self.parcels.iter().filter(|p| p.is_choked()).count();

        // Compound coexistence diagnostics:
        // opposite-spin choke pairs in annihilation range + dwell tracking.
        let pair_range = self.config.shell_interaction_range * self.config.foam_annihilation_range;
        let pair_range2 = pair_range * pair_range;
        let (_, pressures) = field::saturation_pressure_state(&self.parcels, &self.config);

        let mut current_pairs: Vec<((u64, u64), f64)> = Vec::new();
        for i in 0..self.parcels.len() {
            let ci = match &self.parcels[i].choke {
                Some(c) => c,
                None => continue,
            };
            for j in (i + 1)..self.parcels.len() {
                let cj = match &self.parcels[j].choke {
                    Some(c) => c,
                    None => continue,
                };

                // Only matter/anti compound candidates.
                if ci.spin * cj.spin >= 0.0 {
                    continue;
                }

                let dx = self.parcels[j].x - self.parcels[i].x;
                let dy = self.parcels[j].y - self.parcels[i].y;
                let dz = self.parcels[j].z - self.parcels[i].z;
                let d2 = dx * dx + dy * dy + dz * dz;
                if d2 > pair_range2 {
                    continue;
                }

                let a = self.parcels[i].id.min(self.parcels[j].id);
                let b = self.parcels[i].id.max(self.parcels[j].id);

                // Compound potential proxy: coherence product weighted by local pressure.
                let local_pressure = 0.5 * (pressures[i] + pressures[j]);
                let potential = ci.coherence.max(0.0) * cj.coherence.max(0.0) * local_pressure.max(0.0);
                current_pairs.push(((a, b), potential));
            }
        }

        let current_keys: HashSet<(u64, u64)> = current_pairs.iter().map(|(k, _)| *k).collect();
        self.compound_pair_dwell_ticks.retain(|k, _| current_keys.contains(k));
        for (k, _) in &current_pairs {
            let entry = self.compound_pair_dwell_ticks.entry(*k).or_insert(0);
            *entry += 1;
        }

        let mut compound_dwell_sum = 0.0_f64;
        for (k, _) in &current_pairs {
            if let Some(t) = self.compound_pair_dwell_ticks.get(k) {
                compound_dwell_sum += *t as f64;
            }
        }
        let compound_pair_count = current_pairs.len();
        let compound_dwell_mean_ticks = if compound_pair_count > 0 {
            compound_dwell_sum / compound_pair_count as f64
        } else {
            0.0
        };
        let compound_potential_sum: f64 = current_pairs.iter().map(|(_, p)| *p).sum();
        let compound_potential_mean = if compound_pair_count > 0 {
            compound_potential_sum / compound_pair_count as f64
        } else {
            0.0
        };

        // Chirality oscillator diagnostics: balance and lock-in tracking.
        let mut matter_count = 0usize;
        let mut anti_count = 0usize;
        for p in &self.parcels {
            if let Some(c) = &p.choke {
                if c.spin >= 0.0 {
                    matter_count += 1;
                } else {
                    anti_count += 1;
                }
            }
        }
        let total_chiral = matter_count + anti_count;
        let chirality = if total_chiral > 0 {
            (matter_count as f64 - anti_count as f64) / total_chiral as f64
        } else {
            0.0
        };
        let chirality_abs = chirality.abs();
        let current_sign: i8 = if chirality > 1e-12 {
            1
        } else if chirality < -1e-12 {
            -1
        } else {
            0
        };

        if current_sign != 0 {
            if self.chirality_prev_sign != 0 && self.chirality_prev_sign != current_sign {
                self.chirality_zero_crossings += 1;
                self.chirality_lock_ticks = 1;
            } else {
                self.chirality_lock_ticks += 1;
            }
            self.chirality_prev_sign = current_sign;
        } else {
            self.chirality_lock_ticks = 0;
        }

        // Per-tick diagnostics ledger and force residuals.
        let mut diag = diagnostics::compute_frame_diagnostics(
            &self.parcels,
            &self.config,
            self.tick + 1,
        );
        diag.compound_pair_count = compound_pair_count;
        diag.compound_dwell_mean_ticks = compound_dwell_mean_ticks;
        diag.compound_potential_sum = compound_potential_sum;
        diag.compound_potential_mean = compound_potential_mean;
        diag.annihilation_energy_this_tick = annihilation_energy_this_tick;
        diag.matter_count = matter_count;
        diag.anti_count = anti_count;
        diag.chirality = chirality;
        diag.chirality_abs = chirality_abs;
        diag.chirality_zero_crossings = self.chirality_zero_crossings;
        diag.chirality_lock_ticks = self.chirality_lock_ticks;
        diag.relay_collapse_count = wave_stats.relay_collapse_count;
        diag.transient_relay_collapses = wave_stats.transient_relay_collapses;
        diag.terminal_capture_collapses = wave_stats.terminal_capture_collapses;
        diag.relay_shell_overshoot_sum = wave_stats.relay_shell_overshoot_sum;
        diag.relay_shell_overshoot_mean = wave_stats.relay_shell_overshoot_sum
            / wave_stats.relay_collapse_count.max(1) as f64;
        diag.relay_relaxation_ticks_sum = wave_stats.relay_relaxation_ticks_sum;
        diag.relay_relaxation_ticks_mean = wave_stats.relay_relaxation_ticks_sum as f64
            / wave_stats.relay_collapse_count.max(1) as f64;
        self.last_diagnostics = Some(diag);

        self.tick += 1;

        FrameMetrics {
            tick: self.tick,
            parcel_count: self.parcels.len(),
            total_ste,
            choke_count,
            active_waves: self.waves.len(),
            captures_this_tick: wave_stats.captures.len(),
            foam_spawns: foam_spawn_count,
            foam_annihilations: foam_annihilation_count,
        }
    }

    /// Emit a new attraction wave from a parcel.
    pub fn emit_wave(&mut self, source_node: usize, energy: f64, frequency: f64) {
        let id = self.next_wave_id;
        self.next_wave_id += 1;
        self.waves.push(AttractionWave::emit(id, source_node, energy, frequency));
    }
}
