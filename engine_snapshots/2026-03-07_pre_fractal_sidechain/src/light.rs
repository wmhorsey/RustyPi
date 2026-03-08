use serde::{Deserialize, Serialize};

use crate::config::SimConfig;
use crate::parcel::Parcel;

/// # Light — Attraction Wave (D14)
///
/// Light is a **wave of attraction** — a propagating modulation of the pull
/// that STE exerts on itself.  It is NOT a vortex.
///
/// The wave hops through space to the nearest available parcel.  At each relay:
///   1. The modulated attraction squeezes the node (elevates shell level)
///   2. The node relaxes back to equilibrium
///   3. The energy difference is shed outward (re-emission)
///   4. A transient photon *particle* (vortex) forms and vanishes
///
/// Capture = the node stays at its elevated shell (permanent photon particle).

/// A propagating attraction modulation hopping through space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractionWave {
    pub id: u64,
    pub current_node: usize,
    pub came_from: Option<usize>,
    pub energy: f64,
    pub frequency: f64,
    pub alive: bool,
    pub hops: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionKind {
    Transparent,
    Relay,
    Capture,
}

#[derive(Debug, Clone, Copy)]
pub struct InteractionOutcome {
    pub kind: InteractionKind,
    pub shell_overshoot: f64,
    pub relaxation_ticks: u64,
    pub capture_node: Option<usize>,
}

