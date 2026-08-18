//! Issue-conditioned prompt plans (PSP-10 system 23).
//!
//! Perspt does not ask a model to write its next system prompt: a reviewed,
//! exhaustive policy rule maps typed issue state to an ordered set of
//! compiled, active section ids. Evidence values stay delimited data; they
//! cannot add instructions, effects, tools, or capabilities.

use serde::{Deserialize, Serialize};

use crate::residual::{CorrectionPacketRef, ResidualClass};

use super::section::PromptSectionId;
use super::stage::PromptStage;

/// A concrete verifier witness (failing input, test, or trace). Only a
/// sound verifier's concrete artifact earns this name; an ordinary
/// diagnostic is correction evidence, never mislabeled a counterexample.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CounterexampleRef {
    /// Content address of the recorded witness artifact.
    pub artifact: String,
    /// The producing verifier's identity.
    pub verifier: String,
}

/// The pages an issue plan asks the resident-context assembler to consider.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ContextRequest {
    /// Page ids, paths, symbols, diagnostic ids, or test ids to prefer.
    pub keys: Vec<String>,
}

/// One compiled issue plan: the policy rule's output for the current state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IssuePromptPlan {
    /// The exhaustive policy rule that produced this plan.
    pub rule_id: String,
    pub stage: PromptStage,
    pub issue: ResidualClass,
    /// Present only when a sound verifier supplied a concrete witness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub witness: Option<CounterexampleRef>,
    pub correction: CorrectionPacketRef,
    /// Ordered active section ids to instantiate.
    pub sections: Vec<PromptSectionId>,
    #[serde(default)]
    pub context_request: ContextRequest,
}
