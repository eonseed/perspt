//! Calibrated risk budgets via conformal risk control (PSP-8 System 7).
//!
//! When autonomous commitment without per-effect approval is enabled, a
//! validator's acceptance threshold is not an asserted constant: it is
//! calibrated from the deployment's own ledger so the marginal accepted-unsafe
//! rate is bounded by the declared budget `rho`. For a validator that accepts
//! when `s(p) > theta`, the conformal threshold is
//!
//! ```text
//! theta_hat = inf { theta in [0,1] : (n R_n(theta) + 1) / (n + 1) <= rho }
//! ```
//!
//! where `R_n(theta)` is the empirical accepted-unsafe rate on the calibration
//! set. A drift monitor compares the live score distribution to the calibration
//! distribution; on divergence the calibration is flagged stale. A stale flag
//! does **not** assert the conformal bound during the stale window, because
//! exchangeability is broken. Scores below the conservative back-off reject;
//! scores above it route mutations to approval. Read-only or independently
//! deterministic-safe effects are handled before this risk-calibration gate.

use serde::{Deserialize, Serialize};

use crate::capability::RiskClass;
use crate::error::{Result, SdkError};

/// One calibration sample: a validator score and whether the accepted state was
/// later found unsafe (from undo/redo boundary or regression residuals).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CalibrationSample {
    pub score: f64,
    pub is_unsafe: bool,
}

impl CalibrationSample {
    pub fn new(score: f64, is_unsafe: bool) -> Self {
        Self { score, is_unsafe }
    }
}

/// Empirical accepted-unsafe rate `R_n(theta)`: the fraction of calibration
/// samples that would be accepted (`score > theta`) and are unsafe.
pub fn accepted_unsafe_rate(samples: &[CalibrationSample], theta: f64) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let unsafe_accepted = samples
        .iter()
        .filter(|s| s.score > theta && s.is_unsafe)
        .count();
    unsafe_accepted as f64 / samples.len() as f64
}

/// Compute the conformal threshold `theta_hat` for a target budget `rho`.
///
/// Returns the infimum threshold whose conformal-adjusted accepted-unsafe rate
/// is at most `rho`. If even rejecting everything cannot meet `rho` (when
/// `1/(n+1) > rho`), returns `1.0` (accept nothing in `[0,1]`).
pub fn conformal_threshold(samples: &[CalibrationSample], rho: f64) -> Result<f64> {
    if !(0.0..=1.0).contains(&rho) {
        return Err(SdkError::InvalidGate(format!(
            "rho must be in [0,1]: {rho}"
        )));
    }
    if samples.is_empty() {
        return Err(SdkError::Domain(
            "conformal calibration needs samples".into(),
        ));
    }
    let n = samples.len() as f64;

    // Candidate thresholds: 0.0 then each unique score, ascending. R_n is a step
    // function that only changes at sample scores.
    let mut candidates: Vec<f64> = vec![0.0];
    let mut scores: Vec<f64> = samples.iter().map(|s| s.score).collect();
    scores.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    candidates.extend(scores);

    for theta in candidates {
        let r = accepted_unsafe_rate(samples, theta);
        let adjusted = (n * r + 1.0) / (n + 1.0);
        if adjusted <= rho {
            return Ok(theta);
        }
    }
    Ok(1.0)
}

/// Two-sample Kolmogorov–Smirnov statistic `D = max |F_live(x) - F_calib(x)|`.
pub fn ks_statistic(live: &[f64], calib: &[f64]) -> f64 {
    if live.is_empty() || calib.is_empty() {
        return 0.0;
    }
    let mut all: Vec<f64> = live.iter().chain(calib.iter()).copied().collect();
    all.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    all.dedup();
    let cdf =
        |data: &[f64], x: f64| data.iter().filter(|&&v| v <= x).count() as f64 / data.len() as f64;
    all.iter()
        .map(|&x| (cdf(live, x) - cdf(calib, x)).abs())
        .fold(0.0, f64::max)
}

/// Whether the live distribution has drifted from the calibration distribution.
/// Uses the KS statistic against the asymptotic critical value at level `alpha`
/// (`c(alpha) * sqrt((n+m)/(n*m))`, with `c(0.05) ≈ 1.36`).
pub fn is_drifted(live: &[f64], calib: &[f64], alpha_c: f64) -> bool {
    if live.is_empty() || calib.is_empty() {
        return false;
    }
    let n = live.len() as f64;
    let m = calib.len() as f64;
    let critical = alpha_c * ((n + m) / (n * m)).sqrt();
    ks_statistic(live, calib) > critical
}