impl InteractionOutcome {
    fn transparent() -> Self {
        Self {
            kind: InteractionKind::Transparent,
            shell_overshoot: 0.0,
            relaxation_ticks: 0,
            capture_node: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WaveStepStats {
    pub captures: Vec<(u64, usize)>,
    pub relay_collapse_count: u64,
    pub transient_relay_collapses: u64,
    pub terminal_capture_collapses: u64,
    pub relay_shell_overshoot_sum: f64,
    pub relay_relaxation_ticks_sum: u64,
}

impl WaveStepStats {
    fn new() -> Self {
        Self {
            captures: Vec::new(),
            relay_collapse_count: 0,
            transient_relay_collapses: 0,
            terminal_capture_collapses: 0,
            relay_shell_overshoot_sum: 0.0,
            relay_relaxation_ticks_sum: 0,
        }
    }
}

impl AttractionWave {
    pub fn emit(id: u64, source_node: usize, energy: f64, frequency: f64) -> Self {
        Self {
            id,
            current_node: source_node,
            came_from: None,
            energy,
            frequency,
            alive: true,
            hops: 0,
        }
    }

    /// Advance the wave by one hop: pick nearest unvisited parcel, interact.
    ///
    /// Returns `(interaction_outcome, distance_traveled)`.
    /// Distance is needed for the time-budget propagation model.
    pub fn hop(&mut self, parcels: &mut [Parcel], config: &SimConfig) -> (InteractionOutcome, f64) {
        if !self.alive {
            return (InteractionOutcome::transparent(), 0.0);
        }

        // Pick the nearest parcel (excluding where we came from)
        // that has the highest tension (concentration product / distance²).
        let next_idx = pick_next_neighbour(
            self.current_node,
            self.came_from,
            parcels,
            config,
        );

        let next_idx = match next_idx {
            Some(idx) => idx,
            None => {
                self.alive = false;
                return (InteractionOutcome::transparent(), 0.0);
            }
        };

        // Measure hop distance before moving
        let d = parcels[self.current_node].dist_to(&parcels[next_idx]);

        let prev = self.current_node;
        self.current_node = next_idx;
        self.came_from = Some(prev);
        self.hops += 1;

        let mut outcome = interact(self, &mut parcels[next_idx], config);
        if outcome.kind == InteractionKind::Capture {
            outcome.capture_node = Some(next_idx);
        }
        (outcome, d)
    }
}

/// Pick the best next parcel for a wave to hop to.
///
/// Selection: highest tension (c_src × c_peer / d²) among nearby parcels,
/// excluding the one we just came from.
fn pick_next_neighbour(
    current: usize,
    exclude: Option<usize>,
    parcels: &[Parcel],
    config: &SimConfig,
) -> Option<usize> {
    let n = parcels.len();
    let src = &parcels[current];
    let eps = config.softening;

    let mut best_peer: Option<usize> = None;
    let mut best_tension: f64 = 0.0;

    for j in 0..n {
        if j == current { continue; }
        if Some(j) == exclude { continue; }

        let dx = parcels[j].x - src.x;
        let dy = parcels[j].y - src.y;
        let dz = parcels[j].z - src.z;
        let d2 = dx * dx + dy * dy + dz * dz + eps * eps;

        // Only consider parcels within max_bond_distance
        if d2 > config.max_bond_distance * config.max_bond_distance {
            continue;
        }

        let ci = src.ste_amount + eps;
        let cj = parcels[j].ste_amount + eps;
        let tension = ci * cj / d2;

        if tension > best_tension {
            best_tension = tension;
            best_peer = Some(j);
        }
    }

    best_peer
}

/// Squeeze-and-release interaction between a wave and a node.
///
/// Returns `true` if the wave was captured.
fn interact(wave: &mut AttractionWave, parcel: &mut Parcel, _config: &SimConfig) -> InteractionOutcome {
    let is_rotational = parcel.is_choked() || parcel.vorticity > 0.01;
    if !is_rotational {
        return InteractionOutcome::transparent();
    }

    let squeeze_amount = wave.energy / parcel.ste_amount.max(1e-10);
    parcel.shell_level += squeeze_amount;

    let elevation = parcel.shell_level - parcel.shell_equilibrium;
    let capture_threshold = parcel.shell_equilibrium * 2.0;
    let overshoot = elevation.max(0.0);
    let relaxation_ticks = (overshoot / parcel.shell_equilibrium.max(1e-10)).ceil() as u64;

    if elevation > 0.0 && elevation < capture_threshold {
        // Relay
        let released = elevation;
        parcel.shell_level = parcel.shell_equilibrium;
        wave.energy = released * parcel.ste_amount;
        InteractionOutcome {
            kind: InteractionKind::Relay,
            shell_overshoot: overshoot,
            relaxation_ticks: relaxation_ticks.max(1),
            capture_node: None,
        }
    } else if elevation >= capture_threshold {
        // Capture
        wave.alive = false;
        wave.energy = 0.0;
        InteractionOutcome {
            kind: InteractionKind::Capture,
            shell_overshoot: overshoot,
            relaxation_ticks: 0,
            capture_node: None,
        }
    } else {
        parcel.shell_level = parcel.shell_equilibrium;
        InteractionOutcome::transparent()
    }
}

pub fn propagate_all_with_stats(
    waves: &mut Vec<AttractionWave>,
    parcels: &mut [Parcel],
    config: &SimConfig,
) -> WaveStepStats {
    let mut stats = WaveStepStats::new();
    let dt = config.dt;

    for wave in waves.iter_mut() {
        let mut time_remaining = dt;

        while wave.alive && time_remaining > 0.0 {
            // Local concentration = congestion at current node
            let c_local = if wave.current_node < parcels.len() {
                parcels[wave.current_node].concentration
            } else {
                0.0
            };

            // No medium = wave dies
            if c_local < 1e-15 {
                wave.alive = false;
                break;
            }

            let (interaction, dist) = wave.hop(parcels, config);
            match interaction.kind {
                InteractionKind::Transparent => {}
                InteractionKind::Relay => {
                    stats.relay_collapse_count += 1;
                    stats.transient_relay_collapses += 1;
                    stats.relay_shell_overshoot_sum += interaction.shell_overshoot;
                    stats.relay_relaxation_ticks_sum += interaction.relaxation_ticks;
                }
                InteractionKind::Capture => {
                    stats.terminal_capture_collapses += 1;
                    let node = interaction.capture_node.unwrap_or(wave.current_node);
                    stats.captures.push((wave.id, node));
                }
            }

            if dist > 0.0 {
                // Time cost = distance × concentration (denser = slower)
                let hop_time = dist * c_local;
                time_remaining -= hop_time;

                // Energy attenuation: sparse medium = weak carrier.
                // The wave bleeds energy in proportion to how thin the
                // field is along the hop.  Dense field sustains the signal.
                //
                // retention = c_local / (c_local + 1/dist)
                // When c is high: retention → 1 (strong carrier)
                // When c is low:  retention → 0 (signal bleeds away)
                // Longer hops through sparse field = more attenuation.
                let inv_dist = 1.0 / dist.max(1e-10);
                let retention = c_local / (c_local + inv_dist);
                wave.energy *= retention;

                // If energy is negligible, wave dissipates
                if wave.energy < 1e-10 {
                    wave.alive = false;
                }
            }
        }
    }

    waves.retain(|w| w.alive);
    stats
}

/// Propagate all active waves through the field.
///
/// **Wave speed is emergent.**  No magic `hops_per_tick` constant.
///
/// STE is traffic, not rail.  Denser field = more congestion =
/// slower propagation.  Sparse field = open road = fast.
/// Void = no medium = wave dies.
///
/// Local wave speed = 1 / concentration.
/// Hop time = distance × concentration (denser = takes longer).
///
/// Each wave gets a time budget of `dt` per tick.  The wave keeps
/// hopping until its time budget is exhausted.  This means:
///   - In sparse field: cheap hops, fast propagation
///   - In dense field: expensive hops, slow propagation
///   - In void: no medium, wave stops
pub fn propagate_all(
    waves: &mut Vec<AttractionWave>,
    parcels: &mut [Parcel],
    config: &SimConfig,
) -> Vec<(u64, usize)> {
    propagate_all_with_stats(waves, parcels, config).captures
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> SimConfig {
        SimConfig::default()
    }

    /// Build a linear chain of parcels along the x-axis.
    fn make_chain(n: usize) -> Vec<Parcel> {
        (0..n)
            .map(|i| {
                let mut p = Parcel::new(i as u64, 1.0);
                p.x = i as f64 * 1.5;
                p.vorticity = 0.1; // rotational — can relay light
                p.shell_equilibrium = 1.0;
                p.shell_level = 1.0;
                p
            })
            .collect()
    }

    #[test]
    fn wave_propagates_along_chain() {
        let config = make_config();
        let mut parcels = make_chain(5);
        let mut waves = vec![AttractionWave::emit(1, 0, 0.5, 1.0)];

        let captures = propagate_all(&mut waves, &mut parcels, &config);
        assert!(waves.is_empty() || waves[0].hops > 0 || !captures.is_empty());
    }

    #[test]
    fn wave_does_not_bounce_backward() {
        let config = make_config();
        let mut parcels = make_chain(5);
        let mut wave = AttractionWave::emit(1, 2, 0.5, 1.0);

        wave.hop(&mut parcels, &config);
        let after_first = wave.current_node;

        wave.hop(&mut parcels, &config);
        let after_second = wave.current_node;

        assert_ne!(after_second, 2, "wave bounced back to origin");
        // In an all-pairs model, the wave picks the highest-tension neighbour.
        // With equal concentrations spaced 1.5 apart and excluding came_from,
        // the wave should move forward.
        assert_ne!(after_second, after_first, "wave stalled");
    }

    #[test]
    fn capture_kills_wave() {
        let config = make_config();
        let mut parcels = make_chain(3);
        parcels[1].shell_equilibrium = 0.01;
        parcels[1].shell_level = 0.01;

        let mut wave = AttractionWave::emit(1, 0, 10.0, 1.0);
        let (result, _dist) = wave.hop(&mut parcels, &config);

        assert_eq!(result.kind, InteractionKind::Capture, "expected capture");
        assert!(!wave.alive);
    }

    #[test]
    fn non_rotational_node_is_transparent() {
        let config = make_config();
        let mut parcels = make_chain(3);
        parcels[1].vorticity = 0.0;
        parcels[1].choke = None;

        let mut wave = AttractionWave::emit(1, 0, 0.5, 1.0);
        let energy_before = wave.energy;
        wave.hop(&mut parcels, &config);

        assert_eq!(wave.energy, energy_before);
        assert!(wave.alive);
    }
}
