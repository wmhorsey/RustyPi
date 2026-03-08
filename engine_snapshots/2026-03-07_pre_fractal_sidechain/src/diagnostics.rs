use serde::{Deserialize, Serialize};

use crate::config::SimConfig;
use crate::field;
use crate::parcel::Parcel;

/// Per-tick diagnostics used for conservation and force-law auditing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameDiagnostics {
    pub tick: u64,

    // Conservation ledger
    pub total_ste: f64,
    pub linear_momentum: [f64; 3],
    pub linear_momentum_mag: f64,
    pub angular_momentum: [f64; 3],
    pub angular_momentum_mag: f64,
    pub kinetic_energy: f64,
    pub shell_excess_energy_proxy: f64,
    pub choke_coherence_total: f64,
    pub mean_velocity_slip: f64,
    pub anchored_depression_count: usize,
    pub inside_out_bubble_count: usize,
    pub anchored_gap_mean: f64,
    pub void_core_count: usize,
    pub up_quark_candidate_count: usize,
    pub void_core_shell_support_mean: f64,
    pub fractalization_index: f64,
    pub cross_scale_flux_mean: f64,
    pub scale_phase_lock: f64,
    pub cascade_break_count: usize,

    // Force-channel diagnostics
    pub total_attractive_force: f64,
    pub total_pressure_force: f64,
    pub net_internal_force: [f64; 3],
    pub net_internal_force_mag: f64,
    pub action_reaction_residual: f64,

    // Compound coexistence diagnostics (matter/anti interaction readiness)
    pub compound_pair_count: usize,
    pub compound_dwell_mean_ticks: f64,
    pub compound_potential_sum: f64,
    pub compound_potential_mean: f64,
    pub annihilation_energy_this_tick: f64,

    // Matter/antimatter chirality oscillator diagnostics
    pub matter_count: usize,
    pub anti_count: usize,
    pub chirality: f64,
    pub chirality_abs: f64,
    pub chirality_zero_crossings: u64,
    pub chirality_lock_ticks: u64,

    // Light as driving-wave collapse diagnostics
    pub relay_collapse_count: u64,
    pub transient_relay_collapses: u64,
    pub terminal_capture_collapses: u64,
    pub relay_shell_overshoot_sum: f64,
    pub relay_shell_overshoot_mean: f64,
    pub relay_relaxation_ticks_sum: u64,
    pub relay_relaxation_ticks_mean: f64,
}

