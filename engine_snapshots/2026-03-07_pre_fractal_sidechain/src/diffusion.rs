use crate::config::SimConfig;
use crate::parcel::Parcel;

/// # STE Diffusion (Heat Equation on Bond Graph)
///
/// STE is energy.  Energy spreads.  High-concentration parcels
/// radiate STE outward through their bonds to lower-concentration
/// neighbours — exactly like heat conducting through matter.
///
/// ## The discrete heat equation on a graph
///
///   flux(j→i) = κ · (c_j − c_i) / d_ij
///   ΔE_i      = Σ_j  flux(j→i) · dt
///
/// κ = diffusivity — how fast energy spreads per unit concentration
/// gradient per unit distance.
///
/// ## Why heat "waits"
///
/// Real thermal diffusivity  α = k / (ρ · c_p):
///
///   - Copper:  1.1 × 10⁻⁴ m²/s  (fast — like sparse field)
///   - Water:   1.4 × 10⁻⁷ m²/s  (slow — like dense QGP)
///   - Rock:    1.0 × 10⁻⁶ m²/s  (medium)
///
/// The ~1000× ratio between fast and slow diffusion is what creates
/// visible structure: energy races through the dilute field but
/// *waits* at dense boundaries.  That waiting IS mass.  That waiting
/// IS why protons hold together — the QGP soup between quarks is so
/// dense that energy barely moves through it.
///
/// ## Conservation
///
/// Because each bond appears symmetrically (i has bond to j, j has
/// bond to i) and flux(i→j) = −flux(j→i), the net STE change across
/// the whole system sums to zero.  Energy is conserved.
///
/// ## Equilibrium feedback
///
/// Diffusion alone would equalise everything (heat death).  But the
/// tension wells *compress* geometry: they pull bonds shorter near
/// dense nodes → smaller volume → higher concentration.  This
/// compression pump counteracts the outward diffusion, maintaining
/// the density gradient — like gravity keeping the sun's core hot
/// even as it radiates outward.

