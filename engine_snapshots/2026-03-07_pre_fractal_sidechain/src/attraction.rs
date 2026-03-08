use crate::config::SimConfig;
use crate::parcel::Parcel;

/// # Tension Well Model (A1, D1, D3, D4) — Grid-Free, Constant-Free
///
/// Each concentrated node creates a **tension well** in the surrounding
/// STE field.  The field between nodes is real STE under tension — it
/// tapers with distance from each source.
///
/// ## Bond Tension (the well overlap)
///
/// The tension on a bond = the overlap of two tapering wells.  At each
/// point along the channel, both nodes are pulling on the fill.  The
/// integrated overlap gives the bond's tension:
///
///   tension(i,j) = c_i · c_j / d^n
///
/// This is dimensionless — concentrations are ratios (A2), distances
/// are bond-graph scalars.  No coupling constant (no G).
///
/// ## Force (tension gradient)
///
/// Force = the rate at which tension changes with distance.  This is
/// what drives `closing_speed`:
///
///   force = n · c_i · c_j / d^(n+1)
///
/// Always positive = always attractive (A1).  There is no repulsive
/// force.  Compression (D1) emerges from the inertia of accumulated
/// field tension, not from a separate push.
///
/// ## Effective Mass (D3: mass = interior tension)
///
/// A parcel's resistance to acceleration = its STE amount + the total
/// tension locked in its surrounding well:
///
///   mass_i = ste_i + Σ_bonds tension(i,j)
///
/// This is why denser atoms are heavier (deeper wells), and why denser
/// arrangements are heavier still (overlapping wells reinforce).
///
/// ## Gravity (D4: exterior measure)
///
/// The same tension well that creates mass (viewed from inside) creates
/// gravity (viewed from outside).  Long bonds sample the tenuous tail
/// of the well → weak pull.  Short bonds sample the steep inner well
/// → strong pull.  No separate gravity law needed.

/// Compute the tension on a bond — the overlap of two wells.
///
/// This measures how much the fill between two nodes is stressed.
/// Higher concentration + shorter distance = deeper overlap = more tension.
///
/// Returns a non-negative scalar (tension is always ≥ 0).
pub fn well_tension(
    c_i: f64,
    c_j: f64,
    distance: f64,
    config: &SimConfig,
) -> f64 {
    let eps = config.softening;
    let d2 = distance * distance + eps * eps;
    let ci = c_i + eps; // ensure non-zero even in void
    let cj = c_j + eps;
    ci * cj / d2.powf(config.distance_exponent / 2.0)
}

/// Compute the force from the tension gradient along a bond.
///
/// Force = d(tension)/d(distance) — how fast the well deepens as
/// you approach.  Always positive (always attractive).
///
/// This is the magnitude of the pull experienced by node i toward
/// node j through the channel fill.
pub fn tension_gradient(
    c_i: f64,
    c_j: f64,
    distance: f64,
    config: &SimConfig,
) -> f64 {
    let eps = config.softening;
    let d2 = distance * distance + eps * eps;
    let n = config.distance_exponent;
    let ci = c_i + eps;
    let cj = c_j + eps;
    n * ci * cj / d2.powf((n + 1.0) / 2.0)
}

/// Compute a parcel's effective mass (D3).
///
/// Mass = concentration = STE / volume = local density.
/// The density of the STE at this spot IS its resistance to
/// acceleration.  Denser parcels are heavier.  Period.
///
/// The tension wells are CREATED BY the mass — they are the
/// output of concentration, not an input to it.
pub fn effective_mass(parcel: &Parcel) -> f64 {
    // concentration is ste/volume, already computed by estimate_concentration.
    // Ensure a minimum so we never divide by zero.
    parcel.concentration.max(1e-10)
}

