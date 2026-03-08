use crate::config::SimConfig;
use crate::parcel::{ChokePhase, Parcel};

/// # Foam Dynamics — Bubble Detachment & Annihilation
///
/// The STE field around a dense structure behaves like a pot of soup:
///
/// 1. Energy flows inward (attraction / tension wells).
/// 2. At the dense shell, flows collide and choke — pinching off
///    micro-vortex bubbles (matter/antimatter pairs).
/// 3. These bubbles detach and float outward.
/// 4. Opposite-spin bubbles that meet annihilate — returning
///    their STE to the field.
///
/// No bonds needed — proximity is Euclidean distance.

/// Events produced by foam dynamics for the renderer.
#[derive(Debug, Clone)]
pub enum FoamEvent {
    Spawn { parent: usize, child: usize },
    Annihilation { a: usize, b: usize, energy: f64 },
}

/// Scan for chokes ready to detach and spawn daughter bubbles.
pub fn find_detachments(parcels: &[Parcel], config: &SimConfig) -> Vec<(usize, f64, f64)> {
    let mut detachments = Vec::new();

    for (i, parcel) in parcels.iter().enumerate() {
        let choke = match &parcel.choke {
            Some(c) => c,
            None => continue,
        };

        if !matches!(choke.phase, ChokePhase::Drift) { continue; }
        if choke.coherence < config.foam_spawn_coherence { continue; }

        let donate = parcel.ste_amount * config.foam_spawn_fraction;
        if donate < config.foam_spawn_min_ste { continue; }
        if parcels.len() >= config.max_parcels { continue; }

        detachments.push((i, donate, choke.spin));
    }

    detachments
}

/// Create a daughter parcel near the parent's position.
pub fn spawn_bubble(
    parent: &mut Parcel,
    ste: f64,
    spin: f64,
    new_id: u64,
) -> Parcel {
    parent.ste_amount -= ste;
    parent.choke = None;

    let mut daughter = Parcel::new(new_id, ste);
    daughter.spin = spin;

    // Place daughter near parent with a small offset
    let seed = new_id as f64 * 2.654;
    let seed2 = new_id as f64 * 1.337;
    let offset = 0.8;
    daughter.x = parent.x + seed.sin() * offset;
    daughter.y = parent.y + seed.cos() * offset;
    daughter.z = parent.z + seed2.sin() * offset;

    // Inherit a small outward velocity
    let dx = daughter.x - parent.x;
    let dy = daughter.y - parent.y;
    let dz = daughter.z - parent.z;
    let d = (dx * dx + dy * dy + dz * dz).sqrt().max(1e-10);
    let drift_speed = 0.1;
    daughter.vx = dx / d * drift_speed;
    daughter.vy = dy / d * drift_speed;
    daughter.vz = dz / d * drift_speed;

    daughter
}

/// Find annihilation candidates: opposite-spin choked parcels within range.
pub fn find_annihilations(parcels: &[Parcel], config: &SimConfig) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    let mut consumed = vec![false; parcels.len()];
    let range = config.shell_interaction_range * config.foam_annihilation_range;

    let choked: Vec<usize> = parcels.iter().enumerate()
        .filter(|(_, p)| p.choke.is_some())
        .map(|(i, _)| i)
        .collect();

    for ai in 0..choked.len() {
        let i = choked[ai];
        if consumed[i] { continue; }
        let ci = parcels[i].choke.as_ref().unwrap();

        for bi in (ai + 1)..choked.len() {
            let j = choked[bi];
            if consumed[j] { continue; }
            let cj = parcels[j].choke.as_ref().unwrap();

            if ci.spin * cj.spin >= 0.0 { continue; }

            let d = parcels[i].dist_to(&parcels[j]);
            if d > range { continue; }

            pairs.push((i, j));
            consumed[i] = true;
            consumed[j] = true;
            break;
        }
    }

    pairs
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnnihilationClass {
    Heat,
    Light,
    Gamma,
    Cavitation,
}

#[derive(Debug, Clone)]
pub struct AnnihilationResult {
    pub a: usize,
    pub b: usize,
    pub energy: f64,
    pub class: AnnihilationClass,
    pub recipients: Vec<usize>,
    pub void_core_index: Option<usize>,
}

