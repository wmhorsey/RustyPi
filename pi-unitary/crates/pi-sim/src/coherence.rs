use pi_core::{safe_div, MathError, Phase, PhaseWindow};

/// Smallest angular distance between two phases, in radians within [0, pi].
pub fn phase_delta(a: Phase, b: Phase) -> f64 {
    let raw = (a.rad() - b.rad()).abs();
    raw.min(core::f64::consts::TAU - raw)
}

/// Coherence score in [0, 1], where 1 means phase-aligned.
///
/// The gate uses a cosine falloff over the provided phase window.
pub fn coherence_gate_checked(
    current: Phase,
    target: Phase,
    window: PhaseWindow,
) -> Result<f64, MathError> {
    let d = phase_delta(current, target);
    if d >= window.rad() {
        return Ok(0.0);
    }
    let x = safe_div(d, window.rad())?;
    Ok(0.5 * (1.0 + (core::f64::consts::PI * x).cos()))
}

/// Convenience adapter for callers that still pass raw window radians.
pub fn coherence_gate(current: Phase, target: Phase, window_rad: f64) -> f64 {
    match PhaseWindow::new(window_rad).and_then(|w| coherence_gate_checked(current, target, w)) {
        Ok(v) => v,
        Err(_) => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn delta_wraps_short_way() {
        let a = Phase::from_tau(0.99);
        let b = Phase::from_tau(0.01);
        let d = phase_delta(a, b);
        assert_relative_eq!(d, 0.02 * core::f64::consts::TAU, epsilon = 1e-12);
    }

    #[test]
    fn gate_peaks_at_alignment() {
        let p = Phase::from_tau(0.25);
        let g = coherence_gate(p, p, core::f64::consts::FRAC_PI_2);
        assert_relative_eq!(g, 1.0, epsilon = 1e-12);
    }

    #[test]
    fn checked_gate_rejects_invalid_window() {
        let p = Phase::from_tau(0.1);
        let bad = PhaseWindow::new(0.0);
        assert!(bad.is_err());
        let good = PhaseWindow::new(core::f64::consts::FRAC_PI_2).expect("valid window");
        let v = coherence_gate_checked(p, p, good).expect("valid gate");
        assert_relative_eq!(v, 1.0, epsilon = 1e-12);
    }
}
