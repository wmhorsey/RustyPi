//! Smoke tests for the all-pairs field-based STE engine.
//!
//! Checks:
//! 1. The sim loop doesn't panic.
//! 2. STE is conserved (total_ste unchanged across ticks).
//! 3. Parcels attract each other (distances shrink).
//! 4. An attraction wave propagates and eventually captures.

use mass_effect_engine::config::SimConfig;
use mass_effect_engine::sim::Sim;
use mass_effect_engine::parcel::Parcel;

/// Build `n` parcels arranged in a ring in the XY plane with given `radius`.
/// Each parcel carries 1.0 STE.
fn make_ring(n: usize, radius: f64) -> Vec<Parcel> {
    use std::f64::consts::PI;
    (0..n)
        .map(|i| {
            let angle = 2.0 * PI * (i as f64) / (n as f64);
            let mut p = Parcel::new(i as u64, 1.0);
            p.x = radius * angle.cos();
            p.y = radius * angle.sin();
            p.z = 0.0;
            p
        })
        .collect()
}

#[test]
fn sim_runs_without_panic() {
    let parcels = make_ring(20, 4.0);
    let config = SimConfig::default();
    let mut sim = Sim::new(parcels, config);

    for _ in 0..100 {
        let _metrics = sim.step();
    }

    assert_eq!(sim.tick, 100);
}

#[test]
fn ste_is_conserved() {
    let parcels = make_ring(20, 4.0);
    let config = SimConfig::default();
    let initial_ste: f64 = parcels.iter().map(|p| p.ste_amount).sum();
    let mut sim = Sim::new(parcels, config);

    let mut final_ste = initial_ste;
    for _ in 0..100 {
        let metrics = sim.step();
        final_ste = metrics.total_ste;
    }

    let error = (final_ste - initial_ste).abs() / initial_ste;
    assert!(
        error < 1e-10,
        "STE not conserved: initial={initial_ste}, final={final_ste}, error={error}"
    );
}

#[test]
fn parcels_attract_each_other() {
    // Place two parcels far apart.  All-pairs attraction should
    // pull them closer together.
    let mut parcels = vec![
        Parcel::new(0, 1.0),
        Parcel::new(1, 1.0),
    ];
    parcels[0].x = -5.0;
    parcels[1].x = 5.0;

    let initial_dist = parcels[0].dist_to(&parcels[1]);

    let config = SimConfig::default();
    let mut sim = Sim::new(parcels, config);

    for _ in 0..200 {
        sim.step();
    }

    let final_dist = sim.parcels[0].dist_to(&sim.parcels[1]);
    assert!(
        final_dist < initial_dist,
        "Expected parcels to attract: initial={initial_dist}, final={final_dist}"
    );
}

#[test]
fn attraction_wave_propagates_and_captures() {
    // Line of parcels along X axis with alternating velocities
    // to create vorticity, plus one weak node easy to capture.
    let n = 10;
    let mut parcels: Vec<Parcel> = (0..n)
        .map(|i| {
            let mut p = Parcel::new(i as u64, 1.0);
            p.x = 2.0 * (i as f64);
            p.y = 0.0;
            p.z = 0.0;
            // Alternating transverse velocity → vorticity
            p.vy = if i % 2 == 0 { 0.5 } else { -0.5 };
            p.shell_equilibrium = 1.0;
            p.shell_level = 1.0;
            p
        })
        .collect();

    // Make one node easy to capture (very low equilibrium).
    parcels[5].shell_equilibrium = 0.001;
    parcels[5].shell_level = 0.001;

    let config = SimConfig::default();
    let mut sim = Sim::new(parcels, config);

    // Emit a wave from parcel 0.
    sim.emit_wave(0, 5.0, 1.0);
    assert_eq!(sim.waves.len(), 1);

    // Run until capture or 50 ticks.
    let mut total_captures = 0;
    for _ in 0..50 {
        let metrics = sim.step();
        total_captures += metrics.captures_this_tick;
        if total_captures > 0 {
            break;
        }
    }

    assert!(
        total_captures > 0,
        "Expected at least one capture event (photon particle)"
    );
}
