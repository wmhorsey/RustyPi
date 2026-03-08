use crate::config::SimConfig;
use crate::parcel::Parcel;

/// # Field Interactions — All-Pairs, Position-Based
///
/// Every parcel interacts with every other parcel through the
/// ever-present STE field.  No bonds, no graph, no topology.
///
/// ## Core identity: STE IS attraction
///
///   Each parcel is a sphere of STE: radius r = scale × STE^(1/3)
///   Surface area SA = 4π r²
///   Concentration (attraction density) c = STE / SA ∝ STE^(1/3)
///
///   STE and attraction are the SAME THING measured differently.
///
/// ## Force model
///
///   F(i,j) = STE_i × STE_j / d²    (always attractive, inverse-square)
///   mass = STE                       (the stuff IS its own inertia)
///   softening = r_i + r_j            (surfaces can't overlap)
///
/// ## Diffusion (heat conduction)
///
///   flux(j→i) = κ · (STE_j - STE_i) / (d · (1 + η_avg))
///   Viscosity-damped Fourier's law on STE amounts directly.

/// Number of nearest neighbours for vorticity estimation.
const K_NEAREST: usize = 8;
const SHELLS_PER_RANGE: f64 = 5.0;

fn spin_alignment(si: f64, sj: f64) -> f64 {
    let abs_i = si.abs();
    let abs_j = sj.abs();
    let max_abs = abs_i.max(abs_j);
    if max_abs <= 1e-10 {
        0.0
    } else {
        si.signum() * sj.signum() * abs_i.min(abs_j) / max_abs
    }
}

/// Layered shell profile for pair interactions.
///
/// Returns:
/// - depth: normalized penetration from ambient edge toward core [0,1]
/// - boundary_wave: periodic boundary emphasis for shell-step resistance [0,1]
/// - inner_bias: stronger barrier near deep/core zones [0,1]
fn layered_shell_profile(d: f64, r_sum: f64, shell_range: f64) -> (f64, f64, f64) {
    let shell_range = shell_range.max(1e-9);
    let contact_start = r_sum + shell_range;
    if d >= contact_start {
        return (0.0, 0.0, 0.0);
    }

    let depth = ((contact_start - d) / shell_range).clamp(0.0, 1.0);

    // Shell density is shared across particles: fixed layer density in space,
    // not a hard-coded shell count tied to one specific parcel size.
    let shell_count = ((contact_start / shell_range) * SHELLS_PER_RANGE)
        .round()
        .clamp(1.0, 64.0);
    let phase = depth * shell_count;
    let boundary_wave = 0.5 * (1.0 + (2.0 * std::f64::consts::PI * phase).cos());
    let inner_bias = depth * depth;

    (depth, boundary_wave, inner_bias)
}

/// Recursive pull gain for field-within-field coupling.
///
/// Returns:
/// - gain: multiplicative pull gain for pair attraction
/// - cross_scale_flux: proxy for inter-level transfer activity [0,1]
/// - phase_lock: proxy for shell phase alignment across levels [0,1]
pub fn recursive_pull_gain(
    depth: f64,
    same_spin: f64,
    counter_spin: f64,
    config: &SimConfig,
) -> (f64, f64, f64) {
    let levels = config.fractal_recursion_levels.max(1);
    let decay = config.fractal_level_decay.clamp(0.05, 0.98);
    let coupling = config.fractal_shell_coupling.clamp(0.0, 1.0);

    let mut gain_sum = 0.0_f64;
    let mut flux_sum = 0.0_f64;
    let mut lock_sum = 0.0_f64;
    let mut norm = 0.0_f64;

    for level in 0..levels {
        let weight = decay.powi(level as i32);
        let depth_power = 1.0 / (1.0 + level as f64);
        let local_depth = depth.clamp(0.0, 1.0).powf(depth_power);
        let freq = (1usize << level) as f64;
        let shell_wave = 0.5 * (1.0 + (2.0 * std::f64::consts::PI * local_depth * freq).cos());
        let inward_gradient = (1.0 - local_depth).powf(1.0 + 0.2 * level as f64);

        // Same-spin reinforces coherent pull; counter-spin keeps edge mobility.
        let spin_mesh = 1.0
            + 0.25 * same_spin / (1.0 + level as f64)
            + 0.15 * counter_spin * inward_gradient;
        let layer_gain = (1.0 - coupling) * inward_gradient + coupling * shell_wave;

        gain_sum += weight * layer_gain * spin_mesh.max(0.1);
        flux_sum += weight * inward_gradient * shell_wave;
        lock_sum += weight * (1.0 - (shell_wave - 0.5).abs() * 2.0).clamp(0.0, 1.0);
        norm += weight;
    }

    let norm = norm.max(1e-12);
    let gain = (gain_sum / norm).max(config.fractal_pull_floor.max(0.0));
    let cross_scale_flux = (flux_sum / norm).clamp(0.0, 1.0);
    let phase_lock = (lock_sum / norm).clamp(0.0, 1.0);
    (gain, cross_scale_flux, phase_lock)
}

