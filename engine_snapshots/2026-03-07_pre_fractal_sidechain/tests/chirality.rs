//! Chirality oscillator diagnostics test.
//!
//! This is a long-run telemetry test that ensures chirality metrics
//! are finite and internally consistent, and reports whether the run
//! ends in lock-in or ongoing oscillation.

use mass_effect_engine::config::SimConfig;
use mass_effect_engine::parcel::Parcel;
use mass_effect_engine::sim::Sim;

fn build_bubble(n: usize) -> Sim {
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
            if placed >= n {
                break;
            }
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
fn chirality_metrics_are_finite_and_consistent() {
    let mut sim = build_bubble(180);
    let ticks = 3000;

    for _ in 0..ticks {
        sim.step();
    }

    let d = sim
        .last_diagnostics
        .clone()
        .expect("diagnostics should exist after stepping");

    println!(
        "\nchiral: m+={} m-={} chi={:.4} |chi|={:.4} zeroX={} lock={} pairs={} dwell={:.2} pot={:.4} yield={:.4}",
        d.matter_count,
        d.anti_count,
        d.chirality,
        d.chirality_abs,
        d.chirality_zero_crossings,
        d.chirality_lock_ticks,
        d.compound_pair_count,
        d.compound_dwell_mean_ticks,
        d.compound_potential_sum,
        d.annihilation_energy_this_tick,
    );

    assert!(d.chirality.is_finite());
    assert!(d.chirality_abs.is_finite());
    assert!(d.compound_dwell_mean_ticks.is_finite());
    assert!(d.compound_potential_sum.is_finite());
    assert!(d.annihilation_energy_this_tick.is_finite());

    assert!(d.chirality_abs >= 0.0 && d.chirality_abs <= 1.0 + 1e-12);
    assert!(d.compound_pair_count <= sim.parcels.len() * sim.parcels.len() / 2);

    // If only one sign remains, lock ticks should be non-zero.
    if d.matter_count == 0 || d.anti_count == 0 {
        assert!(d.chirality_lock_ticks > 0);
    }
}
