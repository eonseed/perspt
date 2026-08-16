//! The five-clause admissibility kernel (PSP-9 system 12, Paper III
//! Definition 3.2).
//!
//! The legacy `check_admissibility` is a useful scope checker, but it
//! hardcodes `contract_ok` and `barrier_increment_ok` to true and equates a
//! caller-supplied `risk_cost` with the debit. This module replaces that
//! shortcut: contract and barrier clauses come from registered evaluators,
//! and **the barrier allowance and the risk debit are the same number** —
//! `BarrierWitness::certified_increment` is Definition 3.2's `c_c` in both
//! numeric clauses, so a budget can never be debited for an increment that
//! was never certified.
//!
//! **No durable effect may occur without a complete witness; an absent
//! clause is recorded as absent, never as true.** Autonomous `SrbnCertified`
//! commitment requires all five clauses; `ConfinementOnly` and
//! `HumanApproved` activate only the named authority and deterministic
//! contract claims and never Theorems 3–5.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::capability::{
    check_admissibility, AdmissibilityDecision, AdmissibilityWitness, Capability, EffectProposal,
    KernelState,
};
use crate::error::{Result, SdkError};

/// A content-addressed, provider-neutral witness of one candidate state.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CandidateStateWitness {
    pub state_root: String,
    pub graph_revision: String,
    pub node_id: String,
    pub node_generation: u32,
    pub canonical_scope: Vec<String>,
    /// Versioned operational barrier channel values measured in this state.
    pub barrier_channels: BTreeMap<String, f64>,
}

/// A fully materialized candidate transition `Adm(x, p, x')`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateTransition {
    pub proposal: EffectProposal,
    pub before: CandidateStateWitness,
    pub after: CandidateStateWitness,
}

impl CandidateTransition {
    pub fn new(
        proposal: EffectProposal,
        before: CandidateStateWitness,
        after: CandidateStateWitness,
    ) -> Self {
        Self {
            proposal,
            before,
            after,
        }
    }

    /// Test and read-only helper for transitions without a workspace driver.
    pub fn unmeasured(proposal: EffectProposal) -> Self {
        Self::new(
            proposal,
            CandidateStateWitness::default(),
            CandidateStateWitness::default(),
        )
    }
}

/// The five clauses of Definition 3.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClauseId {
    Authority,
    Contract,
    Effect,
    BarrierIncrement,
    RiskBudget,
}

/// Contract-clause verdict from a registered evaluator (`perspt-policy` is
/// invoked inside the runtime's implementation).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractWitness {
    pub ok: bool,
    pub policy_version: String,
    pub evidence_refs: Vec<String>,
}

/// Barrier-clause verdict (Paper III Def. 3.2 / Theorem 4).
///
/// For a fully materialized deterministic candidate the certified increment
/// is the exact `c_t = max(0, h(x') − h(x))`; the kernel requires
/// `h(x') < 1`, `c_t ≤ c_c_max`, and debits exactly `c_t`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BarrierWitness {
    pub h_before: f64,
    pub expected_h_after_upper: f64,
    /// The invocation increment `c_t`, bounded by the capability's `c_c^max`.
    pub certified_increment: f64,
    /// `1` in Paper III.
    pub unsafe_threshold: f64,
    pub evidence_refs: Vec<String>,
}

impl BarrierWitness {
    /// Whether the numeric barrier clause holds.
    pub fn clause_holds(&self, c_c_max: f64) -> bool {
        self.h_before.is_finite()
            && self.expected_h_after_upper.is_finite()
            && self.certified_increment.is_finite()
            && self.unsafe_threshold.is_finite()
            && c_c_max.is_finite()
            && self.h_before >= 0.0
            && self.expected_h_after_upper >= 0.0
            && self.unsafe_threshold > 0.0
            && c_c_max >= 0.0
            && self.h_before < self.unsafe_threshold
            && self.expected_h_after_upper < self.unsafe_threshold
            && self.expected_h_after_upper <= self.h_before + self.certified_increment
            && self.certified_increment >= 0.0
            && self.certified_increment <= c_c_max
    }
}

