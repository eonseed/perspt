//! SRBN kernel adapter (PSP-8 System 2 / Gate A).
//!
//! `perspt-sdk` uses the published [`srbn`] crate as the canonical SRBN kernel
//! and [`srbn_serde`] for serializing traces that enter the Perspt ledger,
//! rather than forking the kernel logic. This module is the narrow adapter that
//! keeps Perspt's WorkGraph, residual, and capability types outside the SRBN
//! crate while reusing its `stabilize` loop, `BarrierResult` contract, attempt
//! traces, and terminal statuses.
//!
//! The authoritative acceptance gate is the *measured* gate in [`crate::gate`];
//! `srbn` supplies the loop scaffolding and the serializable attempt trace.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::residual::{CorrectionDirection, ResidualEvent};
use crate::stability::StabilityParameters;

/// Deterministic string evidence map, matching [`srbn::Evidence`].
pub type Evidence = BTreeMap<String, String>;

/// A set of correction directions, the SDK's `Correction` payload.
pub type CorrectionDirectionSet = Vec<CorrectionDirection>;

/// The SDK-side barrier result (PSP-8 adapter type).
#[derive(Debug, Clone, PartialEq)]
pub struct AgentBarrierResult {
    pub ok: bool,
    /// The candidate's total energy `V` (used as the SRBN score).
    pub score: f64,
    pub residuals: Vec<ResidualEvent>,
    pub feedback: CorrectionDirectionSet,
    pub evidence: Evidence,
}

impl AgentBarrierResult {
    pub fn new(ok: bool, score: f64) -> Self {
        Self {
            ok,
            score,
            residuals: Vec::new(),
            feedback: Vec::new(),
            evidence: Evidence::new(),
        }
    }

    pub fn with_residuals(mut self, residuals: Vec<ResidualEvent>) -> Self {
        self.residuals = residuals;
        self
    }

    pub fn with_feedback(mut self, feedback: CorrectionDirectionSet) -> Self {
        self.feedback = feedback;
        self
    }

    /// Convert into the kernel's [`srbn::BarrierResult`]. The first
    /// human-readable instruction becomes the feedback string; the full
    /// correction set rides along as the typed `correction` payload.
    pub fn into_srbn(
        self,
        name: impl Into<String>,
    ) -> srbn::SrbnResult<srbn::BarrierResult<CorrectionDirectionSet, Evidence>> {
        let feedback = self
            .feedback
            .first()
            .map(|d| d.instruction.clone())
            .unwrap_or_default();
        let correction = if self.feedback.is_empty() {
            None
        } else {
            Some(self.feedback)
        };
        srbn::BarrierResult::new(
            name,
            self.ok,
            self.score,
            feedback,
            correction,
            self.evidence,
        )
    }
}

/// Terminal status of an SDK stabilization (PSP-8 adapter enum). Extends the
/// kernel's [`srbn::Status`] with SDK-specific outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStabilizationStatus {
    Stable,
    Descended,
    Stopped,
    Exhausted,
    Degraded,
    ReplanRequired,
}

impl From<srbn::Status> for AgentStabilizationStatus {
    fn from(status: srbn::Status) -> Self {
        match status {
            srbn::Status::Stable => AgentStabilizationStatus::Stable,
            srbn::Status::Stopped => AgentStabilizationStatus::Stopped,
            srbn::Status::Exhausted => AgentStabilizationStatus::Exhausted,
        }
    }
}

/// Build an [`srbn::Policy`] from SDK stability parameters.
///
/// The single descent tolerance `rho_gate` becomes the kernel's `min_descent`,
/// and the energy tolerance becomes the kernel's `score_tolerance`. Descent is
/// required and a stall stops the loop, producing a `Stopped` status that the
/// SDK maps to a residual-certificate path.
pub fn policy(params: &StabilityParameters, max_attempts: usize) -> srbn::Policy {
    srbn::Policy {
        max_attempts: max_attempts.max(1),
        score_tolerance: params.energy_tolerance,
        require_descent: true,
        on_no_descent: srbn::OnNoDescent::Stop,
        min_descent: params.rho_gate,
    }
}

/// Drive the kernel's [`srbn::stabilize`] loop over an arbitrary state, with the
/// barrier producing an [`AgentBarrierResult`] (mapped to the kernel result).
///
/// Returns the kernel's [`srbn::StabilizationResult`] so callers can serialize
/// the attempt trace through [`srbn_serde`] into the ledger.
pub fn stabilize<State, B, U>(
    initial: State,
    mut barrier: B,
    updater: U,
    params: &StabilityParameters,
    max_attempts: usize,
) -> crate::error::Result<srbn::StabilizationResult<State, CorrectionDirectionSet, Evidence>>
where
    State: Clone,
    B: FnMut(&State) -> AgentBarrierResult,
    U: FnMut(
        State,
        &srbn::BarrierResult<CorrectionDirectionSet, Evidence>,
    ) -> srbn::SrbnResult<State>,
{
    let srbn_barrier = move |state: &State| barrier(state).into_srbn("agent-barrier");
    let result = srbn::stabilize(initial, srbn_barrier, updater, policy(params, max_attempts))?;
    Ok(result)
}

/// Serialize a kernel stabilization trace to JSON via [`srbn_serde`] for the
/// ledger. Requires the state and evidence to be serializable.
pub fn trace_to_json<State>(
    result: &srbn::StabilizationResult<State, CorrectionDirectionSet, Evidence>,
) -> crate::error::Result<String>
where
    State: Serialize,
{
    srbn_serde::stabilization_result_json(result)
        .map_err(|e| crate::error::SdkError::Kernel(format!("trace serialization failed: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_kernel_status() {
        assert_eq!(
            AgentStabilizationStatus::from(srbn::Status::Stable),
            AgentStabilizationStatus::Stable
        );
        assert_eq!(
            AgentStabilizationStatus::from(srbn::Status::Stopped),
            AgentStabilizationStatus::Stopped
        );
        assert_eq!(
            AgentStabilizationStatus::from(srbn::Status::Exhausted),
            AgentStabilizationStatus::Exhausted
        );
    }

    #[test]
    fn stabilizes_descending_energy_through_kernel() {
        // State is an integer "distance"; energy V = distance^2. Each step moves
        // one toward zero. The kernel should reach a stable (zero-energy) state.
        let params = StabilityParameters::measured(0.5, 0.0);
        let barrier = |state: &i64| {
            let v = (*state as f64) * (*state as f64);
            AgentBarrierResult::new(*state == 0, v)
        };
        let updater = |state: i64, _b: &srbn::BarrierResult<CorrectionDirectionSet, Evidence>| {
            Ok(state - state.signum())
        };
        let result = stabilize(3, barrier, updater, &params, 10).unwrap();
        assert_eq!(result.status, srbn::Status::Stable);
        assert_eq!(result.state, 0);

        // Trace serializes through srbn-serde for the ledger.
        let json = trace_to_json(&result).unwrap();
        assert!(json.contains("\"status\":\"stable\""));
        assert!(json.contains("\"attempts\""));
    }

    #[test]
    fn rejects_non_finite_score_at_barrier() {
        let params = StabilityParameters::measured(0.5, 0.0);
        let barrier = |_state: &i64| AgentBarrierResult::new(false, f64::NAN);
        let updater =
            |state: i64, _b: &srbn::BarrierResult<CorrectionDirectionSet, Evidence>| Ok(state);
        // The kernel validates scores and surfaces an error.
        assert!(stabilize(1, barrier, updater, &params, 3).is_err());
    }
}
