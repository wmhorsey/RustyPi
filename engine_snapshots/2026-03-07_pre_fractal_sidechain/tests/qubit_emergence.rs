//! Minimal qubit-like emergence probe.
//!
//! This test does not claim full hardware-level qubit physics. It checks whether
//! a controlled drive context produces a two-state-style readout split:
//! - low drive: mostly transient relay (no terminal capture)
//! - high drive: mostly terminal capture
//! - mid drive: mixed outcomes across a slightly heterogeneous ensemble

use std::fs;
use std::path::PathBuf;

use mass_effect_engine::config::SimConfig;
use mass_effect_engine::parcel::{ChokePhase, ChokeState, Parcel};
use mass_effect_engine::sim::Sim;

const E50_MIN: f64 = 1.60;
const E50_MAX: f64 = 1.95;
const TRANSITION_WIDTH_MIN: f64 = 0.60;
const TRANSITION_WIDTH_MAX: f64 = 1.10;

fn trial_capture(energy: f64, target_ste: f64) -> (bool, u64, u64) {
    let mut source = Parcel::new(0, 5.0);
    source.x = 0.0;
    source.y = 0.0;
    source.z = 0.0;

    let mut target = Parcel::new(1, target_ste);
    target.x = 0.25;
    target.y = 0.0;
    target.z = 0.0;

    // Force a rotational relay node so wave interaction survives
    // the per-step vorticity recompute.
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

    let d = sim
        .last_diagnostics
        .as_ref()
        .expect("diagnostics should be available after a step");

    (
        d.terminal_capture_collapses > 0,
        d.transient_relay_collapses,
        d.terminal_capture_collapses,
    )
}

fn capture_fraction(energy: f64, ste_values: &[f64]) -> (f64, u64, u64) {
    let mut captures = 0_u64;
    let mut relays = 0_u64;
    let mut terminals = 0_u64;

    for ste in ste_values {
        let (captured, relay, terminal) = trial_capture(energy, *ste);
        if captured {
            captures += 1;
        }
        relays += relay;
        terminals += terminal;
    }

    let frac = captures as f64 / ste_values.len().max(1) as f64;
    (frac, relays, terminals)
}

