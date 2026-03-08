use serde::{Deserialize, Serialize};

/// A discrete parcel of STE — the atomic simulation unit.
///
/// Each parcel carries STE and has a position in space.
/// Interactions are computed from pairwise distances — no bonds
/// needed.  The field IS the connection between parcels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parcel {
    /// Unique identifier.
    pub id: u64,

    /// The amount of STE this parcel carries (conserved).
    pub ste_amount: f64,

    /// Position in space.
    pub x: f64,
    pub y: f64,
    pub z: f64,

    /// Velocity.
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,

    /// Radius of this parcel's STE sphere.
    /// r = scale × STE^(1/3)  (volume ∝ STE).
    pub radius: f64,

    /// Local concentration — STE per spherical surface area.
    /// c = STE / (4π r²).  Locked to STE amount.
    pub concentration: f64,

    /// Local vorticity — derived from velocity curl.
    pub vorticity: f64,

    /// Signed spin — net angular momentum of this parcel's local flow.
    pub spin: f64,

    /// Choke state.  `None` = free-flowing active STE.
    pub choke: Option<ChokeState>,

    /// Shell level — current compression state relative to equilibrium.
    pub shell_level: f64,

    /// Equilibrium shell level given current local conditions.
    pub shell_equilibrium: f64,

    /// Role tag for rendering: 0 = field, 1 = D quark, 2 = U quark.
    pub role: u8,
}

/// The lifecycle state of a choke (micro-vortex).
///
/// Maps directly to D10:
///   Formation → Lift-off → Coherence → Drift → Dissolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChokePhase {
    /// Just formed — depression being enveloped.
    Formation,
    /// Shear is lifting the vortex away from the core.
    LiftOff,
    /// Stable in the coherence zone — the vortex sustains itself.
    Coherence,
    /// Wandering within the coherence zone.
    Drift,
    /// Ambient density matches core — shell stripping in progress.
    Dissolution,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChokeState {
    /// Current lifecycle phase.
    pub phase: ChokePhase,
    /// Signed spin — circulation magnitude AND direction.
    /// Positive = matter vortex, negative = antimatter vortex.
    /// Opposite-spin chokes annihilate on contact.
    pub spin: f64,
    /// Coherence — how well-defined the vortex is (1.0 → 0.0).
    pub coherence: f64,
    /// Bond-distance to the nearest high-concentration parcel (the "centre").
    pub radius: f64,
    /// The concentration at which this choke is in equilibrium.
    pub equilibrium_concentration: f64,
    /// Age in simulation time.
    pub age: f64,
}

impl Parcel {
    /// Create a new free-flowing STE parcel at the origin.
    /// Radius and concentration are computed from STE amount.
    pub fn new(id: u64, ste_amount: f64) -> Self {
        let r = ste_amount.max(1e-10).cbrt();
        let sa = 4.0 * std::f64::consts::PI * r * r;
        Self {
            id,
            ste_amount,
            x: 0.0, y: 0.0, z: 0.0,
            vx: 0.0, vy: 0.0, vz: 0.0,
            radius: r,
            concentration: ste_amount / sa,
            vorticity: 0.0,
            spin: 0.0,
            choke: None,
            shell_level: 0.0,
            shell_equilibrium: 0.0,
            role: 0,
        }
    }

    /// Recompute radius and concentration from current STE amount.
    /// Call after any STE transfer (diffusion, foam, etc.).
    pub fn update_radius_and_concentration(&mut self, radius_scale: f64) {
        self.radius = radius_scale * self.ste_amount.max(1e-10).cbrt();
        let sa = 4.0 * std::f64::consts::PI * self.radius * self.radius;
        self.concentration = self.ste_amount / sa;
    }

    /// Whether this parcel is currently part of a choke.
    pub fn is_choked(&self) -> bool {
        self.choke.is_some()
    }

    /// Whether this parcel is effectively void (concentration ≈ 0).
    pub fn is_void(&self, threshold: f64) -> bool {
        self.concentration <= threshold
    }

    /// How far above equilibrium shell this parcel is.
    pub fn squeeze_excess(&self) -> f64 {
        (self.shell_level - self.shell_equilibrium).max(0.0)
    }

    /// Distance from another parcel.
    pub fn dist_to(&self, other: &Parcel) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Speed (magnitude of velocity).
    pub fn speed(&self) -> f64 {
        (self.vx * self.vx + self.vy * self.vy + self.vz * self.vz).sqrt()
    }
}