/// Evaluates the contract clause for a candidate transition.
pub trait ContractEvaluator: Send + Sync {
    fn evaluate(&self, transition: &CandidateTransition) -> ContractWitness;
}

/// Evaluates the barrier clause for a candidate transition.
pub trait BarrierEvaluator: Send + Sync {
    fn evaluate(&self, transition: &CandidateTransition) -> Result<BarrierWitness>;
}

/// Which claims the completed witness activates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "profile")]
pub enum AdmissibilityProfile {
    /// All five clauses of Definition 3.2 are satisfied.
    SrbnCertified,
    /// Authority/contract/effect hold, but chance safety is not claimed.
    ConfinementOnly { missing: Vec<ClauseId> },
    /// Human authorization does not manufacture missing theorem evidence.
    HumanApproved { missing: Vec<ClauseId> },
}

/// The complete Definition 3.2 witness.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FullAdmissibilityWitness {
    pub base: AdmissibilityWitness,
    pub contract: Option<ContractWitness>,
    pub barrier: Option<BarrierWitness>,
    pub profile: AdmissibilityProfile,
}

impl FullAdmissibilityWitness {
    /// Whether the decision permits applying the effect at all.
    pub fn allows(&self) -> bool {
        matches!(self.base.decision, AdmissibilityDecision::Allow)
    }
}

/// Evaluate all five clauses.
///
/// `c_c_max` is the capability contract's ceiling on a single certified
/// increment. An absent evaluator leaves its clause **absent** — recorded as
/// missing in the profile, never defaulted to true (Gate K).
pub fn check_full_admissibility(
    transition: &CandidateTransition,
    capabilities: &[Capability],
    state: &KernelState,
    contract: Option<&dyn ContractEvaluator>,
    barrier: Option<&dyn BarrierEvaluator>,
    c_c_max: f64,
) -> Result<FullAdmissibilityWitness> {
    let proposal = &transition.proposal;
    let mut base = check_admissibility(proposal, capabilities, state);
    // The legacy checker hardcodes these true; they are decided here.
    base.contract_ok = false;
    base.barrier_increment_ok = false;

    let mut missing: Vec<ClauseId> = Vec::new();

    let contract_witness = match contract {
        Some(evaluator) => {
            let witness = evaluator.evaluate(transition);
            base.contract_ok = witness.ok;
            if !witness.ok {
                base.decision = AdmissibilityDecision::Deny {
                    reason: crate::capability::DenyReason::PolicyDenied,
                };
            }
            Some(witness)
        }
        None => {
            missing.push(ClauseId::Contract);
            None
        }
    };

    let barrier_witness = match barrier {
        Some(evaluator) => Some(evaluate_barrier_clauses(
            evaluator,
            transition,
            capabilities,
            c_c_max,
            &mut base,
        )?),
        None => {
            missing.push(ClauseId::BarrierIncrement);
            // Without c_c the risk clause cannot be evaluated. Recording it as
            // true would create a certificate with an unbound budget debit.
            missing.push(ClauseId::RiskBudget);
            base.risk_budget_ok = false;
            None
        }
    };

    let profile = if missing.is_empty()
        && base.authority_ok
        && base.contract_ok
        && base.effect_ok
        && base.barrier_increment_ok
        && base.risk_budget_ok
    {
        AdmissibilityProfile::SrbnCertified
    } else {
        AdmissibilityProfile::ConfinementOnly { missing }
    };

    Ok(FullAdmissibilityWitness {
        base,
        contract: contract_witness,
        barrier: barrier_witness,
        profile,
    })
}

