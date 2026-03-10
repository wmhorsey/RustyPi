pub mod additive;
pub mod math_error;
pub mod phase;
pub mod safe;

pub use additive::{PhaseTicks, RemainderAccumulator, TAU_TICKS_DEFAULT};
pub use math_error::MathError;
pub use phase::PhaseWindow;
pub use phase::{Phase, TAU};
pub use safe::{checked_sqrt, safe_div};

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