/// Compute diagnostics for the current parcel state.
pub fn compute_frame_diagnostics(parcels: &[Parcel], config: &SimConfig, tick: u64) -> FrameDiagnostics {
    let mut total_ste = 0.0;
    let mut p = [0.0_f64; 3];
    let mut l = [0.0_f64; 3];
    let mut kinetic = 0.0;
    let mut shell_excess = 0.0;
    let mut choke_coherence_total = 0.0;

    for parcel in parcels {
        let m = parcel.ste_amount.max(0.0);
        total_ste += parcel.ste_amount;

        p[0] += m * parcel.vx;
        p[1] += m * parcel.vy;
        p[2] += m * parcel.vz;

        // L = r x (m v)
        let mvx = m * parcel.vx;
        let mvy = m * parcel.vy;
        let mvz = m * parcel.vz;
        l[0] += parcel.y * mvz - parcel.z * mvy;
        l[1] += parcel.z * mvx - parcel.x * mvz;
        l[2] += parcel.x * mvy - parcel.y * mvx;

        kinetic += 0.5 * m * (parcel.vx * parcel.vx + parcel.vy * parcel.vy + parcel.vz * parcel.vz);

        // Proxy for local compressive storage in shells.
        shell_excess += parcel.squeeze_excess() * m;

        if let Some(c) = &parcel.choke {
            choke_coherence_total += c.coherence.max(0.0);
        }
    }

    let linear_momentum_mag = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
    let angular_momentum_mag = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();

    let (total_attractive_force, total_pressure_force, net_internal_force) = force_channel_diagnostics(parcels, config);
    let mean_velocity_slip = mean_velocity_slip(parcels, config);
    let net_internal_force_mag = (net_internal_force[0] * net_internal_force[0]
        + net_internal_force[1] * net_internal_force[1]
        + net_internal_force[2] * net_internal_force[2])
        .sqrt();

    let force_scale = (total_attractive_force + total_pressure_force).max(1e-10);
    let action_reaction_residual = net_internal_force_mag / force_scale;

    let (anchored_depression_count, inside_out_bubble_count, anchored_gap_mean) =
        shell_anchor_diagnostics(parcels, config);
    let (void_core_count, up_quark_candidate_count, void_core_shell_support_mean) =
        void_core_diagnostics(parcels, config);
    let (fractalization_index, cross_scale_flux_mean, scale_phase_lock, cascade_break_count) =
        fractalization_diagnostics(parcels, config);

    FrameDiagnostics {
        tick,
        total_ste,
        linear_momentum: p,
        linear_momentum_mag,
        angular_momentum: l,
        angular_momentum_mag,
        kinetic_energy: kinetic,
        shell_excess_energy_proxy: shell_excess,
        choke_coherence_total,
        mean_velocity_slip,
        anchored_depression_count,
        inside_out_bubble_count,
        anchored_gap_mean,
        void_core_count,
        up_quark_candidate_count,
        void_core_shell_support_mean,
        fractalization_index,
        cross_scale_flux_mean,
        scale_phase_lock,
        cascade_break_count,
        total_attractive_force,
        total_pressure_force,
        net_internal_force,
        net_internal_force_mag,
        action_reaction_residual,
        compound_pair_count: 0,
        compound_dwell_mean_ticks: 0.0,
        compound_potential_sum: 0.0,
        compound_potential_mean: 0.0,
        annihilation_energy_this_tick: 0.0,
        matter_count: 0,
        anti_count: 0,
        chirality: 0.0,
        chirality_abs: 0.0,
        chirality_zero_crossings: 0,
        chirality_lock_ticks: 0,
        relay_collapse_count: 0,
        transient_relay_collapses: 0,
        terminal_capture_collapses: 0,
        relay_shell_overshoot_sum: 0.0,
        relay_shell_overshoot_mean: 0.0,
        relay_relaxation_ticks_sum: 0,
        relay_relaxation_ticks_mean: 0.0,
    }
}

fn shell_anchor_diagnostics(parcels: &[Parcel], config: &SimConfig) -> (usize, usize, f64) {
    let n = parcels.len();
    if n == 0 {
        return (0, 0, 0.0);
    }

    let neighborhood = (config.shell_interaction_range * 5.0).max(1e-9);
    let mut anchored_count = 0usize;
    let mut inside_out_count = 0usize;
    let mut anchored_gap_sum = 0.0_f64;

    for i in 0..n {
        let choke = match &parcels[i].choke {
            Some(c) => c,
            None => continue,
        };

        let mut sum = 0.0;
        let mut count = 0usize;
        for j in 0..n {
            if i == j {
                continue;
            }
            let d = parcels[i].dist_to(&parcels[j]);
            if d < neighborhood {
                sum += parcels[j].concentration.max(0.0);
                count += 1;
            }
        }
        let ambient = if count > 0 {
            sum / count as f64
        } else {
            parcels[i].concentration
        };

        let core = choke.equilibrium_concentration.max(1e-10);
        let gap = ambient - core;
        let shell_excess = parcels[i].squeeze_excess();

        // Anchored depression: shell coherent and ambient higher than core.
        if gap > 0.0 && shell_excess > 1e-10 && choke.coherence > 0.1 {
            anchored_count += 1;
            anchored_gap_sum += gap;
        }

        // Inside-out bubble: shell exists but ambient is not higher than core.
        if gap <= 0.0 && shell_excess > 1e-10 {
            inside_out_count += 1;
        }
    }

    let anchored_gap_mean = if anchored_count > 0 {
        anchored_gap_sum / anchored_count as f64
    } else {
        0.0
    };

    (anchored_count, inside_out_count, anchored_gap_mean)
}

