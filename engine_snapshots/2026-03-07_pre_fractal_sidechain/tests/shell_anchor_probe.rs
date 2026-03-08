//! Shell anchor integrity probe.
//!
//! Tracks whether depressions remain properly anchored by coherent shells
//! (ambient > core) or drift into inside-out bubble states (ambient <= core).

use std::fs;
use std::path::PathBuf;

use mass_effect_engine::config::SimConfig;
use mass_effect_engine::parcel::Parcel;
use mass_effect_engine::sim::Sim;

fn build_proton_like(n_field: usize, ste_scale: f64) -> Sim {
    let pi = std::f64::consts::PI;
    let golden_angle = pi * (3.0 - (5.0_f64).sqrt());

    let rng = |state: &mut u64| -> f64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        (*state as f64) / (u64::MAX as f64)
    };
    let mut seed: u64 = 616161;

    let mut parcels = Vec::with_capacity(n_field);
    let shells = 6;
    let per_shell_base = n_field / shells;
    let mut placed = 0usize;

    for shell in 0..shells {
        let r = 2.5 + (shell as f64) * 1.8;
        let count = if shell < shells - 1 {
            per_shell_base + shell * 2
        } else {
            n_field - placed
        };

        for j in 0..count {
            if placed >= n_field {
                break;
            }
            let y_frac = 1.0 - (j as f64 / (count as f64 - 1.0).max(1.0)) * 2.0;
            let phi = (j as f64) * golden_angle + (shell as f64) * 0.37;

            let cos_theta = y_frac;
            let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
            let jr = r + (rng(&mut seed) - 0.5) * 0.6;
            let jphi = phi + (rng(&mut seed) - 0.5) * 0.15;

            let ste = (3.5 + rng(&mut seed) * 1.0) * ste_scale;
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
fn shell_anchor_probe_reports_inside_out_transitions() {
    let scales = [1.0_f64, 1.5_f64, 2.0_f64];
    let ticks = 450usize;

    let mut csv = String::from(
        "scale,tick,choke_count,anchored_count,inside_out_count,anchored_gap_mean,shell_intensity,total_ste\n",
    );
    let mut summaries = Vec::new();

    for &scale in &scales {
        let mut sim = build_proton_like(260, scale);

        let mut anchored_sum = 0.0_f64;
        let mut inside_out_sum = 0.0_f64;
        let mut anchored_gap_sum = 0.0_f64;
        let mut nonzero_anchor_ticks = 0u64;
        let mut inside_out_ticks = 0u64;
        let mut max_inside_out = 0usize;

        for _ in 0..ticks {
            let m = sim.step();
            let d = sim
                .last_diagnostics
                .as_ref()
                .expect("diagnostics should exist each tick");

            let shell_intensity = d.shell_excess_energy_proxy / d.total_ste.max(1e-12);

            anchored_sum += d.anchored_depression_count as f64;
            inside_out_sum += d.inside_out_bubble_count as f64;
            if d.anchored_depression_count > 0 {
                nonzero_anchor_ticks += 1;
                anchored_gap_sum += d.anchored_gap_mean;
            }
            if d.inside_out_bubble_count > 0 {
                inside_out_ticks += 1;
            }
            max_inside_out = max_inside_out.max(d.inside_out_bubble_count);

            csv.push_str(&format!(
                "{:.3},{},{},{},{},{:.8},{:.8},{:.8}\n",
                scale,
                d.tick,
                m.choke_count,
                d.anchored_depression_count,
                d.inside_out_bubble_count,
                d.anchored_gap_mean,
                shell_intensity,
                d.total_ste,
            ));
        }

        summaries.push(serde_json::json!({
            "scale": scale,
            "ticks": ticks,
            "anchored_count_mean": anchored_sum / ticks as f64,
            "inside_out_count_mean": inside_out_sum / ticks as f64,
            "inside_out_tick_ratio": inside_out_ticks as f64 / ticks as f64,
            "max_inside_out_count": max_inside_out,
            "anchored_gap_mean_when_present": if nonzero_anchor_ticks > 0 {
                anchored_gap_sum / nonzero_anchor_ticks as f64
            } else {
                0.0
            }
        }));
    }

    let summary = serde_json::json!({
        "probe": "shell_anchor_probe",
        "scale_summaries": summaries,
        "note": "Anchored means coherent shell with ambient > core; inside-out means shell exists while ambient <= core."
    });

    let mut out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    out_dir.push("target");
    out_dir.push("diagnostics");
    fs::create_dir_all(&out_dir).expect("failed to create diagnostics output dir");

    fs::write(out_dir.join("shell_anchor_probe.csv"), csv)
        .expect("failed to write shell_anchor_probe.csv");
    fs::write(
        out_dir.join("shell_anchor_probe_summary.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .expect("failed to write shell_anchor_probe_summary.json");

    println!("\nSHELL ANCHOR PROBE complete; see diagnostics artifacts.");

    assert!(true);
}
