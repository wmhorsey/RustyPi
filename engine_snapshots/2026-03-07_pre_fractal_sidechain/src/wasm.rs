//! # WASM Bridge — All-Pairs Field Model
//!
//! Thin wrapper exposing the simulation to JavaScript via wasm-bindgen.
//! Positions live on parcels.  No embed layer.  Visual bonds are
//! computed per-frame as K-nearest neighbours (rendering only).

#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "wasm")]
use crate::config::SimConfig;
#[cfg(feature = "wasm")]
use crate::field;
#[cfg(feature = "wasm")]
use crate::parcel::Parcel;
#[cfg(feature = "wasm")]
use crate::sim::Sim;

/// Diagnostic snapshot of a parcel at escape time.
#[cfg(feature = "wasm")]
#[derive(Clone, Debug)]
struct EscapeEvent {
    tick: u64,
    index: usize,
    ste: f64,
    concentration: f64,
    vorticity: f64,
    was_choked: bool,
    choke_phase: String,
    spin: f64,
    radius: f64, // distance from centroid at cull time
}

/// JS-visible simulation handle.
#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub struct WasmSim {
    inner: Sim,
    /// Per-node visual rendering K for bond display.
    visual_k: usize,
    /// Cumulative escape counters.
    escaped_count: u32,
    escaped_ste: f64,
    escaped_choked: u32,
    /// Rich per-event escape log.
    escape_events: Vec<EscapeEvent>,
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
impl WasmSim {
    /// Create a cluster of `n` parcels scattered randomly.
    #[wasm_bindgen(constructor)]
    pub fn new(n: usize) -> Self {
        console_error_panic_hook::set_once();

        let rng = |state: &mut u64| -> f64 {
            *state ^= *state << 13;
            *state ^= *state >> 7;
            *state ^= *state << 17;
            (*state as f64) / (u64::MAX as f64)
        };
        let mut seed: u64 = 42;

        let pi = std::f64::consts::PI;
        let spread = (n as f64).sqrt() * 1.2;
        let mut parcels: Vec<Parcel> = (0..n)
            .map(|i| {
                let ste = 0.5 + rng(&mut seed) * 1.5;
                let mut p = Parcel::new(i as u64, ste);
                let r = rng(&mut seed).sqrt() * spread;
                let theta = rng(&mut seed) * 2.0 * pi;
                p.x = r * theta.cos();
                p.y = r * theta.sin();
                p.z = 0.0;
                p
            })
            .collect();

        // Seed a few parcels with higher initial STE
        for _ in 0..n.min(5) {
            let idx = (rng(&mut seed) * n as f64) as usize % n;
            parcels[idx].ste_amount = 2.0 + rng(&mut seed) * 3.0;
        }

        let mut config = SimConfig::default();
        config.dt = 0.005;

        Self {
            inner: Sim::new(parcels, config),
            visual_k: 6,
            escaped_count: 0,
            escaped_ste: 0.0,
            escaped_choked: 0,
            escape_events: Vec::new(),
        }
    }

    /// Advance physics by one tick.  Returns JSON metrics string.
    pub fn step(&mut self) -> String {
        let metrics = self.inner.step();

        // ── Escape cull: remove parcels that drifted beyond the field ──
        // Distance-from-centroid based (no bonds needed).
        self.escape_cull();

        serde_json::to_string(&metrics).unwrap_or_default()
    }

    pub fn tick(&self) -> u64 {
        self.inner.tick
    }

    pub fn parcel_count(&self) -> usize {
        self.inner.parcels.len()
    }

    /// Flat f64 array: [x0, y0, x1, y1, ...] for 2D canvas rendering.
    pub fn positions_2d(&self) -> Vec<f64> {
        let mut out = Vec::with_capacity(self.inner.parcels.len() * 2);
        for p in &self.inner.parcels {
            out.push(p.x);
            out.push(p.y);
        }
        out
    }

    /// Flat f64 array: [x0, y0, z0, x1, y1, z1, ...] for 3D rendering.
    pub fn positions_3d(&self) -> Vec<f64> {
        let mut out = Vec::with_capacity(self.inner.parcels.len() * 3);
        for p in &self.inner.parcels {
            out.push(p.x);
            out.push(p.y);
            out.push(p.z);
        }
        out
    }

