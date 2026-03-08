//! Basic qubit-context smoke test.
//!
//! This is the quick "run through the paces" check:
//! - execute a small drive sweep
//! - verify expected capture-fraction shape
//! - verify threshold calibration outputs are sensible

use mass_effect_engine::config::SimConfig;
use mass_effect_engine::parcel::{ChokePhase, ChokeState, Parcel};
use mass_effect_engine::sim::Sim;

fn trial_capture(energy: f64, target_ste: f64) -> bool {
    let mut source = Parcel::new(0, 5.0);
    source.x = 0.0;

    let mut target = Parcel::new(1, target_ste);
    target.x = 0.25;
    target.choke = Some(ChokeState {
        phase: ChokePhase::Coherence,
        spin: 1.0,
        coherence: 1.0,
        radius: 1.0,
        equilibrium_concentration: target.concentration,
        age: 0.0,
    });
    target.vorticity = 0.2;
    target.shell_equilibrium = 0.5;
    target.shell_level = target.shell_equilibrium;

    let mut cfg = SimConfig::default();
    cfg.dt = 0.02;
    cfg.max_bond_distance = 2.0;
    cfg.diffusivity = 0.0;
    cfg.viscosity_base = 0.0;
    cfg.foam_spawn_fraction = 0.0;
    cfg.foam_annihilation_range = 0.0;

    let mut sim = Sim::new(vec![source, target], cfg);
    sim.emit_wave(0, energy, energy.sqrt());
    sim.step();

    sim.last_diagnostics
        .as_ref()
        .expect("diagnostics should exist")
        .terminal_capture_collapses
        > 0
}

fn capture_fraction(energy: f64, ste_values: &[f64]) -> f64 {
    let captures = ste_values
        .iter()
        .filter(|ste| trial_capture(energy, **ste))
        .count();
    captures as f64 / ste_values.len().max(1) as f64
}

fn threshold_energy(sweep: &[(f64, f64)], target_capture: f64) -> Option<f64> {
    if sweep.len() < 2 {
        return None;
    }
    for i in 0..(sweep.len() - 1) {
        let (e0, f0) = sweep[i];
        let (e1, f1) = sweep[i + 1];
        if (f0 <= target_capture && target_capture <= f1) || (f1 <= target_capture && target_capture <= f0) {
            let df = f1 - f0;
            if df.abs() < 1e-12 {
                return Some(0.5 * (e0 + e1));
            }
            let t = (target_capture - f0) / df;
            return Some(e0 + t * (e1 - e0));
        }
    }
    None
}

#[test]
fn qubit_context_smoke_expected_outputs() {
    let ste_values: Vec<f64> = (0..25)
        .map(|i| 0.8 + (i as f64) * (0.4 / 24.0))
        .collect();

    let energies = [0.6, 1.0, 1.4, 1.8, 2.2];
    let sweep: Vec<(f64, f64)> = energies
        .iter()
        .map(|e| (*e, capture_fraction(*e, &ste_values)))
        .collect();

    let e10 = threshold_energy(&sweep, 0.10).expect("E10 should resolve");
    let e50 = threshold_energy(&sweep, 0.50).expect("E50 should resolve");
    let e90 = threshold_energy(&sweep, 0.90).expect("E90 should resolve");

    println!(
        "\nQUBIT SMOKE | sweep={:?} | E10={:.3} E50={:.3} E90={:.3}",
        sweep, e10, e50, e90
    );

    // Expected shape: relay-dominant -> mixed -> capture-dominant.
    assert!(sweep[0].1 <= 0.05, "E=0.6 capture fraction too high: {}", sweep[0].1);
    assert!(sweep[1].1 <= 0.05, "E=1.0 capture fraction too high: {}", sweep[1].1);
    assert!(sweep[2].1 >= 0.05 && sweep[2].1 <= 0.25, "E=1.4 out of expected mixed edge: {}", sweep[2].1);
    assert!(sweep[3].1 >= 0.35 && sweep[3].1 <= 0.70, "E=1.8 out of expected mixed center: {}", sweep[3].1);
    assert!(sweep[4].1 >= 0.80, "E=2.2 capture fraction too low: {}", sweep[4].1);

    // Calibration sanity windows.
    assert!(e10 < e50 && e50 < e90, "threshold ordering broken");
    assert!((1.60..=1.95).contains(&e50), "E50 outside expected calibration band: {e50}");
    assert!((0.60..=1.10).contains(&(e90 - e10)), "transition width outside expected band: {}", e90 - e10);
}