fn void_core_diagnostics(parcels: &[Parcel], config: &SimConfig) -> (usize, usize, f64) {
    if parcels.is_empty() {
        return (0, 0, 0.0);
    }

    let support_range = (config.shell_interaction_range * 1.5).max(1e-9);
    let mut void_core_count = 0usize;
    let mut up_candidates = 0usize;
    let mut support_sum = 0.0_f64;

    for (i, p) in parcels.iter().enumerate() {
        // role=2 is reserved for cavitation-generated true void-core markers.
        if p.role != 2 {
            continue;
        }
        void_core_count += 1;

        let mut neigh_sum = 0.0_f64;
        let mut neigh_count = 0usize;
        for (j, q) in parcels.iter().enumerate() {
            if i == j {
                continue;
            }
            let d = p.dist_to(q);
            if d <= support_range {
                neigh_sum += q.concentration.max(0.0);
                neigh_count += 1;
            }
        }

        let support = if neigh_count > 0 {
            neigh_sum / neigh_count as f64
        } else {
            0.0
        };
        support_sum += support;

        // Up-quark candidate: true void core + surrounding shell support.
        if p.ste_amount <= config.void_threshold && support > config.equilibrium_concentration {
            up_candidates += 1;
        }
    }

    let support_mean = if void_core_count > 0 {
        support_sum / void_core_count as f64
    } else {
        0.0
    };

    (void_core_count, up_candidates, support_mean)
}

