//! STE tunneling benchmark scaffold.
//!
//! This harness compares an ontology-native pre-choke permeation scenario
//! against a stochastic-style transmission baseline and writes artifacts.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use mass_effect_engine::config::SimConfig;
use mass_effect_engine::parcel::Parcel;
use mass_effect_engine::sim::Sim;

#[derive(Debug, Clone)]
struct SweepCase {
    barrier_width: f64,
    ste_transmission: f64,
    detector_transmission: f64,
    progress_transmission: f64,
    stochastic_transmission: f64,
    residual: f64,
    max_ar_residual: f64,
    ste_drift_fraction: f64,
    seconds_per_step: f64,
}

fn default_benchmark_config() -> SimConfig {
    let mut config = SimConfig::default();
    config.dt = 0.002;
    config.max_bond_distance = 10.0;
    config.shell_interaction_range = 1.4;
    config.parcel_radius_scale = 0.3;
    // Allow controlled packet/barrier exchange so width meaningfully changes outcomes.
    config.diffusivity = 0.02;
    config.viscosity_base = 0.18;
    config.viscosity_exponent = 1.4;
    config.annihilation_light_threshold = 1e12;
    config.annihilation_gamma_threshold = 1e12;
    config.annihilation_cavitation_threshold = 1e12;

    // Keep the benchmark focused on barrier transmission, not foam side-effects.
    config.foam_spawn_coherence = 2.0;
    config.foam_spawn_fraction = 0.0;
    config.foam_spawn_min_ste = f64::MAX;
    config.foam_annihilation_range = 0.0;

    config
}

const PACKET_START_X: f64 = -2.0;
const BARRIER_CENTER_X: f64 = -1.55;

fn build_packet_barrier_case(barrier_width: f64) -> (Sim, HashSet<u64>, f64, f64) {
    let mut parcels = Vec::new();

    // Packet: compact moving cluster launched toward +x.
    let mut id = 0_u64;
    let mut packet_ids = HashSet::new();
    for iy in 0..4 {
        for iz in 0..4 {
            let mut p = Parcel::new(id, 1.8);
            p.x = PACKET_START_X + (iy as f64) * 0.14;
            p.y = -0.8 + (iy as f64) * 0.52;
            p.z = -0.8 + (iz as f64) * 0.52;
            p.vx = 0.9;
            p.vy = 0.0;
            p.vz = 0.0;
            parcels.push(p);
            packet_ids.insert(id);
            id += 1;
        }
    }

    // Barrier: static-ish slab near x=0, width swept by x-layers.
    let spacing_x = 0.32;
    let layers = if barrier_width <= 0.0 {
        0
    } else {
        ((barrier_width / spacing_x).round() as i32).max(1) as usize
    };
    let start_x = BARRIER_CENTER_X - 0.5 * (layers as f64 - 1.0) * spacing_x;

    for ix in 0..layers {
        let x = start_x + (ix as f64) * spacing_x;
        for iy in 0..8 {
            for iz in 0..2 {
                let mut p = Parcel::new(id, 7.0);
                p.x = x;
                p.y = -1.4 + (iy as f64) * 0.4;
                p.z = -0.25 + (iz as f64) * 0.5;
                p.vx = 0.0;
                p.vy = 0.0;
                p.vz = 0.0;
                parcels.push(p);
                id += 1;
            }
        }
    }

    let mut sim = Sim::new(parcels, default_benchmark_config());
    for p in &mut sim.parcels {
        p.update_radius_and_concentration(sim.config.parcel_radius_scale);
    }

    let total_ste0: f64 = sim.parcels.iter().map(|p| p.ste_amount).sum();
    let packet_ste0: f64 = sim
        .parcels
        .iter()
        .filter(|p| packet_ids.contains(&p.id))
        .map(|p| p.ste_amount)
        .sum();

    (sim, packet_ids, total_ste0, packet_ste0)
}