/// Update radius and concentration for all parcels from their current STE.
///
/// r = scale × STE^(1/3), c = STE / (4π r²).  No spatial estimation needed —
/// STE IS the attraction, locked together through surface area.
pub fn estimate_concentrations(parcels: &mut [Parcel], config: &SimConfig) {
    let scale = config.parcel_radius_scale;
    for p in parcels.iter_mut() {
        p.update_radius_and_concentration(scale);
        // Shell equilibrium IS the concentration — when the shell
        // matches the local field, it's in equilibrium.
        p.shell_equilibrium = p.concentration;
        if p.shell_level < p.shell_equilibrium {
            p.shell_level = p.shell_equilibrium;
        }
    }
}

/// Estimate vorticity from the curl of the local velocity field.
///
/// For each parcel, compute the curl of the velocity field at its
/// position using its K nearest neighbours.  High curl = the
/// neighbourhood is rotating rather than flowing uniformly.
pub fn estimate_vorticities(parcels: &mut [Parcel], config: &SimConfig) {
    let n = parcels.len();
    if n < 3 { return; }

    // Snapshot velocities
    let vels: Vec<[f64; 3]> = parcels.iter()
        .map(|p| [p.vx, p.vy, p.vz])
        .collect();
    let pos: Vec<[f64; 3]> = parcels.iter()
        .map(|p| [p.x, p.y, p.z])
        .collect();

    for i in 0..n {
        // Find K nearest
        let mut dists: Vec<(usize, f64)> = (0..n)
            .filter(|&j| j != i)
            .map(|j| {
                let dx = pos[j][0] - pos[i][0];
                let dy = pos[j][1] - pos[i][1];
                let dz = pos[j][2] - pos[i][2];
                (j, (dx * dx + dy * dy + dz * dz).sqrt())
            })
            .collect();
        dists.sort_by(|a, b| a.1.total_cmp(&b.1));

        let k = K_NEAREST.min(dists.len());
        if k == 0 {
            parcels[i].vorticity = 0.0;
            continue;
        }

        // Estimate curl: ω ≈ Σ (r × Δv) / (|r|² k)
        // where r = position relative to i, Δv = velocity relative to i
        let mut curl = [0.0_f64; 3];
        for &(j, d) in dists.iter().take(k) {
            if d < 1e-10 { continue; }
            let rx = pos[j][0] - pos[i][0];
            let ry = pos[j][1] - pos[i][1];
            let rz = pos[j][2] - pos[i][2];
            let dvx = vels[j][0] - vels[i][0];
            let dvy = vels[j][1] - vels[i][1];
            let dvz = vels[j][2] - vels[i][2];
            // r × Δv
            let cx = ry * dvz - rz * dvy;
            let cy = rz * dvx - rx * dvz;
            let cz = rx * dvy - ry * dvx;
            let w = 1.0 / (d * d); // weight by proximity
            curl[0] += cx * w;
            curl[1] += cy * w;
            curl[2] += cz * w;
        }
        let curl_mag = (curl[0] * curl[0] + curl[1] * curl[1] + curl[2] * curl[2]).sqrt()
            / k as f64;
        parcels[i].vorticity = curl_mag;

        // Spin is angular momentum — CONSERVED by default.
        // Only changes via viscous torque from the surrounding field.
        //
        // Superfluid (low concentration) → viscosity ≈ 0 → spin locked.
        // Normal fluid (high concentration) → viscosity couples spin to local curl.
        //
        // torque = (local_curl - my_spin) × viscosity
        // This is viscous drag on rotation, not a magic decay.
        let signed_vort = curl[2] / k as f64;
        let visc = config.viscosity_base
            * parcels[i].concentration.powf(config.viscosity_exponent);
        let torque = (signed_vort - parcels[i].spin) * visc;
        parcels[i].spin += torque * config.dt;
    }
}

