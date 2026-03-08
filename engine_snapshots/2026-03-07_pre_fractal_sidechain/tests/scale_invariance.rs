//! Scale-invariance diagnostics runner.
//!
//! Compares normalized observables for the same dimensionless setup
//! at two scales ("proton" and "nebula") to test collapse behavior.

use mass_effect_engine::config::SimConfig;
use mass_effect_engine::field;
use mass_effect_engine::parcel::Parcel;
use mass_effect_engine::sim::Sim;

#[derive(Debug, Clone)]
struct ScaleSummary {
    name: &'static str,
    scale: f64,
    n: usize,
    ticks: usize,
    ste_drift: f64,
    ar_residual: f64,
    avg_saturation: f64,
    pressure_front_fraction: f64,
    avg_choke_fraction: f64,
    avg_matter_fraction: f64,
    avg_anti_fraction: f64,
    final_mean_radius: f64,
    final_mean_radius_norm: f64,
}

fn build_scaled_bubble(n: usize, ste_base: f64, geom_scale: f64, ste_scale: f64, dt: f64) -> (Sim, f64) {
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
        let r = geom_scale * (2.5 + (shell as f64) * 1.8);
        let count = if shell < shells - 1 {
            per_shell_base + shell * 2
        } else {
            n - placed
        };

        for j in 0..count {
            if placed >= n {
                break;
            }
            let y_frac = 1.0 - (j as f64 / (count as f64 - 1.0).max(1.0)) * 2.0;
            let phi = (j as f64) * golden_angle + (shell as f64) * 0.37;
            let cos_theta = y_frac;
            let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
            let jr = r + geom_scale * (rng(&mut seed) - 0.5) * 0.6;
            let jphi = phi + (rng(&mut seed) - 0.5) * 0.15;

            let ste = ste_scale * ste_base * (0.9 + rng(&mut seed) * 0.2);
            let mut p = Parcel::new(placed as u64, ste);
            p.x = jr * sin_theta * jphi.cos();
            p.y = jr * sin_theta * jphi.sin();
            p.z = jr * cos_theta;
            parcels.push(p);
            placed += 1;
        }
    }

    let mut config = SimConfig::default();
    config.dt = dt;
    config.max_bond_distance = 15.0 * geom_scale;
    config.shell_interaction_range = 1.5 * geom_scale;
    config.parcel_radius_scale = 0.3 * geom_scale;
    config.diffusivity = 0.5;
    config.viscosity_base = 0.08;
    config.viscosity_exponent = 1.5;
    // Disable foam side-effects for clean scale diagnostics.
    config.foam_spawn_coherence = 2.0;
    config.foam_spawn_fraction = 0.0;
    config.foam_spawn_min_ste = f64::MAX;
    config.foam_annihilation_range = 0.0;

    let mean_r0 = mean_centroid_radius(&parcels);
    (Sim::new(parcels, config), mean_r0)
}

fn mean_centroid_radius(parcels: &[Parcel]) -> f64 {
    if parcels.is_empty() {
        return 0.0;
    }

    let mut cx = 0.0;
    let mut cy = 0.0;
    let mut cz = 0.0;
    for p in parcels {
        cx += p.x;
        cy += p.y;
        cz += p.z;
    }
    let inv_n = 1.0 / parcels.len() as f64;
    cx *= inv_n;
    cy *= inv_n;
    cz *= inv_n;

    let mut sum = 0.0;
    for p in parcels {
        let dx = p.x - cx;
        let dy = p.y - cy;
        let dz = p.z - cz;
        sum += (dx * dx + dy * dy + dz * dz).sqrt();
    }
    sum * inv_n
}

