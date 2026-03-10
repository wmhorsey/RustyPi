use serde::{Deserialize, Serialize};

use crate::math_error::MathError;

pub const TAU: f64 = core::f64::consts::TAU;

/// Tau-wrapped phase angle in radians.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Phase {
    rad: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhaseWindow {
    rad: f64,
}

impl PhaseWindow {
    pub fn new(rad: f64) -> Result<Self, MathError> {
        if !rad.is_finite() {
            return Err(MathError::DomainViolation("window must be finite"));
        }
        if rad <= 0.0 {
            return Err(MathError::DomainViolation("window must be > 0"));
        }
        if rad > core::f64::consts::PI {
            return Err(MathError::DomainViolation("window must be <= pi"));
        }
        Ok(Self { rad })
    }

    #[inline]
    pub fn rad(self) -> f64 {
        self.rad
    }
}

impl Phase {
    #[inline]
    pub fn from_rad(rad: f64) -> Self {
        Self { rad: wrap_tau(rad) }
    }

    #[inline]
    pub fn from_pi(mult: f64) -> Self {
        Self::from_rad(core::f64::consts::PI * mult)
    }

    #[inline]
    pub fn from_tau(mult: f64) -> Self {
        Self::from_rad(TAU * mult)
    }

    #[inline]
    pub fn rad(self) -> f64 {
        self.rad
    }

    #[inline]
    pub fn turns(self) -> f64 {
        self.rad / TAU
    }

    #[inline]
    pub fn sin(self) -> f64 {
        self.rad.sin()
    }

    #[inline]
    pub fn cos(self) -> f64 {
        self.rad.cos()
    }

    #[inline]
    pub fn add_rad(self, delta: f64) -> Self {
        Self::from_rad(self.rad + delta)
    }
}

#[inline]
pub fn wrap_tau(rad: f64) -> f64 {
    let mut x = rad % TAU;
    if x < 0.0 {
        x += TAU;
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn wraps_into_zero_to_tau() {
        let a = Phase::from_rad(-0.25 * TAU);
        assert!(a.rad() >= 0.0 && a.rad() < TAU);
        assert_relative_eq!(a.turns(), 0.75, epsilon = 1e-12);
    }

    #[test]
    fn pi_constructor_matches_expected() {
        let p = Phase::from_pi(0.5);
        assert_relative_eq!(p.rad(), core::f64::consts::FRAC_PI_2, epsilon = 1e-12);
    }

    #[test]
    fn phase_window_rejects_invalid_ranges() {
        assert!(PhaseWindow::new(0.0).is_err());
        assert!(PhaseWindow::new(core::f64::consts::PI * 1.01).is_err());
        assert!(PhaseWindow::new(core::f64::consts::FRAC_PI_2).is_ok());
    }
}
