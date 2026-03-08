//! Geometry-mode dispersion probe.
//!
//! Tests whether two pulse geometries (with signed twist channel) remain
//! separable across field-density strata, and whether output vectors shift
//! with density (compressibility response).

use std::fs;
use std::path::PathBuf;

use mass_effect_engine::config::SimConfig;
use mass_effect_engine::parcel::{ChokePhase, ChokeState, Parcel};
use mass_effect_engine::sim::Sim;

#[derive(Debug, Clone, Copy)]
struct Pulse {
    energy: f64,
    twist: i8, // +1 or -1 (encoded by source geometry)
}

#[derive(Debug, Clone)]
struct Mode {
    name: &'static str,
    pulses: [Pulse; 3],
}

#[derive(Debug, Clone, Copy)]
struct OutputVec {
    capture_ratio: f64,
    overshoot_mean: f64,
    relax_mean: f64,
}

fn build_sim(target_ste: f64) -> Sim {
    // Two source geometries around one central target node.
    let mut source_plus = Parcel::new(0, 5.0);
    source_plus.x = -0.25;
    source_plus.y = 0.0;
    source_plus.z = 0.0;

    let mut source_minus = Parcel::new(1, 5.0);
    source_minus.x = 0.0;
    source_minus.y = -0.25;
    source_minus.z = 0.0;

    let mut target = Parcel::new(2, target_ste);
    target.x = 0.0;
    target.y = 0.0;
    target.z = 0.0;
    target.choke = Some(ChokeState {
        phase: ChokePhase::Coherence,
        spin: 1.0,
        coherence: 1.0,
        radius: 1.0,
        equilibrium_concentration: target.concentration,
        age: 0.0,
    });
    target.vorticity = 0.25;
    target.shell_equilibrium = 0.5;
    target.shell_level = target.shell_equilibrium;

    let mut cfg = SimConfig::default();
    cfg.dt = 0.02;
    cfg.max_bond_distance = 2.0;
    cfg.diffusivity = 0.0;
    cfg.viscosity_base = 0.0;
    cfg.foam_spawn_fraction = 0.0;
    cfg.foam_annihilation_range = 0.0;

    Sim::new(vec![source_plus, source_minus, target], cfg)
}

fn run_mode(mode: &Mode, target_ste: f64) -> OutputVec {
    let mut sim = build_sim(target_ste);

    let mut terminal_total = 0_u64;
    let mut relay_total = 0_u64;
    let mut overshoot_sum = 0.0_f64;
    let mut relax_sum = 0.0_f64;

    for pulse in mode.pulses {
        let source_idx = if pulse.twist >= 0 { 0 } else { 1 };
        sim.emit_wave(source_idx, pulse.energy, pulse.energy.sqrt());
        sim.step();

        let d = sim
            .last_diagnostics
            .as_ref()
            .expect("diagnostics should be available each tick");

        terminal_total += d.terminal_capture_collapses;
        relay_total += d.transient_relay_collapses;
        overshoot_sum += d.relay_shell_overshoot_sum;
        relax_sum += d.relay_relaxation_ticks_sum as f64;
    }

    let pulses = mode.pulses.len() as f64;
    OutputVec {
        capture_ratio: terminal_total as f64 / pulses.max(1.0),
        overshoot_mean: overshoot_sum / relay_total.max(1) as f64,
        relax_mean: relax_sum / relay_total.max(1) as f64,
    }
}

fn l2(a: OutputVec, b: OutputVec) -> f64 {
    let dx = a.capture_ratio - b.capture_ratio;
    let dy = a.overshoot_mean - b.overshoot_mean;
    let dz = a.relax_mean - b.relax_mean;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

#[test]
fn geometry_modes_separate_and_disperse_with_density() {
    let mode_a = Mode {
        name: "A",
        pulses: [
            Pulse { energy: 0.9, twist: 1 },
            Pulse { energy: 1.4, twist: 1 },
            Pulse { energy: 1.8, twist: -1 },
        ],
    };
    let mode_b = Mode {
        name: "B",
        pulses: [
            Pulse { energy: 1.4, twist: -1 },
            Pulse { energy: 2.2, twist: 1 },
            Pulse { energy: 0.9, twist: -1 },
        ],
    };

    let density_strata = [0.8, 1.0, 1.2, 1.4];

    let mut csv = String::from(
        "density,mode,capture_ratio,overshoot_mean,relax_mean\n",
    );
    let mut rows = Vec::new();

    let mut min_mode_distance = f64::INFINITY;
    let mut mode_a_overshoot = Vec::new();

    for density in density_strata {
        let a = run_mode(&mode_a, density);
        let b = run_mode(&mode_b, density);
        let d = l2(a, b);
        min_mode_distance = min_mode_distance.min(d);

        mode_a_overshoot.push(a.overshoot_mean);

        rows.push((density, mode_a.name, a));
        rows.push((density, mode_b.name, b));

        csv.push_str(&format!(
            "{:.6},{},{:.6},{:.6},{:.6}\n",
            density, mode_a.name, a.capture_ratio, a.overshoot_mean, a.relax_mean
        ));
        csv.push_str(&format!(
            "{:.6},{},{:.6},{:.6},{:.6}\n",
            density, mode_b.name, b.capture_ratio, b.overshoot_mean, b.relax_mean
        ));
    }

    let min_ovr = mode_a_overshoot
        .iter()
        .fold(f64::INFINITY, |acc, v| acc.min(*v));
    let max_ovr = mode_a_overshoot
        .iter()
        .fold(f64::NEG_INFINITY, |acc, v| acc.max(*v));
    let overshoot_span = (max_ovr - min_ovr).abs();

    let separable_gate = min_mode_distance > 0.05;
    let density_response_gate = overshoot_span > 0.05;

    let summary = serde_json::json!({
        "probe": "geometry_mode_dispersion",
        "gates": {
            "mode_separable_each_stratum": separable_gate,
            "density_response_present": density_response_gate
        },
        "metrics": {
            "min_mode_distance": min_mode_distance,
            "mode_a_overshoot_span": overshoot_span
        },
        "rows": rows.iter().map(|(density, mode, out)| serde_json::json!({
            "density": density,
            "mode": mode,
            "capture_ratio": out.capture_ratio,
            "overshoot_mean": out.overshoot_mean,
            "relax_mean": out.relax_mean
        })).collect::<Vec<_>>()
    });

    let mut out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    out_dir.push("target");
    out_dir.push("diagnostics");
    fs::create_dir_all(&out_dir).expect("failed to create diagnostics output dir");

    let csv_path = out_dir.join("qubit_dispersion.csv");
    let json_path = out_dir.join("qubit_dispersion_summary.json");
    fs::write(&csv_path, csv).expect("failed to write qubit dispersion csv");
    fs::write(&json_path, serde_json::to_string_pretty(&summary).unwrap())
        .expect("failed to write qubit dispersion summary");

    println!(
        "\nQUBIT DISPERSION | strata={} min_mode_distance={:.4} overshoot_span={:.4}",
        density_strata.len(),
        min_mode_distance,
        overshoot_span,
    );

    assert!(min_mode_distance.is_finite(), "mode distance should be finite");
    assert!(overshoot_span.is_finite(), "overshoot span should be finite");
    assert!(separable_gate, "modes not separable across strata (min d={min_mode_distance})");
    assert!(density_response_gate, "no density response detected (span={overshoot_span})");
}
