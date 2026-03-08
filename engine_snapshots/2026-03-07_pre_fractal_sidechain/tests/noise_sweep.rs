/// # STE Noise-Level Sweep
///
/// Sweeps initial STE per parcel from "supercold" (0.1) to "dusty plasma" (100.0)
/// and measures emergent behavior at each level.
///
/// Question: is the behavior curve linear or exponential?
///
/// Metrics collected per run:
///   - avg_speed: average parcel velocity (kinetic chaos)
///   - max_speed: fastest parcel (peak chaos) — if this > wave_speed, causality broken
///   - choke_count: stable micro-vortices at end of run
///   - total_choke_ticks: cumulative chokes across all ticks (choke persistence)
///   - ste_variance: spread of STE values (structure vs uniformity)
///   - avg_concentration: average surface concentration
///   - foam_spawns: total bubble detachments
///   - foam_annihilations: total annihilation events
///   - wave_speed: average distance a light wave covers per tick (c)
///   - causality: max_speed / wave_speed (>1 = FTL, broken causality)

use mass_effect_engine::config::SimConfig;
use mass_effect_engine::parcel::Parcel;
use mass_effect_engine::sim::Sim;

fn build_bubble(n: usize, ste_per_parcel: f64) -> Sim {
    let pi = std::f64::consts::PI;
    let golden_angle = pi * (3.0 - (5.0_f64).sqrt());

    let mut parcels = Vec::with_capacity(n);
    let mut seed: u64 = 7777;
    let rng = |state: &mut u64| -> f64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        (*state as f64) / (u64::MAX as f64)
    };

    let shells = 6;
    let per_shell_base = n / shells;
    let mut placed = 0;

    for shell in 0..shells {
        let r = 2.5 + (shell as f64) * 1.8;
        let count = if shell < shells - 1 {
            per_shell_base + shell * 2
        } else {
            n - placed
        };
        for j in 0..count {
            if placed >= n { break; }
            let y_frac = 1.0 - (j as f64 / (count as f64 - 1.0).max(1.0)) * 2.0;
            let phi = (j as f64) * golden_angle + (shell as f64) * 0.37;
            let cos_theta = y_frac;
            let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
            let jr = r + (rng(&mut seed) - 0.5) * 0.6;
            let jphi = phi + (rng(&mut seed) - 0.5) * 0.15;

            // Small jitter around the target STE
            let ste = ste_per_parcel * (0.9 + rng(&mut seed) * 0.2);
            let mut p = Parcel::new(placed as u64, ste);
            p.x = jr * sin_theta * jphi.cos();
            p.y = jr * sin_theta * jphi.sin();
            p.z = jr * cos_theta;
            parcels.push(p);
            placed += 1;
        }
    }

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

    Sim::new(parcels, config)
}

#[test]
fn noise_level_sweep() {
    let n_parcels = 100; // smaller for speed
    let ticks = 2000;

    // STE levels: extreme range — from near-void to plasma inferno
    // Skip 4000+ which cause NaN explosions with 100 parcels
    let ste_levels: Vec<f64> = vec![
        0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 4.0, 16.0, 64.0,
        256.0, 1000.0,
    ];

    println!("\n{:>8} {:>10} {:>10} {:>8} {:>12} {:>12} {:>10} {:>10} {:>8} {:>8} {:>8}",
        "STE/p", "avg_speed", "max_speed", "chokes", "choke_ticks", "ste_var",
        "avg_conc", "wave_c", "v/c", "spawns", "annihi");
    println!("{}", "-".repeat(130));

    for &ste in &ste_levels {
        let mut sim = build_bubble(n_parcels, ste);

        let mut total_choke_ticks: u64 = 0;
        let mut total_foam_spawns: u64 = 0;
        let mut total_foam_annih: u64 = 0;

        for _ in 0..ticks {
            let metrics = sim.step();
            total_choke_ticks += metrics.choke_count as u64;
            total_foam_spawns += metrics.foam_spawns as u64;
            total_foam_annih += metrics.foam_annihilations as u64;
        }

        // End-state measurements
        let _n = sim.parcels.len() as f64;

        // Speeds
        let speeds: Vec<f64> = sim.parcels.iter()
            .filter(|p| p.speed().is_finite())
            .map(|p| p.speed())
            .collect();
        let avg_speed = if speeds.is_empty() { 0.0 }
            else { speeds.iter().sum::<f64>() / speeds.len() as f64 };
        let max_speed = speeds.iter().cloned().fold(0.0_f64, f64::max);

        // Chokes at end
        let choke_count = sim.parcels.iter().filter(|p| p.is_choked()).count();

        // STE variance (only finite parcels)
        let stes: Vec<f64> = sim.parcels.iter()
            .filter(|p| p.ste_amount.is_finite())
            .map(|p| p.ste_amount)
            .collect();
        let avg_ste = if stes.is_empty() { 0.0 }
            else { stes.iter().sum::<f64>() / stes.len() as f64 };
        let ste_var = if stes.is_empty() { 0.0 }
            else { stes.iter().map(|s| (s - avg_ste).powi(2)).sum::<f64>() / stes.len() as f64 };

        // Average concentration
        let concs: Vec<f64> = sim.parcels.iter()
            .filter(|p| p.concentration.is_finite())
            .map(|p| p.concentration)
            .collect();
        let avg_conc = if concs.is_empty() { 0.0 }
            else { concs.iter().sum::<f64>() / concs.len() as f64 };

        // Wave speed (c): average nearest-neighbor distance × hops_per_tick / dt
        // This is the speed of light in spatial units per time unit.
        let finite_parcels: Vec<&mass_effect_engine::parcel::Parcel> = sim.parcels.iter()
            .filter(|p| p.x.is_finite() && p.y.is_finite() && p.z.is_finite())
            .collect();
        let wave_c = if finite_parcels.len() > 1 {
            let mut total_nn_dist = 0.0;
            for i in 0..finite_parcels.len() {
                let mut min_d = f64::MAX;
                for j in 0..finite_parcels.len() {
                    if i == j { continue; }
                    let dx = finite_parcels[j].x - finite_parcels[i].x;
                    let dy = finite_parcels[j].y - finite_parcels[i].y;
                    let dz = finite_parcels[j].z - finite_parcels[i].z;
                    let d = (dx*dx + dy*dy + dz*dz).sqrt();
                    if d.is_finite() && d < min_d { min_d = d; }
                }
                total_nn_dist += min_d;
            }
            let avg_nn = total_nn_dist / finite_parcels.len() as f64;
            // c = emergent: avg_hop_distance / (avg_concentration × dt)
            // Denser field = slower c.  This mirrors the engine's time-budget model.
            let avg_conc_for_c = concs.iter().sum::<f64>() / concs.len().max(1) as f64;
            avg_nn / (avg_conc_for_c.max(1e-10) * sim.config.dt)
        } else {
            0.0
        };

        // Causality ratio: max_speed / wave_speed. >1 = FTL
        let v_over_c = if wave_c > 1e-10 { max_speed / wave_c } else { 0.0 };

        println!("{:>8.3} {:>10.4} {:>10.4} {:>8} {:>12} {:>12.4} {:>10.4} {:>10.2} {:>8.4} {:>8} {:>8}",
            ste, avg_speed, max_speed, choke_count, total_choke_ticks,
            ste_var, avg_conc, wave_c, v_over_c, total_foam_spawns, total_foam_annih);
    }

    // Always pass — this is a data-collection test
    assert!(true);
}