/// Apply tension-well forces to every bond on every parcel.
///
/// For each bond:
/// 1. Compute the well tension (used for rendering + light routing)
/// 2. Compute the tension gradient (= force magnitude, always attractive)
/// 3. Divide by effective mass (= concentration = density)
/// 4. Apply acceleration to closing_speed
///
/// Equilibrium emerges because:
/// - Force ∝ c_i · c_j / d^(n+1) — grows at short range
/// - Mass = concentration — also grows when compressed (smaller volume)
/// - Plus viscosity (A3) damps in proportion to concentration
/// - Result: self-regulation (D9)
pub fn apply_forces(parcels: &mut [Parcel], config: &SimConfig) {
    let n = parcels.len();
    let dt = config.dt;

    // Snapshot concentrations for immutable reads.
    let concs: Vec<f64> = parcels.iter().map(|p| p.concentration).collect();

    // ── Pass 1: compute effective masses (= concentration) ──
    let masses: Vec<f64> = parcels.iter().map(|p| effective_mass(p)).collect();

    // ── Pass 2: compute tensions and apply forces ──
    for i in 0..n {
        let mass_i = masses[i];

        for b in 0..parcels[i].bonds.len() {
            let peer = parcels[i].bonds[b].peer;
            if peer >= n {
                continue;
            }

            // Well tension on this bond (for rendering + light routing).
            let t = well_tension(concs[i], concs[peer], parcels[i].bonds[b].distance, config);
            parcels[i].bonds[b].tension = t;

            // Force = tension gradient (always attractive = approach).
            let force = tension_gradient(
                concs[i],
                concs[peer],
                parcels[i].bonds[b].distance,
                config,
            );

            // Acceleration = force / mass.  Denser nodes barely respond.
            let accel = force / mass_i;

            // Attraction → closing_speed more negative (approaching).
            parcels[i].bonds[b].closing_speed -= accel * dt;

            // ── CFL stability clamp ──
            // A bond must not close by more than half its length per
            // timestep.  This is the discrete equivalent of "information
            // can't propagate faster than one bond per tick" — the
            // local speed-of-sound limit in the STE medium.
            let d = parcels[i].bonds[b].distance;
            let v_max = 0.5 * d / dt;  // CFL factor 0.5
            parcels[i].bonds[b].closing_speed =
                parcels[i].bonds[b].closing_speed.clamp(-v_max, v_max);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tension_increases_with_concentration() {
        let config = SimConfig::default();
        let t_lo = well_tension(0.5, 0.5, 2.0, &config);
        let t_hi = well_tension(5.0, 5.0, 2.0, &config);
        assert!(
            t_hi > t_lo,
            "Higher concentration should mean more tension: lo={t_lo}, hi={t_hi}"
        );
    }

    #[test]
    fn tension_decreases_with_distance() {
        let config = SimConfig::default();
        let t_near = well_tension(1.0, 1.0, 1.0, &config);
        let t_far = well_tension(1.0, 1.0, 5.0, &config);
        assert!(
            t_near > t_far,
            "Tension should taper with distance: near={t_near}, far={t_far}"
        );
    }

    #[test]
    fn force_is_always_attractive() {
        let config = SimConfig::default();
        // Every combination should produce positive (attractive) force
        for &ci in &[0.01, 0.5, 5.0] {
            for &cj in &[0.01, 0.5, 5.0] {
                for &d in &[0.1, 1.0, 5.0] {
                    let f = tension_gradient(ci, cj, d, &config);
                    assert!(
                        f > 0.0,
                        "Force must always be attractive: ci={ci}, cj={cj}, d={d}, f={f}"
                    );
                }
            }
        }
    }

    #[test]
    fn closer_bonds_pull_harder() {
        let config = SimConfig::default();
        let f_near = tension_gradient(1.0, 1.0, 1.0, &config);
        let f_far = tension_gradient(1.0, 1.0, 5.0, &config);
        assert!(
            f_near > f_far,
            "Shorter bonds should pull harder: near={f_near}, far={f_far}"
        );
    }

    #[test]
    fn effective_mass_grows_with_concentration() {
        // Higher concentration (denser STE) → more mass.
        let mut p_dilute = crate::parcel::Parcel::new(0, 1.0);
        p_dilute.concentration = 0.5;
        let mut p_dense = crate::parcel::Parcel::new(1, 1.0);
        p_dense.concentration = 5.0;
        let m_low = effective_mass(&p_dilute);
        let m_high = effective_mass(&p_dense);
        assert!(
            m_high > m_low,
            "Denser STE → more mass: low={m_low}, high={m_high}"
        );
    }

    #[test]
    fn denser_arrangement_is_heavier() {
        // A parcel with tight spacing has higher concentration (ste/volume),
        // therefore more mass.  Compressed structures are heavier.
        let mut p_close = crate::parcel::Parcel::new(0, 1.0);
        p_close.concentration = 4.0;  // tight spacing → high density
        let mut p_far = crate::parcel::Parcel::new(1, 1.0);
        p_far.concentration = 0.1;    // wide spacing → low density
        let m_close = effective_mass(&p_close);
        let m_far = effective_mass(&p_far);
        assert!(
            m_close > m_far,
            "Denser arrangement should be heavier: close={m_close}, far={m_far}"
        );
    }
}
