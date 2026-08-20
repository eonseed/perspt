//! Context checkpoints (PSP-9 system 14).
//!
//! Conversation compaction is a **recorded context projection, never ledger
//! deletion**: the model context is rebuilt from the immutable ledger, and a
//! deterministic [`ControlFrame`] is always inserted verbatim — it is never
//! summarized. An optional cheap-model narrative improves recall but is an
//! untrusted recorded observation; it cannot supply a state witness,
//! authority, tool result, or verifier verdict.

use serde::{Deserialize, Serialize};

use crate::error::{Result, SdkError};
use crate::model::ModelId;

/// The verbatim, never-summarized control state of a compacted conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlFrame {
    /// Canonical digest of the ledger-folded conversation projection.
    #[serde(default)]
    pub projection_digest: String,
    /// Schema version used by conversation seed/delta events.
    #[serde(default)]
    pub event_schema_version: u32,
    /// PSP-10 prompt provenance (Gate Z). Empty marks a pre-PSP-10
    /// checkpoint; a nonempty value requires exact reconstruction from
    /// built-ins or pinned bundle artifacts on resume.
    #[serde(default)]
    pub prompt_invocation_digest: String,
    #[serde(default)]
    pub prompt_manifest_digest: String,
    /// Resident-context digest (Gate AF); empty until the paged assembler
    /// runs per turn.
    #[serde(default)]
    pub resident_context_digest: String,
    /// The work-graph node this checkpoint belongs to. Empty marks a
    /// legacy checkpoint; resume falls back to the historical single-node
    /// identity for those.
    #[serde(default)]
    pub node_id: String,
    pub goal: String,
    pub node_generation: u32,
    pub accepted_state_root: String,
    pub graph_revision: String,
    pub capability_ids: Vec<String>,
    pub authority_epoch: u64,
    pub remaining_rejection_budget: u32,
    pub remaining_turns: u32,
    /// Active route after sticky failover and its unconsumed fallback suffix.
    /// Resume must not silently refill or reroute this recovery state.
    pub active_model: ModelId,
    pub remaining_fallback_models: Vec<ModelId>,
    /// Deferred tools activated by governed discovery before this checkpoint.
    pub activated_tools: Vec<String>,
    /// Tool calls whose results are still outstanding; a checkpoint must
    /// preserve these exactly.
    pub unresolved_call_ids: Vec<String>,
    /// The current residual vector, by class name and magnitude.
    pub residual_summary: Vec<(String, f64)>,
}

/// A provider-neutral context checkpoint over a covered event range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextCheckpoint {
    pub parent: Option<String>,
    pub covered_from: u64,
    pub covered_to: u64,
    /// Chain head over the covered range.
    pub covered_event_root: String,
    pub control: ControlFrame,
    /// Content-addressed artifact references the projection keeps.
    pub artifact_refs: Vec<String>,
    /// Optional narrative summary, as a recorded-observation handle. It is
    /// untrusted and can never substitute for control-frame facts.
    pub narrative_observation: Option<String>,
}

impl ContextCheckpoint {
    /// Structural validation before use (PSP-9 system 14): range sanity,
    /// and equality of the live accepted root, revision, and epoch. A stale
    /// checkpoint is rebuilt, never patched by a model.
    pub fn validate_against(
        &self,
        live_accepted_root: &str,
        live_graph_revision: &str,
        live_authority_epoch: u64,
    ) -> Result<()> {
        if self.covered_from > self.covered_to {
            return Err(SdkError::Domain(
                "checkpoint covers an inverted range".into(),
            ));
        }
        if self.control.accepted_state_root != live_accepted_root {
            return Err(SdkError::Domain(
                "stale checkpoint: accepted state root changed; rebuild it".into(),
            ));
        }
        if self.control.graph_revision != live_graph_revision {
            return Err(SdkError::Domain(
                "stale checkpoint: graph revision changed; rebuild it".into(),
            ));
        }
        if self.control.authority_epoch != live_authority_epoch {
            return Err(SdkError::Domain(
                "stale checkpoint: authority epoch changed; rebuild it".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkpoint() -> ContextCheckpoint {
        ContextCheckpoint {
            parent: None,
            covered_from: 0,
            covered_to: 41,
            covered_event_root: "head".into(),
            control: ControlFrame {
                projection_digest: "projection".into(),
                prompt_invocation_digest: String::new(),
                prompt_manifest_digest: String::new(),
                resident_context_digest: String::new(),
                node_id: "implement-1".into(),
                event_schema_version: crate::CONVERSATION_EVENT_SCHEMA_VERSION,
                goal: "fix the build".into(),
                node_generation: 2,
                accepted_state_root: "root-a".into(),
                graph_revision: "rev-3".into(),
                capability_ids: vec!["cap-1".into()],
                authority_epoch: 7,
                remaining_rejection_budget: 2,
                remaining_turns: 5,
                active_model: ModelId::new("test", "model"),
                remaining_fallback_models: vec![ModelId::new("test", "fallback")],
                activated_tools: vec!["read_file".into()],
                unresolved_call_ids: vec!["c9".into()],
                residual_summary: vec![("build".into(), 2.0)],
            },
            artifact_refs: vec![],
            narrative_observation: None,
        }
    }

    #[test]
    fn a_current_checkpoint_validates() {
        assert!(checkpoint().validate_against("root-a", "rev-3", 7).is_ok());
    }

    #[test]
    fn any_staleness_axis_rejects_the_checkpoint() {
        let c = checkpoint();
        assert!(c.validate_against("root-b", "rev-3", 7).is_err());
        assert!(c.validate_against("root-a", "rev-4", 7).is_err());
        assert!(c.validate_against("root-a", "rev-3", 8).is_err());
    }

    #[test]
    fn unresolved_calls_survive_the_projection() {
        let json = serde_json::to_string(&checkpoint()).unwrap();
        let back: ContextCheckpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(back.control.unresolved_call_ids, ["c9"]);
    }
}
