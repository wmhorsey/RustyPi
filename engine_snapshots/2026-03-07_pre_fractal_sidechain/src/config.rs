use serde::{Deserialize, Serialize};

/// Tunable parameters for the STE simulation.
///
/// Design principle: NO MAGIC NUMBERS.  Every parameter is either:
///   - A numerical safety floor (softening, void_threshold)
///   - A unit-setting scale (parcel_radius_scale)
///   - An integration resolution (dt, max_parcels)
///   - A physical ratio that should eventually be derived
///
/// Parameters that were magic thresholds have been removed:
///   - choke_vorticity_threshold → now relative to local average
///   - choke_depression_threshold → now relative to local average
///   - choke_decay_rate → now = ambient_concentration × dt
///   - light_hops_per_tick → now emergent from concentration
///   - relay_gain → removed (medium carries or it doesn't)
///   - sectors_per_hemisphere, sector_half_angle → dead (bond-era)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimConfig {
    // ── Geometry ─────────────────────────────────────────────────
    /// Inverse-square exponent.  Must be 2.0 (spherical 3D geometry).
    /// Not tunable — this is geometry, not a knob.
    pub distance_exponent: f64,

    /// Softening length — numerical guard against division by zero.
    /// Not a physical constant; just keeps math stable.
    pub softening: f64,

    /// Maximum interaction range.  Parcels farther apart than this
    /// cannot see each other.  Sets the simulation's spatial horizon.
    pub max_bond_distance: f64,

    /// Shell interaction range.  Distance within which parcels'
    /// shells overlap and structure-forming interactions occur.
    pub shell_interaction_range: f64,

    /// Background equilibrium concentration — the ambient STE density.
    pub equilibrium_concentration: f64,

    // ── Fractal pull recursion ─────────────────────────────────
    /// Number of nested pull layers evaluated per pair interaction.
    /// 1 = classical single-scale pull, >1 = recursive field-within-field pull.
    pub fractal_recursion_levels: usize,

    /// Per-level weight decay for recursive pull layers.
    /// Lower values emphasize outer shells; higher values retain deeper layers.
    pub fractal_level_decay: f64,

    /// Mix between smooth inward gradient and shell-wave coupling per level.
    /// 0.0 = pure gradient pull, 1.0 = pure shell-wave pull.
    pub fractal_shell_coupling: f64,

    /// Lower bound on recursive pull gain to prevent numerical dead-zones.
    pub fractal_pull_floor: f64,

    // ── STE Equilibration ───────────────────────────────────────
    /// STE diffusivity — how fast the substrate equilibrates.
    /// κ in: flux = κ · min(STE_i, STE_j) · ΔSTE / (d · (1 + η)).
    pub diffusivity: f64,

    // ── Viscosity ───────────────────────────────────────────────
    /// Base viscosity coefficient.  η = base × concentration^exponent.
    pub viscosity_base: f64,

    /// Exponent on the concentration→viscosity curve.
    /// 1.0 = linear, >1 = superlinear (dense regions resist more).
    pub viscosity_exponent: f64,

    // ── Void ────────────────────────────────────────────────────
    /// STE at or below which a parcel is structurally void.
    /// This is floating-point zero — not a physical threshold.
    pub void_threshold: f64,

    // ── Foam (bubble spawn + annihilation) ───────────────────────
    /// Minimum choke coherence required for a bubble to detach.
    pub foam_spawn_coherence: f64,

    /// Fraction of the parent's STE donated to the daughter bubble.
    pub foam_spawn_fraction: f64,

    /// Minimum STE a parent must have for a bubble to spawn.
    pub foam_spawn_min_ste: f64,

    /// Range multiplier for annihilation (× shell_interaction_range).
    pub foam_annihilation_range: f64,

    // ── Annihilation energy thresholds ───────────────────────
    /// Energy above which annihilation emits attraction waves (light).
    pub annihilation_light_threshold: f64,
    /// Energy above which annihilation produces a shockwave (gamma burst).
    pub annihilation_gamma_threshold: f64,
    /// Energy above which annihilation cavitates the field (void).
    pub annihilation_cavitation_threshold: f64,

    // ── Parcel geometry ──────────────────────────────────────────
    /// Scale factor for parcel radius: r = scale × STE^(1/3).
    /// This is the ONE free parameter that sets the unit ratio
    /// between STE amount and spatial extent.
    pub parcel_radius_scale: f64,

    // ── Integration ─────────────────────────────────────────────
    /// Simulation timestep.
    pub dt: f64,

    /// Maximum number of parcels in the simulation.
    pub max_parcels: usize,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            // Geometry
            distance_exponent: 2.0,
            softening: 0.01,
            max_bond_distance: 10.0,
            shell_interaction_range: 1.5,
            equilibrium_concentration: 0.01,

            // Fractal pull recursion
            fractal_recursion_levels: 3,
            fractal_level_decay: 0.62,
            fractal_shell_coupling: 0.35,
            fractal_pull_floor: 0.12,

            // STE Equilibration
            diffusivity: 0.05,

            // Viscosity
            viscosity_base: 0.5,
            viscosity_exponent: 1.0,

            // Void
            void_threshold: 1e-10,

            // Foam
            foam_spawn_coherence: 0.5,
            foam_spawn_fraction: 0.05,
            foam_spawn_min_ste: 0.01,
            foam_annihilation_range: 2.0,

            // Annihilation thresholds (tiered response)
            annihilation_light_threshold: 0.5,
            annihilation_gamma_threshold: 2.0,
            annihilation_cavitation_threshold: 8.0,

            // Parcel geometry
            parcel_radius_scale: 0.3,

            // Integration
            dt: 0.001,
            max_parcels: 10_000,
        }
    }
}
