//! Ambient well profile probe.
//!
//! Measures cumulative ambient STE from core outward for:
//! 1) flow-formed chokes,
//! 2) annihilation-seeded deep depressions (void cores).
//!
//! Also records where flow chokes disappear (pop radius) to test whether
//! they tend to pop close-in versus deep-annihilation structures.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use mass_effect_engine::config::SimConfig;
use mass_effect_engine::parcel::{ChokePhase, ChokeState, Parcel};
use mass_effect_engine::sim::Sim;

#[derive(Debug, Clone)]
struct FlowPopEvent {
    id: u64,
    tick: u64,
    radius: f64,
    integral: f64,
    profile: Vec<f64>,
}

#[derive(Debug, Clone)]
struct VoidProfileSample {
    tick: u64,
    integral: f64,
    profile: Vec<f64>,
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
    let mut seed: u64 = 717171;

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

fn build_annihilation_seeded() -> Sim {
    let mut parcels = Vec::new();
    parcels.push(make_choked(0, 14.0, 0.9, -0.15, 0.0, 0.0));
    parcels.push(make_choked(1, 13.0, -0.85, 0.15, 0.0, 0.0));

    for i in 0..24 {
        let a = i as f64 * std::f64::consts::TAU / 24.0;
        let mut p = Parcel::new((100 + i) as u64, 2.8);
        p.x = a.cos() * 1.2;
        p.y = a.sin() * 1.2;
        p.z = if i % 2 == 0 { 0.25 } else { -0.25 };
        parcels.push(p);
    }

    let mut config = SimConfig::default();
    config.dt = 0.008;
    config.max_bond_distance = 12.0;
    config.shell_interaction_range = 1.6;
    config.foam_annihilation_range = 2.5;
    config.annihilation_light_threshold = 0.5;
    config.annihilation_gamma_threshold = 1.0;
    config.annihilation_cavitation_threshold = 2.0;

    Sim::new(parcels, config)
}

fn cumulative_ambient_ste(parcels: &[Parcel], center: [f64; 3], max_r: f64, bins: usize) -> (f64, Vec<f64>) {
    let mut cum = vec![0.0_f64; bins];
    if bins == 0 || max_r <= 1e-12 {
        return (0.0, cum);
    }

    for p in parcels {
        let dx = p.x - center[0];
        let dy = p.y - center[1];
        let dz = p.z - center[2];
        let r = (dx * dx + dy * dy + dz * dz).sqrt();
        if r > max_r {
            continue;
        }
        let frac = (r / max_r).clamp(0.0, 0.999_999);
        let b = (frac * bins as f64).floor() as usize;
        for slot in cum.iter_mut().skip(b) {
            *slot += p.ste_amount.max(0.0);
        }
    }

    let integral = cum.iter().sum::<f64>();
    (integral, cum)
}

fn percentile(mut data: Vec<f64>, q: f64) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    data.sort_by(|a, b| a.total_cmp(b));
    let idx = ((data.len() - 1) as f64 * q.clamp(0.0, 1.0)).round() as usize;
    data[idx]
}

