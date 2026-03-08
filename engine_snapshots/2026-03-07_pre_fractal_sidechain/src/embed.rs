use crate::config::SimConfig;
use crate::parcel::Parcel;

/// # Force-Directed Graph Embedding
///
/// Converts the relational bond graph into 2D/3D positions for rendering.
///
/// The renderer IS the model: attraction pulls nodes together, bonds
/// set rest lengths, and the layout emerges from the same forces that
/// drive the simulation.  This is not a separate coordinate system —
/// it is the simulation's bond topology made visible.

/// Embedded position for rendering.
#[derive(Debug, Clone, Copy)]
pub struct EmbedPos {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Default for EmbedPos {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

/// Compute a force-directed layout from the bond graph.
///
/// Each bond acts as a spring with rest length = bond.distance.
/// All parcels repel each other gently (to prevent overlap in the
/// visual embedding).
///
/// A universal gravitational sink pulls every parcel toward the
/// STE-weighted centroid — this ensures continuous infall without
/// requiring every parcel to be bonded to a central attractor.
/// The field naturally recycles energy outward via diffusion.
///
/// After force application, positions are re-centred and scaled so
/// the average inter-node distance stays ~1.0.  This keeps the
/// layout stable regardless of the absolute bond distance scale.
///
/// `positions` is mutated in-place.  Call once per frame.
pub fn layout_step(
    parcels: &[Parcel],
    positions: &mut [EmbedPos],
    config: &SimConfig,
    _repulsion: f64,
    spring_k: f64,
    damping: f64,
) {
    let n = parcels.len();
    if positions.len() != n || n == 0 {
        return;
    }

    let mut forces: Vec<[f64; 3]> = vec![[0.0; 3]; n];

    // ── Compute global average bond distance for normalisation ──
    let mut total_bond_dist = 0.0;
    let mut bond_count = 0usize;
    for parcel in parcels {
        for bond in &parcel.bonds {
            total_bond_dist += bond.distance;
            bond_count += 1;
        }
    }
    let avg_bond = if bond_count > 0 {
        total_bond_dist / bond_count as f64
    } else {
        1.0
    };

    // ── Spring forces along bonds ──
    // Rest length uses a blend: mostly normalised (for layout stability)
    // but with a fraction of the actual bond distance ratio so that
    // collapsing bonds (shorter distance) visibly pull nodes together
    // and expanding bonds push them apart.
    let blend = 0.5; // 0 = pure normalised, 1 = pure absolute ratio
    for (i, parcel) in parcels.iter().enumerate() {
        for bond in &parcel.bonds {
            let j = bond.peer;
            if j >= n {
                continue;
            }
            let dx = positions[j].x - positions[i].x;
            let dy = positions[j].y - positions[i].y;
            let dz = positions[j].z - positions[i].z;
            let dist = (dx * dx + dy * dy + dz * dz).sqrt().max(1e-6);

            let norm_rest = bond.distance / avg_bond; // normalised rest length
            // Shorter bonds → smaller rest → pull tighter in the layout.
            // The blend lets the absolute distance difference show through.
            let rest = (1.0 - blend) * 1.0 + blend * norm_rest;
            let displacement = dist - rest;
            let f = spring_k * displacement / dist;

            forces[i][0] += f * dx;
            forces[i][1] += f * dy;
            forces[i][2] += f * dz;
        }
    }

    // ── Per-parcel gravity wells (O(n²) — fine for ~400 parcels) ──
    // Every parcel attracts every other parcel.  "Mass" = STE concentration.
    // F = G · m_i · m_j / r²   (always attractive, as the ontology demands)
    //
    // No artificial repulsion.  Resistance to collapse emerges from:
    //   1. Bond springs pushing back when compressed below rest length
    //   2. Per-parcel viscous damping: dense regions = thick syrup = hard to move
    //   3. Choking + foam dynamics at extreme concentrations
    let gravity_g = 0.02; // residual attraction — weak vs bond/diffusion forces

    for i in 0..n {
        let m_i = parcels[i].concentration.max(0.001);
        for j in (i + 1)..n {
            let m_j = parcels[j].concentration.max(0.001);
            let dx = positions[j].x - positions[i].x;
            let dy = positions[j].y - positions[i].y;
            let dz = positions[j].z - positions[i].z;
            let dist_sq = (dx * dx + dy * dy + dz * dz).max(1e-4);
            let dist = dist_sq.sqrt();

            // Pure gravitational attraction — no repulsion, no overlap hack
            let f_grav = gravity_g * m_i * m_j / dist_sq;

            let fx = f_grav * dx / dist;
            let fy = f_grav * dy / dist;
            let fz = f_grav * dz / dist;

            forces[i][0] += fx;
            forces[i][1] += fy;
            forces[i][2] += fz;
            forces[j][0] -= fx;
            forces[j][1] -= fy;
            forces[j][2] -= fz;
        }
    }

    // No artificial containment — the tension wells ARE the containment.
    // Springs pull along bonds; re-centering below keeps the view stable.

    // ── Apply forces with per-parcel viscous damping ──
    // Movement = force × damping / local_density.
    // Dense regions are thick syrup — huge forces produce tiny
    // displacements.  Dilute regions are thin — small forces
    // still move you.  This is why the core stays stable even
    // under enormous gravitational pull: it's swimming in molasses.
    for i in 0..n {
        let density = parcels[i].concentration.max(0.01);
        let local_damping = damping / density; // thick field → small step
        // Cap maximum displacement to prevent blowup from extreme forces
        let max_step = 0.5;
        let dx = (forces[i][0] * local_damping).clamp(-max_step, max_step);
        let dy = (forces[i][1] * local_damping).clamp(-max_step, max_step);
        let dz = (forces[i][2] * local_damping).clamp(-max_step, max_step);
        positions[i].x += dx;
        positions[i].y += dy;
        positions[i].z += dz;
    }

    // ── Re-centre around origin ──
    let mut cx = 0.0;
    let mut cy = 0.0;
    let mut cz = 0.0;
    for p in positions.iter() {
        cx += p.x;
        cy += p.y;
        cz += p.z;
    }
    cx /= n as f64;
    cy /= n as f64;
    cz /= n as f64;
    for p in positions.iter_mut() {
        p.x -= cx;
        p.y -= cy;
        p.z -= cz;
    }

    // ── Remove net angular momentum (prevents layout spin) ──
    // Compute net angular velocity around all 3 axes from the
    // velocity-like displacements we just applied.
    // L_axis = Σ (r × v)_axis,  I_axis = Σ |r_perp|²,  ω = L / I
    // Then subtract the rigid-body rotation from each position.
    {
        // Accumulate angular momentum and moment of inertia for each axis
        let mut lx = 0.0_f64; // angular momentum around x-axis
        let mut ly = 0.0_f64; // angular momentum around y-axis
        let mut lz = 0.0_f64; // angular momentum around z-axis
        let mut ix = 0.0_f64; // moment of inertia around x-axis
        let mut iy = 0.0_f64; // moment of inertia around y-axis
        let mut iz = 0.0_f64; // moment of inertia around z-axis
        for i in 0..n {
            let rx = positions[i].x;
            let ry = positions[i].y;
            let rz = positions[i].z;
            let density = parcels[i].concentration.max(0.01);
            let local_damping = damping / density;
            let vx = forces[i][0] * local_damping;
            let vy = forces[i][1] * local_damping;
            let vz = forces[i][2] * local_damping;
            // Cross product r × v
            lx += ry * vz - rz * vy;
            ly += rz * vx - rx * vz;
            lz += rx * vy - ry * vx;
            // Moments of inertia (perpendicular distance squared)
            ix += ry * ry + rz * rz;
            iy += rx * rx + rz * rz;
            iz += rx * rx + ry * ry;
        }
        // Compute angular velocities
        let omega_x = if ix > 1e-10 { lx / ix } else { 0.0 };
        let omega_y = if iy > 1e-10 { ly / iy } else { 0.0 };
        let omega_z = if iz > 1e-10 { lz / iz } else { 0.0 };
        // Subtract rigid-body rotation: v_rot = ω × r
        for p in positions.iter_mut() {
            let cx = omega_y * p.z - omega_z * p.y;
            let cy = omega_z * p.x - omega_x * p.z;
            let cz = omega_x * p.y - omega_y * p.x;
            p.x -= cx;
            p.y -= cy;
            p.z -= cz;
        }
    }
}

/// Initialise positions randomly in a unit cube.
pub fn init_random(n: usize, seed: u64) -> Vec<EmbedPos> {
    // Simple deterministic pseudo-random for reproducibility.
    let mut state = seed;
    let mut positions = Vec::with_capacity(n);
    for _ in 0..n {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let x = (state as f64) / (u64::MAX as f64);
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let y = (state as f64) / (u64::MAX as f64);
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let z = (state as f64) / (u64::MAX as f64);
        positions.push(EmbedPos { x, y, z });
    }
    positions
}