/// Compute local saturation and pressure potential for each parcel.
///
/// This is the thermodynamic closure used by the force model:
///
///   saturation S_i = C_i / <C_neighbourhood>
///   permeability K_i = 1 / (1 + S_i)
///   pressure    P_i = C_i * (1 - K_i)
///
/// All terms are relative/local.  No absolute phase thresholds.
pub fn saturation_pressure_state(parcels: &[Parcel], config: &SimConfig) -> (Vec<f64>, Vec<f64>) {
    let n = parcels.len();
    if n == 0 {
        return (Vec::new(), Vec::new());
    }

    let concs: Vec<f64> = parcels.iter().map(|p| p.concentration.max(1e-10)).collect();
    let mut local_avg = vec![0.0_f64; n];
    let range2 = config.max_bond_distance * config.max_bond_distance;

    for i in 0..n {
        let mut sum = 0.0;
        let mut count = 0usize;
        for j in 0..n {
            if i == j { continue; }
            let dx = parcels[j].x - parcels[i].x;
            let dy = parcels[j].y - parcels[i].y;
            let dz = parcels[j].z - parcels[i].z;
            let d2 = dx * dx + dy * dy + dz * dz;
            if d2 <= range2 {
                sum += concs[j];
                count += 1;
            }
        }
        local_avg[i] = if count > 0 { sum / count as f64 } else { concs[i] };
    }

    let mut saturations = vec![0.0_f64; n];
    let mut pressures = vec![0.0_f64; n];
    for i in 0..n {
        let s = concs[i] / local_avg[i].max(1e-10);
        let k = 1.0 / (1.0 + s);
        saturations[i] = s;
        pressures[i] = concs[i] * (1.0 - k);
    }

    (saturations, pressures)
}

