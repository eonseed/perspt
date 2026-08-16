//! Heterogeneous proposal ensembles (PSP-9 system 7).
//!
//! An exploration policy, **not** Corollary 5.2: proposers do not form a
//! conjunctive validator gate, and proposer diversity has no certified risk
//! interpretation. The rules that keep this an SRBN mechanism rather than a
//! lottery:
//!
//! * **Ensembles propose; they never decide.** Every candidate is scored by
//!   the same deterministic verifier suite and admitted only through the
//!   ordinary gate.
//! * **Each candidate costs one gate decision** (Paper II Lemma 1). The
//!   bound is not weakened, and the loop cannot "try more models" past it.
//! * **No majority voting.** Selection is by measured energy — a
//!   compiler-and-test quantity, not a popularity one.
//! * **Distinct family required by default.** Two samples of one model at
//!   different temperatures are a re-roll, not a heterogeneous proposal set.

use serde::{Deserialize, Serialize};

use super::family::ModelFamily;
use super::id::ModelId;
use crate::error::{Result, SdkError};

/// When an ensemble round may be drawn at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnsembleTrigger {
    /// Only for nodes that have already failed a gate decision.
    AfterGateFailure,
    /// Never (the default posture; ensembles are opt-in).
    Never,
}

/// The ensemble policy block (`[ensemble]` in configuration; default off).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnsemblePolicy {
    pub trigger: EnsembleTrigger,
    /// Candidates per round. Hard maximum 4.
    pub width: u8,
    pub require_distinct_family: bool,
}

impl Default for EnsemblePolicy {
    fn default() -> Self {
        Self {
            trigger: EnsembleTrigger::Never,
            width: 2,
            require_distinct_family: true,
        }
    }
}

impl EnsemblePolicy {
    /// Hard maximum ensemble width.
    pub const MAX_WIDTH: u8 = 4;

    /// Select the proposer routes for one ensemble round.
    ///
    /// Fails when the policy requires distinct families and the candidate
    /// routes cannot supply them: the round is refused rather than silently
    /// degraded to a re-roll of one lineage.
    pub fn select_round(
        &self,
        candidates: &[ModelId],
        family_of: &dyn Fn(&ModelId) -> ModelFamily,
        gate_decisions_remaining: u64,
    ) -> Result<Vec<ModelId>> {
        if self.trigger == EnsembleTrigger::Never {
            return Err(SdkError::Domain("ensembles are disabled by policy".into()));
        }
        let width = self.width.min(Self::MAX_WIDTH) as usize;
        if width == 0 {
            return Err(SdkError::Domain("ensemble width must be at least 1".into()));
        }
        // Each candidate consumes one Lemma 1 gate decision: a round wider
        // than the remaining budget is refused, never truncated silently.
        if (width as u64) > gate_decisions_remaining {
            return Err(SdkError::Domain(format!(
                "ensemble width {width} exceeds the {gate_decisions_remaining} remaining \
                 gate decisions"
            )));
        }

        let mut selected: Vec<ModelId> = Vec::new();
        for candidate in candidates {
            if selected.len() == width {
                break;
            }
            let family = family_of(candidate);
            let duplicate_family = selected
                .iter()
                .any(|s| !family_of(s).is_distinct_from(&family));
            if self.require_distinct_family && duplicate_family {
                continue;
            }
            if selected.contains(candidate) {
                continue;
            }
            selected.push(candidate.clone());
        }

        if selected.len() < width {
            return Err(SdkError::Domain(format!(
                "portfolio cannot supply {width} distinct-family proposers \
                 (found {})",
                selected.len()
            )));
        }
        Ok(selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn by_name(m: &ModelId) -> ModelFamily {
        ModelFamily::from_model_name(&m.model)
    }

    fn portfolio() -> Vec<ModelId> {
        vec![
            ModelId::new("anthropic", "claude-opus-5"),
            ModelId::new("openai", "gpt-5.5"),
            ModelId::new("groq", "llama-3.3-70b"),
            ModelId::new("local", "llama3.3:70b"),
        ]
    }

    fn enabled() -> EnsemblePolicy {
        EnsemblePolicy {
            trigger: EnsembleTrigger::AfterGateFailure,
            ..EnsemblePolicy::default()
        }
    }

    #[test]
    fn disabled_policy_refuses_rounds() {
        let policy = EnsemblePolicy::default();
        assert!(policy.select_round(&portfolio(), &by_name, 10).is_err());
    }

    #[test]
    fn a_round_never_exceeds_the_remaining_gate_budget() {
        let policy = enabled();
        assert!(policy.select_round(&portfolio(), &by_name, 1).is_err());
        assert!(policy.select_round(&portfolio(), &by_name, 2).is_ok());
    }

    #[test]
    fn same_lineage_on_two_hosts_is_one_proposer_not_two() {
        // Llama on groq and Llama on ollama are the same lineage: with
        // distinct-family required and width 2, the round uses claude+gpt or
        // one llama — never both llamas.
        let policy = enabled();
        let round = policy.select_round(&portfolio(), &by_name, 10).unwrap();
        let families: Vec<ModelFamily> = round.iter().map(by_name).collect();
        assert_eq!(round.len(), 2);
        assert!(families[0].is_distinct_from(&families[1]));
    }

    #[test]
    fn an_all_one_family_portfolio_refuses_rather_than_rerolls() {
        let policy = enabled();
        let clones = vec![
            ModelId::new("groq", "llama-3.3-70b"),
            ModelId::new("local", "llama3.3:70b"),
        ];
        assert!(policy.select_round(&clones, &by_name, 10).is_err());
    }

    #[test]
    fn width_is_capped_at_the_hard_maximum() {
        let mut policy = enabled();
        policy.width = 200;
        policy.require_distinct_family = false;
        let round = policy.select_round(&portfolio(), &by_name, 100).unwrap();
        assert!(round.len() <= EnsemblePolicy::MAX_WIDTH as usize);
    }
}
