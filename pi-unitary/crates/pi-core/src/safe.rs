use crate::math_error::MathError;

#[inline]
pub fn safe_div(n: f64, d: f64) -> Result<f64, MathError> {
    if d == 0.0 {
        return Err(MathError::DivideByZero);
    }
    let out = n / d;
    if !out.is_finite() {
        return Err(MathError::DomainViolation("non-finite result"));
    }
    Ok(out)
}

#[inline]
pub fn checked_sqrt(x: f64) -> Result<f64, MathError> {
    if x < 0.0 {
        return Err(MathError::DomainViolation("sqrt of negative input"));
    }
    let out = x.sqrt();
    if !out.is_finite() {
        return Err(MathError::DomainViolation("non-finite sqrt result"));
    }
    Ok(out)
}
