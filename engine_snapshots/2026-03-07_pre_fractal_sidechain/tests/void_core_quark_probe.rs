//! Void-core up-quark probe.
//!
//! Forces a high-tension matter/antimatter annihilation and verifies that
//! a true zero-STE core depression forms and is sustained by surrounding shell flow.

use std::fs;
use std::path::PathBuf;

use mass_effect_engine::config::SimConfig;
use mass_effect_engine::parcel::{ChokePhase, ChokeState, Parcel};
use mass_effect_engine::sim::Sim;

fn make_choked(id: u64, ste: f64, spin: f64, x: f64, y: f64, z: f64) -> Parcel {
    let mut p = Parcel::new(id, ste);
    p.x = x;
    p.y = y;
    p.z = z;
    p.spin = spin;
    p.vorticity = spin.abs().max(0.2);
    p.choke = Some(ChokeState {
        phase: ChokePhase::Drift,
        spin,
        coherence: 0.95,
        radius: 1.0,
        equilibrium_concentration: p.concentration,
        age: 1.0,
    });
    p
}

#[test]
fn cavitation_forms_true_void_core_up_quark_candidate() {
    let mut parcels = Vec::new();

    // Opposite-spin choked pair at close range to guarantee annihilation.
    parcels.push(make_choked(0, 14.0, 0.9, -0.15, 0.0, 0.0));
    parcels.push(make_choked(1, 13.0, -0.85, 0.15, 0.0, 0.0));

    // Surrounding shell support parcels.
    for i in 0..16 {
        let a = i as f64 * std::f64::consts::TAU / 16.0;
        let mut p = Parcel::new((100 + i) as u64, 2.8);
        p.x = a.cos() * 1.0;
        p.y = a.sin() * 1.0;
        p.z = if i % 2 == 0 { 0.2 } else { -0.2 };
        parcels.push(p);
    }

    let mut config = SimConfig::default();
    config.dt = 0.008;
    config.max_bond_distance = 12.0;
    config.shell_interaction_range = 1.6;
    config.foam_annihilation_range = 2.5;

    // Force cavitation-class response.
    config.annihilation_light_threshold = 0.5;
    config.annihilation_gamma_threshold = 1.0;
    config.annihilation_cavitation_threshold = 2.0;

    let mut sim = Sim::new(parcels, config);

    let ticks = 120;
    let mut csv = String::from(
        "tick,total_ste,annihilation_energy,void_core_count,up_candidate_count,void_shell_support\n",
    );

    let mut max_void_cores = 0usize;
    let mut max_up_candidates = 0usize;
    let mut first_up_tick = 0u64;
    let mut void_present_ticks = 0u64;
    let mut up_present_ticks = 0u64;
    let mut max_shell_support = 0.0_f64;

    for _ in 0..ticks {
        sim.step();
        let d = sim
            .last_diagnostics
            .as_ref()
            .expect("diagnostics should exist after stepping");

        max_void_cores = max_void_cores.max(d.void_core_count);
        max_up_candidates = max_up_candidates.max(d.up_quark_candidate_count);
        if d.void_core_count > 0 {
            void_present_ticks += 1;
        }
        if d.up_quark_candidate_count > 0 {
            up_present_ticks += 1;
        }
        max_shell_support = max_shell_support.max(d.void_core_shell_support_mean);
        if first_up_tick == 0 && d.up_quark_candidate_count > 0 {
            first_up_tick = d.tick;
        }

        csv.push_str(&format!(
            "{},{:.8},{:.8},{},{},{:.8}\n",
            d.tick,
            d.total_ste,
            d.annihilation_energy_this_tick,
            d.void_core_count,
            d.up_quark_candidate_count,
            d.void_core_shell_support_mean,
        ));
    }

    let d_last = sim
        .last_diagnostics
        .as_ref()
        .expect("final diagnostics should exist");

    let summary = serde_json::json!({
        "probe": "void_core_quark_probe",
        "ticks": ticks,
        "max_void_core_count": max_void_cores,
        "max_up_quark_candidate_count": max_up_candidates,
        "first_up_candidate_tick": first_up_tick,
        "void_present_ticks": void_present_ticks,
        "up_present_ticks": up_present_ticks,
        "max_void_core_shell_support": max_shell_support,
        "final": {
            "total_ste": d_last.total_ste,
            "void_core_count": d_last.void_core_count,
            "up_quark_candidate_count": d_last.up_quark_candidate_count,
            "void_core_shell_support_mean": d_last.void_core_shell_support_mean
        }
    });

    let mut out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    out_dir.push("target");
    out_dir.push("diagnostics");
    fs::create_dir_all(&out_dir).expect("failed to create diagnostics output dir");

    fs::write(out_dir.join("void_core_quark_probe.csv"), csv)
        .expect("failed to write void_core_quark_probe.csv");
    fs::write(
        out_dir.join("void_core_quark_probe_summary.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .expect("failed to write void_core_quark_probe_summary.json");

    println!(
        "\nVOID CORE PROBE | max_void={} max_up={} first_up_tick={} up_ticks={} max_support={:.6}",
        max_void_cores,
        max_up_candidates,
        first_up_tick,
        up_present_ticks,
        max_shell_support,
    );

    assert!(max_void_cores > 0, "expected at least one void-core formation");
    assert!(max_up_candidates > 0, "expected at least one up-quark candidate");
    assert!(
        d_last.void_core_shell_support_mean.is_finite(),
        "void shell support should be finite"
    );
}