/// Calibration state of the conformal threshold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CalibrationState {
    /// Calibrated and exchangeable: the conformal bound may be asserted.
    Calibrated { theta_hat: f64, rho: f64 },
    /// Stale: exchangeability broken. The bound is NOT asserted; a back-off
    /// threshold applies and recalibration runs in the background.
    Stale { backoff_theta: f64, last_theta: f64 },
}

impl CalibrationState {
    /// Build a calibrated state from samples.
    pub fn calibrate(samples: &[CalibrationSample], rho: f64) -> Result<Self> {
        match conformal_threshold_checked(samples, rho)? {
            ThresholdOutcome::Feasible { theta_hat } => {
                Ok(CalibrationState::Calibrated { theta_hat, rho })
            }
            ThresholdOutcome::InsufficientCalibration { have, need } => Err(SdkError::Domain(
                format!("insufficient calibration: have {have} labeled samples, need {need}"),
            )),
        }
    }

    /// Transition to a stale state with a conservative back-off (the threshold
    /// is inflated toward 1.0 to accept fewer candidates).
    pub fn mark_stale(&self) -> Self {
        let last = match self {
            CalibrationState::Calibrated { theta_hat, .. } => *theta_hat,
            CalibrationState::Stale { last_theta, .. } => *last_theta,
        };
        // Inflate halfway toward 1.0.
        let backoff = (last + 1.0) / 2.0;
        CalibrationState::Stale {
            backoff_theta: backoff,
            last_theta: last,
        }
    }

    /// Whether the conformal accepted-unsafe bound may currently be asserted.
    /// Only true while calibrated and exchangeable (PSP-8 System 7).
    pub fn bound_is_asserted(&self) -> bool {
        matches!(self, CalibrationState::Calibrated { .. })
    }

    fn active_threshold(&self) -> f64 {
        match self {
            CalibrationState::Calibrated { theta_hat, .. } => *theta_hat,
            CalibrationState::Stale { backoff_theta, .. } => *backoff_theta,
        }
    }
}

/// The outcome of an autonomous-commit acceptance decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptOutcome {
    /// Accepted under the asserted conformal bound (certified).
    CertifiedAccept,
    /// Routed to approval because the calibration bound is unavailable.
    RouteToApproval,
    /// Rejected: score below the active threshold.
    Reject,
}

/// Decide autonomous acceptance for a validator score under the current
/// calibration state and the proposed effect's risk class.
///
/// During a stale window the backed-off threshold remains useful for routing,
/// but no score alone authorizes a mutation: passing proposals route to
/// approval and failing proposals reject. The conformal bound is never asserted
/// in that window (PSP-9 Gate Q).
pub fn decide(state: &CalibrationState, score: f64, _risk: RiskClass) -> AcceptOutcome {
    let threshold = state.active_threshold();
    match state {
        CalibrationState::Calibrated { .. } => {
            if score > threshold {
                AcceptOutcome::CertifiedAccept
            } else {
                AcceptOutcome::Reject
            }
        }
        CalibrationState::Stale { .. } => {
            if score > threshold {
                // Exchangeability is broken, so no mutation may commit from
                // this score alone. Read-only and separately certified
                // deterministic effects are admitted before this risk gate.
                AcceptOutcome::RouteToApproval
            } else {
                AcceptOutcome::Reject
            }
        }
    }
}

// ---------------------------------------------------------------------------
// PSP-9 system 16: feasibility floor, strata, and readiness epochs
// ---------------------------------------------------------------------------

/// The sample floor a declared budget implies: the feasible set of Theorem 5
/// is empty exactly when `rho < 1/(n+1)`, so `n >= 1/rho - 1` labeled
/// samples are required before any proposal can be autonomously accepted at
/// all. For the common `rho = 0.05` this is 19.
pub fn sample_floor(rho: f64) -> Result<usize> {
    if !(0.0..=1.0).contains(&rho) || rho == 0.0 {
        return Err(SdkError::InvalidGate(format!(
            "rho must be in (0,1]: {rho}"
        )));
    }
    Ok((1.0 / rho - 1.0).ceil().max(0.0) as usize)
}

