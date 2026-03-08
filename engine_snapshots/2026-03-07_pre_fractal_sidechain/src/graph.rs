use crate::bond::{Bond, ChannelRegime};
use crate::config::SimConfig;
use crate::parcel::Parcel;

/// # The Bond Graph
///
/// This is the simulation's topology.  No coordinates, no grid.
/// Just parcels and the bonds between them.
///
/// ## Line-of-Sight (LOS)
///
/// Each parcel has a solid-angle budget — its local sphere divided
/// into sectors.  A bond occupies a sector.  If a nearer parcel
/// already fills that sector, farther parcels in the same direction
/// are occluded and cannot bond.
///
/// This naturally produces:
/// - Shell/surface topology (interior parcels are hidden behind surfaces)
/// - Dense packing = all sectors filled with short bonds = high concentration
/// - Void = no bonds at all
///
/// ## Channel Fill
///
/// Every bond channel is filled with equilibrium STE.  Beyond
/// shell-interaction range the channel is inert (transparent).
/// Within range, shells overlap and the real physics begins.
pub struct Graph {
    /// Total number of sectors per parcel (2 * sectors_per_hemisphere).
    pub total_sectors: usize,
}

impl Graph {
    pub fn new(config: &SimConfig) -> Self {
        Self {
            total_sectors: config.sectors_per_hemisphere * 2,
        }
    }

    /// Rebuild all bonds for every parcel based on LOS visibility.
    ///
    /// This is the "who can see whom" pass.  For each parcel:
    /// 1. Gather all candidate peers (within max_bond_distance).
    /// 2. Sort candidates by distance (nearest first).
    /// 3. For each candidate, check if its sector is already occupied.
    /// 4. If the sector is free, form the bond and mark the sector occupied.
    /// 5. If occupied, the candidate is occluded — skip it.
    ///
    /// Because the simulation is relational, we need a way to know
    /// candidate distances.  We maintain the bond list itself as the
    /// source of truth — bonds whose distance exceeds max_bond_distance
    /// are pruned, and new bonds are discovered through peer-of-peer
    /// introductions.
    pub fn rebuild_bonds(parcels: &mut [Parcel], config: &SimConfig) {
        let n = parcels.len();
        let total_sectors = config.sectors_per_hemisphere * 2;

        for i in 0..n {
            // Sort existing bonds by distance (nearest first)
            parcels[i].bonds.sort_by(|a, b| {
                a.distance.total_cmp(&b.distance)
            });

            // Prune bonds beyond max range
            parcels[i].bonds.retain(|b| b.distance <= config.max_bond_distance);

            // LOS pass: keep only the nearest bond per sector
            let mut occupied_sectors: Vec<bool> = vec![false; total_sectors];
            let mut kept = Vec::new();

            for bond in &parcels[i].bonds {
                let sector = angle_to_sector(bond.angle, bond.elevation, total_sectors);
                if !occupied_sectors[sector] {
                    occupied_sectors[sector] = true;
                    kept.push(bond.clone());
                }
                // else: occluded by a nearer bond in the same sector — drop it
            }

            parcels[i].bonds = kept;

            // Update channel regime on each surviving bond
            for bond in &mut parcels[i].bonds {
                bond.update_regime(config.shell_interaction_range);

                // Set channel concentration based on regime
                match bond.regime {
                    ChannelRegime::Equilibrium => {
                        bond.channel_concentration = config.equilibrium_concentration;
                    }
                    ChannelRegime::ShellInteraction => {
                        // Channel concentration ramps up as shells overlap.
                        // Linear interpolation: at shell_interaction_range it equals
                        // equilibrium; at distance=0 it equals the parcel's own
                        // concentration (capped to avoid explosion).
                        let t = (1.0 - bond.distance / config.shell_interaction_range).max(0.0);
                        bond.channel_concentration = config.equilibrium_concentration
                            + t * (parcels[i].concentration - config.equilibrium_concentration);
                    }
                }
            }
        }
    }

