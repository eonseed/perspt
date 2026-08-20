//! Branch isolation plumbing (PSP-10 system 19).
//!
//! A branch runs the ordinary governed loop — failover, kernel mediation,
//! batching, digest-chained conversation deltas — but its internal
//! measurements and gate decisions are private search state: the
//! [`BranchRecorder`] rewrites them into the search alphabet so a
//! discarded branch leaves the accepted fold untouched (Gate W), and
//! suppresses durable checkpoints so branch workspaces never become
//! resume points.

use anyhow::Result;
use perspt_sdk::search::ReservationRequest;

use crate::runtime::Psp9Recorder;
use crate::toolloop::{LoopEvent, LoopRecorder};

/// Wraps the session recorder for one branch.
pub(crate) struct BranchRecorder<'a> {
    pub(crate) inner: &'a Psp9Recorder,
    pub(crate) forest_id: String,
    pub(crate) branch_id: String,
}

impl LoopRecorder for BranchRecorder<'_> {
    fn record(&self, event: &LoopEvent) -> Result<()> {
        let rewritten = match event {
            // Ordinary trajectory events are the accepted fold's alphabet;
            // inside a branch they become observations (Gate W).
            LoopEvent::CandidateMeasured {
                energy, hard_pass, ..
            } => Some(format!(
                "branch measurement V={energy:.4} hard_pass={hard_pass}"
            )),
            LoopEvent::GateDecisionRecorded { decision, .. } => {
                Some(format!("branch-internal gate decision {decision:?}"))
            }
            LoopEvent::DecisionBoundRefused { bound, .. } => {
                Some(format!("branch-internal decision bound {bound} reached"))
            }
            // Branch workspaces are never durable resume points in this
            // release; the node's own checkpoints carry resume.
            LoopEvent::DurableCandidateCheckpoint { .. }
            | LoopEvent::ContextCheckpointCreated { .. } => return Ok(()),
            _ => None,
        };
        match rewritten {
            Some(observation) => self.inner.record(&LoopEvent::BranchObservation {
                forest_id: self.forest_id.clone(),
                branch_id: self.branch_id.clone(),
                observation,
            }),
            None => self.inner.record(event),
        }
    }

    fn external_intent(&self, key: &str, intent: &serde_json::Value) -> Result<()> {
        self.inner.external_intent(key, intent)
    }

    fn external_result(&self, key: &str, result: &serde_json::Value) -> Result<()> {
        self.inner.external_result(key, result)
    }

    fn record_artifact(&self, content: &[u8], media_type: &str) -> Result<String> {
        self.inner.record_artifact(content, media_type)
    }

    fn fetch_artifact(&self, handle: &str) -> Result<Option<Vec<u8>>> {
        self.inner.fetch_artifact(handle)
    }
}

/// Measure the fork cost of a workspace tree without following symlinks
/// (PSP-10 system 19: reservation precedes the eager copy).
pub(crate) fn measure_fork_cost(root: &std::path::Path) -> ReservationRequest {
    let mut files = 0u64;
    let mut bytes = 0u64;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                // Never followed; the copy will not traverse it either.
                continue;
            }
            if kind.is_dir() {
                let name = entry.file_name();
                let skip = matches!(
                    name.to_str(),
                    Some(
                        ".git"
                            | ".perspt"
                            | ".perspt-target"
                            | ".perspt-tmp"
                            | ".perspt-home"
                            | ".venv"
                            | ".pytest_cache"
                            | ".ruff_cache"
                            | "__pycache__"
                            | "node_modules"
                            | "target"
                    )
                );
                if !skip {
                    stack.push(entry.path());
                }
            } else if kind.is_file() {
                files += 1;
                bytes += entry.metadata().map(|meta| meta.len()).unwrap_or(0);
            }
        }
    }
    ReservationRequest {
        workspace_files: files,
        workspace_bytes: bytes,
        ..Default::default()
    }
}

/// Fold one branch attempt's observed events into the consumed-resource
/// vector (Definition 5): every model turn, tool call, mutation, verifier
/// run, output token (under the declared conservative accountant), and
/// result byte is charged — never only the fork's file cost.
pub(crate) fn consumed_usage(
    outcome: &crate::toolloop::LoopOutcome,
) -> perspt_sdk::search::ReservationRequest {
    let accountant = perspt_sdk::prompt::TokenAccountantRef::approx_bytes_v1();
    let mut consumed = ReservationRequest::default();
    for event in &outcome.events {
        match event {
            LoopEvent::ToolCallObserved { .. } => consumed.tool_calls += 1,
            LoopEvent::EffectApplied {
                mutated, output, ..
            } => {
                if *mutated {
                    consumed.mutations += 1;
                }
                consumed.result_bytes += output.len() as u64;
            }
            LoopEvent::CandidateMeasured { .. } | LoopEvent::EffectBoundaryMeasured { .. } => {
                consumed.verifier_runs += 1;
            }
            // A turn is consumed when the transport actually answered — a
            // turn aborted by a refused reservation made no call.
            LoopEvent::TurnObserved { output, .. } => {
                consumed.model_turns += 1;
                let serialized = serde_json::to_string(output).unwrap_or_default();
                consumed.tokens += accountant.count_message(&serialized);
            }
            _ => {}
        }
    }
    consumed
}