/// Apply all-pairs attraction forces.
///
/// Force uses concentration (STE per surface area), not raw STE.
/// This means bigger parcels self-cap: more STE → bigger sphere →
/// more SA → concentration grows only as STE^(1/3).
/// Two 4-STE parcels nearby don't superpose to 8 — each radiates
/// c ∝ 4^(1/3) ≈ 1.59, naturally capping combined outward pull.
///
///   F(i,j) = overlap × c_i × c_j × spin_factor × recursive_pull / d²
///   mass   = STE  (the stuff IS its own inertia)
///
/// Parcels are hot-spots (peaks) in the continuous STE field.
/// They want to merge into hotter spots — until they can't get hotter.
///
/// The key insight: there is field BETWEEN the parcels.  When two
/// peaks are far apart, the gradient between them is clear and the
/// pull is strong (inverse-square).  As they approach and their
/// radii overlap, they're merging into one peak — the internal
/// gradient flattens and the force drops to zero.
///
///   overlap = clamp(d / (r_i + r_j), 0, 1)
///   - d > r_i + r_j: fully separated, overlap = 1, full force
///   - d = r_i + r_j: just touching, overlap = 1, peak attraction
///   - d < r_i + r_j: merging, overlap < 1, gradient fading
///   - d → 0: same peak, overlap → 0, no internal gradient
///
/// **Spin coupling** (subatomic shell interaction):
///
/// At these scales parcels are barely mass — they're superfluid
/// STE whose shells are driven by environmental flows.  When two
/// shells are close enough to overlap, their rotational states
/// affect each other:
///
///   spin_factor = 1 + alignment × (1 - overlap)
///   alignment  = sign(spin_i × spin_j) × min(|spin_i|, |spin_j|)
///                / max(|spin_i|, |spin_j|)
///
///   - Co-rotating (same sign): alignment > 0 → enhanced coupling
///   - Counter-rotating (opposite sign): alignment < 0 → reduced/repulsive
///   - Only matters when shells overlap (1 - overlap > 0)
///   - At distance, spin irrelevant (overlap = 1 → factor = 1)
///
/// This side-chain branch uses pull-only recursion: nested shell levels
/// modulate how much of that pull is transmitted across scale.
///
/// Updates velocities directly.
pub fn apply_forces(parcels: &mut [Parcel], config: &SimConfig) {
    let n = parcels.len();
    let dt = config.dt;
    let eps = config.softening;

    // Snapshot concentration (STE/SA), STE (mass), radii, and spin
    let concs: Vec<f64> = parcels.iter().map(|p| p.concentration.max(1e-10)).collect();
    let stes: Vec<f64> = parcels.iter().map(|p| p.ste_amount.max(1e-10)).collect();
    let radii: Vec<f64> = parcels.iter().map(|p| p.radius).collect();
    let spins: Vec<f64> = parcels.iter().map(|p| p.spin).collect();

    // Accumulate accelerations
    let mut accel: Vec<[f64; 3]> = vec![[0.0; 3]; n];

    let void_th = config.void_threshold;

    for i in 0..n {
        for j in (i + 1)..n {
            // No STE = no medium = no force carrier.
            if stes[i] <= void_th || stes[j] <= void_th { continue; }

            let dx = parcels[j].x - parcels[i].x;
            let dy = parcels[j].y - parcels[i].y;
            let dz = parcels[j].z - parcels[i].z;
            let d_raw = (dx * dx + dy * dy + dz * dz).sqrt();
            let d = d_raw.max(eps);
            let d2 = d * d;

            // Overlap: when peaks merge (d < r_i + r_j), the gradient
            // between them flattens.  Force scales linearly to zero.
            let r_sum = (radii[i] + radii[j]).max(eps);
            let overlap = (d / r_sum).min(1.0);   // 1 = separated, 0 = merged

            // Spin coupling: shells in contact feel each other's rotation.
            // Co-rotating → mesh → enhanced attraction.
            // Counter-rotating → grind → reduced/repulsive.
            // Only matters in overlap zone (1 - overlap > 0).
            let shell_contact = 1.0 - overlap;  // 0 at distance, 1 when merged
            let alignment = spin_alignment(spins[i], spins[j]);
            let spin_factor = if shell_contact > 1e-6 {
                1.0 + alignment * shell_contact
            } else {
                1.0
            };

            // Attractive channel: overlap × c_i × c_j × spin_factor / d²
            // Self-capping: c ∝ STE^(1/3), so doubling STE only
            // increases outward pull by 2^(1/3) ≈ 1.26×
            // Gradient-driven: no gradient (overlap=0) → no force
            // Spin-modulated: shells that mesh pull harder
            let (shell_depth, _boundary_wave, inner_bias) =
                layered_shell_profile(d, r_sum, config.shell_interaction_range);
            let same_spin = alignment.max(0.0);
            let counter_spin = (-alignment).max(0.0);

            // Outer shells can still pull, but deep same-spin approach is damped.
            let outer_shell_pull = 1.0 + 0.25 * counter_spin * (1.0 - shell_depth);
            let merge_damping = 1.0 - 0.2 * same_spin * inner_bias;
            let (recursive_gain, _cross_scale_flux, _phase_lock) =
                recursive_pull_gain(shell_depth, same_spin, counter_spin, config);
            let attractive_force = overlap
                * concs[i]
                * concs[j]
                * spin_factor
                * outer_shell_pull
                * merge_damping.max(0.2)
                * recursive_gain
                / d2;

            // Side-chain rule: pull-only dynamics. Resistance appears as reduced
            // recursive gain, not a sign-flipped repulsive channel.
            let force = attractive_force;

            // Direction: unit vector from i to j
            let inv_d = if d_raw > 1e-15 { 1.0 / d_raw } else { 0.0 };
            let ux = dx * inv_d;
            let uy = dy * inv_d;
            let uz = dz * inv_d;

            // Acceleration = force / mass, mass = STE
            let ai = force / stes[i];
            let aj = force / stes[j];

            accel[i][0] += ai * ux;
            accel[i][1] += ai * uy;
            accel[i][2] += ai * uz;
            accel[j][0] -= aj * ux;
            accel[j][1] -= aj * uy;
            accel[j][2] -= aj * uz;
        }
    }

    // Apply accelerations to velocities
    for i in 0..n {
        parcels[i].vx += accel[i][0] * dt;
        parcels[i].vy += accel[i][1] * dt;
        parcels[i].vz += accel[i][2] * dt;
    }
}

