//! 3-pulse relay codec probe.
//!
//! Encodes short pulse sequences into a central node and reads a 3D output:
//! [capture_ratio, relay_overshoot_mean, relay_relax_mean].
//! The test passes when codewords produce separable output vectors.

use std::fs;
use std::path::PathBuf;

use mass_effect_engine::config::SimConfig;
use mass_effect_engine::parcel::{ChokePhase, ChokeState, Parcel};
use mass_effect_engine::sim::Sim;

#[derive(Debug, Clone)]
struct Codeword {
    name: &'static str,
    pulses: [f64; 3],
}

#[derive(Debug, Clone, Copy)]
struct OutputVec {
    capture_ratio: f64,
    overshoot_mean: f64,
    relax_mean: f64,
}

fn build_codec_sim(target_ste: f64) -> Sim {
    let mut source = Parcel::new(0, 5.0);
    source.x = 0.0;

    let mut target = Parcel::new(1, target_ste);
    target.x = 0.25;
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

    Sim::new(vec![source, target], cfg)
}

fn run_codeword(code: &Codeword, target_ste: f64) -> OutputVec {
    let mut sim = build_codec_sim(target_ste);

    let mut terminal_total = 0_u64;
    let mut relay_total = 0_u64;
    let mut overshoot_sum = 0.0_f64;
    let mut relax_sum = 0.0_f64;

    for energy in code.pulses {
        sim.emit_wave(0, energy, energy.sqrt());
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

    let pulse_count = code.pulses.len() as f64;
    let capture_ratio = terminal_total as f64 / pulse_count.max(1.0);
    let overshoot_mean = overshoot_sum / relay_total.max(1) as f64;
    let relax_mean = relax_sum / relay_total.max(1) as f64;

    OutputVec {
        capture_ratio,
        overshoot_mean,
        relax_mean,
    }
}

fn l2(a: OutputVec, b: OutputVec) -> f64 {
    let dx = a.capture_ratio - b.capture_ratio;
    let dy = a.overshoot_mean - b.overshoot_mean;
    let dz = a.relax_mean - b.relax_mean;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

#[test]
fn pulse_codec_outputs_separable_3d_vectors() {
    let codebook = vec![
        Codeword {
            name: "L-L-L",
            pulses: [0.8, 0.8, 0.8],
        },
        Codeword {
            name: "M-M-M",
            pulses: [1.4, 1.4, 1.4],
        },
        Codeword {
            name: "H-H-H",
            pulses: [2.2, 2.2, 2.2],
        },
        Codeword {
            name: "L-H-M",
            pulses: [0.8, 2.2, 1.4],
        },
    ];

    let target_ste = 1.0;
    let mut rows: Vec<(String, [f64; 3], OutputVec)> = Vec::new();

    for code in &codebook {
        let out = run_codeword(code, target_ste);
        rows.push((code.name.to_string(), code.pulses, out));
    }

    let mut min_pair_distance = f64::INFINITY;
    for i in 0..rows.len() {
        for j in (i + 1)..rows.len() {
            let d = l2(rows[i].2, rows[j].2);
            min_pair_distance = min_pair_distance.min(d);
        }
    }

    let mut csv = String::from(
        "codeword,p1,p2,p3,capture_ratio,overshoot_mean,relax_mean\n",
    );
    for (name, pulses, out) in &rows {
        csv.push_str(&format!(
            "{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}\n",
            name,
            pulses[0],
            pulses[1],
            pulses[2],
            out.capture_ratio,
            out.overshoot_mean,
            out.relax_mean,
        ));
    }

    let summary = serde_json::json!({
        "probe": "qubit_pulse_codec_3d",
        "target_ste": target_ste,
        "min_pair_distance": min_pair_distance,
        "separable_gate": min_pair_distance > 0.05,
        "rows": rows.iter().map(|(name, pulses, out)| serde_json::json!({
            "codeword": name,
            "pulses": pulses,
            "output": {
                "capture_ratio": out.capture_ratio,
                "overshoot_mean": out.overshoot_mean,
                "relax_mean": out.relax_mean
            }
        })).collect::<Vec<_>>()
    });

    let mut out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    out_dir.push("target");
    out_dir.push("diagnostics");
    fs::create_dir_all(&out_dir).expect("failed to create diagnostics output dir");

    let csv_path = out_dir.join("qubit_codec.csv");
    let json_path = out_dir.join("qubit_codec_summary.json");
    fs::write(&csv_path, csv).expect("failed to write qubit codec csv");
    fs::write(&json_path, serde_json::to_string_pretty(&summary).unwrap())
        .expect("failed to write qubit codec summary");

    println!(
        "\nQUBIT CODEC | codewords={} min_pair_distance={:.4}",
        rows.len(),
        min_pair_distance,
    );

    assert!(min_pair_distance.is_finite(), "distance must be finite");
    assert!(min_pair_distance > 0.05, "codec outputs not separable enough: {min_pair_distance}");
}