    /// Discover new potential bonds through peer-of-peer introductions.
    ///
    /// For each parcel A, look at the bonds of A's bonded peers.
    /// If peer B has a bond to C, and A does not know C, then C is
    /// a candidate for A — but only if A has a free sector in C's
    /// direction (estimated from the angular chain A→B→C).
    pub fn discover_bonds(parcels: &mut [Parcel], config: &SimConfig) {
        let n = parcels.len();
        let total_sectors = config.sectors_per_hemisphere * 2;

        // Collect introductions first to avoid borrow conflicts.
        // (parcel, new_peer, est_distance, est_angle, est_elevation)
        let mut introductions: Vec<(usize, usize, f64, f64, f64)> = Vec::new();

        for i in 0..n {
            let my_peers: Vec<usize> = parcels[i].bonds.iter().map(|b| b.peer).collect();

            for bond_ib in &parcels[i].bonds {
                let b = bond_ib.peer;
                if b >= n { continue; }

                for bond_bc in &parcels[b].bonds {
                    let c = bond_bc.peer;
                    if c == i || c >= n { continue; }
                    if my_peers.contains(&c) { continue; }

                    // Estimate distance A→C via triangle inequality heuristic.
                    let est_dist = (bond_ib.distance + bond_bc.distance) * 0.7;
                    if est_dist <= config.max_bond_distance {
                        // Estimate angle: C is "past" B from A's perspective.
                        // Offset the A→B angle by B→C's relative angle, keeping
                        // the result in [0, 2π).  This prevents discovered bonds
                        // from all piling into sector 0.
                        let offset = bond_bc.angle - std::f64::consts::PI; // B's outward direction reversed
                        let est_angle = (bond_ib.angle + offset)
                            .rem_euclid(2.0 * std::f64::consts::PI);
                        let est_elev = (bond_ib.elevation + bond_bc.elevation) * 0.5;
                        introductions.push((i, c, est_dist, est_angle, est_elev));
                    }
                }
            }
        }

        // Apply introductions
        for (i, c, est_dist, est_angle, est_elev) in introductions {
            if parcels[i].bonds.iter().any(|b| b.peer == c) {
                continue;
            }
            if parcels[i].bonds.len() >= total_sectors {
                continue; // All sectors full
            }

            // Only add if the estimated sector is free.
            let sector = angle_to_sector(est_angle, est_elev, total_sectors);
            let occupied = parcels[i].bonds.iter().any(|b| {
                angle_to_sector(b.angle, b.elevation, total_sectors) == sector
            });
            if occupied {
                continue; // Don't displace an existing bond
            }

            let mut new_bond = Bond::new(c, est_dist);
            new_bond.angle = est_angle;
            new_bond.elevation = est_elev;
            new_bond.update_regime(config.shell_interaction_range);
            new_bond.channel_concentration = config.equilibrium_concentration;
            parcels[i].bonds.push(new_bond);
        }
    }

    /// Estimate local concentration — purely intrinsic.
    ///
    /// Concentration = STE / volume = the density of STE at this spot.
    /// This IS the parcel's mass.  It exists on its own — it does not
    /// come from neighbours.  Neighbours' effect on the field is handled
    /// by tension wells, not by modifying concentration.
    ///
    ///   c_i = ste_i / V_i
    ///
    /// Volume is estimated from the average spacing to neighbours
    /// (avg_bond_distance²).  Tighter spacing → smaller volume →
    /// higher density → heavier.  This is why compressed structures
    /// are denser and more massive.
    pub fn estimate_concentration(parcels: &mut [Parcel], config: &SimConfig) {
        for i in 0..parcels.len() {
            let avg_d = parcels[i].avg_bond_distance();
            let volume = if avg_d.is_finite() && avg_d > config.softening {
                avg_d * avg_d
            } else {
                // Unbonded — use a large default volume (dilute, not void).
                config.max_bond_distance * config.max_bond_distance
            };

            parcels[i].concentration = parcels[i].ste_amount / volume;

            // Equilibrium shell level scales with concentration
            parcels[i].shell_equilibrium = parcels[i].concentration * 0.1;
            if parcels[i].shell_level < parcels[i].shell_equilibrium {
                parcels[i].shell_level = parcels[i].shell_equilibrium;
            }
        }
    }

