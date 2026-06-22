//! SDK error type.
//!
//! The SDK refuses to silently accept a malformed residual, weight, or
//! stability constant. Every numerical contract in PSP-8 (finite non-negative
//! residual scores, strictly positive edge weights, `alpha > beta`, `mu > 0`)
//! is enforced as a typed error rather than a soft pass.

use thiserror::Error;

/// Errors produced by the SRBN agent SDK.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum SdkError {
    /// A residual score was negative, NaN, or infinite.
    #[error("invalid residual score: {0}")]
    InvalidScore(String),

    /// An energy weight was non-positive, NaN, or infinite.
    #[error("invalid energy weight: {0}")]
    InvalidWeight(String),

    /// A descent tolerance, energy, or other gate input was out of range.
    #[error("invalid gate parameter: {0}")]
    InvalidGate(String),

    /// A declared analytic stability constant violated its precondition.
    #[error("invalid stability claim: {0}")]
    InvalidStability(String),

    /// The spectral energy-slope computation could not be completed.
    #[error("spectral computation error: {0}")]
    Spectral(String),

    /// The underlying `srbn` kernel reported an error.
    #[error("srbn kernel error: {0}")]
    Kernel(String),

    /// A domain package produced an inconsistent contract.
    #[error("domain error: {0}")]
    Domain(String),
}

impl From<srbn::Error> for SdkError {
    fn from(err: srbn::Error) -> Self {
        SdkError::Kernel(err.to_string())
    }
}

/// SDK result alias.
pub type Result<T> = std::result::Result<T, SdkError>;

/// Validate that a value is finite and non-negative (PSP-8 residual contract).
pub(crate) fn check_non_negative_finite(value: f64, what: &str) -> Result<()> {
    if !value.is_finite() {
        return Err(SdkError::InvalidScore(format!(
            "{what} is not finite: {value}"
        )));
    }
    if value < 0.0 {
        return Err(SdkError::InvalidScore(format!(
            "{what} is negative: {value}"
        )));
    }
    Ok(())
}

/// Validate that a weight is finite and strictly positive (PSP-8 `w_e > 0`).
pub(crate) fn check_positive_finite(value: f64, what: &str) -> Result<()> {
    if !value.is_finite() {
        return Err(SdkError::InvalidWeight(format!(
            "{what} is not finite: {value}"
        )));
    }
    if value <= 0.0 {
        return Err(SdkError::InvalidWeight(format!(
            "{what} must be strictly positive: {value}"
        )));
    }
    Ok(())
}
