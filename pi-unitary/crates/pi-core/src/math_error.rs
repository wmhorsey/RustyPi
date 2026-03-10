use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MathError {
    #[error("divide by zero")]
    DivideByZero,
    #[error("domain violation: {0}")]
    DomainViolation(&'static str),
    #[error("invalid configuration: {0}")]
    InvalidConfig(&'static str),
}