fn run_scale_case(name: &'static str, scale: f64, n: usize, dt: f64, physical_time: f64) -> ScaleSummary {
    // Baseline invariance test: scale geometry while keeping STE bounded.
    let ste_scale = 1.0;
    let (mut sim, mean_r0) = build_scaled_bubble(n, 3.5, scale, ste_scale, dt);
    let initial_ste: f64 = sim.parcels.iter().map(|p| p.ste_amount).sum();

    let ticks = (physical_time / dt).round() as usize;

    let mut sat_sum = 0.0;
    let mut sat_count = 0usize;
    let mut front_sum = 0.0;
    let mut choke_sum = 0.0;
    let mut matter_sum = 0.0;
    let mut anti_sum = 0.0;

    for _ in 0..ticks {
        sim.step();

        let (sats, pressures) = field::saturation_pressure_state(&sim.parcels, &sim.config);
        let max_p = pressures
            .iter()
            .cloned()
            .fold(0.0_f64, f64::max)
            .max(1e-10);

        let high_fronts = pressures.iter().filter(|&&p| p > 0.5 * max_p).count();
        front_sum += high_fronts as f64 / sim.parcels.len().max(1) as f64;

        sat_sum += sats.iter().sum::<f64>();
        sat_count += sats.len();

        let mut choked = 0usize;
        let mut matter = 0usize;
        let mut anti = 0usize;
        for p in &sim.parcels {
            if let Some(c) = &p.choke {
                choked += 1;
                if c.spin >= 0.0 {
                    matter += 1;
                } else {
                    anti += 1;
                }
            }
        }

        let inv_n = 1.0 / sim.parcels.len().max(1) as f64;
        choke_sum += choked as f64 * inv_n;
        matter_sum += matter as f64 * inv_n;
        anti_sum += anti as f64 * inv_n;
    }

    let d = sim
        .last_diagnostics
        .clone()
        .expect("diagnostics should be present after stepping sim");

    let final_mean_r = mean_centroid_radius(&sim.parcels);

    ScaleSummary {
        name,
        scale,
        n,
        ticks,
        ste_drift: (d.total_ste - initial_ste).abs() / initial_ste.max(1e-10),
        ar_residual: d.action_reaction_residual,
        avg_saturation: sat_sum / sat_count.max(1) as f64,
        pressure_front_fraction: front_sum / ticks.max(1) as f64,
        avg_choke_fraction: choke_sum / ticks.max(1) as f64,
        avg_matter_fraction: matter_sum / ticks.max(1) as f64,
        avg_anti_fraction: anti_sum / ticks.max(1) as f64,
        final_mean_radius: final_mean_r,
        final_mean_radius_norm: final_mean_r / mean_r0.max(1e-10),
    }
}

#[test]
fn scale_invariance_baseline() {
    // Same dimensionless setup, two scales.
    let proton = run_scale_case("proton", 1.0, 120, 0.008, 1.2);
    let nebula = run_scale_case("nebula", 2.0, 120, 0.008, 1.2);

    println!(
        "\n{:>8} {:>8} {:>8} {:>8} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "name", "scale", "n", "ticks", "ste_drift", "AR_res",
        "Savg", "fronts", "choke", "matter", "anti"
    );

    for r in [&proton, &nebula] {
        println!(
            "{:>8} {:>8.2} {:>8} {:>8} {:>10.3e} {:>10.3e} {:>10.4} {:>10.4} {:>10.4} {:>10.4} {:>10.4}",
            r.name,
            r.scale,
            r.n,
            r.ticks,
            r.ste_drift,
            r.ar_residual,
            r.avg_saturation,
            r.pressure_front_fraction,
            r.avg_choke_fraction,
            r.avg_matter_fraction,
            r.avg_anti_fraction,
        );
    }

    println!(
        "mean radius norm: proton={:.4}, nebula={:.4}",
        proton.final_mean_radius_norm,
        nebula.final_mean_radius_norm,
    );
    println!(
        "mean radius raw: proton={:.4}, nebula={:.4}",
        proton.final_mean_radius,
        nebula.final_mean_radius,
    );

    // Hard correctness checks.
    assert!(proton.ste_drift.is_finite());
    assert!(nebula.ste_drift.is_finite());
    assert!(proton.ar_residual.is_finite());
    assert!(nebula.ar_residual.is_finite());
    assert!(proton.final_mean_radius_norm.is_finite());
    assert!(nebula.final_mean_radius_norm.is_finite());
    assert!(proton.ste_drift < 1e-6);
    assert!(nebula.ste_drift < 1e-6);
    assert!(proton.ar_residual < 1e-8);
    assert!(nebula.ar_residual < 1e-8);

    // Soft similarity checks: same law stack should remain same order of behavior.
    let sat_ratio = proton.avg_saturation / nebula.avg_saturation.max(1e-10);
    let choke_ratio = proton.avg_choke_fraction / nebula.avg_choke_fraction.max(1e-10);
    let front_ratio = proton.pressure_front_fraction / nebula.pressure_front_fraction.max(1e-10);

    assert!(sat_ratio > 0.5 && sat_ratio < 2.0, "Savg ratio out of range: {sat_ratio}");
    assert!(choke_ratio > 0.2 && choke_ratio < 5.0, "choke ratio out of range: {choke_ratio}");
    assert!(front_ratio > 0.2 && front_ratio < 5.0, "front ratio out of range: {front_ratio}");
}