    /// Flat array of visual bond pairs: [from0, to0, from1, to1, ...].
    /// These are K-nearest neighbours — rendering only, not physics.
    pub fn bond_pairs(&self) -> Vec<usize> {
        let (pairs, _) = field::visual_bonds(
            &self.inner.parcels,
            &self.inner.config,
            self.visual_k,
        );
        pairs
    }

    /// Per-bond tension values matching bond_pairs() order.
    pub fn bond_tensions(&self) -> Vec<f64> {
        let (_, tensions) = field::visual_bonds(
            &self.inner.parcels,
            &self.inner.config,
            self.visual_k,
        );
        tensions
    }

    /// Per-parcel state flags: 0 = free, 1 = choked, 2 = void.
    pub fn parcel_states(&self) -> Vec<u8> {
        self.inner.parcels.iter().map(|p| {
            if p.is_void(self.inner.config.void_threshold) {
                2
            } else if p.is_choked() {
                1
            } else {
                0
            }
        }).collect()
    }

    /// Per-parcel concentration.
    pub fn concentrations(&self) -> Vec<f64> {
        self.inner.parcels.iter().map(|p| p.concentration).collect()
    }

    /// Per-parcel local saturation index S = C / <C_neighbourhood>.
    pub fn saturation_indices(&self) -> Vec<f64> {
        let (saturations, _) = field::saturation_pressure_state(&self.inner.parcels, &self.inner.config);
        saturations
    }

    /// Per-parcel pressure potential derived from local saturation.
    pub fn pressure_potentials(&self) -> Vec<f64> {
        let (_, pressures) = field::saturation_pressure_state(&self.inner.parcels, &self.inner.config);
        pressures
    }

    /// Per-parcel radius (STE sphere size).
    pub fn parcel_radii(&self) -> Vec<f64> {
        self.inner.parcels.iter().map(|p| p.radius).collect()
    }

    pub fn emit_wave(&mut self, source: usize, energy: f64, freq: f64) {
        self.inner.emit_wave(source, energy, freq);
    }

    pub fn wave_count(&self) -> usize {
        self.inner.waves.len()
    }

    /// Wave positions as flat [x, y, z, ...].
    pub fn wave_positions_3d(&self) -> Vec<f64> {
        let mut out = Vec::with_capacity(self.inner.waves.len() * 3);
        for w in &self.inner.waves {
            if w.current_node < self.inner.parcels.len() {
                let p = &self.inner.parcels[w.current_node];
                out.push(p.x);
                out.push(p.y);
                out.push(p.z);
            }
        }
        out
    }

    pub fn wave_positions_2d(&self) -> Vec<f64> {
        let mut out = Vec::with_capacity(self.inner.waves.len() * 2);
        for w in &self.inner.waves {
            if w.current_node < self.inner.parcels.len() {
                let p = &self.inner.parcels[w.current_node];
                out.push(p.x);
                out.push(p.y);
            }
        }
        out
    }

    /// Per-parcel STE amounts.
    pub fn ste_amounts(&self) -> Vec<f64> {
        self.inner.parcels.iter().map(|p| p.ste_amount).collect()
    }

    /// Per-parcel effective mass (= STE amount).
    pub fn effective_masses(&self) -> Vec<f64> {
        self.inner.parcels.iter().map(|p| p.ste_amount.max(1e-10)).collect()
    }

    pub fn set_parcel_ste(&mut self, index: usize, amount: f64) {
        if index < self.inner.parcels.len() {
            self.inner.parcels[index].ste_amount = amount;
        }
    }

    /// Add a new parcel near the centroid.
    pub fn add_parcel(&mut self, ste_amount: f64, _connect_to: usize) -> usize {
        let n = self.inner.parcels.len();
        let new_id = n as u64;
        let mut new_parcel = Parcel::new(new_id, ste_amount);

        // Place near the centroid
        let (cx, cy, cz) = self.centroid();
        new_parcel.x = cx + 0.3;
        new_parcel.y = cy + 0.3;
        new_parcel.z = cz + 0.3;

        self.inner.parcels.push(new_parcel);
        n
    }