/// Execute annihilation: redistribute energy to nearby parcels.
pub fn annihilate(parcels: &mut [Parcel], a: usize, b: usize, config: &SimConfig) -> AnnihilationResult {
    let energy = parcels[a].ste_amount + parcels[b].ste_amount;

    let class = if energy >= config.annihilation_cavitation_threshold {
        AnnihilationClass::Cavitation
    } else if energy >= config.annihilation_gamma_threshold {
        AnnihilationClass::Gamma
    } else if energy >= config.annihilation_light_threshold {
        AnnihilationClass::Light
    } else {
        AnnihilationClass::Heat
    };

    // Find nearby parcels by Euclidean distance
    let range = config.shell_interaction_range * config.foam_annihilation_range * 2.0;
    let mid_x = (parcels[a].x + parcels[b].x) * 0.5;
    let mid_y = (parcels[a].y + parcels[b].y) * 0.5;
    let mid_z = (parcels[a].z + parcels[b].z) * 0.5;

    let mut recipients: Vec<usize> = Vec::new();
    for k in 0..parcels.len() {
        if k == a || k == b { continue; }
        let dx = parcels[k].x - mid_x;
        let dy = parcels[k].y - mid_y;
        let dz = parcels[k].z - mid_z;
        let d = (dx * dx + dy * dy + dz * dz).sqrt();
        if d < range {
            recipients.push(k);
        }
    }

    let heat_fraction = match class {
        AnnihilationClass::Heat       => 1.0,
        AnnihilationClass::Light      => 0.6,
        AnnihilationClass::Gamma      => 0.3,
        AnnihilationClass::Cavitation => 0.1,
    };
    let heat_energy = energy * heat_fraction;

    let ambient_ste = if !recipients.is_empty() {
        recipients.iter().map(|&r| parcels[r].ste_amount).sum::<f64>() / recipients.len() as f64
    } else {
        0.01
    };
    let recycle_ste = (ambient_ste * 0.1).min(heat_energy * 0.1);
    let distribute_energy = heat_energy - recycle_ste * 2.0;

    if !recipients.is_empty() && distribute_energy > 0.0 {
        let share = distribute_energy / recipients.len() as f64;
        for &r in &recipients {
            parcels[r].ste_amount += share;
        }
    }

    // Shockwave: push nearby parcels outward via velocity
    if matches!(class, AnnihilationClass::Gamma | AnnihilationClass::Cavitation) {
        let shock_strength = energy * 0.5;
        for &r in &recipients {
            let dx = parcels[r].x - mid_x;
            let dy = parcels[r].y - mid_y;
            let dz = parcels[r].z - mid_z;
            let d = (dx * dx + dy * dy + dz * dz).sqrt().max(0.01);
            let impulse = shock_strength * 0.1 / d;
            parcels[r].vx += dx / d * impulse;
            parcels[r].vy += dy / d * impulse;
            parcels[r].vz += dz / d * impulse;
        }
    }

    // Cavitation funnel: before rebound, local flows converge toward a single
    // collapse point, carving a steep depression in the substrate.
    if class == AnnihilationClass::Cavitation {
        for &r in &recipients {
            let dx = parcels[r].x - mid_x;
            let dy = parcels[r].y - mid_y;
            let dz = parcels[r].z - mid_z;
            let d = (dx * dx + dy * dy + dz * dz).sqrt().max(0.01);
            let pull = (1.0 - (d / range).min(1.0)).max(0.0);
            let funnel = energy * 0.12 * pull / d;
            parcels[r].vx -= dx / d * funnel;
            parcels[r].vy -= dy / d * funnel;
            parcels[r].vz -= dz / d * funnel;
        }
    }

    let mut void_core_index = None;
    if class == AnnihilationClass::Cavitation {
        // True core void marker: exact zero STE at the collapse center.
        parcels[a].x = mid_x;
        parcels[a].y = mid_y;
        parcels[a].z = mid_z;
        parcels[a].vx = 0.0;
        parcels[a].vy = 0.0;
        parcels[a].vz = 0.0;
        parcels[a].ste_amount = 0.0;
        parcels[a].spin = 0.0;
        parcels[a].choke = None;
        parcels[a].role = 2; // U quark candidate (void-core depression)

        // Partner becomes shell-collar residue around the void core.
        parcels[b].spin = 0.0;
        parcels[b].choke = None;
        parcels[b].role = 1; // D-like shell residue

        void_core_index = Some(a);

        for &r in &recipients {
            parcels[r].ste_amount *= 0.5;
        }
    }

    if class == AnnihilationClass::Cavitation {
        // Keep the void center at exactly zero STE.
        parcels[b].ste_amount = recycle_ste;
    } else {
        parcels[a].ste_amount = recycle_ste;
        parcels[a].spin = 0.0;
        parcels[a].choke = None;
        parcels[a].role = 0;

        parcels[b].ste_amount = recycle_ste;
        parcels[b].spin = 0.0;
        parcels[b].choke = None;
        parcels[b].role = 0;
    }

    AnnihilationResult { a, b, energy, class, recipients, void_core_index }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_choked_parcel(id: u64, ste: f64, spin: f64, phase: ChokePhase) -> Parcel {
        let mut p = Parcel::new(id, ste);
        p.spin = spin;
        p.vorticity = spin.abs();
        p.choke = Some(crate::parcel::ChokeState {
            phase,
            spin,
            coherence: 0.9,
            radius: 1.0,
            equilibrium_concentration: p.concentration,
            age: 1.0,
        });
        p
    }

    #[test]
    fn spawn_conserves_ste() {
        let mut parent = make_choked_parcel(0, 10.0, 0.5, ChokePhase::Drift);
        let daughter = spawn_bubble(&mut parent, 2.0, 0.5, 1);
        assert!(
            (parent.ste_amount + daughter.ste_amount - 10.0).abs() < 1e-10,
            "STE must be conserved across spawn"
        );
        assert!(parent.choke.is_none());
    }

    #[test]
    fn daughter_inherits_spin() {
        let mut parent = make_choked_parcel(0, 10.0, -0.3, ChokePhase::Drift);
        let daughter = spawn_bubble(&mut parent, 2.0, -0.3, 1);
        assert!((daughter.spin - (-0.3)).abs() < 1e-10);
    }

    #[test]
    fn annihilation_conserves_ste() {
        let mut a = make_choked_parcel(0, 5.0, 0.5, ChokePhase::Drift);
        let mut b = make_choked_parcel(1, 3.0, -0.5, ChokePhase::Drift);
        let mut f = Parcel::new(2, 1.0);
        a.x = 0.0; b.x = 0.3; f.x = 0.5;

        let total_before = a.ste_amount + b.ste_amount + f.ste_amount;
        let mut parcels = vec![a, b, f];
        let mut config = SimConfig::default();
        config.annihilation_light_threshold = 100.0;
        config.annihilation_gamma_threshold = 100.0;
        config.annihilation_cavitation_threshold = 100.0;
        config.foam_annihilation_range = 100.0;
        let result = annihilate(&mut parcels, 0, 1, &config);

        let total_after: f64 = parcels.iter().map(|p| p.ste_amount).sum();
        assert!((total_after - total_before).abs() < 1e-10);
        assert!((result.energy - 8.0).abs() < 1e-10);
        assert_eq!(result.class, AnnihilationClass::Heat);
    }

    #[test]
    fn same_spin_does_not_annihilate() {
        let mut config = SimConfig::default();
        config.foam_annihilation_range = 2.0;
        let mut a = make_choked_parcel(0, 5.0, 0.5, ChokePhase::Drift);
        let b = make_choked_parcel(1, 3.0, 0.5, ChokePhase::Drift);
        a.x = 0.0;
        let parcels = vec![a, b];
        assert!(find_annihilations(&parcels, &config).is_empty());
    }

    #[test]
    fn opposite_spin_annihilates() {
        let mut config = SimConfig::default();
        config.foam_annihilation_range = 2.0;
        let mut a = make_choked_parcel(0, 5.0, 0.5, ChokePhase::Drift);
        let mut b = make_choked_parcel(1, 3.0, -0.5, ChokePhase::Drift);
        a.x = 0.0; b.x = 0.3;
        let parcels = vec![a, b];
        assert_eq!(find_annihilations(&parcels, &config).len(), 1);
    }
}
