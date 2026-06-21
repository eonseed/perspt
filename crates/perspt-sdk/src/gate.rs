//! Measured acceptance gate and finite-decision bound (PSP-8 System 2).
//!
//! The gate is the harness paper's measured discrete contract and applies even
//! when the continuous analytic constants are unavailable:
//!
//! ```text
//! accept(y) <=> hard(y) OR V(y) <= V(x_best) - rho_gate.
//! ```
//!
//! Descent is measured against the *best* accepted energy `V(x_best)`, not the
//! most recent. There is a single descent tolerance `rho_gate > 0`. With
//! `V >= 0`, baseline `V_0`, and rejection budget `B`, the run terminates
//! within
//!
//! ```text
//! N_gate <= floor(V_0 / rho_gate) + B + 1
//! ```
//!
//! gate decisions.

use serde::{Deserialize, Serialize};

use crate::error::{check_positive_finite, Result, SdkError};

/// Outcome of evaluating one candidate against the gate (PSP-8 `GateDecision`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GateDecision {
    /// All required verifiers and hard constraints passed.
    HardPass,
    /// Energy descended below the best accepted energy by at least `rho_gate`.
    AcceptedByDescent { delta_v: f64 },
    /// Candidate did not descend enough; retained as an observation only.
    RejectedNonDescending { delta_v: f64 },
    /// Stopped at a declared analytic floor.
    StoppedAtDeclaredFloor,
    /// Correction budget exhausted; a residual certificate was issued.
    ExhaustedWithCertificate { certificate_id: String },
}

impl GateDecision {
    /// Whether this decision admits the candidate into the accepted trajectory.
    pub fn is_accepted(&self) -> bool {
        matches!(self, GateDecision::HardPass | GateDecision::AcceptedByDescent { .. })
    }
}

/// Evaluate the measured acceptance gate for a candidate.
///
/// * `hard_pass` — all required verifiers and hard policy constraints passed.
/// * `candidate_v` — the candidate's total energy `V(y)`.
/// * `best_accepted_v` — the best accepted energy `V(x_best)` so far.
/// * `rho_gate` — the single descent tolerance (`> 0`).
pub fn evaluate_gate(
    hard_pass: bool,
    candidate_v: f64,
    best_accepted_v: f64,
    rho_gate: f64,
) -> Result<GateDecision> {
    check_positive_finite(rho_gate, "rho_gate")?;
    crate::error::check_non_negative_finite(candidate_v, "candidate energy")?;
    crate::error::check_non_negative_finite(best_accepted_v, "best accepted energy")?;

    if hard_pass {
        return Ok(GateDecision::HardPass);
    }
    let delta_v = best_accepted_v - candidate_v;
    if candidate_v <= best_accepted_v - rho_gate {
        Ok(GateDecision::AcceptedByDescent { delta_v })
    } else {
        Ok(GateDecision::RejectedNonDescending { delta_v })
    }
}

/// Finite-decision bound `floor(V_0 / rho_gate) + B + 1` (PSP-8 System 2).
pub fn finite_decision_bound(baseline_energy: f64, rho_gate: f64, rejection_budget: u32) -> Result<u64> {
    check_positive_finite(rho_gate, "rho_gate")?;
    crate::error::check_non_negative_finite(baseline_energy, "baseline energy")?;
    let descents = (baseline_energy / rho_gate).floor();
    if !descents.is_finite() {
        return Err(SdkError::InvalidGate("finite-decision bound overflow".into()));
    }
    Ok(descents as u64 + rejection_budget as u64 + 1)
}

/// A reference to a recorded gate decision (PSP-8 `GateDecisionRef`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateDecisionRef {
    pub decision: GateDecision,
    pub observed_energy: f64,
    pub best_accepted_before: f64,
}

/// Accepted-trajectory record for one node generation (PSP-8 System 2 / 11).
///
/// The accepted trajectory contains only hard-pass or descent-gated states;
/// observed candidates that fail the gate are retained as observations but
/// never advance accepted progress.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcceptedTrajectory {
    pub node_id: String,
    pub generation: u32,
    pub baseline_energy: f64,
    /// The best accepted energy `V(x_best)` so far.
    pub best_accepted_energy: f64,
    pub rho_gate: f64,
    /// Finite rejection budget `B`.
    pub rejection_budget: u32,
    pub rejections_used: u32,
    pub gate_decisions: Vec<GateDecisionRef>,
}