fn threshold_energy(sweep: &[(f64, f64, u64, u64)], target_capture: f64) -> Option<f64> {
    if sweep.len() < 2 {
        return None;
    }

    for i in 0..(sweep.len() - 1) {
        let (e0, f0, _, _) = sweep[i];
        let (e1, f1, _, _) = sweep[i + 1];
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
fn qubit_like_threshold_behavior_emerges_in_controlled_context() {
    // Slightly heterogeneous ensemble acts like many nominally-identical devices
    // with tiny fabrication/environment variation.
    let ste_values: Vec<f64> = (0..25)
        .map(|i| 0.8 + (i as f64) * (0.4 / 24.0))
        .collect();

    let drive_energies = [0.6, 1.0, 1.4, 1.8, 2.2];
    let mut sweep: Vec<(f64, f64, u64, u64)> = Vec::new();
    for energy in drive_energies {
        let (frac, relays, terminals) = capture_fraction(energy, &ste_values);
        sweep.push((energy, frac, relays, terminals));
    }

    let low = sweep.first().expect("sweep should have at least one point");
    let high = sweep.last().expect("sweep should have at least one point");
    let mixed_exists = sweep.iter().any(|(_, frac, _, _)| *frac > 0.0 && *frac < 1.0);

    let gate_high_gt_low = low.1 < high.1;
    let gate_mixed_window = mixed_exists;

    let e10 = threshold_energy(&sweep, 0.10);
    let e50 = threshold_energy(&sweep, 0.50);
    let e90 = threshold_energy(&sweep, 0.90);
    let gate_thresholds_resolved = e10.is_some() && e50.is_some() && e90.is_some();
    let gate_threshold_ordering = match (e10, e50, e90) {
        (Some(a), Some(b), Some(c)) => a < b && b < c,
        _ => false,
    };
    let gate_e50_in_band = match e50 {
        Some(v) => v >= E50_MIN && v <= E50_MAX,
        None => false,
    };
    let transition_width = match (e10, e90) {
        (Some(a), Some(c)) => Some(c - a),
        _ => None,
    };
    let gate_transition_width_in_band = match transition_width {
        Some(w) => w >= TRANSITION_WIDTH_MIN && w <= TRANSITION_WIDTH_MAX,
        None => false,
    };

    let mut csv = String::from(
        "drive_energy,capture_fraction,relay_collapses,terminal_captures\n",
    );
    for (energy, frac, relays, terminals) in &sweep {
        csv.push_str(&format!(
            "{:.6},{:.6},{},{}\n",
            energy,
            frac,
            relays,
            terminals,
        ));
    }

    let summary = serde_json::json!({
        "probe": "qubit_like_threshold_context",
        "gates": {
            "high_drive_capture_gt_low_drive": gate_high_gt_low,
            "mixed_window_exists": gate_mixed_window,
            "thresholds_resolved": gate_thresholds_resolved,
            "threshold_ordering_e10_lt_e50_lt_e90": gate_threshold_ordering,
            "e50_in_band": gate_e50_in_band,
            "transition_width_in_band": gate_transition_width_in_band
        },
        "thresholds": {
            "E10": e10,
            "E50": e50,
            "E90": e90,
            "transition_width": transition_width
        },
        "bands": {
            "E50_min": E50_MIN,
            "E50_max": E50_MAX,
            "transition_width_min": TRANSITION_WIDTH_MIN,
            "transition_width_max": TRANSITION_WIDTH_MAX
        },
        "sweep": sweep.iter().map(|(energy, frac, relays, terminals)| serde_json::json!({
            "drive_energy": energy,
            "capture_fraction": frac,
            "relay_collapses": relays,
            "terminal_captures": terminals
        })).collect::<Vec<_>>()
    });

    let mut out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    out_dir.push("target");
    out_dir.push("diagnostics");
    fs::create_dir_all(&out_dir).expect("failed to create diagnostics output dir");

    let csv_path = out_dir.join("qubit_emergence.csv");
    let json_path = out_dir.join("qubit_emergence_summary.json");
    fs::write(&csv_path, csv).expect("failed to write qubit emergence csv");
    fs::write(&json_path, serde_json::to_string_pretty(&summary).unwrap())
        .expect("failed to write qubit emergence summary");

    println!(
        "\nQUBIT PROBE | low E={:.2} frac={:.3} -> high E={:.2} frac={:.3} | mixed_window={} | E10={:?} E50={:?} E90={:?} dE={:?}",
        low.0,
        low.1,
        high.0,
        high.1,
        mixed_exists,
        e10,
        e50,
        e90,
        transition_width,
    );

    assert!(gate_high_gt_low, "higher drive should capture more than lower drive");

    // A mixed regime indicates context-dependent two-state-style readout,
    // rather than all-relay or all-capture behavior.
    assert!(gate_mixed_window, "expected at least one mixed capture regime in drive sweep");
    assert!(gate_thresholds_resolved, "expected E10/E50/E90 thresholds to be resolvable");
    assert!(gate_threshold_ordering, "expected E10 < E50 < E90 ordering");
    assert!(gate_e50_in_band, "expected E50 to stay within calibration band [{E50_MIN}, {E50_MAX}] (got {e50:?})");
    assert!(
        gate_transition_width_in_band,
        "expected transition width E90-E10 within band [{TRANSITION_WIDTH_MIN}, {TRANSITION_WIDTH_MAX}] (got {transition_width:?})"
    );

    assert!(low.2 > 0, "low drive should include relay collapses");
    assert!(high.3 > 0, "high drive should include terminal captures");
}