    /// Per-parcel role: 0=field, 1=D, 2=U.
    pub fn parcel_roles(&self) -> Vec<u8> {
        self.inner.parcels.iter().map(|p| p.role).collect()
    }

    pub fn parcel_spins(&self) -> Vec<f64> {
        self.inner.parcels.iter().map(|p| p.spin).collect()
    }

    pub fn parcel_choke_states(&self) -> Vec<u8> {
        self.inner.parcels.iter().map(|p| {
            match &p.choke {
                None => 0,
                Some(c) if c.spin >= 0.0 => 1,
                Some(_) => 2,
            }
        }).collect()
    }

    pub fn parcel_choke_phases(&self) -> Vec<u8> {
        use crate::parcel::ChokePhase;
        self.inner.parcels.iter().map(|p| {
            match &p.choke {
                None => 0,
                Some(c) => match c.phase {
                    ChokePhase::Formation => 1,
                    ChokePhase::LiftOff => 2,
                    ChokePhase::Coherence => 3,
                    ChokePhase::Drift => 4,
                    ChokePhase::Dissolution => 5,
                },
            }
        }).collect()
    }

    pub fn parcel_coherences(&self) -> Vec<f64> {
        self.inner.parcels.iter().map(|p| {
            p.choke.as_ref().map_or(0.0, |c| c.coherence)
        }).collect()
    }

    pub fn escaped_count(&self) -> u32 {
        self.escaped_count
    }

    pub fn escaped_ste(&self) -> f64 {
        self.escaped_ste
    }

    pub fn escaped_choked(&self) -> u32 {
        self.escaped_choked
    }

    /// Latest per-tick diagnostics as JSON.
    pub fn diagnostics_json(&self) -> String {
        match &self.inner.last_diagnostics {
            Some(d) => serde_json::to_string(d).unwrap_or_default(),
            None => {
                let d = crate::diagnostics::compute_frame_diagnostics(
                    &self.inner.parcels,
                    &self.inner.config,
                    self.inner.tick,
                );
                serde_json::to_string(&d).unwrap_or_default()
            }
        }
    }

    pub fn escape_event_count(&self) -> u32 {
        self.escape_events.len() as u32
    }

    pub fn escape_log_json(&self) -> String {
        let entries: Vec<String> = self.escape_events.iter().map(|e| {
            format!(
                concat!(
                    "{{",
                    "\"tick\":{},",
                    "\"idx\":{},",
                    "\"ste\":{:.4},",
                    "\"conc\":{:.4},",
                    "\"vort\":{:.4},",
                    "\"choked\":{},",
                    "\"phase\":\"{}\",",
                    "\"spin\":{:.2},",
                    "\"radius\":{:.2}",
                    "}}"
                ),
                e.tick, e.index, e.ste, e.concentration, e.vorticity,
                e.was_choked, e.choke_phase, e.spin, e.radius
            )
        }).collect();
        format!("[{}]", entries.join(","))
    }

    pub fn parcel_vorticities(&self) -> Vec<f64> {
        self.inner.parcels.iter().map(|p| p.vorticity).collect()
    }