/// Apply all-pairs STE equilibration through the field.
///
/// This is NOT heat conduction — conducting STE and *showing* heat
/// are different things.  What we observe as heat is chokes:
/// micro-vortex depressions that emerge where a deeper STE well
/// soaks the local field, creating chaotic motion.  This function
/// just moves the substrate toward equilibrium.
///
/// Normal parcels equilibrate toward the local average — never to zero.
/// Zero is structurally special (voids), not a diffusion target.
///
/// **STE is the medium.**  If either parcel is void (STE ≈ 0),
/// no equilibration occurs — there's nothing there to conduct through.
/// Voids are structurally zero; they don't receive or transmit.
/// This is why voids buffer peaks: the equilibration path hits a wall.
///
/// The conduction rate scales by the minimum STE of the pair —
/// the weakest link in the medium.  Thin STE = weak conductor.
///
/// flux = κ · min(STE_i, STE_j) · (STE_j - STE_i) / (d · (1 + η_avg))
pub fn apply_diffusion(parcels: &mut [Parcel], config: &SimConfig) {
    let n = parcels.len();
    let dt = config.dt;
    let kappa = config.diffusivity;
    if kappa <= 0.0 || n < 2 { return; }

    let stes: Vec<f64> = parcels.iter().map(|p| p.ste_amount).collect();
    let concs: Vec<f64> = parcels.iter().map(|p| p.concentration).collect();
    let mut delta: Vec<f64> = vec![0.0; n];

    let range = config.max_bond_distance;
    let void_th = config.void_threshold;

    for i in 0..n {
        for j in (i + 1)..n {
            // STE is the medium.  No STE = no conduction.
            if stes[i] <= void_th || stes[j] <= void_th { continue; }

            let dx = parcels[j].x - parcels[i].x;
            let dy = parcels[j].y - parcels[i].y;
            let dz = parcels[j].z - parcels[i].z;
            let d2 = dx * dx + dy * dy + dz * dz;
            if d2 > range * range { continue; }
            let d = d2.sqrt().max(config.softening);

            // STE difference drives the flow toward equilibrium (ambient)
            let d_ste = stes[j] - stes[i];

            // Conductivity scales by the weakest link — min STE of pair
            let medium_strength = stes[i].min(stes[j]);

            // Viscosity-damped Fourier's law
            let eta_i = config.viscosity_base * concs[i].max(0.0).powf(config.viscosity_exponent);
            let eta_j = config.viscosity_base * concs[j].max(0.0).powf(config.viscosity_exponent);
            let eta_avg = 0.5 * (eta_i + eta_j);
            let flux = kappa * medium_strength * d_ste / (d * (1.0 + eta_avg));

            delta[i] += flux * dt;
            delta[j] -= flux * dt;
        }
    }

    // Apply transfers.  Floor at 0 — STE cannot go negative.
    for i in 0..n {
        parcels[i].ste_amount = (stes[i] + delta[i]).max(0.0);
    }
}

/// Apply viscous velocity damping.
///
/// Dense regions resist motion.  η = viscosity_base × c^exponent.
/// Damping acts on velocity magnitude proportionally.
pub fn apply_viscosity(parcels: &mut [Parcel], config: &SimConfig) {
    let dt = config.dt;
    for p in parcels.iter_mut() {
        let eta = config.viscosity_base * p.concentration.max(0.0).powf(config.viscosity_exponent);
        // Damping factor: 1.0 in void, approaches 0 in dense regions
        let damp = 1.0 / (1.0 + eta * dt * 10.0);
        p.vx *= damp;
        p.vy *= damp;
        p.vz *= damp;
    }
}

/// Integrate positions from velocities with CFL clamp.
///
/// No parcel moves more than half the distance to its nearest
/// neighbour per tick — the discrete speed-of-light limit.
pub fn integrate_positions(parcels: &mut [Parcel], config: &SimConfig) {
    let n = parcels.len();
    let dt = config.dt;

    // Find nearest-neighbour distance for each parcel (for CFL)
    let positions: Vec<[f64; 3]> = parcels.iter()
        .map(|p| [p.x, p.y, p.z])
        .collect();

    for i in 0..n {
        // Find nearest neighbour distance
        let mut min_d = f64::MAX;
        for j in 0..n {
            if j == i { continue; }
            let dx = positions[j][0] - positions[i][0];
            let dy = positions[j][1] - positions[i][1];
            let dz = positions[j][2] - positions[i][2];
            let d = (dx * dx + dy * dy + dz * dz).sqrt();
            if d < min_d { min_d = d; }
        }

        // CFL: max displacement = 0.5 × nearest neighbour distance
        let v_max = if min_d > config.softening {
            0.5 * min_d / dt
        } else {
            1.0 / dt // fallback
        };

        let speed = parcels[i].speed();
        if speed > v_max {
            let scale = v_max / speed;
            parcels[i].vx *= scale;
            parcels[i].vy *= scale;
            parcels[i].vz *= scale;
        }

        parcels[i].x += parcels[i].vx * dt;
        parcels[i].y += parcels[i].vy * dt;
        parcels[i].z += parcels[i].vz * dt;
    }
}