/// Apply one tick of STE diffusion along all bonds.
///
/// For each bond, STE flows from the denser side to the sparser side.
/// Flow rate = κ · ΔC / d.  A hard floor at 0.0 prevents negative STE
/// if extreme parameters cause overdrain.
pub fn apply_diffusion(parcels: &mut [Parcel], config: &SimConfig) {
    let n = parcels.len();
    let dt = config.dt;
    let kappa = config.diffusivity;

    if kappa <= 0.0 || n == 0 {
        return;
    }

    // Snapshot concentrations for immutable reads.
    let concs: Vec<f64> = parcels.iter().map(|p| p.concentration).collect();
    let stes: Vec<f64> = parcels.iter().map(|p| p.ste_amount).collect();

    // Accumulate net STE change per parcel.
    // Positive = gaining energy.  Negative = losing energy.
    let mut delta: Vec<f64> = vec![0.0; n];

    for i in 0..n {
        for bond in &parcels[i].bonds {
            let j = bond.peer;
            if j >= n {
                continue;
            }

            // Concentration gradient drives flow: high → low.
            // flux > 0  means energy flows FROM j TO i.
            let dc = concs[j] - concs[i];
            let d = bond.distance.max(config.softening);

            // Viscosity-damped Fourier's law:
            //   flux = κ · ΔC / (d · (1 + η_avg))
            //
            // Thermal diffusivity α = k / (ρ·c_p).  Denser materials
            // conduct slower per unit gradient.  The viscosity term
            // (which scales with concentration) naturally throttles
            // diffusion in exactly the regions that need to hold their
            // structure — the D quark keeps its STE spike because the
            // dense soup around it resists energy flow.
            //
            // Dilute field: η ≈ 0, flux ≈ κ·ΔC/d (fast fountain ✔)
            // Dense core:   η » 1, flux ≈ κ·ΔC/(d·η) (throttled ✔)
            let eta_i = config.viscosity_base * concs[i].max(0.0).powf(config.viscosity_exponent);
            let eta_j = config.viscosity_base * concs[j].max(0.0).powf(config.viscosity_exponent);
            let eta_avg = 0.5 * (eta_i + eta_j);
            let flux = kappa * dc / (d * (1.0 + eta_avg));

            delta[i] += flux * dt;
        }
    }

    // Apply transfers.  Floor at 0 — STE cannot go negative.
    for i in 0..n {
        parcels[i].ste_amount = (stes[i] + delta[i]).max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bond::Bond;

    fn pair_with_bond(ste_a: f64, ste_b: f64, distance: f64) -> (Vec<Parcel>, SimConfig) {
        let mut a = Parcel::new(0, ste_a);
        let mut b = Parcel::new(1, ste_b);
        a.bonds.push(Bond::new(1, distance));
        b.bonds.push(Bond::new(0, distance));
        // Set concentrations proportional to STE for testing
        a.concentration = ste_a;
        b.concentration = ste_b;
        let mut config = SimConfig::default();
        config.diffusivity = 0.5;
        config.dt = 0.01;
        (vec![a, b], config)
    }

    #[test]
    fn energy_flows_from_hot_to_cold() {
        let (mut parcels, config) = pair_with_bond(10.0, 1.0, 1.0);
        let before_a = parcels[0].ste_amount;
        let before_b = parcels[1].ste_amount;
        apply_diffusion(&mut parcels, &config);
        assert!(
            parcels[0].ste_amount < before_a,
            "Hot parcel should lose STE: was {before_a}, now {}",
            parcels[0].ste_amount
        );
        assert!(
            parcels[1].ste_amount > before_b,
            "Cold parcel should gain STE: was {before_b}, now {}",
            parcels[1].ste_amount
        );
    }

    #[test]
    fn total_ste_conserved() {
        let (mut parcels, config) = pair_with_bond(10.0, 1.0, 1.0);
        let total_before: f64 = parcels.iter().map(|p| p.ste_amount).sum();
        apply_diffusion(&mut parcels, &config);
        let total_after: f64 = parcels.iter().map(|p| p.ste_amount).sum();
        assert!(
            (total_after - total_before).abs() < 1e-10,
            "Total STE must be conserved: before={total_before}, after={total_after}"
        );
    }

    #[test]
    fn no_flow_at_equilibrium() {
        let (mut parcels, config) = pair_with_bond(5.0, 5.0, 1.0);
        apply_diffusion(&mut parcels, &config);
        assert!(
            (parcels[0].ste_amount - 5.0).abs() < 1e-10,
            "No flow when concentrations are equal"
        );
    }

    #[test]
    fn distance_attenuates_flow() {
        let (mut close, config) = pair_with_bond(10.0, 1.0, 1.0);
        let (mut far, config2) = pair_with_bond(10.0, 1.0, 5.0);
        apply_diffusion(&mut close, &config);
        apply_diffusion(&mut far, &config2);
        let loss_close = 10.0 - close[0].ste_amount;
        let loss_far = 10.0 - far[0].ste_amount;
        assert!(
            loss_close > loss_far,
            "Closer bonds should conduct more: close_loss={loss_close}, far_loss={loss_far}"
        );
    }

    #[test]
    fn ste_never_goes_negative() {
        // Extreme: tiny STE parcel bonded to huge STE parcel with high diffusivity
        let mut a = Parcel::new(0, 0.001);
        let mut b = Parcel::new(1, 100.0);
        a.bonds.push(Bond::new(1, 0.1));
        b.bonds.push(Bond::new(0, 0.1));
        a.concentration = 0.001;
        b.concentration = 100.0;
        let mut config = SimConfig::default();
        config.diffusivity = 100.0;
        config.dt = 0.1;
        let mut parcels = vec![a, b];
        apply_diffusion(&mut parcels, &config);
        assert!(
            parcels[0].ste_amount >= 0.0,
            "STE must never go negative"
        );
        assert!(
            parcels[1].ste_amount >= 0.0,
            "STE must never go negative"
        );
    }

    #[test]
    fn disabled_when_kappa_zero() {
        let (mut parcels, mut config) = pair_with_bond(10.0, 1.0, 1.0);
        config.diffusivity = 0.0;
        apply_diffusion(&mut parcels, &config);
        assert!(
            (parcels[0].ste_amount - 10.0).abs() < 1e-10,
            "No diffusion when κ = 0"
        );
    }
}