/// A threshold computation that names infeasibility instead of hiding it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum ThresholdOutcome {
    /// The finite-sample certificate is active at this threshold; acceptance
    /// is strict (`score > theta`), ties rejected.
    Feasible { theta_hat: f64 },
    /// `rho < 1/(n+1)`: the feasible set is empty and everything would be
    /// rejected. Reporting this as a satisfied budget is prohibited — it is
    /// technically true and operationally a lie (Gate Q).
    InsufficientCalibration { have: usize, need: usize },
}

/// Compute the threshold, reporting the feasibility floor explicitly.
pub fn conformal_threshold_checked(
    samples: &[CalibrationSample],
    rho: f64,
) -> Result<ThresholdOutcome> {
    let need = sample_floor(rho)?;
    if samples.len() < need {
        return Ok(ThresholdOutcome::InsufficientCalibration {
            have: samples.len(),
            need,
        });
    }
    let theta_hat = conformal_threshold(samples, rho)?;
    Ok(ThresholdOutcome::Feasible { theta_hat })
}

/// The exchangeability stratum a calibration set is keyed by (PSP-9
/// system 16). `(capability, model)` alone is not enough: different models
/// fail differently, and so do different verifier suites and score
/// definitions. Changing **any** field marks the calibration stale.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CalibrationStratum {
    pub domain_package: String,
    pub domain_version: String,
    pub effect_kind: String,
    pub risk_class: String,
    pub model_route: String,
    pub verifier_suite_fingerprint: String,
    pub tool_catalog_fingerprint: String,
    pub policy_version: String,
    pub score_definition: String,
}

/// Cold-start readiness (PSP-9 system 16): before `Active`, read-only work
/// and deterministically contracted effects continue; other mutations route
/// to approval while shadow decisions build the local set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalibrationReadiness {
    InsufficientSamples,
    /// Decisions are recorded counterfactually; nothing is certified.
    Shadow,
    Active,
    Stale,
}