    /// Create a pure STE field bubble — no quarks, just field parcels.
    ///
    /// A spherical shell of identical field parcels to observe what
    /// structure (if any) emerges from pure all-pairs attraction.
    /// Positions are physical — no separate embed layer.
    pub fn new_proton(n_field: usize) -> Self {
        console_error_panic_hook::set_once();

        let pi = std::f64::consts::PI;

        let mut parcels: Vec<Parcel> = Vec::with_capacity(n_field);

        // ── Field parcels: concentric spherical shells ──
        let rng = |state: &mut u64| -> f64 {
            *state ^= *state << 13;
            *state ^= *state >> 7;
            *state ^= *state << 17;
            (*state as f64) / (u64::MAX as f64)
        };
        let mut seed: u64 = 7777;

        let shells = 6;
        let per_shell_base = n_field / shells;
        let mut placed = 0;
        let golden_angle = pi * (3.0 - (5.0_f64).sqrt());

        for shell in 0..shells {
            let r = 2.5 + (shell as f64) * 1.8;
            let count = if shell < shells - 1 {
                per_shell_base + shell * 2
            } else {
                n_field - placed
            };
            for j in 0..count {
                if placed >= n_field { break; }
                let y_frac = 1.0 - (j as f64 / (count as f64 - 1.0).max(1.0)) * 2.0;
                let phi = (j as f64) * golden_angle + (shell as f64) * 0.37;

                let cos_theta = y_frac;
                let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
                let jr = r + (rng(&mut seed) - 0.5) * 0.6;
                let jphi = phi + (rng(&mut seed) - 0.5) * 0.15;

                let ste = 3.5 + rng(&mut seed) * 1.0;
                let mut p = Parcel::new(placed as u64, ste);
                p.x = jr * sin_theta * jphi.cos();
                p.y = jr * sin_theta * jphi.sin();
                p.z = jr * cos_theta;
                parcels.push(p);
                placed += 1;
            }
        }

        // ── Config tuned for field bubble observation ──
        let mut config = SimConfig::default();
        config.dt = 0.008;
        config.max_bond_distance = 15.0;
        config.parcel_radius_scale = 0.3;
        config.diffusivity = 0.5;
        config.viscosity_base = 0.08;
        config.viscosity_exponent = 1.5;
        config.foam_spawn_coherence = 0.4;
        config.foam_spawn_fraction = 0.05;
        config.foam_spawn_min_ste = 0.02;
        config.foam_annihilation_range = 2.0;

        Self {
            inner: Sim::new(parcels, config),
            visual_k: 6,
            escaped_count: 0,
            escaped_ste: 0.0,
            escaped_choked: 0,
            escape_events: Vec::new(),
        }
    }