fn run_case(barrier_width: f64, ticks: usize) -> (f64, f64, f64, f64, f64, f64) {
    let (mut sim, packet_ids, total_ste0, packet_ste0) = build_packet_barrier_case(barrier_width);

    let packet_mean_x0 = sim
        .parcels
        .iter()
        .filter(|p| packet_ids.contains(&p.id))
        .map(|p| p.x)
        .sum::<f64>()
        / packet_ids.len().max(1) as f64;

    let mut max_ar_residual = 0.0_f64;
    let t0 = Instant::now();
    for _ in 0..ticks {
        sim.step();
        if let Some(d) = &sim.last_diagnostics {
            max_ar_residual = max_ar_residual.max(d.action_reaction_residual.abs());
        }
    }
    let elapsed_s = t0.elapsed().as_secs_f64();

    let total_ste_final: f64 = sim.parcels.iter().map(|p| p.ste_amount).sum();
    let ste_drift_fraction = (total_ste_final - total_ste0).abs() / total_ste0.max(1e-12);

    let packet_mean_x_final = sim
        .parcels
        .iter()
        .filter(|p| packet_ids.contains(&p.id))
        .map(|p| p.x)
        .sum::<f64>()
        / packet_ids.len().max(1) as f64;

    // Place detector just beyond the barrier front so each width is measured
    // against a consistent post-barrier slice.
    let detector_x = BARRIER_CENTER_X + 0.05;
    let transmitted_ste: f64 = sim
        .parcels
        .iter()
        .filter(|p| packet_ids.contains(&p.id) && p.x > detector_x)
        .map(|p| p.ste_amount)
        .sum();
    let transmission = (transmitted_ste / packet_ste0.max(1e-12)).clamp(0.0, 1.0);

    let seconds_per_step = elapsed_s / ticks.max(1) as f64;

    (
        transmission,
        packet_mean_x0,
        packet_mean_x_final,
        max_ar_residual,
        ste_drift_fraction,
        seconds_per_step,
    )
}

fn fit_stochastic_exponential(widths: &[f64], transmissions: &[f64]) -> (f64, f64) {
    let eps = 1e-6;
    let n = widths.len().max(1) as f64;

    let mut sx = 0.0;
    let mut sy = 0.0;
    let mut sxx = 0.0;
    let mut sxy = 0.0;

    for (w, t) in widths.iter().zip(transmissions.iter()) {
        let y = t.clamp(eps, 1.0 - eps).ln();
        sx += *w;
        sy += y;
        sxx += *w * *w;
        sxy += *w * y;
    }

    let denom = (n * sxx - sx * sx).abs().max(1e-12);
    let slope = (n * sxy - sx * sy) / denom;
    let intercept = (sy - slope * sx) / n;

    (intercept, slope)
}