/// Evaluate the barrier-increment and risk-budget clauses against one
/// witness. Definition 3.2 has one `c_c`: the barrier's certified increment
/// is also the amount tested against the cumulative risk budget (the legacy
/// scope checker sees `proposal.risk_cost`, always zero on the governed
/// path, so its risk verdict is replaced here).
fn evaluate_barrier_clauses(
    evaluator: &dyn BarrierEvaluator,
    transition: &CandidateTransition,
    capabilities: &[Capability],
    c_c_max: f64,
    base: &mut AdmissibilityWitness,
) -> Result<BarrierWitness> {
    let witness = evaluator.evaluate(transition)?;
    base.barrier_increment_ok = witness.clause_holds(c_c_max);
    if !base.barrier_increment_ok {
        base.decision = AdmissibilityDecision::Deny {
            reason: crate::capability::DenyReason::RiskBudgetExhausted,
        };
    }
    base.risk_budget_ok = base
        .capability_id
        .as_ref()
        .and_then(|id| capabilities.iter().find(|cap| &cap.capability_id == id))
        .map(|cap| {
            cap.risk_budget
                .as_ref()
                .is_none_or(|budget| budget.admits(witness.certified_increment))
        })
        .unwrap_or(false);
    if !base.risk_budget_ok {
        base.decision = AdmissibilityDecision::Deny {
            reason: crate::capability::DenyReason::RiskBudgetExhausted,
        };
    }
    Ok(witness)
}