    /// Create a Crab-Nebula-scale simulation.
    ///
    /// Dense spinning pulsar core surrounded by progressively thinner
    /// nebula shells.  Same physics as new_proton() — just bigger.
    /// Expect polar jets, equatorial exhaust, and toroidal structure
    /// to emerge from the STE density gradient + spin.
    pub fn new_nebula(n_total: usize) -> Self {
        console_error_panic_hook::set_once();

        let pi = std::f64::consts::PI;
        let golden_angle = pi * (3.0 - (5.0_f64).sqrt());

        let rng = |state: &mut u64| -> f64 {
            *state ^= *state << 13;
            *state ^= *state >> 7;
            *state ^= *state << 17;
            (*state as f64) / (u64::MAX as f64)
        };
        let mut seed: u64 = 31415;

        let mut parcels: Vec<Parcel> = Vec::with_capacity(n_total);
        let mut placed = 0_usize;

        // ── Region 1: Pulsar core (dense, spinning) ──
        // ~15% of parcels, high STE, tight cluster, coherent spin
        let n_core = (n_total as f64 * 0.15).round() as usize;
        let core_shells = 3;
        let core_per_shell = n_core / core_shells;
        for shell in 0..core_shells {
            let r = 1.0 + (shell as f64) * 1.2;
            let count = if shell < core_shells - 1 {
                core_per_shell
            } else {
                n_core - placed
            };
            for j in 0..count {
                if placed >= n_total { break; }
                let y_frac = 1.0 - (j as f64 / (count as f64 - 1.0).max(1.0)) * 2.0;
                let phi = (j as f64) * golden_angle + (shell as f64) * 0.5;
                let cos_theta = y_frac;
                let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
                let jr = r + (rng(&mut seed) - 0.5) * 0.3;
                let jphi = phi + (rng(&mut seed) - 0.5) * 0.1;

                let ste = 25.0 + rng(&mut seed) * 10.0; // very dense
                let mut p = Parcel::new(placed as u64, ste);
                p.x = jr * sin_theta * jphi.cos();
                p.y = jr * sin_theta * jphi.sin();
                p.z = jr * cos_theta;

                // Coherent spin around Z-axis (pulsar rotation)
                let spin_speed = 2.0 / (r.max(0.5));
                p.vx = -p.y * spin_speed + (rng(&mut seed) - 0.5) * 0.2;
                p.vy =  p.x * spin_speed + (rng(&mut seed) - 0.5) * 0.2;
                p.vz = (rng(&mut seed) - 0.5) * 0.3;

                parcels.push(p);
                placed += 1;
            }
        }

        // ── Region 2: Inner wind nebula (~30% of parcels) ──
        // Medium STE, expanding shells with some spin inheritance
        let n_wind = (n_total as f64 * 0.30).round() as usize;
        let wind_target = placed + n_wind;
        let wind_shells = 5;
        let wind_per_shell = n_wind / wind_shells;
        for shell in 0..wind_shells {
            let r = 5.0 + (shell as f64) * 3.0;
            let count = if shell < wind_shells - 1 {
                wind_per_shell
            } else {
                wind_target - placed
            };
            for j in 0..count {
                if placed >= n_total { break; }
                let y_frac = 1.0 - (j as f64 / (count as f64 - 1.0).max(1.0)) * 2.0;
                let phi = (j as f64) * golden_angle + (shell as f64) * 0.37;
                let cos_theta = y_frac;
                let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
                let jr = r + (rng(&mut seed) - 0.5) * 1.5;
                let jphi = phi + (rng(&mut seed) - 0.5) * 0.2;

                let ste = 6.0 + rng(&mut seed) * 4.0; // medium density
                let mut p = Parcel::new(placed as u64, ste);
                p.x = jr * sin_theta * jphi.cos();
                p.y = jr * sin_theta * jphi.sin();
                p.z = jr * cos_theta;

                // Mild outward velocity + residual spin
                let norm = (p.x * p.x + p.y * p.y + p.z * p.z).sqrt().max(0.1);
                let outward = 0.3 / norm;
                p.vx = p.x * outward + (rng(&mut seed) - 0.5) * 0.4;
                p.vy = p.y * outward + (rng(&mut seed) - 0.5) * 0.4;
                p.vz = p.z * outward + (rng(&mut seed) - 0.5) * 0.4;

                parcels.push(p);
                placed += 1;
            }
        }

        // ── Region 3: Outer remnant (~55% of parcels) ──
        // Low STE, near-stationary, large shells — the "ejecta"
        let n_outer = n_total - placed;
        let outer_shells = 6;
        let outer_per_shell = n_outer / outer_shells;
        for shell in 0..outer_shells {
            let r = 22.0 + (shell as f64) * 5.0;
            let count = if shell < outer_shells - 1 {
                outer_per_shell + shell * 3 // more parcels at larger shells
            } else {
                n_total - placed
            };
            for j in 0..count {
                if placed >= n_total { break; }
                let y_frac = 1.0 - (j as f64 / (count as f64 - 1.0).max(1.0)) * 2.0;
                let phi = (j as f64) * golden_angle + (shell as f64) * 0.29;
                let cos_theta = y_frac;
                let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
                let jr = r + (rng(&mut seed) - 0.5) * 3.0;
                let jphi = phi + (rng(&mut seed) - 0.5) * 0.25;

                let ste = 1.0 + rng(&mut seed) * 2.0; // sparse
                let mut p = Parcel::new(placed as u64, ste);
                p.x = jr * sin_theta * jphi.cos();
                p.y = jr * sin_theta * jphi.sin();
                p.z = jr * cos_theta;
                // Nearly stationary
                p.vx = (rng(&mut seed) - 0.5) * 0.05;
                p.vy = (rng(&mut seed) - 0.5) * 0.05;
                p.vz = (rng(&mut seed) - 0.5) * 0.05;

                parcels.push(p);
                placed += 1;
            }
        }

        // ── Config tuned for nebula-scale observation ──
        let mut config = SimConfig::default();
        config.dt = 0.005;
        config.max_bond_distance = 50.0;
        config.parcel_radius_scale = 0.3;
        config.diffusivity = 0.3;
        config.viscosity_base = 0.06;
        config.viscosity_exponent = 1.5;
        config.shell_interaction_range = 3.0;
        config.foam_spawn_coherence = 0.3;
        config.foam_spawn_fraction = 0.04;
        config.foam_spawn_min_ste = 0.1;
        config.foam_annihilation_range = 2.5;

        Self {
            inner: Sim::new(parcels, config),
            visual_k: 8,
            escaped_count: 0,
            escaped_ste: 0.0,
            escaped_choked: 0,
            escape_events: Vec::new(),
        }
    }