/// Compute tension between two parcels (for rendering).
/// Tension = c_i × c_j / d² — matches force law.
pub fn tension(c_i: f64, c_j: f64, distance: f64, config: &SimConfig) -> f64 {
    let eps = config.softening;
    let d2 = (distance * distance).max(eps * eps);
    c_i * c_j / d2
}

/// Find K nearest neighbour pairs for rendering (visual bonds).
///
/// Returns (pairs, tensions): flat arrays [i0, j0, i1, j1, ...] and
/// per-pair tension values.
pub fn visual_bonds(parcels: &[Parcel], config: &SimConfig, k: usize) -> (Vec<usize>, Vec<f64>) {
    let n = parcels.len();
    let mut pairs: Vec<usize> = Vec::new();
    let mut tensions: Vec<f64> = Vec::new();

    // For each parcel, find its K nearest and emit bonds (i < j only)
    let mut seen = std::collections::HashSet::new();

    for i in 0..n {
        let mut dists: Vec<(usize, f64)> = (0..n)
            .filter(|&j| j != i)
            .map(|j| {
                let dx = parcels[j].x - parcels[i].x;
                let dy = parcels[j].y - parcels[i].y;
                let dz = parcels[j].z - parcels[i].z;
                (j, (dx * dx + dy * dy + dz * dz).sqrt())
            })
            .collect();
        dists.sort_by(|a, b| a.1.total_cmp(&b.1));

        for &(j, d) in dists.iter().take(k) {
            let (lo, hi) = if i < j { (i, j) } else { (j, i) };
            if seen.insert((lo, hi)) {
                pairs.push(lo);
                pairs.push(hi);
                let t = tension(
                    parcels[lo].concentration,
                    parcels[hi].concentration,
                    d,
                    config,
                );
                tensions.push(t);
            }
        }
    }

    (pairs, tensions)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_parcels(ste_a: f64, ste_b: f64, dist: f64) -> (Vec<Parcel>, SimConfig) {
        let mut a = Parcel::new(0, ste_a);
        let mut b = Parcel::new(1, ste_b);
        a.x = 0.0;
        b.x = dist;
        let mut config = SimConfig::default();
        config.diffusivity = 0.5;
        config.dt = 0.01;
        (vec![a, b], config)
    }

    #[test]
    fn forces_are_attractive() {
        let (mut parcels, config) = two_parcels(5.0, 5.0, 2.0);
        apply_forces(&mut parcels, &config);
        // Parcel 0 should be pulled toward parcel 1 (positive x direction)
        assert!(parcels[0].vx > 0.0, "parcel 0 should be pulled toward parcel 1");
        assert!(parcels[1].vx < 0.0, "parcel 1 should be pulled toward parcel 0");
    }

    #[test]
    fn diffusion_flows_hot_to_cold() {
        let (mut parcels, config) = two_parcels(10.0, 1.0, 1.0);
        let before_a = parcels[0].ste_amount;
        apply_diffusion(&mut parcels, &config);
        assert!(parcels[0].ste_amount < before_a, "hot parcel should lose STE");
    }

    #[test]
    fn diffusion_conserves_ste() {
        let (mut parcels, config) = two_parcels(10.0, 1.0, 1.0);
        let total_before: f64 = parcels.iter().map(|p| p.ste_amount).sum();
        apply_diffusion(&mut parcels, &config);
        let total_after: f64 = parcels.iter().map(|p| p.ste_amount).sum();
        assert!(
            (total_after - total_before).abs() < 1e-10,
            "STE must be conserved: before={total_before}, after={total_after}"
        );
    }

    #[test]
    fn viscosity_damps_velocity() {
        let mut p = Parcel::new(0, 1.0);
        p.vx = 10.0;
        p.concentration = 5.0;
        let config = SimConfig::default();
        let mut parcels = vec![p];
        apply_viscosity(&mut parcels, &config);
        assert!(parcels[0].vx < 10.0, "viscosity should damp velocity");
        assert!(parcels[0].vx > 0.0, "velocity should stay positive");
    }
}
