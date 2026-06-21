//! Analytic stability claims (PSP-8 System 2).
//!
//! Perspt's primary contract is the *measured* discrete gate ([`crate::gate`]).
//! The continuous embedding-space constants `alpha` (correction strength),
//! `beta` (drift), `delta` (disturbance), `L` (smoothness), and `eta` (step
//! size) are optional: a domain registers them only when it can instrument a
//! continuous correction coordinate. The coding domain exposes no such
//! coordinate, so these remain `NotClaimed` for it.
//!
//! The energy-slope constant `mu` is the exception — it is combinatorial and is
//! computed from the verification graph in [`crate::spectral`].
//!
//! When the constants are present and satisfy their preconditions, the SDK
//! reports the input-to-state-stability floor
//!
//! ```text
//! V_inf = delta^2 / (2 (alpha - beta)^2 mu),   alpha > beta, mu > 0,
//! ```
//!
//! and the discrete step-size certificate.

use serde::{Deserialize, Serialize};

use crate::error::{Result, SdkError};

/// A registered analytic stability claim (PSP-8 `StabilityClaim`). A missing
/// constant is an explicit `NotClaimed` status, never a soft pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StabilityClaim {
    pub claim_id: String,
    /// The sublevel set / analysis domain on which the constants are justified.
    pub analysis_domain: String,
    pub alpha: Option<f64>,
    pub beta: Option<f64>,
    pub delta: Option<f64>,
    /// Spectral energy-slope constant; usually filled from [`crate::spectral`].
    pub mu: Option<f64>,
    pub smoothness_l: Option<f64>,
    pub eta: Option<f64>,
    /// Cached ISS floor `V_inf` if it could be computed.
    pub ultimate_floor: Option<f64>,
    pub evidence_refs: Vec<String>,
}

impl StabilityClaim {
    pub fn not_claimed(analysis_domain: impl Into<String>) -> Self {
        Self {
            claim_id: uuid::Uuid::new_v4().to_string(),
            analysis_domain: analysis_domain.into(),
            alpha: None,
            beta: None,
            delta: None,
            mu: None,
            smoothness_l: None,
            eta: None,
            ultimate_floor: None,
            evidence_refs: Vec::new(),
        }
    }

    /// Whether enough constants are present to assert an analytic floor.
    pub fn claims_floor(&self) -> bool {
        self.alpha.is_some() && self.beta.is_some() && self.delta.is_some() && self.mu.is_some()
    }

    /// Compute and cache the ISS floor `V_inf`, returning it when the
    /// preconditions `alpha > beta` and `mu > 0` hold.
    pub fn resolve_floor(&mut self) -> Result<Option<f64>> {
        let floor = match (self.alpha, self.beta, self.delta, self.mu) {
            (Some(a), Some(b), Some(d), Some(m)) => Some(ultimate_floor(a, b, d, m)?),
            _ => None,
        };
        self.ultimate_floor = floor;
        Ok(floor)
    }
}

/// ISS energy floor `V_inf = delta^2 / (2 (alpha - beta)^2 mu)`.
///
/// Requires `alpha > beta` and `mu > 0` (PSP-8 System 2).
pub fn ultimate_floor(alpha: f64, beta: f64, delta: f64, mu: f64) -> Result<f64> {
    for (name, v) in [("alpha", alpha), ("beta", beta), ("delta", delta), ("mu", mu)] {
        if !v.is_finite() {
            return Err(SdkError::InvalidStability(format!("{name} is not finite")));
        }
    }
    if alpha <= beta {
        return Err(SdkError::InvalidStability(format!(
            "ISS floor requires alpha > beta (got alpha={alpha}, beta={beta})"
        )));
    }
    if mu <= 0.0 {
        return Err(SdkError::InvalidStability(format!(
            "ISS floor requires mu > 0 (got mu={mu})"
        )));
    }
    let gap = alpha - beta;
    Ok((delta * delta) / (2.0 * gap * gap * mu))
}

/// Sufficient discrete step-size upper bound (PSP-8 Theorem 12.1):
///
/// ```text
/// eta < min{ 2(alpha-beta) / (L (alpha+beta)^2),  1 / (2 mu (alpha-beta)) }.
/// ```
pub fn step_size_upper_bound(alpha: f64, beta: f64, smoothness_l: f64, mu: f64) -> Result<f64> {
    if alpha <= beta {
        return Err(SdkError::InvalidStability("step-size bound requires alpha > beta".into()));
    }
    if smoothness_l <= 0.0 || mu <= 0.0 {
        return Err(SdkError::InvalidStability("step-size bound requires L > 0 and mu > 0".into()));
    }
    let gap = alpha - beta;
    let sum = alpha + beta;
    let smoothness_term = (2.0 * gap) / (smoothness_l * sum * sum);
    let curvature_term = 1.0 / (2.0 * mu * gap);
    Ok(smoothness_term.min(curvature_term))
}