    /// Get JSON info for a specific parcel.
    pub fn parcel_info(&self, index: usize) -> String {
        if index >= self.inner.parcels.len() {
            return "{}".to_string();
        }
        let p = &self.inner.parcels[index];
        let eff_mass = p.ste_amount.max(1e-10);
        let san = |v: f64| -> f64 { if v.is_finite() { v } else { 0.0 } };

        format!(
            r#"{{"id":{},"ste":{:.4},"concentration":{:.4},"radius":{:.4},"effective_mass":{:.4},"vorticity":{:.4},"shell_level":{:.4},"speed":{:.4},"role":{}}}"#,
            p.id, san(p.ste_amount), san(p.concentration), san(p.radius),
            san(eff_mass), san(p.vorticity), san(p.shell_level),
            san(p.speed()), p.role
        )
    }

    // ── Internal helpers ──

    /// No bond_counts anymore — return visual K-nearest count for all.
    pub fn bond_counts(&self) -> Vec<usize> {
        vec![self.visual_k; self.inner.parcels.len()]
    }

    /// Discovery pulses are no longer needed (no bond graph to discover).
    pub fn pulse_radii(&self) -> Vec<f64> {
        vec![0.0; self.inner.parcels.len()]
    }

    pub fn discoveries(&self) -> Vec<usize> {
        Vec::new()
    }
}

/// Internal method — escape cull based on distance from centroid.
#[cfg(feature = "wasm")]
impl WasmSim {
    fn centroid(&self) -> (f64, f64, f64) {
        let n = self.inner.parcels.len();
        if n == 0 { return (0.0, 0.0, 0.0); }
        let mut cx = 0.0;
        let mut cy = 0.0;
        let mut cz = 0.0;
        for p in &self.inner.parcels {
            cx += p.x;
            cy += p.y;
            cz += p.z;
        }
        (cx / n as f64, cy / n as f64, cz / n as f64)
    }

    fn escape_cull(&mut self) {
        let n = self.inner.parcels.len();
        if n < 5 { return; }

        let (cx, cy, cz) = self.centroid();

        // Average distance from centroid
        let mut total_dist = 0.0;
        for p in &self.inner.parcels {
            let dx = p.x - cx;
            let dy = p.y - cy;
            let dz = p.z - cz;
            total_dist += (dx * dx + dy * dy + dz * dz).sqrt();
        }
        let avg_dist = total_dist / n as f64;
        let escape_radius = (avg_dist * 10.0).max(5.0);

        for i in (0..n).rev() {
            // Never cull quarks
            let pid = self.inner.parcels[i].id;
            if pid <= 2 { continue; }

            let dx = self.inner.parcels[i].x - cx;
            let dy = self.inner.parcels[i].y - cy;
            let dz = self.inner.parcels[i].z - cz;
            let r = (dx * dx + dy * dy + dz * dz).sqrt();

            if r > escape_radius {
                let p = &self.inner.parcels[i];
                let phase_str = match &p.choke {
                    Some(c) => format!("{:?}", c.phase),
                    None => "free".to_string(),
                };

                self.escape_events.push(EscapeEvent {
                    tick: self.inner.tick,
                    index: i,
                    ste: p.ste_amount,
                    concentration: p.concentration,
                    vorticity: p.vorticity,
                    was_choked: p.is_choked(),
                    choke_phase: phase_str,
                    spin: p.spin,
                    radius: r,
                });

                self.escaped_count += 1;
                self.escaped_ste += p.ste_amount;
                if p.is_choked() {
                    self.escaped_choked += 1;
                }

                // Recycle: zero out and move to centroid
                self.inner.parcels[i].ste_amount = 0.0;
                self.inner.parcels[i].spin = 0.0;
                self.inner.parcels[i].choke = None;
                self.inner.parcels[i].vx = 0.0;
                self.inner.parcels[i].vy = 0.0;
                self.inner.parcels[i].vz = 0.0;
                self.inner.parcels[i].x = cx;
                self.inner.parcels[i].y = cy;
                self.inner.parcels[i].z = cz;
            }
        }
    }
}
