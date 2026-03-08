//! Timestep convergence checks for the STE engine.
//!
//! This test compares the same physical setup at dt, dt/2, dt/4 and
//! checks that key observables move toward convergence as dt shrinks.

use mass_effect_engine::config::SimConfig;
use mass_effect_engine::parcel::Parcel;
use mass_effect_engine::sim::Sim;

#[derive(Debug, Clone)]
struct RunSummary {
    dt: f64,
    ticks: usize,
    total_ste: f64,
    ste_drift: f64,
    linear_momentum_mag: f64,
    angular_momentum_mag: f64,
    action_reaction_residual: f64,
    mean_radius: f64,
    kinetic_energy: f64,
}

fn build_case(n: usize, ste_per_parcel: f64, dt: f64) -> Sim {
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

    let shells = 4;
    let per_shell_base = n / shells;
    let mut placed = 0;

    for shell in 0..shells {
        let r = 2.0 + (shell as f64) * 1.6;
        let count = if shell < shells - 1 {
            per_shell_base + shell
        } else {
            n - placed
        };

        for j in 0..count {
            if placed >= n {
                break;
            }

            let y_frac = 1.0 - (j as f64 / (count as f64 - 1.0).max(1.0)) * 2.0;
            let phi = (j as f64) * golden_angle + (shell as f64) * 0.31;
            let cos_theta = y_frac;
            let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
            let jr = r + (rng(&mut seed) - 0.5) * 0.35;
            let jphi = phi + (rng(&mut seed) - 0.5) * 0.12;

            let ste = ste_per_parcel * (0.92 + rng(&mut seed) * 0.16);
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
    config.max_bond_distance = 12.0;
    config.parcel_radius_scale = 0.3;
    config.diffusivity = 0.4;
    config.viscosity_base = 0.07;
    config.viscosity_exponent = 1.5;
    config.shell_interaction_range = 1.8;
    config.foam_spawn_coherence = 0.4;
    config.foam_spawn_fraction = 0.05;
    config.foam_spawn_min_ste = 0.02;
    config.foam_annihilation_range = 2.0;

    Sim::new(parcels, config)
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

    let mut sum_r = 0.0;
    for p in parcels {
        let dx = p.x - cx;
        let dy = p.y - cy;
        let dz = p.z - cz;
        sum_r += (dx * dx + dy * dy + dz * dz).sqrt();
    }
    sum_r * inv_n
}

fn run_case(dt: f64, physical_time: f64) -> RunSummary {
    let mut sim = build_case(60, 2.5, dt);
    let initial_ste: f64 = sim.parcels.iter().map(|p| p.ste_amount).sum();
    let ticks = (physical_time / dt).round() as usize;

    for _ in 0..ticks {
        sim.step();
    }

    let d = sim
        .last_diagnostics
        .clone()
        .expect("diagnostics should exist after stepping the sim");

    RunSummary {
        dt,
        ticks,
        total_ste: d.total_ste,
        ste_drift: (d.total_ste - initial_ste).abs() / initial_ste.max(1e-10),
        linear_momentum_mag: d.linear_momentum_mag,
        angular_momentum_mag: d.angular_momentum_mag,
        action_reaction_residual: d.action_reaction_residual,
        mean_radius: mean_centroid_radius(&sim.parcels),
        kinetic_energy: d.kinetic_energy,
    }
}

#[test]
fn timestep_convergence_baseline() {
    let physical_time = 1.0;

    let coarse = run_case(0.008, physical_time);
    let medium = run_case(0.004, physical_time);
    let fine = run_case(0.002, physical_time);

    println!(
        "\n{:>8} {:>8} {:>12} {:>12} {:>12} {:>12} {:>12} {:>12} {:>12}",
        "dt", "ticks", "STE", "ste_drift", "|P|", "|L|", "E_kin", "AR_res", "mean_r"
    );
    for r in [&coarse, &medium, &fine] {
        println!(
            "{:>8.4} {:>8} {:>12.6} {:>12.3e} {:>12.6} {:>12.6} {:>12.6} {:>12.6e} {:>12.6}",
            r.dt,
            r.ticks,
            r.total_ste,
            r.ste_drift,
            r.linear_momentum_mag,
            r.angular_momentum_mag,
            r.kinetic_energy,
            r.action_reaction_residual,
            r.mean_radius,
        );
    }

    // Action-reaction residual should remain tiny.
    assert!(coarse.action_reaction_residual < 1e-8);
    assert!(medium.action_reaction_residual < 1e-8);
    assert!(fine.action_reaction_residual < 1e-8);

    // Convergence check on morphology proxy: mean radius.
    // Error should decrease as dt shrinks.
    let e_coarse_medium = (coarse.mean_radius - medium.mean_radius).abs();
    let e_medium_fine = (medium.mean_radius - fine.mean_radius).abs();

    println!(
        "mean_r deltas: |dt-dt/2|={:.6e}, |dt/2-dt/4|={:.6e}",
        e_coarse_medium, e_medium_fine
    );

    assert!(
        e_medium_fine <= e_coarse_medium,
        "expected finer dt to move toward convergence: e_cm={e_coarse_medium}, e_mf={e_medium_fine}"
    );
}