/// Geometric contraction coefficient `c(eta) = (alpha-beta) - L eta (alpha+beta)^2 / 2`.
pub fn c_eta(alpha: f64, beta: f64, smoothness_l: f64, eta: f64) -> f64 {
    let sum = alpha + beta;
    (alpha - beta) - (smoothness_l * eta * sum * sum) / 2.0
}

/// Expected one-step geometric factor `1 - 2 eta c(eta) mu` from
/// `V(x_k) <= (1 - 2 eta c(eta) mu)^k V(x_0)`.
pub fn geometric_factor(eta: f64, c: f64, mu: f64) -> f64 {
    1.0 - 2.0 * eta * c * mu
}

/// Check that a proposed step size respects the sufficient bound.
pub fn validate_step_size(eta: f64, alpha: f64, beta: f64, smoothness_l: f64, mu: f64) -> Result<bool> {
    if eta <= 0.0 {
        return Ok(false);
    }
    let bound = step_size_upper_bound(alpha, beta, smoothness_l, mu)?;
    Ok(eta < bound)
}

/// The kernel-facing stability parameters (PSP-8 `StabilityParameters`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StabilityParameters {
    pub energy_tolerance: f64,
    /// Single descent tolerance for the gate.
    pub rho_gate: f64,
    /// Advisory analytic decrease only; never relaxes `rho_gate`.
    pub predicted_descent: Option<f64>,
    /// Spectral, computed from the verification graph.
    pub mu: Option<f64>,
    pub alpha: Option<f64>,
    pub beta: Option<f64>,
    pub delta: Option<f64>,
    pub smoothness_l: Option<f64>,
    pub eta: Option<f64>,
    /// Requires alpha, beta, delta, mu.
    pub ultimate_floor: Option<f64>,
}

impl StabilityParameters {
    /// Measured-only parameters: just `rho_gate` and `energy_tolerance`, with
    /// all analytic constants `NotClaimed`. This is the coding-domain default.
    pub fn measured(rho_gate: f64, energy_tolerance: f64) -> Self {
        Self {
            energy_tolerance,
            rho_gate,
            predicted_descent: None,
            mu: None,
            alpha: None,
            beta: None,
            delta: None,
            smoothness_l: None,
            eta: None,
            ultimate_floor: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iss_floor_matches_formula() {
        // delta=1, alpha=2, beta=1, mu=0.5 -> 1 / (2 * 1 * 0.5) = 1.0
        let v = ultimate_floor(2.0, 1.0, 1.0, 0.5).unwrap();
        assert!((v - 1.0).abs() < 1e-12);
    }

    #[test]
    fn iss_floor_requires_alpha_gt_beta() {
        assert!(ultimate_floor(1.0, 1.0, 1.0, 0.5).is_err());
        assert!(ultimate_floor(1.0, 2.0, 1.0, 0.5).is_err());
    }

    #[test]
    fn iss_floor_requires_positive_mu() {
        assert!(ultimate_floor(2.0, 1.0, 1.0, 0.0).is_err());
    }

    #[test]
    fn step_size_bound_is_min_of_two_terms() {
        // alpha=2,beta=1,L=1,mu=10: smoothness=2/(1*9)=0.222; curvature=1/(2*10*1)=0.05
        let bound = step_size_upper_bound(2.0, 1.0, 1.0, 10.0).unwrap();
        assert!((bound - 0.05).abs() < 1e-12);
        assert!(validate_step_size(0.04, 2.0, 1.0, 1.0, 10.0).unwrap());
        assert!(!validate_step_size(0.06, 2.0, 1.0, 1.0, 10.0).unwrap());
    }

    #[test]
    fn claim_resolves_floor() {
        let mut claim = StabilityClaim::not_claimed("toy");
        claim.alpha = Some(2.0);
        claim.beta = Some(1.0);
        claim.delta = Some(1.0);
        claim.mu = Some(0.5);
        assert!(claim.claims_floor());
        let f = claim.resolve_floor().unwrap().unwrap();
        assert!((f - 1.0).abs() < 1e-12);
        assert_eq!(claim.ultimate_floor, Some(f));
    }

    #[test]
    fn not_claimed_has_no_floor() {
        let mut claim = StabilityClaim::not_claimed("coding");
        assert!(!claim.claims_floor());
        assert_eq!(claim.resolve_floor().unwrap(), None);
    }
}
