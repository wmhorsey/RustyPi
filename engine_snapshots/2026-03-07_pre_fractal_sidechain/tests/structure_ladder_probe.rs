//! Structure ladder probe.
//!
//! Measures whether recurring shell behavior supports hierarchical motifs:
//! 1) triadic choke groups (quark-like),
//! 2) larger composite clusters (nucleon/atom-like),
//! 3) shell-profile similarity between triad-level and composite-level structures.

use std::collections::{HashMap, HashSet, VecDeque};
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
    let mut seed: u64 = 121212;

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

fn triad_key(a: u64, b: u64, c: u64) -> (u64, u64, u64) {
    let mut ids = [a, b, c];
    ids.sort_unstable();
    (ids[0], ids[1], ids[2])
}

fn component_key(ids: &[u64]) -> String {
    let mut sorted = ids.to_vec();
    sorted.sort_unstable();
    sorted
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join("-")
}

fn radial_shell_profile(parcels: &[Parcel], center: [f64; 3], max_r: f64, bins: usize) -> Vec<f64> {
    let mut hist = vec![0.0_f64; bins];
    if bins == 0 || max_r <= 1e-12 {
        return hist;
    }

    for p in parcels {
        let dx = p.x - center[0];
        let dy = p.y - center[1];
        let dz = p.z - center[2];
        let r = (dx * dx + dy * dy + dz * dz).sqrt();
        if r <= max_r {
            let frac = (r / max_r).clamp(0.0, 0.999_999);
            let idx = (frac * bins as f64).floor() as usize;
            hist[idx] += p.concentration.max(0.0);
        }
    }

    let sum = hist.iter().sum::<f64>().max(1e-12);
    for v in &mut hist {
        *v /= sum;
    }
    hist
}

fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0;
    let mut na = 0.0;
    let mut nb = 0.0;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na <= 1e-12 || nb <= 1e-12 {
        0.0
    } else {
        (dot / (na.sqrt() * nb.sqrt())).clamp(-1.0, 1.0)
    }
}

