//! Plugin policy decisions (rehomed from the retired PSP-7 `prompt` module
//! during PSP-10 Phase 5; the correction-attempt bookkeeping that lived
//! beside them was dead and is deleted).

use super::*;

/// Policy decision for a dependency command (PSP-7 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandPolicyDecision {
    /// Command is allowed.
    Allow,
    /// Command is denied.
    Deny,
    /// Command requires user approval before execution.
    RequireApproval,
}

/// Policy decision for a manifest mutation (PSP-7 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManifestMutationPolicy {
    /// Mutation is allowed.
    Allow,
    /// Mutation is denied.
    Deny,
}