fn fractalization_diagnostics(parcels: &[Parcel], config: &SimConfig) -> (f64, f64, f64, usize) {
    let n = parcels.len();
    if n < 2 {
        return (0.0, 0.0, 0.0, 0);
    }

    let eps = config.softening.max(1e-10);
    let mut pair_count = 0usize;
    let mut index_sum = 0.0_f64;
    let mut flux_sum = 0.0_f64;
    let mut lock_sum = 0.0_f64;

    let mut cascade_break_count = 0usize;
    for p in parcels {
        let shell_ratio = p.squeeze_excess() / (p.shell_equilibrium + 1e-10);
        if p.is_choked() && shell_ratio > 1.5 && p.vorticity < p.spin.abs() {
            cascade_break_count += 1;
        }
    }

    for i in 0..n {
        for j in (i + 1)..n {
            let dx = parcels[j].x - parcels[i].x;
            let dy = parcels[j].y - parcels[i].y;
            let dz = parcels[j].z - parcels[i].z;
            let d = (dx * dx + dy * dy + dz * dz).sqrt().max(eps);

            let r_sum = (parcels[i].radius + parcels[j].radius).max(eps);
            let shell_range = config.shell_interaction_range.max(1e-9);
            let contact_start = r_sum + shell_range;
            let depth = if d < contact_start {
                ((contact_start - d) / shell_range).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let si = parcels[i].spin;
            let sj = parcels[j].spin;
            let abs_i = si.abs();
            let abs_j = sj.abs();
            let max_abs = abs_i.max(abs_j);
            let alignment = if max_abs > 1e-10 {
                si.signum() * sj.signum() * abs_i.min(abs_j) / max_abs
            } else {
                0.0
            };

            let (gain, flux, lock) = field::recursive_pull_gain(
                depth,
                alignment.max(0.0),
                (-alignment).max(0.0),
                config,
            );
            index_sum += gain;
            flux_sum += flux;
            lock_sum += lock;
            pair_count += 1;
        }
    }

    if pair_count == 0 {
        return (0.0, 0.0, 0.0, cascade_break_count);
    }

    (
        index_sum / pair_count as f64,
        flux_sum / pair_count as f64,
        lock_sum / pair_count as f64,
        cascade_break_count,
    )
}

/// Mean parcel slip relative to local neighborhood flow.
///
/// This gives a dissipative proxy that is separate from potential-driven
/// attraction: high slip implies stronger drag/backreaction loading.
fn mean_velocity_slip(parcels: &[Parcel], config: &SimConfig) -> f64 {
    let n = parcels.len();
    if n < 2 {
        return 0.0;
    }

    let neighbor_range = (config.shell_interaction_range * 5.0).max(1e-9);
    let mut slip_sum = 0.0_f64;

    for i in 0..n {
        let mut sx = 0.0;
        let mut sy = 0.0;
        let mut sz = 0.0;
        let mut count = 0usize;

        for j in 0..n {
            if i == j {
                continue;
            }
            let d = parcels[i].dist_to(&parcels[j]);
            if d < neighbor_range {
                sx += parcels[j].vx;
                sy += parcels[j].vy;
                sz += parcels[j].vz;
                count += 1;
            }
        }

        let (ax, ay, az) = if count > 0 {
            (
                sx / count as f64,
                sy / count as f64,
                sz / count as f64,
            )
        } else {
            (parcels[i].vx, parcels[i].vy, parcels[i].vz)
        };

        let dx = parcels[i].vx - ax;
        let dy = parcels[i].vy - ay;
        let dz = parcels[i].vz - az;
        slip_sum += (dx * dx + dy * dy + dz * dz).sqrt();
    }

    slip_sum / n as f64
}

/// Recompute force channels to audit action-reaction balance.
///
/// Returns:
/// - total_attractive_force: sum of pair attractive magnitudes
/// - total_pressure_force: retained for compatibility; pull-only branch keeps this at 0
/// - net_internal_force: vector sum over all internal pair forces
fn force_channel_diagnostics(parcels: &[Parcel], config: &SimConfig) -> (f64, f64, [f64; 3]) {
    let n = parcels.len();
    if n < 2 {
        return (0.0, 0.0, [0.0; 3]);
    }

    let eps = config.softening;
    let void_th = config.void_threshold;

    let concs: Vec<f64> = parcels.iter().map(|p| p.concentration.max(1e-10)).collect();
    let stes: Vec<f64> = parcels.iter().map(|p| p.ste_amount.max(1e-10)).collect();
    let radii: Vec<f64> = parcels.iter().map(|p| p.radius).collect();
    let spins: Vec<f64> = parcels.iter().map(|p| p.spin).collect();
    let mut total_attractive_force = 0.0;
    let total_pressure_force = 0.0;
    let mut net_internal_force = [0.0_f64; 3];

    for i in 0..n {
        for j in (i + 1)..n {
            if stes[i] <= void_th || stes[j] <= void_th {
                continue;
            }

            let dx = parcels[j].x - parcels[i].x;
            let dy = parcels[j].y - parcels[i].y;
            let dz = parcels[j].z - parcels[i].z;
            let d_raw = (dx * dx + dy * dy + dz * dz).sqrt();
            let d = d_raw.max(eps);
            let d2 = d * d;

            let r_sum = (radii[i] + radii[j]).max(eps);
            let overlap = (d / r_sum).min(1.0);

            let shell_contact = 1.0 - overlap;
            let si = spins[i];
            let sj = spins[j];
            let abs_i = si.abs();
            let abs_j = sj.abs();
            let max_abs = abs_i.max(abs_j);
            let alignment = if max_abs > 1e-10 {
                si.signum() * sj.signum() * abs_i.min(abs_j) / max_abs
            } else {
                0.0
            };
            let spin_factor = if shell_contact > 1e-6 {
                1.0 + alignment * shell_contact
            } else {
                1.0
            };

            let shell_range = config.shell_interaction_range.max(1e-9);
            let contact_start = r_sum + shell_range;
            let shell_depth = if d < contact_start {
                ((contact_start - d) / shell_range).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let shell_count = ((contact_start / shell_range) * 5.0).round().clamp(1.0, 64.0);
            let phase = shell_depth * shell_count;
            let _boundary_wave = 0.5 * (1.0 + (2.0 * std::f64::consts::PI * phase).cos());
            let inner_bias = shell_depth * shell_depth;
            let same_spin = alignment.max(0.0);
            let counter_spin = (-alignment).max(0.0);

            let outer_shell_pull = 1.0 + 0.25 * counter_spin * (1.0 - shell_depth);
            let merge_damping = (1.0 - 0.2 * same_spin * inner_bias).max(0.2);
            let (recursive_gain, _, _) = field::recursive_pull_gain(shell_depth, same_spin, counter_spin, config);
            let attractive_force = overlap
                * concs[i]
                * concs[j]
                * spin_factor
                * outer_shell_pull
                * merge_damping
                * recursive_gain
                / d2;

            let force = attractive_force;
            total_attractive_force += attractive_force.abs();

            let inv_d = if d_raw > 1e-15 { 1.0 / d_raw } else { 0.0 };
            let ux = dx * inv_d;
            let uy = dy * inv_d;
            let uz = dz * inv_d;

            // Internal pair force on i and j (equal/opposite by construction)
            let f_ix = force * ux;
            let f_iy = force * uy;
            let f_iz = force * uz;
            let f_jx = -f_ix;
            let f_jy = -f_iy;
            let f_jz = -f_iz;

            net_internal_force[0] += f_ix + f_jx;
            net_internal_force[1] += f_iy + f_jy;
            net_internal_force[2] += f_iz + f_jz;
        }
    }

    (total_attractive_force, total_pressure_force, net_internal_force)
}
