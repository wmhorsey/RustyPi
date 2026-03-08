//! STE -> MeV scale-stability probe.
//!
//! Runs the same setup at STE scales 1x/1.5x/2x and compares fitted
//! conversion factors k (for fixed MeV anchors) to quantify calibration drift.

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
    let mut seed: u64 = 9001;

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

#[derive(Debug, Clone, Copy)]
struct ProbeStats {
    e_mean: f64,
    e_peak: f64,
    e_integral_choked: f64,
    r_mean: f64,
    shell_intensity_mean: f64,
    choke_density_mean: f64,
    choked_tick_ratio: f64,
}

fn run_probe(ste_scale: f64, ticks: usize) -> ProbeStats {
    let mut sim = build_proton_like(260, ste_scale);
    let dt = sim.config.dt;

    let mut e_sum = 0.0_f64;
    let mut e_peak = 0.0_f64;
    let mut e_integral_choked = 0.0_f64;
    let mut r_sum = 0.0_f64;
    let mut shell_sum = 0.0_f64;
    let mut choke_density_sum = 0.0_f64;
    let mut choked_ticks = 0usize;

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

        let resistance_star = (
            0.45 * shell_intensity + 0.25 * choke_density + 0.30 * mean_slip
        )
            .max(0.0);
        let e_star = d.total_ste * resistance_star;

        e_sum += e_star;
        r_sum += resistance_star;
        shell_sum += shell_intensity;
        choke_density_sum += choke_density;
        e_peak = e_peak.max(e_star);
        if choke_density > 1e-12 {
            e_integral_choked += e_star * dt;
            choked_ticks += 1;
        }
    }

    ProbeStats {
        e_mean: e_sum / ticks as f64,
        e_peak,
        e_integral_choked,
        r_mean: r_sum / ticks as f64,
        shell_intensity_mean: shell_sum / ticks as f64,
        choke_density_mean: choke_density_sum / ticks as f64,
        choked_tick_ratio: choked_ticks as f64 / ticks.max(1) as f64,
    }
}

#[test]
fn ste_mev_scale_stability_reports_k_drift() {
    let ticks = 450;
    let base = run_probe(1.0, ticks);
    let one_half = run_probe(1.5, ticks);
    let doubled = run_probe(2.0, ticks);

    let anchors = [
        ("electron_rest_mev", 0.511_f64),
        ("pair_threshold_mev", 1.022_f64),
    ];

    let mut rows = Vec::new();
    for (name, mev) in anchors {
        let k_base = mev / base.e_mean.max(1e-12);
        let k_15x = mev / one_half.e_mean.max(1e-12);
        let k_2x = mev / doubled.e_mean.max(1e-12);
        let drift_ratio_15x = k_15x / k_base.max(1e-12);
        let drift_ratio_2x = k_2x / k_base.max(1e-12);

        let k_base_event = mev / base.e_integral_choked.max(1e-12);
        let k_15x_event = mev / one_half.e_integral_choked.max(1e-12);
        let k_2x_event = mev / doubled.e_integral_choked.max(1e-12);
        let drift_ratio_15x_event = k_15x_event / k_base_event.max(1e-12);
        let drift_ratio_2x_event = k_2x_event / k_base_event.max(1e-12);

        rows.push(serde_json::json!({
            "anchor": name,
            "mev": mev,
            "k_base": k_base,
            "k_15x": k_15x,
            "k_2x": k_2x,
            "k_drift_ratio_15x": drift_ratio_15x,
            "k_drift_ratio_2x": drift_ratio_2x,
            "k_drift_abs_pct_15x": ((drift_ratio_15x - 1.0).abs()) * 100.0,
            "k_drift_abs_pct_2x": ((drift_ratio_2x - 1.0).abs()) * 100.0,
            "k_base_event": k_base_event,
            "k_15x_event": k_15x_event,
            "k_2x_event": k_2x_event,
            "k_event_drift_ratio_15x": drift_ratio_15x_event,
            "k_event_drift_ratio_2x": drift_ratio_2x_event,
            "k_event_drift_abs_pct_15x": ((drift_ratio_15x_event - 1.0).abs()) * 100.0,
            "k_event_drift_abs_pct_2x": ((drift_ratio_2x_event - 1.0).abs()) * 100.0
        }));
    }

    let summary = serde_json::json!({
        "probe": "ste_mev_scale_stability",
        "ticks": ticks,
        "base": {
            "ste_scale": 1.0,
            "e_star_mean": base.e_mean,
            "e_star_peak": base.e_peak,
            "e_star_integral_choked": base.e_integral_choked,
            "r_star_mean": base.r_mean,
            "shell_intensity_mean": base.shell_intensity_mean,
            "choke_density_mean": base.choke_density_mean,
            "choked_tick_ratio": base.choked_tick_ratio
        },
        "one_half": {
            "ste_scale": 1.5,
            "e_star_mean": one_half.e_mean,
            "e_star_peak": one_half.e_peak,
            "e_star_integral_choked": one_half.e_integral_choked,
            "r_star_mean": one_half.r_mean,
            "shell_intensity_mean": one_half.shell_intensity_mean,
            "choke_density_mean": one_half.choke_density_mean,
            "choked_tick_ratio": one_half.choked_tick_ratio
        },
        "doubled": {
            "ste_scale": 2.0,
            "e_star_mean": doubled.e_mean,
            "e_star_peak": doubled.e_peak,
            "e_star_integral_choked": doubled.e_integral_choked,
            "r_star_mean": doubled.r_mean,
            "shell_intensity_mean": doubled.shell_intensity_mean,
            "choke_density_mean": doubled.choke_density_mean,
            "choked_tick_ratio": doubled.choked_tick_ratio
        },
        "structure_retention": {
            "one_half_vs_base": {
                "choke_density_ratio": one_half.choke_density_mean / base.choke_density_mean.max(1e-12),
                "shell_intensity_ratio": one_half.shell_intensity_mean / base.shell_intensity_mean.max(1e-12),
                "choked_tick_ratio_delta": one_half.choked_tick_ratio - base.choked_tick_ratio
            },
            "doubled_vs_base": {
                "choke_density_ratio": doubled.choke_density_mean / base.choke_density_mean.max(1e-12),
                "shell_intensity_ratio": doubled.shell_intensity_mean / base.shell_intensity_mean.max(1e-12),
                "choked_tick_ratio_delta": doubled.choked_tick_ratio - base.choked_tick_ratio
            }
        },
        "k_drift": rows
    });

    let mut out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    out_dir.push("target");
    out_dir.push("diagnostics");
    fs::create_dir_all(&out_dir).expect("failed to create diagnostics output dir");

    fs::write(
        out_dir.join("ste_mev_scale_stability_summary.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .expect("failed to write ste_mev_scale_stability_summary.json");

    println!(
        "\nSTE->MeV SCALE STABILITY | E*_mean base={:.6} 1.5x={:.6} 2x={:.6} | choke_ratio base={:.3} 1.5x={:.3} 2x={:.3}",
        base.e_mean,
        one_half.e_mean,
        doubled.e_mean,
        base.choked_tick_ratio,
        one_half.choked_tick_ratio,
        doubled.choked_tick_ratio,
    );

    assert!(base.e_mean.is_finite() && base.e_mean > 0.0, "base E*_mean should be finite and >0");
    assert!(one_half.e_mean.is_finite() && one_half.e_mean > 0.0, "1.5x E*_mean should be finite and >0");
    assert!(doubled.e_mean.is_finite() && doubled.e_mean > 0.0, "2x E*_mean should be finite and >0");
}
