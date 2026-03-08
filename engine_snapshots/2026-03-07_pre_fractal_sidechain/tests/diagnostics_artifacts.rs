//! Diagnostics artifact generation.
//!
//! Produces persisted CSV + JSON artifacts in `target/diagnostics/`
//! so runs can be compared over time without relying on screenshots.

use std::fs;
use std::path::PathBuf;

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

    let shells = 5;
    let per_shell_base = n / shells;
    let mut placed = 0;

    for shell in 0..shells {
        let r = 2.5 + (shell as f64) * 1.7;
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
            let jr = r + (rng(&mut seed) - 0.5) * 0.55;
            let jphi = phi + (rng(&mut seed) - 0.5) * 0.14;

            let ste = 3.2 + rng(&mut seed) * 1.2;
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
fn diagnostics_artifacts_are_written() {
    let mut sim = build_bubble(140);
    let ticks = 1200;

    let mut csv = String::from(
        "tick,total_ste,lin_mom,ang_mom,kinetic,ar_res,pairs,dwell,potential,yield,chirality,zero_cross,lock,matter,anti,relay_collapses,transient_collapses,terminal_captures,relay_overshoot_sum,relay_overshoot_mean,relay_relax_sum,relay_relax_mean\n",
    );

    let mut max_pairs = 0usize;
    let mut max_yield = 0.0_f64;
    let mut chi_abs_sum = 0.0_f64;

    for _ in 0..ticks {
        sim.step();
        let d = sim
            .last_diagnostics
            .as_ref()
            .expect("diagnostics should be present each tick");

        max_pairs = max_pairs.max(d.compound_pair_count);
        max_yield = max_yield.max(d.annihilation_energy_this_tick);
        chi_abs_sum += d.chirality_abs;

        csv.push_str(&format!(
            "{},{:.8},{:.8},{:.8},{:.8},{:.8e},{},{:.4},{:.8},{:.8},{:.6},{},{},{},{},{},{},{},{:.8},{:.8},{},{:.8}\n",
            d.tick,
            d.total_ste,
            d.linear_momentum_mag,
            d.angular_momentum_mag,
            d.kinetic_energy,
            d.action_reaction_residual,
            d.compound_pair_count,
            d.compound_dwell_mean_ticks,
            d.compound_potential_sum,
            d.annihilation_energy_this_tick,
            d.chirality,
            d.chirality_zero_crossings,
            d.chirality_lock_ticks,
            d.matter_count,
            d.anti_count,
            d.relay_collapse_count,
            d.transient_relay_collapses,
            d.terminal_capture_collapses,
            d.relay_shell_overshoot_sum,
            d.relay_shell_overshoot_mean,
            d.relay_relaxation_ticks_sum,
            d.relay_relaxation_ticks_mean,
        ));
    }

    let d_last = sim
        .last_diagnostics
        .as_ref()
        .expect("last diagnostics should exist after run");

    let summary = serde_json::json!({
        "run": {
            "ticks": ticks,
            "parcels_final": sim.parcels.len(),
            "dt": sim.config.dt
        },
        "final": {
            "total_ste": d_last.total_ste,
            "action_reaction_residual": d_last.action_reaction_residual,
            "chirality": d_last.chirality,
            "chirality_abs": d_last.chirality_abs,
            "zero_crossings": d_last.chirality_zero_crossings,
            "lock_ticks": d_last.chirality_lock_ticks,
            "compound_pairs": d_last.compound_pair_count,
            "compound_dwell": d_last.compound_dwell_mean_ticks,
            "compound_potential": d_last.compound_potential_sum,
            "yield": d_last.annihilation_energy_this_tick
            ,"relay_collapse_count": d_last.relay_collapse_count
            ,"transient_relay_collapses": d_last.transient_relay_collapses
            ,"terminal_capture_collapses": d_last.terminal_capture_collapses
            ,"relay_shell_overshoot_sum": d_last.relay_shell_overshoot_sum
            ,"relay_shell_overshoot_mean": d_last.relay_shell_overshoot_mean
            ,"relay_relaxation_ticks_sum": d_last.relay_relaxation_ticks_sum
            ,"relay_relaxation_ticks_mean": d_last.relay_relaxation_ticks_mean
        },
        "aggregates": {
            "max_pairs": max_pairs,
            "max_yield": max_yield,
            "mean_chirality_abs": chi_abs_sum / ticks as f64
        }
    });

    let mut out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    out_dir.push("target");
    out_dir.push("diagnostics");
    fs::create_dir_all(&out_dir).expect("failed to create diagnostics output dir");

    let csv_path = out_dir.join("latest_run.csv");
    let json_path = out_dir.join("latest_run_summary.json");

    fs::write(&csv_path, csv).expect("failed to write csv diagnostics artifact");
    fs::write(&json_path, serde_json::to_string_pretty(&summary).unwrap())
        .expect("failed to write json diagnostics artifact");

    let csv_len = fs::metadata(&csv_path).expect("csv metadata missing").len();
    let json_len = fs::metadata(&json_path).expect("json metadata missing").len();

    assert!(csv_len > 256, "csv artifact unexpectedly small");
    assert!(json_len > 64, "json artifact unexpectedly small");
    assert!(d_last.action_reaction_residual.is_finite());
    assert!(d_last.chirality.is_finite());
}
