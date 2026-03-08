//! STE -> MeV exploratory calibration probe.
//!
//! Computes an internal energy proxy E* = STE * R* where R* is a
//! resistance index derived from shell compression + choke coherence + slip drag.
//! Then emits candidate linear conversion factors k for several MeV anchors.

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
    let mut seed: u64 = 7777;

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
fn ste_mev_probe_outputs_calibration_candidates() {
    let ticks = 600;
    let mut sim = build_proton_like(260, 1.0);
    let dt = sim.config.dt;

    let mut csv = String::from(
        "tick,total_ste,shell_intensity,choke_density,mean_slip,resistance_star,e_star\n",
    );

    let mut e_sum = 0.0_f64;
    let mut e_peak = 0.0_f64;
    let mut e_integral = 0.0_f64;
    let mut e_integral_choked = 0.0_f64;
    let mut choked_ticks = 0_u64;
    let mut r_sum = 0.0_f64;

    for _ in 0..ticks {
        sim.step();
        let d = sim
            .last_diagnostics
            .as_ref()
            .expect("diagnostics should exist each tick");

        let n = sim.parcels.len().max(1) as f64;
        let shell_intensity = d.shell_excess_energy_proxy / d.total_ste.max(1e-12);
        let choke_density = d.choke_coherence_total / n;
        let mean_slip = d.mean_velocity_slip;

        // R* combines compression, retained choke coherence, and slip drag.
        let resistance_star = (
            0.45 * shell_intensity + 0.25 * choke_density + 0.30 * mean_slip
        )
            .max(0.0);
        let e_star = d.total_ste * resistance_star;

        e_sum += e_star;
        r_sum += resistance_star;
        e_peak = e_peak.max(e_star);
        e_integral += e_star * dt;
        let has_choke = sim.parcels.iter().any(|p| p.is_choked());
        if has_choke {
            e_integral_choked += e_star * dt;
            choked_ticks += 1;
        }

        csv.push_str(&format!(
            "{},{:.8},{:.8},{:.8},{:.8},{:.8},{:.8}\n",
            d.tick,
            d.total_ste,
            shell_intensity,
            choke_density,
            mean_slip,
            resistance_star,
            e_star,
        ));
    }

    let e_mean = e_sum / ticks as f64;
    let r_mean = r_sum / ticks as f64;

    let anchors = [
        ("electron_rest_mev", 0.511_f64),
        ("pair_threshold_mev", 1.022_f64),
        ("pion_scale_mev", 139.57_f64),
    ];

    let k_rows: Vec<_> = anchors
        .iter()
        .map(|(name, mev)| {
            serde_json::json!({
                "anchor": name,
                "mev": mev,
                "k_from_e_mean": mev / e_mean.max(1e-12),
                "k_from_e_peak": mev / e_peak.max(1e-12),
                "k_from_e_integral": mev / e_integral.max(1e-12),
                "k_from_e_integral_choked": mev / e_integral_choked.max(1e-12)
            })
        })
        .collect();

    let summary = serde_json::json!({
        "probe": "ste_mev_calibration_exploratory",
        "ticks": ticks,
        "e_star": {
            "mean": e_mean,
            "peak": e_peak,
            "integral": e_integral,
            "integral_choked": e_integral_choked
        },
        "event_windows": {
            "dt": dt,
            "choked_ticks": choked_ticks
        },
        "resistance_star": {
            "mean": r_mean
        },
        "formula": {
            "resistance_star": "0.45*shell_intensity + 0.25*choke_density + 0.30*mean_slip",
            "energy_proxy": "E* = total_ste * resistance_star"
        },
        "anchors": k_rows
    });

    let mut out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    out_dir.push("target");
    out_dir.push("diagnostics");
    fs::create_dir_all(&out_dir).expect("failed to create diagnostics output dir");

    fs::write(out_dir.join("ste_mev_probe.csv"), csv)
        .expect("failed to write ste_mev_probe.csv");
    fs::write(
        out_dir.join("ste_mev_probe_summary.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .expect("failed to write ste_mev_probe_summary.json");

    println!(
        "\nSTE->MeV PROBE | E*_mean={:.6} E*_peak={:.6} R*_mean={:.6}",
        e_mean,
        e_peak,
        r_mean,
    );

    assert!(e_mean.is_finite() && e_mean > 0.0, "E*_mean should be finite and >0");
    assert!(e_peak.is_finite() && e_peak > 0.0, "E*_peak should be finite and >0");
    assert!(r_mean.is_finite() && r_mean >= 0.0, "R*_mean should be finite and >=0");
}
