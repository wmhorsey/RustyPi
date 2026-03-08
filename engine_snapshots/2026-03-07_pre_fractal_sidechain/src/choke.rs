use crate::config::SimConfig;
use crate::parcel::{ChokePhase, ChokeState, Parcel};

/// # Choke Formation and Lifecycle (D2, D10)
///
/// A choke is a micro-vortex — a low-energy depression enveloped by
/// higher-strength flows.
///
/// **Chokes are what we observe as heat.**  The underlying reality
/// is a deeper STE well in the local field: more substrate soaked
/// into the area means more chaotic interaction, which manifests as
/// vorticity.  The choke (depression + swirl) is the visible face;
/// the STE well depth is the cause.
///
/// Detection: when local vorticity (from velocity curl) exceeds
/// threshold AND local concentration is below the depression
/// threshold, a parcel transitions to choked.  This means chokes
/// form at the *edges* of STE wells — the depression boundary —
/// not in the dense core itself.
///
/// Lifecycle (D10):
///   Formation → Lift-off → Coherence → Drift → Dissolution

/// Dimensionless resistance threshold for mass-flip onset.
///
/// The index sums four [0,1]-bounded channels, so 2.0 means
/// at least half of the resistance channels are strongly active.
const MASS_FLIP_THRESHOLD: f64 = 1.6;

