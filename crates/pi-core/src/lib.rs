pub mod phase;

pub use phase::{Phase, TAU};

/// Construct a scalar in radians from a multiple of pi.
#[inline]
pub fn from_pi(k: f64) -> f64 {
    core::f64::consts::PI * k
}

/// Construct a scalar in radians from a multiple of tau (2*pi).
#[inline]
pub fn from_tau(k: f64) -> f64 {
    TAU * k
}