#[test]
fn ambient_well_profile_probe_reports_flow_vs_annihilation_depth() {
    let bins = 10usize;
    let flow_ticks = 500usize;
    let ann_ticks = 180usize;

    let mut csv = String::from(
        "scenario,tick,flow_chokes,void_cores,mean_ambient_integral,max_ambient_integral,pop_count\n",
    );

    // ---- Flow-formed choke scenario (scale sweep for observability) ----
    let mut pop_radii = Vec::new();
    let mut flow_integral_sum = 0.0_f64;
    let mut flow_integral_samples = 0u64;
    let mut selected_flow_scale = 0.0_f64;
    let mut selected_flow_pop_events: Vec<FlowPopEvent> = Vec::new();

    for &flow_scale in &[1.0_f64, 1.5_f64, 2.0_f64] {
        let mut flow = build_proton_like(260, flow_scale);
        let max_r_flow = flow.config.shell_interaction_range * 4.0;

        let mut prev_active: HashSet<u64> = HashSet::new();
        let mut last_radius: HashMap<u64, f64> = HashMap::new();
        let mut last_profile: HashMap<u64, Vec<f64>> = HashMap::new();
        let mut last_integral: HashMap<u64, f64> = HashMap::new();
        let mut last_tick: HashMap<u64, u64> = HashMap::new();

        let mut scale_integral_sum = 0.0_f64;
        let mut scale_integral_samples = 0u64;
        let mut scale_pop_radii = Vec::new();
        let mut scale_pop_events: Vec<FlowPopEvent> = Vec::new();

        for _ in 0..flow_ticks {
            let m = flow.step();
            let d = flow
                .last_diagnostics
                .as_ref()
                .expect("flow diagnostics should exist each tick");

            let mut active_now: HashSet<u64> = HashSet::new();
            let mut integral_tick_sum = 0.0_f64;
            let mut integral_tick_max = 0.0_f64;

            for p in &flow.parcels {
                let c = match &p.choke {
                    Some(c) => c,
                    None => continue,
                };
                active_now.insert(p.id);
                last_radius.insert(p.id, c.radius);

                let (integral, profile) = cumulative_ambient_ste(
                    &flow.parcels,
                    [p.x, p.y, p.z],
                    max_r_flow,
                    bins,
                );
                last_profile.insert(p.id, profile);
                last_integral.insert(p.id, integral);
                last_tick.insert(p.id, d.tick);
                integral_tick_sum += integral;
                integral_tick_max = integral_tick_max.max(integral);
                scale_integral_sum += integral;
                scale_integral_samples += 1;
            }

            for id in prev_active.difference(&active_now) {
                if let Some(r) = last_radius.get(id) {
                    scale_pop_radii.push(*r);
                }
                if let (Some(radius), Some(integral), Some(profile), Some(tick)) = (
                    last_radius.get(id),
                    last_integral.get(id),
                    last_profile.get(id),
                    last_tick.get(id),
                ) {
                    scale_pop_events.push(FlowPopEvent {
                        id: *id,
                        tick: *tick,
                        radius: *radius,
                        integral: *integral,
                        profile: profile.clone(),
                    });
                }
            }
            prev_active = active_now;

            let mean_integral_tick = if m.choke_count > 0 {
                integral_tick_sum / m.choke_count as f64
            } else {
                0.0
            };

            csv.push_str(&format!(
                "flow_{:.1},{},{},{},{:.8},{:.8},{}\n",
                flow_scale,
                d.tick,
                m.choke_count,
                d.void_core_count,
                mean_integral_tick,
                integral_tick_max,
                scale_pop_radii.len(),
            ));
        }

        if scale_integral_samples > flow_integral_samples {
            selected_flow_scale = flow_scale;
            flow_integral_sum = scale_integral_sum;
            flow_integral_samples = scale_integral_samples;
            pop_radii = scale_pop_radii;
            selected_flow_pop_events = scale_pop_events;
        }
    }

    // ---- Annihilation-seeded deep depression scenario ----
    let mut ann = build_annihilation_seeded();
    let max_r_ann = ann.config.shell_interaction_range * 4.0;

    let mut ann_integral_sum = 0.0_f64;
    let mut ann_integral_samples = 0u64;
    let mut void_present_ticks = 0u64;
    let mut ann_void_samples: Vec<VoidProfileSample> = Vec::new();

    for _ in 0..ann_ticks {
        let m = ann.step();
        let d = ann
            .last_diagnostics
            .as_ref()
            .expect("ann diagnostics should exist each tick");

        let mut integral_tick_sum = 0.0_f64;
        let mut integral_tick_max = 0.0_f64;
        let mut void_count = 0usize;

        for p in &ann.parcels {
            if p.role != 2 {
                continue;
            }
            void_count += 1;
            let (integral, profile) = cumulative_ambient_ste(
                &ann.parcels,
                [p.x, p.y, p.z],
                max_r_ann,
                bins,
            );
            integral_tick_sum += integral;
            integral_tick_max = integral_tick_max.max(integral);
            ann_integral_sum += integral;
            ann_integral_samples += 1;

            if ann_void_samples.len() < 80 {
                ann_void_samples.push(VoidProfileSample {
                    tick: d.tick,
                    integral,
                    profile,
                });
            }
        }

        if void_count > 0 {
            void_present_ticks += 1;
        }

        let mean_integral_tick = if void_count > 0 {
            integral_tick_sum / void_count as f64
        } else {
            0.0
        };

        csv.push_str(&format!(
            "annihilation,{},{},{},{:.8},{:.8},{}\n",
            d.tick,
            m.choke_count,
            d.void_core_count,
            mean_integral_tick,
            integral_tick_max,
            0,
        ));
    }

    let flow_integral_mean = if flow_integral_samples > 0 {
        flow_integral_sum / flow_integral_samples as f64
    } else {
        0.0
    };
    let ann_integral_mean = if ann_integral_samples > 0 {
        ann_integral_sum / ann_integral_samples as f64
    } else {
        0.0
    };

    let pop_mean = if pop_radii.is_empty() {
        0.0
    } else {
        pop_radii.iter().sum::<f64>() / pop_radii.len() as f64
    };
    let pop_p75 = percentile(pop_radii.clone(), 0.75);

    // Top pop events: smallest radii (closest-in pops) are the most informative.
    selected_flow_pop_events.sort_by(|a, b| a.radius.total_cmp(&b.radius));
    let top_pop_profiles: Vec<_> = selected_flow_pop_events
        .iter()
        .take(40)
        .map(|e| serde_json::json!({
            "id": e.id,
            "tick": e.tick,
            "radius": e.radius,
            "ambient_integral": e.integral,
            "cumulative_profile": e.profile
        }))
        .collect();

    ann_void_samples.sort_by(|a, b| b.integral.total_cmp(&a.integral));
    let top_void_profiles: Vec<_> = ann_void_samples
        .iter()
        .take(40)
        .map(|s| serde_json::json!({
            "tick": s.tick,
            "ambient_integral": s.integral,
            "cumulative_profile": s.profile
        }))
        .collect();

    let summary = serde_json::json!({
        "probe": "ambient_well_profile_probe",
        "flow": {
            "ticks": flow_ticks,
            "selected_scale": selected_flow_scale,
            "ambient_ste_integral_samples": flow_integral_samples,
            "ambient_ste_integral_mean": flow_integral_mean,
            "pop_count": pop_radii.len(),
            "pop_radius_mean": pop_mean,
            "pop_radius_p75": pop_p75
        },
        "annihilation": {
            "ticks": ann_ticks,
            "ambient_ste_integral_mean": ann_integral_mean,
            "void_present_ticks": void_present_ticks,
            "void_present_ratio": void_present_ticks as f64 / ann_ticks as f64
        },
        "comparison": {
            "annihilation_to_flow_integral_ratio": ann_integral_mean / flow_integral_mean.max(1e-12)
        },
        "profile_bins": {
            "count": bins,
            "max_radius_scale": 4.0,
            "interpretation": "cumulative_profile[k] = cumulative ambient STE from center out to bin k"
        },
        "top_flow_pop_profiles": top_pop_profiles,
        "top_annihilation_void_profiles": top_void_profiles,
        "note": "Flow chokes use pop radius tracking from last active tick; annihilation scenario tracks role=2 void cores."
    });

    let mut out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    out_dir.push("target");
    out_dir.push("diagnostics");
    fs::create_dir_all(&out_dir).expect("failed to create diagnostics output dir");

    fs::write(out_dir.join("ambient_well_profile_probe.csv"), csv)
        .expect("failed to write ambient_well_profile_probe.csv");
    fs::write(
        out_dir.join("ambient_well_profile_probe_summary.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .expect("failed to write ambient_well_profile_probe_summary.json");

    println!(
        "\nAMBIENT WELL PROBE | flow_integral_mean={:.3} ann_integral_mean={:.3} pop_mean={:.3} pop_p75={:.3}",
        flow_integral_mean,
        ann_integral_mean,
        pop_mean,
        pop_p75,
    );

    assert!(flow_integral_mean.is_finite());
    assert!(ann_integral_mean.is_finite() && ann_integral_mean > 0.0);
}
