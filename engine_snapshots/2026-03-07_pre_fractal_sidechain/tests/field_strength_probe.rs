//! Field-strength A/B probe.
//!
//! Compares a proton-like setup at baseline STE scale vs doubled STE scale
//! to answer "what happens if we double field strength?" with data.

use std::fs;
use std::path::PathBuf;

use mass_effect_engine::config::SimConfig;
use mass_effect_engine::parcel::Parcel;
use mass_effect_engine::sim::Sim;

#[derive(Debug, Clone)]
struct RunSummary {
    ste_scale: f64,
    ticks: usize,
    mean_choke_count: f64,
    max_choke_count: usize,
    total_captures: usize,
    final_total_ste: f64,
}

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

fn run_case(ste_scale: f64, ticks: usize) -> RunSummary {
    let mut sim = build_proton_like(260, ste_scale);

    let mut choke_sum: usize = 0;
    let mut max_choke: usize = 0;
    let mut total_captures: usize = 0;

    for _ in 0..ticks {
        let m = sim.step();
        choke_sum += m.choke_count;
        max_choke = max_choke.max(m.choke_count);
        total_captures += m.captures_this_tick;
    }

    let final_total_ste = sim
        .last_diagnostics
        .as_ref()
        .map(|d| d.total_ste)
        .unwrap_or_else(|| sim.parcels.iter().map(|p| p.ste_amount).sum());

    RunSummary {
        ste_scale,
        ticks,
        mean_choke_count: choke_sum as f64 / ticks.max(1) as f64,
        max_choke_count: max_choke,
        total_captures,
        final_total_ste,
    }
}

#[test]
fn doubled_field_strength_probe() {
    let ticks = 700;

    let base = run_case(1.0, ticks);
    let doubled = run_case(2.0, ticks);

    let choke_ratio = doubled.mean_choke_count / base.mean_choke_count.max(1e-9);
    let capture_ratio = doubled.total_captures as f64 / (base.total_captures.max(1) as f64);

    let mut csv = String::from(
        "ste_scale,ticks,mean_choke_count,max_choke_count,total_captures,final_total_ste\n",
    );
    for r in [&base, &doubled] {
        csv.push_str(&format!(
            "{:.2},{},{:.6},{},{},{:.6}\n",
            r.ste_scale,
            r.ticks,
            r.mean_choke_count,
            r.max_choke_count,
            r.total_captures,
            r.final_total_ste,
        ));
    }

    let summary = serde_json::json!({
        "probe": "doubled_field_strength",
        "base": {
            "ste_scale": base.ste_scale,
            "mean_choke_count": base.mean_choke_count,
            "max_choke_count": base.max_choke_count,
            "total_captures": base.total_captures,
            "final_total_ste": base.final_total_ste
        },
        "doubled": {
            "ste_scale": doubled.ste_scale,
            "mean_choke_count": doubled.mean_choke_count,
            "max_choke_count": doubled.max_choke_count,
            "total_captures": doubled.total_captures,
            "final_total_ste": doubled.final_total_ste
        },
        "ratios": {
            "mean_choke_ratio": choke_ratio,
            "capture_ratio": capture_ratio,
            "final_total_ste_ratio": doubled.final_total_ste / base.final_total_ste.max(1e-12)
        }
    });

    let mut out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    out_dir.push("target");
    out_dir.push("diagnostics");
    fs::create_dir_all(&out_dir).expect("failed to create diagnostics output dir");

    fs::write(out_dir.join("field_strength_probe.csv"), csv)
        .expect("failed to write field strength csv");
    fs::write(
        out_dir.join("field_strength_probe_summary.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .expect("failed to write field strength summary");

    println!(
        "\nFIELD STRENGTH PROBE | mean_choke base={:.2} doubled={:.2} ratio={:.2} | captures base={} doubled={} ratio={:.2}",
        base.mean_choke_count,
        doubled.mean_choke_count,
        choke_ratio,
        base.total_captures,
        doubled.total_captures,
        capture_ratio,
    );

    assert!(base.mean_choke_count.is_finite() && doubled.mean_choke_count.is_finite());
    assert!(base.final_total_ste.is_finite() && doubled.final_total_ste.is_finite());
    assert!(doubled.final_total_ste > base.final_total_ste, "doubled field should carry more total STE");
}