/// Derive readiness from the sample count and stratum staleness.
pub fn readiness(samples: usize, rho: f64, stratum_changed: bool) -> Result<CalibrationReadiness> {
    if stratum_changed {
        return Ok(CalibrationReadiness::Stale);
    }
    let need = sample_floor(rho)?;
    if samples < need {
        return Ok(if samples == 0 {
            CalibrationReadiness::InsufficientSamples
        } else {
            CalibrationReadiness::Shadow
        });
    }
    Ok(CalibrationReadiness::Active)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples() -> Vec<CalibrationSample> {
        // Unsafe states tend to have low scores; safe states high scores.
        vec![
            CalibrationSample::new(0.1, true),
            CalibrationSample::new(0.2, true),
            CalibrationSample::new(0.3, true),
            CalibrationSample::new(0.6, false),
            CalibrationSample::new(0.7, false),
            CalibrationSample::new(0.8, false),
            CalibrationSample::new(0.9, false),
            CalibrationSample::new(0.95, false),
        ]
    }

    #[test]
    fn threshold_excludes_unsafe_low_scores() {
        // With a tight budget, theta_hat must rise above the unsafe scores.
        let theta = conformal_threshold(&samples(), 0.2).unwrap();
        assert!(
            theta >= 0.3,
            "theta={theta} should exclude unsafe scores <= 0.3"
        );
        // No unsafe sample is accepted above the threshold.
        assert_eq!(accepted_unsafe_rate(&samples(), theta), 0.0);
    }

    #[test]
    fn looser_budget_allows_lower_threshold() {
        let tight = conformal_threshold(&samples(), 0.15).unwrap();
        let loose = conformal_threshold(&samples(), 0.5).unwrap();
        assert!(loose <= tight);
    }

    #[test]
    fn impossible_budget_rejects_everything() {
        // rho smaller than 1/(n+1) cannot be met even by rejecting all.
        let theta = conformal_threshold(&samples(), 0.0).unwrap();
        assert_eq!(theta, 1.0);
    }

    #[test]
    fn rho_out_of_range_is_error() {
        assert!(conformal_threshold(&samples(), 1.5).is_err());
    }

    #[test]
    fn ks_detects_distribution_shift() {
        let calib: Vec<f64> = (0..100).map(|i| i as f64 / 100.0).collect();
        let same: Vec<f64> = (0..100).map(|i| i as f64 / 100.0).collect();
        let shifted: Vec<f64> = (0..100).map(|i| 0.5 + i as f64 / 200.0).collect();
        assert!(!is_drifted(&same, &calib, 1.36));
        assert!(is_drifted(&shifted, &calib, 1.36));
    }

    #[test]
    fn calibrated_state_asserts_bound_stale_does_not() {
        let state = CalibrationState::calibrate(&samples(), 0.2).unwrap();
        assert!(state.bound_is_asserted());
        let stale = state.mark_stale();
        assert!(!stale.bound_is_asserted());
    }

    #[test]
    fn stale_window_backs_off_to_approval_instead_of_claiming_a_bound() {
        let state = CalibrationState::calibrate(&samples(), 0.3)
            .unwrap()
            .mark_stale();
        // A stale score can still route a proposal, but cannot commit it.
        assert_eq!(
            decide(&state, 0.99, RiskClass::Low),
            AcceptOutcome::RouteToApproval
        );
        // High-risk effect routes to approval rather than halting.
        assert_eq!(
            decide(&state, 0.99, RiskClass::High),
            AcceptOutcome::RouteToApproval
        );
    }

    #[test]
    fn calibrated_state_cannot_bypass_the_finite_sample_floor() {
        let too_small = &samples()[..2];
        assert!(CalibrationState::calibrate(too_small, 0.2).is_err());
    }

    #[test]
    fn calibrated_window_certifies_accepts() {
        let state = CalibrationState::calibrate(&samples(), 0.3).unwrap();
        let threshold = match state {
            CalibrationState::Calibrated { theta_hat, .. } => theta_hat,
            _ => unreachable!(),
        };
        assert_eq!(
            decide(&state, threshold + 0.05, RiskClass::Low),
            AcceptOutcome::CertifiedAccept
        );
        assert_eq!(decide(&state, 0.0, RiskClass::Low), AcceptOutcome::Reject);
    }

    #[test]
    fn mc_q_sub_feasible_target_reports_insufficient_calibration() {
        // rho = 0.05 needs n >= 19; with 5 samples the outcome names the
        // floor rather than presenting reject-all as a satisfied budget.
        let samples: Vec<CalibrationSample> = (0..5)
            .map(|i| CalibrationSample {
                score: 0.5 + (i as f64) * 0.05,
                is_unsafe: false,
            })
            .collect();
        match conformal_threshold_checked(&samples, 0.05).unwrap() {
            ThresholdOutcome::InsufficientCalibration { have, need } => {
                assert_eq!(have, 5);
                assert_eq!(need, 19);
            }
            other => panic!("expected insufficiency, got {other:?}"),
        }
    }

    #[test]
    fn mc_q_the_sample_floor_matches_the_theorem() {
        assert_eq!(sample_floor(0.05).unwrap(), 19);
        assert_eq!(sample_floor(0.5).unwrap(), 1);
        assert!(sample_floor(0.0).is_err());
    }

    #[test]
    fn mc_q_a_score_exactly_at_the_threshold_is_rejected() {
        // Acceptance is strict: right-continuity of the strict loss is what
        // makes the infimum attained, so ties reject.
        let state = CalibrationState::Calibrated {
            theta_hat: 0.7,
            rho: 0.05,
        };
        assert_eq!(
            decide(&state, 0.7, RiskClass::Medium),
            AcceptOutcome::Reject,
            "tie at the threshold must reject"
        );
        assert_eq!(
            decide(&state, 0.7 + 1e-9, RiskClass::Medium),
            AcceptOutcome::CertifiedAccept
        );
    }

    #[test]
    fn mc_q_any_stratum_field_change_marks_stale() {
        let stratum = CalibrationStratum {
            domain_package: "coding".into(),
            domain_version: "0.6.6".into(),
            effect_kind: "apply_patch".into(),
            risk_class: "medium".into(),
            model_route: "anthropic::claude-opus-5".into(),
            verifier_suite_fingerprint: "vs1".into(),
            tool_catalog_fingerprint: "tc1".into(),
            policy_version: "p1".into(),
            score_definition: "s1".into(),
        };
        let mut failover = stratum.clone();
        failover.model_route = "openai::gpt-5.5".into();
        assert_ne!(stratum, failover, "automatic failover changes the stratum");
        assert_eq!(
            readiness(100, 0.05, stratum != failover).unwrap(),
            CalibrationReadiness::Stale
        );
    }

    #[test]
    fn readiness_progresses_from_insufficient_through_shadow_to_active() {
        assert_eq!(
            readiness(0, 0.05, false).unwrap(),
            CalibrationReadiness::InsufficientSamples
        );
        assert_eq!(
            readiness(5, 0.05, false).unwrap(),
            CalibrationReadiness::Shadow
        );
        assert_eq!(
            readiness(19, 0.05, false).unwrap(),
            CalibrationReadiness::Active
        );
    }
}
