//! STE -> MeV resistance-weight fitting probe.
//!
//! Searches shell/choke/slip weights for R* to minimize k drift across
//! STE scales [1.0, 1.5, 2.0] using an electron rest-energy anchor.

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
    let mut seed: u64 = 4242;

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

#[derive(Debug, Clone)]
struct TickSample {
    total_ste: f64,
    shell_intensity: f64,
    choke_density: f64,
    mean_slip: f64,
    choked: bool,
}

#[derive(Debug, Clone, Copy)]
struct ProbeStats {
    e_mean: f64,
    e_integral_choked: f64,
    choked_ticks: usize,
}

fn collect_samples(ste_scale: f64, ticks: usize) -> (Vec<TickSample>, f64) {
    let mut sim = build_proton_like(260, ste_scale);
    let dt = sim.config.dt;
    let mut samples = Vec::with_capacity(ticks);

    for _ in 0..ticks {
        sim.step();
        let d = sim
            .last_diagnostics
            .as_ref()
            .expect("diagnostics should exist each tick");

        let n = sim.parcels.len().max(1) as f64;
        let has_choke = sim.parcels.iter().any(|p| p.is_choked());

        samples.push(TickSample {
            total_ste: d.total_ste,
            shell_intensity: d.shell_excess_energy_proxy / d.total_ste.max(1e-12),
            choke_density: d.choke_coherence_total / n,
            mean_slip: d.mean_velocity_slip * if has_choke { 1.0 } else { 0.5 },
            choked: has_choke,
        });
    }

    (samples, dt)
}

fn evaluate(samples: &[TickSample], dt: f64, w_shell: f64, w_choke: f64, w_slip: f64) -> ProbeStats {
    let mut e_sum = 0.0_f64;
    let mut e_integral_choked = 0.0_f64;
    let mut choked_ticks = 0usize;

    for s in samples {
        let r_star = (w_shell * s.shell_intensity + w_choke * s.choke_density + w_slip * s.mean_slip).max(0.0);
        let e_star = s.total_ste * r_star;
        e_sum += e_star;

        if s.choked {
            e_integral_choked += e_star * dt;
            choked_ticks += 1;
        }
    }

    ProbeStats {
        e_mean: e_sum / samples.len().max(1) as f64,
        e_integral_choked,
        choked_ticks,
    }
}

#[test]
fn ste_mev_weight_fit_minimizes_k_drift() {
    let ticks = 320;
    let scales = [1.0_f64, 1.5_f64, 2.0_f64];
    let mev_anchor = 0.511_f64;

    let mut cached = Vec::new();
    for s in scales {
        let (samples, dt) = collect_samples(s, ticks);
        cached.push((s, samples, dt));
    }

    let mut best_score = f64::INFINITY;
    let mut best = (0.0_f64, 0.0_f64, 0.0_f64);
    let mut best_rows = Vec::new();

    // Coarse simplex grid with 5% steps.
    for a_i in 0..=20 {
        for b_i in 0..=(20 - a_i) {
            let c_i = 20 - a_i - b_i;
            let w_shell = a_i as f64 / 20.0;
            let w_choke = b_i as f64 / 20.0;
            let w_slip = c_i as f64 / 20.0;

            let mut rows = Vec::new();
            for (scale, samples, dt) in &cached {
                let st = evaluate(samples, *dt, w_shell, w_choke, w_slip);
                let k_mean = mev_anchor / st.e_mean.max(1e-12);
                let k_event = if st.choked_ticks > 0 {
                    mev_anchor / st.e_integral_choked.max(1e-12)
                } else {
                    f64::NAN
                };
                rows.push((*scale, k_mean, k_event, st.choked_ticks));
            }

            let k_base_mean = rows[0].1;
            let k_base_event = rows[0].2;

            let mut mean_drift = 0.0_f64;
            let mut event_drift = 0.0_f64;
            let mut event_pairs = 0usize;
            let mut low_event_coverage_penalty = 0.0_f64;

            for row in &rows[1..] {
                mean_drift += ((row.1 / k_base_mean.max(1e-12)) - 1.0).abs();
                if row.3 < 5 {
                    low_event_coverage_penalty += 0.25;
                }
                if k_base_event.is_finite() && row.2.is_finite() {
                    event_drift += ((row.2 / k_base_event.max(1e-12)) - 1.0).abs();
                    event_pairs += 1;
                }
            }
            mean_drift /= (rows.len() - 1) as f64;
            if event_pairs > 0 {
                event_drift /= event_pairs as f64;
            }

            let event_weight = if event_pairs > 0 { 0.45 } else { 0.0 };
            let mean_weight = 1.0 - event_weight;

            // Blend mean and event-integral stability.
            let score = mean_weight * mean_drift
                + event_weight * event_drift
                + low_event_coverage_penalty;

            if score < best_score {
                best_score = score;
                best = (w_shell, w_choke, w_slip);
                best_rows = rows;
            }
        }
    }

    let details: Vec<_> = best_rows
        .iter()
        .map(|(scale, k_mean, k_event, choked_ticks)| {
            serde_json::json!({
                "scale": scale,
                "k_mean": k_mean,
                "k_event_integral": k_event,
                "choked_ticks": choked_ticks
            })
        })
        .collect();

    let summary = serde_json::json!({
        "probe": "ste_mev_weight_fit",
        "ticks": ticks,
        "anchor_mev": mev_anchor,
        "scales": scales,
        "best_weights": {
            "shell": best.0,
            "choke": best.1,
            "slip": best.2
        },
        "score": best_score,
        "fit_rows": details
    });

    let mut out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    out_dir.push("target");
    out_dir.push("diagnostics");
    fs::create_dir_all(&out_dir).expect("failed to create diagnostics output dir");

    fs::write(
        out_dir.join("ste_mev_weight_fit_summary.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .expect("failed to write ste_mev_weight_fit_summary.json");

    println!(
        "\nSTE->MeV WEIGHT FIT | best(shell,choke,slip)=({:.2},{:.2},{:.2}) score={:.6}",
        best.0,
        best.1,
        best.2,
        best_score,
    );

    assert!(best_score.is_finite(), "fit score must be finite");
}
