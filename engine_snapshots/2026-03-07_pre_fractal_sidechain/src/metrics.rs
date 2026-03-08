use serde::{Deserialize, Serialize};

use crate::config::SimConfig;

/// # Metrics — Calibration Telemetry
///
/// Every simulation tick captures key observables.
/// In the game, these feed the scoring system.
/// In the research pipeline, these are the distributed
/// parameter-search results that calibrate the model.

/// Snapshot of simulation state at a single tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameMetrics {
    /// Tick number.
    pub tick: u64,

    /// Number of parcels in the simulation.
    pub parcel_count: usize,

    /// Total STE in the system (should be conserved).
    pub total_ste: f64,

    /// Number of choked parcels (micro-vortices).
    pub choke_count: usize,

    /// Number of active attraction waves (light).
    pub active_waves: usize,

    /// Number of capture events this tick (photon particles formed).
    pub captures_this_tick: usize,

    /// Number of bubbles that detached this tick.
    pub foam_spawns: usize,

    /// Number of annihilation events this tick.
    pub foam_annihilations: usize,
}

/// Cumulative metrics for a full simulation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMetrics {
    /// The config that produced this run.
    pub config: SimConfig,

    /// Total ticks computed.
    pub total_ticks: u64,

    /// Did the simulation produce stable structures?
    pub structures_formed: bool,

    /// Longest-lived choke (in ticks).
    pub max_choke_lifespan: u64,

    /// Total photon capture events.
    pub total_captures: u64,

    /// STE conservation error: |STE_final - STE_initial| / STE_initial.
    pub ste_conservation_error: f64,

    /// All frame snapshots (optional, for replay/analysis).
    pub frames: Vec<FrameMetrics>,
}

impl RunMetrics {
    pub fn new(config: SimConfig) -> Self {
        Self {
            config,
            total_ticks: 0,
            structures_formed: false,
            max_choke_lifespan: 0,
            total_captures: 0,
            ste_conservation_error: 0.0,
            frames: Vec::new(),
        }
    }

    pub fn record_frame(&mut self, frame: FrameMetrics) {
        self.total_ticks = frame.tick;
        self.total_captures += frame.captures_this_tick as u64;
        if frame.choke_count > 0 {
            self.structures_formed = true;
        }
        self.frames.push(frame);
    }
}
