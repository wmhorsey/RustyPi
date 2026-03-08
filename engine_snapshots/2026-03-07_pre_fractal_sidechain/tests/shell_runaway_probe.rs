//! Shell runaway event probe.
//!
//! Detects and records explosive shell-growth episodes without clamping.
//! This is analysis-first instrumentation for understanding true dynamics.

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
    let mut seed: u64 = 515151;

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
struct EventRecord {
    scale: f64,
    tick: u64,
    parcel_id: u64,
    excess: f64,
    growth: f64,
    growth_ratio: f64,
    depression_gap: f64,
    shell_intensity: f64,
}

#[test]
fn shell_runaway_probe_records_events() {
    let scales = [1.0_f64, 1.5_f64, 2.0_f64];
    let ticks = 450usize;

    let mut csv = String::from(
        "scale,tick,events_this_tick,max_growth_ratio,max_excess,shell_intensity,choke_count,total_ste\n",
    );
    let mut top_events: Vec<EventRecord> = Vec::new();
    let mut summaries = Vec::new();

    for &scale in &scales {
        let mut sim = build_proton_like(260, scale);
        let mut prev_excess: Vec<f64> = Vec::new();

        let mut event_count = 0u64;
        let mut event_ticks = 0u64;
        let mut max_growth_ratio = 0.0_f64;
        let mut max_excess = 0.0_f64;
        let mut sum_gap_event = 0.0_f64;

        for _ in 0..ticks {
            let m = sim.step();
            let d = sim
                .last_diagnostics
                .as_ref()
                .expect("diagnostics should exist each tick");

            if prev_excess.len() < sim.parcels.len() {
                let old_len = prev_excess.len();
                prev_excess.resize(sim.parcels.len(), 0.0);
                for i in old_len..sim.parcels.len() {
                    prev_excess[i] = sim.parcels[i].squeeze_excess();
                }
            }

            let n = sim.parcels.len();
            let neighborhood = sim.config.shell_interaction_range * 5.0;
            let mut local_avg_conc = vec![0.0_f64; n];
            for i in 0..n {
                let mut sum = 0.0;
                let mut count = 0usize;
                for j in 0..n {
                    if i == j {
                        continue;
                    }
                    let d_ij = sim.parcels[i].dist_to(&sim.parcels[j]);
                    if d_ij < neighborhood {
                        sum += sim.parcels[j].concentration;
                        count += 1;
                    }
                }
                local_avg_conc[i] = if count > 0 {
                    sum / count as f64
                } else {
                    sim.parcels[i].concentration
                };
            }

            let mut events_this_tick = 0u64;
            let mut max_growth_ratio_tick = 0.0_f64;
            let mut max_excess_tick = 0.0_f64;

            for i in 0..n {
                let p = &sim.parcels[i];
                let excess = p.squeeze_excess();
                let prev = prev_excess[i];
                prev_excess[i] = excess;

                max_excess_tick = max_excess_tick.max(excess);
                max_excess = max_excess.max(excess);

                let growth = (excess - prev).max(0.0);
                if growth <= 0.0 {
                    continue;
                }

                let growth_ratio = growth / prev.max(1e-10);
                max_growth_ratio_tick = max_growth_ratio_tick.max(growth_ratio);
                max_growth_ratio = max_growth_ratio.max(growth_ratio);

                let c = match &p.choke {
                    Some(c) => c,
                    None => continue,
                };
                let depression_gap = local_avg_conc[i] - c.equilibrium_concentration;

                // Event gate: fast shell-excess growth while exterior > core depression.
                if depression_gap > 0.0 && growth_ratio > 0.5 && excess > 1e-6 {
                    events_this_tick += 1;
                    event_count += 1;
                    sum_gap_event += depression_gap;

                    top_events.push(EventRecord {
                        scale,
                        tick: d.tick,
                        parcel_id: p.id,
                        excess,
                        growth,
                        growth_ratio,
                        depression_gap,
                        shell_intensity: d.shell_excess_energy_proxy / d.total_ste.max(1e-12),
                    });
                }
            }

            if events_this_tick > 0 {
                event_ticks += 1;
            }

            let shell_intensity = d.shell_excess_energy_proxy / d.total_ste.max(1e-12);
            csv.push_str(&format!(
                "{:.3},{},{},{:.8},{:.8},{:.8},{},{}\n",
                scale,
                d.tick,
                events_this_tick,
                max_growth_ratio_tick,
                max_excess_tick,
                shell_intensity,
                m.choke_count,
                d.total_ste,
            ));
        }

        let mean_gap_event = if event_count > 0 {
            sum_gap_event / event_count as f64
        } else {
            0.0
        };

        summaries.push(serde_json::json!({
            "scale": scale,
            "ticks": ticks,
            "event_count": event_count,
            "event_tick_count": event_ticks,
            "event_tick_ratio": event_ticks as f64 / ticks as f64,
            "max_growth_ratio": max_growth_ratio,
            "max_shell_excess": max_excess,
            "mean_depression_gap_event": mean_gap_event,
        }));
    }

    top_events.sort_by(|a, b| b.growth_ratio.total_cmp(&a.growth_ratio));
    let top_rows: Vec<_> = top_events
        .iter()
        .take(40)
        .map(|e| {
            serde_json::json!({
                "scale": e.scale,
                "tick": e.tick,
                "parcel_id": e.parcel_id,
                "excess": e.excess,
                "growth": e.growth,
                "growth_ratio": e.growth_ratio,
                "depression_gap": e.depression_gap,
                "shell_intensity": e.shell_intensity
            })
        })
        .collect();

    let summary = serde_json::json!({
        "probe": "shell_runaway_probe",
        "definition": {
            "event": "depression_gap > 0 && growth_ratio > 0.5 && shell_excess > 1e-6",
            "growth_ratio": "(shell_excess_t - shell_excess_t-1) / max(shell_excess_t-1, 1e-10)"
        },
        "scale_summaries": summaries,
        "top_events": top_rows
    });

    let mut out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    out_dir.push("target");
    out_dir.push("diagnostics");
    fs::create_dir_all(&out_dir).expect("failed to create diagnostics output dir");

    fs::write(out_dir.join("shell_runaway_probe.csv"), csv)
        .expect("failed to write shell_runaway_probe.csv");
    fs::write(
        out_dir.join("shell_runaway_probe_summary.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .expect("failed to write shell_runaway_probe_summary.json");

    println!("\nSHELL RUNAWAY PROBE complete; see diagnostics artifacts.");

    assert!(!top_rows.is_empty(), "expected at least one detected shell-growth event");
}
