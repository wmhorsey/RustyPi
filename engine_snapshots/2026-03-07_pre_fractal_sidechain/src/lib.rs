//! # Mass Effect Engine
//!
//! An attraction-only physics engine built from the STE ontology.
//!
//! ## Core Principle
//! There is one substance (STE) and one rule (self-attraction).
//! Everything else — structure, heat, light, gravity — emerges from
//! attraction being frustrated at different scales.
//!
//! ## Architecture (All-Pairs, Position-Based)
//!
//! Every parcel owns its position (x, y, z) and velocity (vx, vy, vz).
//! Every tick, all pairs of parcels interact through the ever-present
//! STE field.  Distance = Euclidean distance between positions.
//! At n=200 parcels, this is 20,000 pairs per tick — trivial.
//!
//! No bonds, no graph topology, no separate embed positions.
//! The field IS the connection.  Distances ARE the physics.
//!
//! - `parcel`: A discrete chunk of STE with position and velocity.
//! - `field`: All-pairs interactions — forces, diffusion, viscosity,
//!   concentration, vorticity.  The heart of the engine.
//! - `choke`: Micro-vortex detection from velocity curl.
//! - `light`: Attraction-wave propagation through K-nearest neighbours.
//! - `foam`: Bubble detachment + annihilation.
//! - `metrics`: Telemetry capture for calibration.
//! - `sim`: The integration loop.
//! - `config`: Tunable parameters.

pub mod parcel;
pub mod field;
pub mod foam;
pub mod choke;
pub mod light;
pub mod metrics;
pub mod diagnostics;
pub mod sim;
pub mod config;

#[cfg(feature = "wasm")]
pub mod wasm;

pub use config::SimConfig;
pub use sim::Sim;