impl AcceptedTrajectory {
    pub fn new(
        node_id: impl Into<String>,
        generation: u32,
        baseline_energy: f64,
        rho_gate: f64,
        rejection_budget: u32,
    ) -> Result<Self> {
        check_positive_finite(rho_gate, "rho_gate")?;
        crate::error::check_non_negative_finite(baseline_energy, "baseline energy")?;
        Ok(Self {
            node_id: node_id.into(),
            generation,
            baseline_energy,
            best_accepted_energy: baseline_energy,
            rho_gate,
            rejection_budget,
            rejections_used: 0,
            gate_decisions: Vec::new(),
        })
    }

    /// The finite-decision bound for this trajectory.
    pub fn decision_bound(&self) -> Result<u64> {
        finite_decision_bound(self.baseline_energy, self.rho_gate, self.rejection_budget)
    }

    /// Evaluate a candidate and fold the decision into the trajectory, updating
    /// the best accepted energy on acceptance and the rejection count on
    /// rejection. Returns the decision taken.
    pub fn submit(&mut self, hard_pass: bool, candidate_v: f64) -> Result<GateDecision> {
        let decision = evaluate_gate(hard_pass, candidate_v, self.best_accepted_energy, self.rho_gate)?;
        self.gate_decisions.push(GateDecisionRef {
            decision: decision.clone(),
            observed_energy: candidate_v,
            best_accepted_before: self.best_accepted_energy,
        });
        if decision.is_accepted() {
            if candidate_v < self.best_accepted_energy {
                self.best_accepted_energy = candidate_v;
            }
        } else if matches!(decision, GateDecision::RejectedNonDescending { .. }) {
            self.rejections_used += 1;
        }
        Ok(decision)
    }

    /// Whether the rejection budget is exhausted.
    pub fn budget_exhausted(&self) -> bool {
        self.rejections_used >= self.rejection_budget
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_pass_is_accepted() {
        let d = evaluate_gate(true, 100.0, 0.0, 0.5).unwrap();
        assert_eq!(d, GateDecision::HardPass);
        assert!(d.is_accepted());
    }

    #[test]
    fn descent_below_best_minus_rho_accepts() {
        // best=10, candidate=9.4, rho=0.5 -> 9.4 <= 9.5 accept
        let d = evaluate_gate(false, 9.4, 10.0, 0.5).unwrap();
        assert!(matches!(d, GateDecision::AcceptedByDescent { .. }));
        assert!(d.is_accepted());
    }

    #[test]
    fn insufficient_descent_rejected() {
        // best=10, candidate=9.6, rho=0.5 -> 9.6 > 9.5 reject
        let d = evaluate_gate(false, 9.6, 10.0, 0.5).unwrap();
        assert!(matches!(d, GateDecision::RejectedNonDescending { .. }));
        assert!(!d.is_accepted());
    }

    #[test]
    fn descent_measured_against_best_not_latest() {
        let mut traj = AcceptedTrajectory::new("n1", 0, 10.0, 0.5, 8).unwrap();
        // Descend to 5.0 (accept), best = 5.0.
        assert!(traj.submit(false, 5.0).unwrap().is_accepted());
        assert_eq!(traj.best_accepted_energy, 5.0);
        // Candidate 5.2 would descend vs *latest if latest were higher*, but
        // best is 5.0 so 5.2 > 5.0 - 0.5 => rejected.
        assert!(!traj.submit(false, 5.2).unwrap().is_accepted());
        assert_eq!(traj.best_accepted_energy, 5.0);
    }

    #[test]
    fn finite_decision_bound_formula() {
        // floor(10 / 0.5) + 3 + 1 = 20 + 4 = 24
        assert_eq!(finite_decision_bound(10.0, 0.5, 3).unwrap(), 24);
    }

    #[test]
    fn rho_gate_must_be_positive() {
        assert!(evaluate_gate(false, 1.0, 2.0, 0.0).is_err());
        assert!(finite_decision_bound(10.0, 0.0, 1).is_err());
    }

    #[test]
    fn trajectory_terminates_within_bound() {
        // Worst case: a stream of non-descending candidates exhausts B, and a
        // descending stream is bounded by floor(V0/rho)+1.
        let mut traj = AcceptedTrajectory::new("n1", 0, 5.0, 1.0, 3).unwrap();
        let bound = traj.decision_bound().unwrap(); // floor(5)+3+1 = 9
        let mut decisions = 0u64;
        let mut v: f64 = 5.0;
        // Descend one rho per step until zero.
        while v > 0.0 {
            v = (v - 1.0).max(0.0);
            traj.submit(false, v).unwrap();
            decisions += 1;
        }
        assert!(decisions <= bound);
    }
}