#[test]
fn structure_ladder_probe_reports_hierarchical_metrics() {
    let ticks = 700;
    let mut sim = build_proton_like(260, 1.0);

    let triad_range = sim.config.shell_interaction_range * 1.45;
    let composite_range = sim.config.shell_interaction_range * 2.6;
    let shell_probe_radius = sim.config.shell_interaction_range * 4.0;
    let bins = 8usize;

    let mut triad_dwell: HashMap<(u64, u64, u64), u64> = HashMap::new();
    let mut comp_dwell: HashMap<String, u64> = HashMap::new();

    let mut triad_count_sum = 0.0_f64;
    let mut triad_member_ratio_sum = 0.0_f64;
    let mut comp_count_sum = 0.0_f64;
    let mut largest_comp_sum = 0.0_f64;

    let mut max_triad_streak = 0_u64;
    let mut max_comp_streak = 0_u64;

    let mut shell_sim_sum = 0.0_f64;
    let mut shell_sim_count = 0_u64;

    let mut csv = String::from(
        "tick,choked,triads,triad_member_ratio,components_ge6,largest_component,max_triad_streak,max_comp_streak,shell_similarity\n",
    );

    for _ in 0..ticks {
        sim.step();

        let active: Vec<usize> = sim
            .parcels
            .iter()
            .enumerate()
            .filter_map(|(i, p)| if p.is_choked() { Some(i) } else { None })
            .collect();

        let n_all = sim.parcels.len().max(1) as f64;
        let n_active = active.len();

        let mut triads: HashSet<(u64, u64, u64)> = HashSet::new();
        let mut triad_members: HashSet<u64> = HashSet::new();
        let mut triad_centers = Vec::new();

        for ai in 0..n_active {
            let i = active[ai];
            for aj in (ai + 1)..n_active {
                let j = active[aj];
                let dij = sim.parcels[i].dist_to(&sim.parcels[j]);
                if dij > triad_range {
                    continue;
                }
                for ak in (aj + 1)..n_active {
                    let k = active[ak];
                    let dik = sim.parcels[i].dist_to(&sim.parcels[k]);
                    let djk = sim.parcels[j].dist_to(&sim.parcels[k]);
                    if dik > triad_range || djk > triad_range {
                        continue;
                    }

                    // Require coherent shell/depression neighborhood.
                    let ci = sim.parcels[i].concentration;
                    let cj = sim.parcels[j].concentration;
                    let ck = sim.parcels[k].concentration;
                    let c_mean = (ci + cj + ck) / 3.0;
                    let c_var = ((ci - c_mean).powi(2) + (cj - c_mean).powi(2) + (ck - c_mean).powi(2)) / 3.0;
                    if c_var > c_mean.max(1e-9) * 0.45 {
                        continue;
                    }

                    let key = triad_key(sim.parcels[i].id, sim.parcels[j].id, sim.parcels[k].id);
                    triads.insert(key);
                    triad_members.insert(sim.parcels[i].id);
                    triad_members.insert(sim.parcels[j].id);
                    triad_members.insert(sim.parcels[k].id);

                    triad_centers.push([
                        (sim.parcels[i].x + sim.parcels[j].x + sim.parcels[k].x) / 3.0,
                        (sim.parcels[i].y + sim.parcels[j].y + sim.parcels[k].y) / 3.0,
                        (sim.parcels[i].z + sim.parcels[j].z + sim.parcels[k].z) / 3.0,
                    ]);
                }
            }
        }

        let triad_keys: HashSet<(u64, u64, u64)> = triads.iter().copied().collect();
        triad_dwell.retain(|k, _| triad_keys.contains(k));
        for key in triad_keys {
            let entry = triad_dwell.entry(key).or_insert(0);
            *entry += 1;
            max_triad_streak = max_triad_streak.max(*entry);
        }

        // Build component graph among active/choked parcels.
        let mut neighbors: Vec<Vec<usize>> = vec![Vec::new(); n_active];
        for a in 0..n_active {
            for b in (a + 1)..n_active {
                let i = active[a];
                let j = active[b];
                let d = sim.parcels[i].dist_to(&sim.parcels[j]);
                if d <= composite_range {
                    neighbors[a].push(b);
                    neighbors[b].push(a);
                }
            }
        }

        let mut seen = vec![false; n_active];
        let mut component_keys = HashSet::new();
        let mut component_centers = Vec::new();
        let mut comp_count_ge6 = 0usize;
        let mut largest_comp = 0usize;

        for start in 0..n_active {
            if seen[start] {
                continue;
            }
            let mut q = VecDeque::new();
            q.push_back(start);
            seen[start] = true;
            let mut members_local: Vec<usize> = Vec::new();

            while let Some(v) = q.pop_front() {
                members_local.push(v);
                for &nb in &neighbors[v] {
                    if !seen[nb] {
                        seen[nb] = true;
                        q.push_back(nb);
                    }
                }
            }

            let comp_size = members_local.len();
            largest_comp = largest_comp.max(comp_size);
            if comp_size >= 6 {
                comp_count_ge6 += 1;
                let mut ids = Vec::with_capacity(comp_size);
                let mut cx = 0.0;
                let mut cy = 0.0;
                let mut cz = 0.0;
                for idx_local in members_local {
                    let pidx = active[idx_local];
                    ids.push(sim.parcels[pidx].id);
                    cx += sim.parcels[pidx].x;
                    cy += sim.parcels[pidx].y;
                    cz += sim.parcels[pidx].z;
                }
                let denom = ids.len().max(1) as f64;
                component_centers.push([cx / denom, cy / denom, cz / denom]);
                component_keys.insert(component_key(&ids));
            }
        }

        comp_dwell.retain(|k, _| component_keys.contains(k));
        for key in component_keys {
            let entry = comp_dwell.entry(key).or_insert(0);
            *entry += 1;
            max_comp_streak = max_comp_streak.max(*entry);
        }

        // Shell-profile similarity across ladder levels.
        let shell_similarity = if !triad_centers.is_empty() && !component_centers.is_empty() {
            let mut triad_profile = vec![0.0_f64; bins];
            for c in &triad_centers {
                let h = radial_shell_profile(&sim.parcels, *c, shell_probe_radius, bins);
                for i in 0..bins {
                    triad_profile[i] += h[i];
                }
            }
            for v in &mut triad_profile {
                *v /= triad_centers.len() as f64;
            }

            let mut comp_profile = vec![0.0_f64; bins];
            for c in &component_centers {
                let h = radial_shell_profile(&sim.parcels, *c, shell_probe_radius, bins);
                for i in 0..bins {
                    comp_profile[i] += h[i];
                }
            }
            for v in &mut comp_profile {
                *v /= component_centers.len() as f64;
            }

            let s = cosine_similarity(&triad_profile, &comp_profile);
            shell_sim_sum += s;
            shell_sim_count += 1;
            s
        } else {
            0.0
        };

        let triad_count = triads.len() as f64;
        let triad_member_ratio = triad_members.len() as f64 / n_all;

        triad_count_sum += triad_count;
        triad_member_ratio_sum += triad_member_ratio;
        comp_count_sum += comp_count_ge6 as f64;
        largest_comp_sum += largest_comp as f64;

        csv.push_str(&format!(
            "{},{},{},{:.6},{},{},{},{},{:.6}\n",
            sim.tick,
            n_active,
            triads.len(),
            triad_member_ratio,
            comp_count_ge6,
            largest_comp,
            max_triad_streak,
            max_comp_streak,
            shell_similarity,
        ));
    }

    let triad_mean = triad_count_sum / ticks as f64;
    let triad_member_ratio_mean = triad_member_ratio_sum / ticks as f64;
    let comp_count_mean = comp_count_sum / ticks as f64;
    let largest_comp_mean = largest_comp_sum / ticks as f64;
    let shell_similarity_mean = if shell_sim_count > 0 {
        shell_sim_sum / shell_sim_count as f64
    } else {
        0.0
    };

    let ladder_score = 0.35 * (triad_member_ratio_mean * 10.0).min(1.0)
        + 0.25 * (comp_count_mean / 3.0).min(1.0)
        + 0.20 * (max_triad_streak as f64 / ticks as f64).min(1.0)
        + 0.20 * ((shell_similarity_mean + 1.0) * 0.5).clamp(0.0, 1.0);

    let summary = serde_json::json!({
        "probe": "structure_ladder_probe",
        "ticks": ticks,
        "metrics": {
            "triad_count_mean": triad_mean,
            "triad_member_ratio_mean": triad_member_ratio_mean,
            "composite_count_ge6_mean": comp_count_mean,
            "largest_composite_mean": largest_comp_mean,
            "max_triad_streak": max_triad_streak,
            "max_composite_streak": max_comp_streak,
            "shell_similarity_mean": shell_similarity_mean,
            "shell_similarity_samples": shell_sim_count
        },
        "ladder_score": ladder_score,
        "notes": [
            "Triads are distance-and-coherence constrained triples of choked parcels.",
            "Composites are connected components of choked parcels with size >= 6.",
            "Shell similarity compares radial shell profiles around triad and composite centers."
        ]
    });

    let mut out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    out_dir.push("target");
    out_dir.push("diagnostics");
    fs::create_dir_all(&out_dir).expect("failed to create diagnostics output dir");

    fs::write(out_dir.join("structure_ladder_probe.csv"), csv)
        .expect("failed to write structure_ladder_probe.csv");
    fs::write(
        out_dir.join("structure_ladder_probe_summary.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .expect("failed to write structure_ladder_probe_summary.json");

    println!(
        "\nSTRUCTURE LADDER | triad_mean={:.3} triad_ratio={:.3} comp_ge6_mean={:.3} max_triad={} max_comp={} shell_sim={:.3} ladder={:.3}",
        triad_mean,
        triad_member_ratio_mean,
        comp_count_mean,
        max_triad_streak,
        max_comp_streak,
        shell_similarity_mean,
        ladder_score,
    );

    assert!(triad_mean.is_finite(), "triad mean should be finite");
    assert!(shell_similarity_mean.is_finite(), "shell similarity mean should be finite");
    assert!(ladder_score.is_finite(), "ladder score should be finite");
}