/// Promote an allowed transition: one kernel transaction that decrements the
/// call budget, debits the risk budget with **exactly the certified
/// increment**, and records the spend (PSP-9 system 12).
///
/// Spending is monotone: later barrier decrease does not refund budget.
/// A read-only `check` that never consumes budgets is not sufficient.
pub fn promote(capability: &mut Capability, witness: &FullAdmissibilityWitness) -> Result<()> {
    if !witness.allows() {
        return Err(SdkError::Domain(
            "promotion requires an Allow decision".into(),
        ));
    }
    if witness.profile != AdmissibilityProfile::SrbnCertified {
        return Err(SdkError::Domain(
            "certified promotion requires all five admissibility clauses".into(),
        ));
    }
    // The debit is the barrier's certified increment — Definition 3.2 has one
    // number, not two. Without a barrier witness nothing is certified and
    // nothing is debited (the profile already records the clause as absent).
    let debit = witness
        .barrier
        .as_ref()
        .map(|b| b.certified_increment)
        .unwrap_or(0.0);

    if let Some(budget) = capability.risk_budget.as_ref() {
        if !budget.admits(debit) {
            return Err(SdkError::Domain(format!(
                "risk budget exhausted: spent {} + c_t {debit} exceeds {}",
                budget.spent, budget.limit
            )));
        }
    }
    if capability.max_calls == Some(0) {
        return Err(SdkError::Domain("call budget exhausted".into()));
    }

    // All checks passed: apply the whole transaction.
    if let Some(calls) = capability.max_calls.as_mut() {
        *calls -= 1;
    }
    if let Some(budget) = capability.risk_budget.as_mut() {
        budget.spent += debit;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{ActorId, EffectKind, RiskBudget};

    struct PassContract;
    impl ContractEvaluator for PassContract {
        fn evaluate(&self, _transition: &CandidateTransition) -> ContractWitness {
            ContractWitness {
                ok: true,
                policy_version: "1".into(),
                evidence_refs: vec!["policy-run".into()],
            }
        }
    }

    /// A deterministic barrier whose increment is fixed.
    struct FixedBarrier {
        c_t: f64,
    }
    impl BarrierEvaluator for FixedBarrier {
        fn evaluate(&self, _transition: &CandidateTransition) -> Result<BarrierWitness> {
            Ok(BarrierWitness {
                h_before: 0.0,
                expected_h_after_upper: self.c_t,
                certified_increment: self.c_t,
                unsafe_threshold: 1.0,
                evidence_refs: vec!["barrier-eval".into()],
            })
        }
    }

    fn setup() -> (EffectProposal, Vec<Capability>, KernelState) {
        let actor = ActorId::new("worker");
        let mut capability = Capability::new(actor.clone(), vec![EffectKind::ApplyPatch]);
        capability.max_calls = Some(2);
        capability.risk_budget = Some(RiskBudget {
            name: "workspace".into(),
            limit: 0.5,
            spent: 0.0,
        });
        let proposal = EffectProposal::new(actor, "n1", EffectKind::ApplyPatch);
        (proposal, vec![capability], KernelState::new())
    }

    fn transition(proposal: &EffectProposal) -> CandidateTransition {
        CandidateTransition::unmeasured(proposal.clone())
    }

    #[test]
    fn all_five_clauses_yield_srbn_certified() {
        let (proposal, caps, state) = setup();
        let witness = check_full_admissibility(
            &transition(&proposal),
            &caps,
            &state,
            Some(&PassContract),
            Some(&FixedBarrier { c_t: 0.1 }),
            0.2,
        )
        .unwrap();
        assert!(witness.allows());
        assert_eq!(witness.profile, AdmissibilityProfile::SrbnCertified);
    }

    #[test]
    fn an_absent_barrier_evaluator_is_recorded_absent_never_true() {
        let (proposal, caps, state) = setup();
        let witness = check_full_admissibility(
            &transition(&proposal),
            &caps,
            &state,
            Some(&PassContract),
            None,
            0.2,
        )
        .unwrap();
        assert!(!witness.base.barrier_increment_ok, "absent is not true");
        assert!(matches!(
            &witness.profile,
            AdmissibilityProfile::ConfinementOnly { missing }
                if missing.contains(&ClauseId::BarrierIncrement)
        ));
    }

    #[test]
    fn an_increment_above_the_ceiling_denies() {
        let (proposal, caps, state) = setup();
        let witness = check_full_admissibility(
            &transition(&proposal),
            &caps,
            &state,
            Some(&PassContract),
            Some(&FixedBarrier { c_t: 0.4 }),
            0.2, // ceiling below the certified increment
        )
        .unwrap();
        assert!(!witness.allows());
    }

    #[test]
    fn promotion_debits_exactly_the_certified_increment() {
        let (proposal, mut caps, state) = setup();
        let witness = check_full_admissibility(
            &transition(&proposal),
            &caps,
            &state,
            Some(&PassContract),
            Some(&FixedBarrier { c_t: 0.1 }),
            0.2,
        )
        .unwrap();
        promote(&mut caps[0], &witness).unwrap();
        assert_eq!(caps[0].max_calls, Some(1));
        let budget = caps[0].risk_budget.as_ref().unwrap();
        assert!(
            (budget.spent - 0.1).abs() < 1e-12,
            "debit is c_t, nothing else"
        );
    }

    #[test]
    fn spending_is_monotone_and_finances_no_oscillation() {
        let (proposal, mut caps, state) = setup();
        for _ in 0..2 {
            let witness = check_full_admissibility(
                &transition(&proposal),
                &caps,
                &state,
                Some(&PassContract),
                Some(&FixedBarrier { c_t: 0.2 }),
                0.3,
            )
            .unwrap();
            promote(&mut caps[0], &witness).unwrap();
        }
        // Budget 0.5, two promotions of 0.2: a third would need 0.2 more.
        let witness = check_full_admissibility(
            &transition(&proposal),
            &caps,
            &state,
            Some(&PassContract),
            Some(&FixedBarrier { c_t: 0.2 }),
            0.3,
        )
        .unwrap();
        assert!(!witness.base.risk_budget_ok);
        assert!(!witness.allows());
        assert_ne!(witness.profile, AdmissibilityProfile::SrbnCertified);
        assert!(promote(&mut caps[0], &witness).is_err(), "0.4 + 0.2 > 0.5");
    }

    #[test]
    fn missing_barrier_cannot_be_promoted_as_a_zero_cost_fallback() {
        let (proposal, mut caps, state) = setup();
        let witness = check_full_admissibility(
            &transition(&proposal),
            &caps,
            &state,
            Some(&PassContract),
            None,
            0.2,
        )
        .unwrap();
        assert!(
            witness.allows(),
            "scope policy alone still permits the proposal"
        );
        assert!(!witness.base.risk_budget_ok);
        assert!(promote(&mut caps[0], &witness).is_err());
    }
}