    /// Estimate vorticity from circulation in bond loops.
    ///
    /// Vorticity without coordinates: look at the closing_speed pattern
    /// around a parcel's bond ring.  If neighbours are systematically
    /// approaching on one side and receding on the other, that's
    /// circulation — the neighbourhood is spinning.
    ///
    /// ω_i = Σ (closing_speed_j * sign(angle_j)) / n_bonds
    pub fn estimate_vorticity(parcels: &mut [Parcel]) {
        let n = parcels.len();
        for i in 0..n {
            if parcels[i].bonds.is_empty() {
                parcels[i].vorticity = 0.0;
                parcels[i].spin = 0.0;
                continue;
            }

            let mut circulation = 0.0;
            for bond in &parcels[i].bonds {
                // The sign of the angle determines which "side" of the
                // parcel this bond is on.  Systematic approach on one
                // side + recession on the other = rotation.
                let angular_sign = if bond.angle <= std::f64::consts::PI { 1.0 } else { -1.0 };
                circulation += bond.closing_speed * angular_sign;
            }

            let signed_vorticity = circulation / parcels[i].bonds.len() as f64;
            parcels[i].vorticity = signed_vorticity.abs();
            // Spin accumulates: existing spin + new circulation contribution.
            // Damped slightly so it doesn't grow unbounded.
            parcels[i].spin = parcels[i].spin * 0.99 + signed_vorticity * 0.01;
        }
    }
}

/// Map (angle, elevation) to a sector index.
///
/// We tile the parcel's local sphere into sectors using a simple
/// latitude-longitude grid.  This is computationally cheap and
/// sufficient for LOS blocking.
fn angle_to_sector(angle: f64, elevation: f64, total_sectors: usize) -> usize {
    use std::f64::consts::PI;
    let half = total_sectors / 2;
    let cols = (half as f64).sqrt().ceil() as usize;
    let rows = (half + cols - 1) / cols;

    // Normalise angle to [0, 2π)
    let a = ((angle % (2.0 * PI)) + 2.0 * PI) % (2.0 * PI);
    // Normalise elevation to [0, π]
    let e = ((elevation % PI) + PI) % PI;

    let col = ((a / (2.0 * PI)) * cols as f64) as usize % cols;
    let row = ((e / PI) * rows as f64) as usize % rows;

    (row * cols + col) % total_sectors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sector_mapping_in_range() {
        let total = 20;
        for angle in [0.0, 1.0, 3.14, 5.0, 6.28] {
            for elev in [0.0, 0.5, 1.57, 2.5, 3.14] {
                let s = angle_to_sector(angle, elev, total);
                assert!(s < total, "sector {s} out of range for angle={angle} elev={elev}");
            }
        }
    }

    #[test]
    fn los_blocks_far_parcels() {
        let config = SimConfig::default();
        let mut parcels = vec![
            Parcel::new(0, 1.0),
            Parcel::new(1, 1.0),
            Parcel::new(2, 1.0),
        ];
        // Parcel 0 has two bonds in the same direction (same angles)
        // The nearer one (distance 1) should block the farther one (distance 5)
        parcels[0].bonds.push(Bond::new(1, 1.0));
        parcels[0].bonds.push(Bond::new(2, 5.0));
        // Both at angle=0, elevation=0

        Graph::rebuild_bonds(&mut parcels, &config);

        let peers: Vec<usize> = parcels[0].bonds.iter().map(|b| b.peer).collect();
        assert!(peers.contains(&1), "near parcel should survive");
        assert!(!peers.contains(&2), "far parcel in same sector should be occluded");
    }
}
