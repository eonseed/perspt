//! The bounded search forest (PSP-10 system 19, Definitions 2–3).
//!
//! `AcceptedTrajectory` remains the sole owner of accepted state; a forest
//! cannot mutate it. Branch previews use the pure gate evaluator; after
//! selection the runtime submits exactly one candidate through the
//! ordinary trajectory method. Internal branch states are never promoted;
//! a partial checkpoint is private search state, neither accepted nor
//! evidence of progress.

use serde::{Deserialize, Serialize};

use super::budget::{SearchLimits, SearchUsage};
use super::domain_types::{BranchMeasurement, SearchStrategy};
use crate::model::ModelId;

/// A witness that one content-addressed state descends from another
/// through the recorded branch and partial-checkpoint chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WitnessRef {
    /// The accepted root the chain must reach.
    pub accepted_root: String,
    /// State roots from the seed back to the accepted root, oldest last.
    pub chain: Vec<String>,
}

impl WitnessRef {
    /// A root branch's trivial witness: the seed is the accepted root.
    pub fn root(accepted_root: impl Into<String>) -> Self {
        let accepted_root = accepted_root.into();
        Self {
            chain: vec![accepted_root.clone()],
            accepted_root,
        }
    }

    /// Whether the chain reaches the given accepted root.
    pub fn reaches(&self, accepted_root: &str) -> bool {
        self.accepted_root == accepted_root
            && self.chain.last().map(String::as_str) == Some(accepted_root)
    }

    /// Extend the chain with a child checkpoint root.
    pub fn extend(&self, child_root: impl Into<String>) -> Self {
        let mut chain = vec![child_root.into()];
        chain.extend(self.chain.iter().cloned());
        Self {
            accepted_root: self.accepted_root.clone(),
            chain,
        }
    }
}

/// A typed obligation reference carried by partial checkpoints and pages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObligationRef(pub String);

/// The digest of a compiled prompt program a branch runs under.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PromptProgramDigest(pub String);

/// A private, content-addressed partial checkpoint (Definition 2). Never
/// an accepted state; a child branch may seed from it only through a
/// witness chain that reaches the forest's accepted root.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartialCheckpointRef {
    pub state_root: String,
    pub accepted_ancestor: String,
    pub parent_witness: WitnessRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correction: Option<crate::residual::CorrectionPacketRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remaining_obligations: Vec<ObligationRef>,
    pub evidence_digest: String,
}

/// A branch's lifecycle state (system 19's diagram).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum SearchBranchState {
    Ready,
    Running,
    PartialCheckpointed { checkpoint: PartialCheckpointRef },
    CandidateMeasured { measurement: BranchMeasurement },
    Selected,
    Rejected { reason: String },
    Abandoned { reason: String },
    Contained { certificate_id: String },
}

/// One isolated branch of the forest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchBranch {
    pub branch_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_branch: Option<String>,
    /// Must equal the forest's accepted root.
    pub accepted_ancestor: String,
    /// The accepted root, or a parent's partial-checkpoint root.
    pub seed_checkpoint: String,
    /// Proof the seed descends from the accepted ancestor.
    pub seed_witness: WitnessRef,
    pub strategy: SearchStrategy,
    pub route: ModelId,
    pub prompt_program: PromptProgramDigest,
    pub state: SearchBranchState,
    pub usage: SearchUsage,
}

/// The forest rooted at one accepted state (Definition 2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchForest {
    pub forest_id: String,
    pub task_id: String,
    pub node_id: String,
    pub generation: u32,
    /// State root of the immutable accepted root `x_k`.
    pub accepted_root: String,
    pub branches: Vec<SearchBranch>,
    pub limits: SearchLimits,
    pub usage: SearchUsage,
}

impl SearchForest {
    /// Validate the structural invariants: every branch's accepted ancestor
    /// is the forest root and every seed witness reaches it.
    pub fn validate(&self) -> crate::error::Result<()> {
        for branch in &self.branches {
            if branch.accepted_ancestor != self.accepted_root {
                return Err(crate::error::SdkError::Domain(format!(
                    "branch {} does not share the forest's accepted root",
                    branch.branch_id
                )));
            }
            if !branch.seed_witness.reaches(&self.accepted_root) {
                return Err(crate::error::SdkError::Domain(format!(
                    "branch {} seed witness does not reach the accepted root",
                    branch.branch_id
                )));
            }
        }
        Ok(())
    }
}

/// Bounded cross-branch context (system 21). Never another branch's raw
/// conversation.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SearchEvidenceDigest {
    pub residual_clusters: Vec<String>,
    pub no_good_refs: Vec<String>,
    pub tried_strategies: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn witness_chains_reach_or_are_refused() {
        let root = WitnessRef::root("root-a");
        assert!(root.reaches("root-a"));
        assert!(!root.reaches("root-b"));
        let child = root.extend("partial-1");
        assert!(child.reaches("root-a"));
        assert_eq!(child.chain, vec!["partial-1", "root-a"]);
        let broken = WitnessRef {
            accepted_root: "root-a".into(),
            chain: vec!["partial-1".into()],
        };
        assert!(!broken.reaches("root-a"), "chain must end at the root");
    }
}