/// Compute the local mass-flip index (flow → resistance transition).
///
/// Channels:
/// - convergence: local concentration crowding around a depression
/// - stagnation: parcel speed loss relative to neighborhood flow speed
/// - opposition: directional mismatch against neighborhood flow vector
/// - rotation_support: local rotational support for shell formation
pub fn mass_flip_index(
    parcel: &Parcel,
    local_avg_concentration: f64,
    local_avg_vorticity: f64,
    local_avg_velocity: [f64; 3],
) -> f64 {
    let eps = 1e-10;

    let convergence = ((local_avg_concentration - parcel.concentration)
        / local_avg_concentration.max(eps))
        .max(0.0)
        .min(1.0);

    let speed = parcel.speed();
    let local_speed = (local_avg_velocity[0] * local_avg_velocity[0]
        + local_avg_velocity[1] * local_avg_velocity[1]
        + local_avg_velocity[2] * local_avg_velocity[2])
        .sqrt();
    let stagnation = ((local_speed - speed) / local_speed.max(eps))
        .max(0.0)
        .min(1.0);

    let opposition = if speed > eps && local_speed > eps {
        let dot = parcel.vx * local_avg_velocity[0]
            + parcel.vy * local_avg_velocity[1]
            + parcel.vz * local_avg_velocity[2];
        let align = (dot / (speed * local_speed)).clamp(-1.0, 1.0);
        ((1.0 - align) * 0.5).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let rotation_support = (parcel.vorticity.max(0.0)
        / (parcel.vorticity.max(0.0) + local_avg_vorticity.max(0.0) + eps))
        .clamp(0.0, 1.0);

    convergence + stagnation + opposition + rotation_support
}

/// Check whether a parcel should become a choke.
///
/// No magic numbers.  A choke forms when:
///   1. The parcel is a LOCAL DEPRESSION — concentration below its
///      neighbourhood average.  "Depression" is relational, not absolute.
///   2. The parcel has LOCAL ROTATION — vorticity above the local
///      background level.  Rotation is relational, not absolute.
///
/// Both criteria are passed in pre-computed by the sim step.
pub fn should_form_choke(
    parcel: &Parcel,
    local_avg_concentration: f64,
    local_avg_vorticity: f64,
    local_avg_velocity: [f64; 3],
) -> bool {
    let flip = mass_flip_index(
        parcel,
        local_avg_concentration,
        local_avg_vorticity,
        local_avg_velocity,
    );

    !parcel.is_choked()
        && parcel.concentration < local_avg_concentration
        && parcel.vorticity > local_avg_vorticity
        && flip >= MASS_FLIP_THRESHOLD
}

/// Spawn a new choke on a parcel.
pub fn form_choke(parcel: &mut Parcel) {
    let spin = if parcel.spin.abs() > 1e-10 {
        parcel.spin
    } else if parcel.vorticity > 0.0 {
        parcel.vorticity
    } else {
        1e-10
    };
    parcel.choke = Some(ChokeState {
        phase: ChokePhase::Formation,
        spin,
        coherence: 1.0,
        radius: 1.0, // will be updated by estimate_radius
        equilibrium_concentration: parcel.concentration,
        age: 0.0,
    });
}

/// Estimate a choke's distance from the local concentration centre.
///
/// Uses Euclidean distances weighted by peer concentration.
/// A small value = close to dense core; large = drifted outward.
pub fn estimate_radius(parcel_idx: usize, peers: &[Parcel]) -> f64 {
    let parcel = &peers[parcel_idx];
    let mut weighted_dist = 0.0;
    let mut weight_sum = 0.0;

    for (j, peer) in peers.iter().enumerate() {
        if j == parcel_idx { continue; }
        let d = parcel.dist_to(peer);
        let w = peer.concentration;
        weighted_dist += d * w;
        weight_sum += w;
    }

    if weight_sum > 1e-12 {
        weighted_dist / weight_sum
    } else {
        1.0
    }
}

/// Advance a choke through its lifecycle by one timestep.
///
/// No magic thresholds.  Phase transitions are driven by the
/// relationship between the choke's state and its environment:
///
///   Formation → LiftOff:  when the depression has persisted long
///       enough to be established (age > parcel radius / vorticity,
///       i.e. one rotation period).
///   LiftOff → Coherence:  when the choke has drifted beyond its
///       own parcel radius (it's a distinct structure now).
///   Coherence → Drift:    when ambient hasn't filled the depression.
///   Drift → Dissolution:  when ambient fills the depression
///       AND outward drift can no longer outrun inward refill.
///   Dissolution:          coherence decays at the rate the field
///       can fill the void, modulated by whether the shell is escaping
///       outward or being pulled back inward.
///
/// Returns `true` if the choke survives, `false` if dissolved.
pub fn advance_choke(
    parcel: &mut Parcel,
    ambient_concentration: f64,
    radius: f64,
    config: &SimConfig,
) -> bool {
    let choke = match parcel.choke.as_mut() {
        Some(c) => c,
        None => return false,
    };

    choke.age += config.dt;
    let prev_radius = choke.radius;
    choke.radius = radius;

    // Shell protection law:
    // - If exterior STE > interior/core STE, shell excess strengthens.
    // - If exterior and interior equalize (or invert), shell excess radiates away.
    let core_concentration = choke.equilibrium_concentration.max(1e-10);
    let exterior = ambient_concentration.max(0.0);
    let depression_gap = exterior - core_concentration;

    let mut shell_excess = (parcel.shell_level - parcel.shell_equilibrium).max(0.0);
    let spin_support = 1.0 + choke.spin.abs().min(1.0);

    if depression_gap > 0.0 {
        let gap_norm = (depression_gap / exterior.max(1e-10)).clamp(0.0, 1.0);
        let target_excess = exterior * gap_norm * spin_support;
        let grow_rate = 2.0;
        shell_excess += (target_excess - shell_excess).max(0.0) * grow_rate * config.dt;
    } else {
        // Equalized shell has no reason to hold: radiate back toward equilibrium.
        let deficit = (-depression_gap) / core_concentration.max(1e-10);
        let decay_rate = 1.5 + deficit.clamp(0.0, 2.0);
        shell_excess *= (1.0 - decay_rate * config.dt).max(0.0);
    }

    parcel.shell_level = parcel.shell_equilibrium + shell_excess;
    let shell_shield = shell_excess / (parcel.shell_equilibrium + shell_excess + 1e-10);

    // Radial drift relative to local concentration centre.
    // Positive = escaping outward (bubble trying to surface).
    // Negative = sinking inward (down-draft wins).
    let radial_rate = (radius - prev_radius) / config.dt.max(1e-12);

    match choke.phase {
        ChokePhase::Formation => {
            // Formation completes after one rotation period: r / ω
            // If no vorticity, stuck in formation until there is some.
            let rotation_period = parcel.radius / parcel.vorticity.max(1e-10);
            if choke.age > rotation_period {
                choke.phase = ChokePhase::LiftOff;
            }
        }

        ChokePhase::LiftOff => {
            // Lift-off when the choke has drifted beyond its own radius
            if radius > parcel.radius {
                choke.phase = ChokePhase::Coherence;
            }
        }

        ChokePhase::Coherence => {
            let ratio = ambient_concentration / choke.equilibrium_concentration.max(1e-10);
            if ratio > 1.0 {
                // Ambient has filled the depression — dissolve
                choke.phase = ChokePhase::Dissolution;
            } else {
                choke.phase = ChokePhase::Drift;
            }
        }

        ChokePhase::Drift => {
            let ratio = ambient_concentration / choke.equilibrium_concentration.max(1e-10);
            if ratio > 1.0 {
                // Shell contest model:
                // - inward_push: ambient refill + down-draft
                // - shell_cohesion: rotating shell's tendency to hold together
                // - escape_drive: outward shell motion that resists refill
                let fill_pressure = (ratio - 1.0).max(0.0);
                let down_draft = (-radial_rate).max(0.0);
                let inward_push = fill_pressure + down_draft;

                let shell_cohesion = choke.coherence.max(0.0)
                    * spin_support
                    * (1.0 + 1.5 * shell_shield);
                let escape = radial_rate.max(0.0);
                let escape_drive = escape / (escape + 1.0);

                // Dissolution starts only when surrounding flow overcomes shell
                // cohesion and outward escape support.
                if inward_push > shell_cohesion + 0.35 * escape_drive {
                    // Equilibrium-crossing kick:
                    // as core/ambient parity is crossed, release a brief burst
                    // into local rotational activity while shell begins collapsing
                    // toward stronger surrounding flows.
                    let crossing_sharpness = (ratio - 1.0) / ratio.max(1e-10);
                    let crossing_resonance = 1.0 / (1.0 + crossing_sharpness.abs());
                    let kick = crossing_resonance * choke.coherence.max(0.0) * config.dt;
                    parcel.vorticity += kick;

                    // Shell collapses toward ambient-driven equilibrium once
                    // refill overtakes escape/cohesion.
                    parcel.shell_level = 0.5 * (parcel.shell_level + parcel.shell_equilibrium);

                    // If shell was still moving outward, surrounding inward
                    // flow pushes back against that motion.
                    if radial_rate > 0.0 {
                        let retreat = 1.0 / (1.0 + radial_rate);
                        parcel.vx *= retreat;
                        parcel.vy *= retreat;
                        parcel.vz *= retreat;
                    }

                    choke.phase = ChokePhase::Dissolution;
                }
            }
        }

        ChokePhase::Dissolution => {
            let ratio = ambient_concentration / choke.equilibrium_concentration.max(1e-10);

            // If ambient refill is below equilibrium and the structure is
            // escaping outward again, allow recovery back into drift.
            if ratio < 1.0 && radial_rate > 0.0 {
                choke.phase = ChokePhase::Drift;
                return true;
            }

            // Refill competition model:
            // - fill_pressure: ambient over equilibrium (drives collapse)
            // - down_draft: inward radial motion (also drives collapse)
            // - escape_credit: outward radial motion (resists collapse)
            let fill_pressure = (ratio - 1.0).max(0.0);
            let down_draft = (-radial_rate).max(0.0);
            let escape = radial_rate.max(0.0);
            let escape_credit = escape / (escape + 1.0);
            let inward_push = fill_pressure + down_draft;
            let shell_cohesion = choke.coherence.max(0.0)
                * spin_support
                * (1.0 + 1.5 * shell_shield);

            let net_fill = (inward_push - shell_cohesion - 0.35 * escape_credit).max(0.0);

            // More ambient STE and stronger net refill collapse coherence faster.
            let decay = ambient_concentration * config.dt * net_fill;
            choke.coherence -= decay;
            if choke.coherence <= 0.0 {
                parcel.choke = None;
                return false;
            }
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> SimConfig {
        SimConfig::default()
    }

    #[test]
    fn free_flowing_can_become_choke() {
        // Low STE parcel with high vorticity, surrounded by higher-conc neighbors
        let mut p = Parcel::new(0, 0.01);
        p.vorticity = 1.0;
        // local avg concentration above parcel's, local avg vorticity below parcel's
        let local_avg_conc = p.concentration + 0.5;
        let local_avg_vort = p.vorticity * 0.5;
        let local_avg_vel = [1.0, 0.0, 0.0];

        assert!(should_form_choke(&p, local_avg_conc, local_avg_vort, local_avg_vel));
        form_choke(&mut p);
        assert!(p.is_choked());
    }

    #[test]
    fn already_choked_cannot_re_choke() {
        let mut p = Parcel::new(0, 0.01);
        p.vorticity = 1.0;
        form_choke(&mut p);
        // Even with perfect choke conditions, already-choked parcel can't re-choke
        assert!(!should_form_choke(&p, 999.0, 0.0, [0.0, 0.0, 0.0]));
    }

    #[test]
    fn dissolution_removes_choke() {
        let config = make_config();
        let mut p = Parcel::new(0, 1.0);
        form_choke(&mut p);

        if let Some(c) = &mut p.choke {
            c.phase = ChokePhase::Dissolution;
            // Tiny coherence — below ambient × dt so it dissolves in one tick
            c.coherence = config.dt * 0.001;
        }

        let alive = advance_choke(&mut p, 1.0, 5.0, &config);
        assert!(!alive);
        assert!(!p.is_choked());
    }

    #[test]
    fn shell_strengthens_when_exterior_exceeds_core() {
        let config = make_config();
        let mut p = Parcel::new(0, 1.0);
        form_choke(&mut p);

        p.shell_equilibrium = 1.0;
        p.shell_level = 1.0;
        if let Some(c) = &mut p.choke {
            c.phase = ChokePhase::Drift;
            c.equilibrium_concentration = 1.0;
            c.spin = 0.8;
            c.coherence = 0.9;
        }

        let before = p.shell_level;
        let _alive = advance_choke(&mut p, 2.0, 1.2, &config);
        assert!(p.shell_level > before, "shell should strengthen when exterior > core");
    }

    #[test]
    fn shell_radiates_when_gap_collapses() {
        let config = make_config();
        let mut p = Parcel::new(0, 1.0);
        form_choke(&mut p);

        p.shell_equilibrium = 1.0;
        p.shell_level = 2.0;
        if let Some(c) = &mut p.choke {
            c.phase = ChokePhase::Dissolution;
            c.equilibrium_concentration = 1.0;
            c.spin = 0.8;
            c.coherence = 0.95;
        }

        let before_excess = p.squeeze_excess();
        let _alive = advance_choke(&mut p, 0.8, 1.2, &config);
        let after_excess = p.squeeze_excess();
        assert!(after_excess < before_excess, "shell excess should radiate away when exterior <= core");
    }
}
