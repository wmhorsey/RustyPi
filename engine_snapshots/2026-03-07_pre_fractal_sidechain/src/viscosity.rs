use crate::bond::Bond;
use crate::config::SimConfig;
use crate::parcel::Parcel;

/// # Relational Viscosity (A3) — Bond-Based, Grid-Free
///
/// "The resistance of STE to bulk flow increases monotonically
///  with concentration."
///
/// In the relational model, viscosity acts on bonds:
/// the denser the region, the more any relative motion is resisted.
/// This damps ALL closing_speed — approach AND recession — because
/// the STE filling the channel resists being deformed in either
/// direction.
///
/// This creates:
/// - Free-flow regime (low c):  nearly frictionless bonds
/// - Dense packing (high c):    bonds resist all relative motion
/// - Void (c ≈ 0):              no viscosity (nothing there)
///
/// Combined with the tension-well force model, viscosity is what
/// creates equilibrium (D9): attraction accelerates parcels toward
/// each other, but the rising concentration → rising viscosity
/// eventually damps all motion → stable structure.

/// Compute the viscosity coefficient at a given concentration.
///
/// η(c) = base * c^exponent
///
/// Returns 0 for zero/negative concentration (void has no resistance).
pub fn local_viscosity(concentration: f64, config: &SimConfig) -> f64 {
    if concentration <= 0.0 {
        return 0.0;
    }
    config.viscosity_base * concentration.powf(config.viscosity_exponent)
}

/// Compute the viscous damping on a single bond.
///
/// Returns a deceleration magnitude (always ≥ 0).
/// Acts on ALL relative motion — approach and recession —
/// because the channel fill resists being deformed.
pub fn bond_viscous_damping(
    parcel_i: &Parcel,
    parcel_j: &Parcel,
    bond: &Bond,
    config: &SimConfig,
) -> f64 {
    let speed = bond.closing_speed.abs();
    if speed < 1e-20 {
        return 0.0;
    }

    let eta_i = local_viscosity(parcel_i.concentration, config);
    let eta_j = local_viscosity(parcel_j.concentration, config);
    let eta_avg = 0.5 * (eta_i + eta_j);

    let d2 = bond.distance * bond.distance + config.softening * config.softening;

    // Viscous deceleration: opposes relative motion
    eta_avg * speed / d2
}

/// Apply viscous damping across all bonds for all parcels.
///
/// Modifies `closing_speed` on each bond to resist ALL relative motion
/// in proportion to the local concentration.  This is what creates
/// equilibrium structures — without it, pure attraction collapses
/// everything.
pub fn apply_viscosity(parcels: &mut [Parcel], config: &SimConfig) {
    let n = parcels.len();

    // Snapshot concentrations to avoid borrow issues
    let concs: Vec<f64> = parcels.iter().map(|p| p.concentration).collect();

    for i in 0..n {
        for b in 0..parcels[i].bonds.len() {
            let peer = parcels[i].bonds[b].peer;
            if peer >= n { continue; }

            let speed = parcels[i].bonds[b].closing_speed;
            if speed.abs() < 1e-20 { continue; }

            let eta_i = local_viscosity(concs[i], config);
            let eta_j = local_viscosity(concs[peer], config);
            let eta_avg = 0.5 * (eta_i + eta_j);

            let d2 = parcels[i].bonds[b].distance.powi(2) + config.softening.powi(2);
            let damping = eta_avg * speed.abs() / d2;

            // Apply damping opposite to current motion direction.
            // If closing_speed < 0 (approaching) → add positive damping.
            // If closing_speed > 0 (receding) → add negative damping.
            let correction = damping * config.dt * (-speed.signum());
            parcels[i].bonds[b].closing_speed += correction;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bond::Bond;

    #[test]
    fn void_has_no_viscosity() {
        let config = SimConfig::default();
        assert_eq!(local_viscosity(0.0, &config), 0.0);
    }

    #[test]
    fn viscosity_increases_with_concentration() {
        let config = SimConfig::default();
        let lo = local_viscosity(1.0, &config);
        let hi = local_viscosity(10.0, &config);
        assert!(hi > lo);
    }

    #[test]
    fn moving_bonds_get_damped() {
        let config = SimConfig::default();
        let mut a = Parcel::new(0, 1.0);
        a.concentration = 2.0;
        let mut b = Parcel::new(1, 1.0);
        b.concentration = 2.0;

        // Approaching bond
        let mut bond_approach = Bond::new(1, 1.0);
        bond_approach.closing_speed = -1.0;
        let d1 = bond_viscous_damping(&a, &b, &bond_approach, &config);
        assert!(d1 > 0.0, "Approaching bond should be damped");

        // Receding bond
        let mut bond_recede = Bond::new(1, 1.0);
        bond_recede.closing_speed = 1.0;
        let d2 = bond_viscous_damping(&a, &b, &bond_recede, &config);
        assert!(d2 > 0.0, "Receding bond should also be damped");
    }
}