#[test]
fn ste_tunneling_benchmark_artifacts_and_sanity() {
    let widths = [0.0, 0.6, 1.0, 1.4, 1.8, 2.2, 2.6];
    let ticks = 700;

    let mut max_ar = 0.0_f64;
    let mut max_drift = 0.0_f64;

    let mut raw_cases: Vec<(f64, f64, f64, f64, f64, f64, f64)> = Vec::with_capacity(widths.len());
    for w in widths {
        let (t, x0, x1, ar, drift, step_s) = run_case(w, ticks);
        max_ar = max_ar.max(ar);
        max_drift = max_drift.max(drift);
        raw_cases.push((w, t, x0, x1, ar, drift, step_s));
    }

    let x0 = raw_cases
        .iter()
        .find(|(w, _, _, _, _, _, _)| *w == 0.0)
        .map(|(_, _, x0, _, _, _, _)| *x0)
        .unwrap_or(0.0);
    let x_free = raw_cases
        .iter()
        .find(|(w, _, _, _, _, _, _)| *w == 0.0)
        .map(|(_, _, _, x1, _, _, _)| *x1)
        .unwrap_or(x0 + 1e-6);
    let progress_denom = (x_free - x0).abs().max(1e-6);

    let mut model_widths = Vec::new();
    let mut model_transmissions = Vec::new();
    for (w, detector_t, _, x1, _, _, _) in &raw_cases {
        if *w == 0.0 {
            continue;
        }
        let progress = ((x1 - x0) / progress_denom).clamp(0.0, 1.0);
        let blended = 0.5 * detector_t.clamp(0.0, 1.0) + 0.5 * progress;
        model_widths.push(*w);
        model_transmissions.push(blended);
    }

    let (a, b) = fit_stochastic_exponential(&model_widths, &model_transmissions);

    let mut cases = Vec::with_capacity(raw_cases.len());
    let mut rss = 0.0_f64;
    for (w, detector_t, _, x1, ar, drift, step_s) in raw_cases {
        let progress_t = ((x1 - x0) / progress_denom).clamp(0.0, 1.0);
        let ste_t = if w == 0.0 {
            1.0
        } else {
            0.5 * detector_t.clamp(0.0, 1.0) + 0.5 * progress_t
        };
        let baseline_t = (a + b * w).exp().clamp(0.0, 1.0);
        let residual = ste_t - baseline_t;
        rss += residual * residual;
        cases.push(SweepCase {
            barrier_width: w,
            ste_transmission: ste_t,
            detector_transmission: detector_t.clamp(0.0, 1.0),
            progress_transmission: progress_t,
            stochastic_transmission: baseline_t,
            residual,
            max_ar_residual: ar,
            ste_drift_fraction: drift,
            seconds_per_step: step_s,
        });
    }

    let rmse = (rss / cases.len().max(1) as f64).sqrt();
    let min_t = cases
        .iter()
        .map(|c| c.ste_transmission)
        .fold(f64::INFINITY, f64::min);
    let max_t = cases
        .iter()
        .map(|c| c.ste_transmission)
        .fold(f64::NEG_INFINITY, f64::max);
    let transmission_span = (max_t - min_t).abs();

    let mut csv = String::from(
        "barrier_width,ste_transmission,detector_transmission,progress_transmission,stochastic_transmission,residual,max_ar_residual,ste_drift_fraction,seconds_per_step\n",
    );
    for c in &cases {
        csv.push_str(&format!(
            "{:.4},{:.8},{:.8},{:.8},{:.8},{:.8},{:.8e},{:.8e},{:.8e}\n",
            c.barrier_width,
            c.ste_transmission,
            c.detector_transmission,
            c.progress_transmission,
            c.stochastic_transmission,
            c.residual,
            c.max_ar_residual,
            c.ste_drift_fraction,
            c.seconds_per_step,
        ));
    }

    let summary = serde_json::json!({
        "benchmark": {
            "name": "ste_tunneling_vs_stochastic",
            "ticks": ticks,
            "control_case": "width=0 provides free-run packet progress",
            "stochastic_model": "log-linear fit: ln(T)=a+b*width",
            "fit": { "a": a, "b": b, "rmse": rmse }
        },
        "gates": {
            "max_ar_residual": max_ar,
            "max_ste_drift_fraction": max_drift,
            "ar_gate": 1e-6,
            "ste_drift_gate": 1e-4,
            "ar_gate_pass": max_ar <= 1e-6,
            "ste_drift_gate_pass": max_drift <= 1e-4
        },
        "signals": {
            "transmission_min": min_t,
            "transmission_max": max_t,
            "transmission_span": transmission_span,
            "transmission_span_pass": transmission_span > 0.02,
            "free_run_progress": progress_denom
        },
        "cases": cases.iter().map(|c| serde_json::json!({
            "barrier_width": c.barrier_width,
            "ste_transmission": c.ste_transmission,
            "detector_transmission": c.detector_transmission,
            "progress_transmission": c.progress_transmission,
            "stochastic_transmission": c.stochastic_transmission,
            "residual": c.residual,
            "max_ar_residual": c.max_ar_residual,
            "ste_drift_fraction": c.ste_drift_fraction,
            "seconds_per_step": c.seconds_per_step
        })).collect::<Vec<_>>()
    });

    let mut out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    out_dir.push("target");
    out_dir.push("diagnostics");
    fs::create_dir_all(&out_dir).expect("failed to create diagnostics output dir");

    let csv_path = out_dir.join("tunneling_benchmark.csv");
    let json_path = out_dir.join("tunneling_benchmark_summary.json");

    fs::write(&csv_path, csv).expect("failed to write tunneling benchmark csv");
    fs::write(&json_path, serde_json::to_string_pretty(&summary).unwrap())
        .expect("failed to write tunneling benchmark json");

    let csv_len = fs::metadata(&csv_path).expect("missing csv artifact").len();
    let json_len = fs::metadata(&json_path).expect("missing json artifact").len();

    let ar_gate_pass = max_ar <= 1e-6;
    let drift_gate_pass = max_drift <= 1e-4;
    let span_gate_pass = transmission_span > 0.02;

    println!(
        "\nTUNNEL BENCH | rmse={:.6} b={:.6} max_ar={:.3e} max_drift={:.3e} span={:.6} | gates: ar={} drift={} span={}",
        rmse,
        b,
        max_ar,
        max_drift,
        transmission_span,
        ar_gate_pass,
        drift_gate_pass,
        span_gate_pass,
    );

    assert!(max_ar.is_finite(), "AR residual should be finite");
    assert!(max_drift.is_finite(), "STE drift should be finite");
    assert!(rmse.is_finite() && rmse < 0.5, "fit residual too high: {rmse}");
    assert!(csv_len > 128, "csv artifact unexpectedly small");
    assert!(json_len > 128, "json artifact unexpectedly small");
}
