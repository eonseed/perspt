//! Graph staging and the global integration gate (PSP-10 system 22,
//! Gate AA).
//!
//! Two nodes may pass separately and fail together. Node winners enter a
//! content-addressed staging root instead of the user workspace; disjoint
//! winners merge deterministically; one global verifier gate runs the full
//! domain suite and the immutable test oracle on the combined state; and
//! only a hard-passing integration root is promoted, atomically, through
//! the hardened path. Failure restores the prior staging root — the user
//! workspace never contains one winner without the rest of its verified
//! root.

use std::collections::BTreeMap;

use anyhow::Result;
use perspt_sdk::canon::CanonicalEncoder;

use super::{Psp9AgentRuntime, Psp9Recorder};
use crate::candidate::{CandidateWorkspace, CodingCandidateMeasurer};
use crate::toolloop::{CandidateMeasurer, EffectExecutor, SeedFile};

/// One staged node winner.
pub(super) struct StagedWinner {
    pub state_root: String,
    pub files: Vec<SeedFile>,
}

/// The content-addressed staging root: node winners keyed by node id.
#[derive(Default)]
pub(super) struct StagingRoot {
    pub contributions: BTreeMap<String, StagedWinner>,
}

impl StagingRoot {
    pub fn is_empty(&self) -> bool {
        self.contributions.is_empty()
    }

    /// The staging root's content address over its contributions.
    pub fn digest(&self) -> String {
        let mut encoder = CanonicalEncoder::new(b"perspt-staging-v1");
        for (node_id, winner) in &self.contributions {
            encoder.text(node_id).text(&winner.state_root);
            for file in &winner.files {
                encoder.text(&file.path).field(
                    file.content
                        .as_deref()
                        .map(perspt_sdk::ledger::content_hash)
                        .unwrap_or_default()
                        .as_bytes(),
                );
            }
        }
        encoder.digest()
    }

    /// Deterministic merge: overlapping non-identical paths are a typed
    /// integration conflict, never silently last-writer-wins.
    pub fn conflict(&self) -> Option<String> {
        let mut owners: BTreeMap<&str, (&str, Option<&[u8]>)> = BTreeMap::new();
        for (node_id, winner) in &self.contributions {
            for file in &winner.files {
                let content = file.content.as_deref();
                if let Some((other_node, other_content)) = owners.get(file.path.as_str()) {
                    if *other_content != content {
                        return Some(format!(
                            "path {} written divergently by {other_node} and {node_id}",
                            file.path
                        ));
                    }
                }
                owners.insert(file.path.as_str(), (node_id, content));
            }
        }
        None
    }

    /// The merged file set, in deterministic path order.
    pub fn merged_files(&self) -> Vec<SeedFile> {
        let mut merged: BTreeMap<String, SeedFile> = BTreeMap::new();
        for winner in self.contributions.values() {
            for file in &winner.files {
                merged.insert(file.path.clone(), file.clone());
            }
        }
        merged.into_values().collect()
    }
}

impl Psp9AgentRuntime {
    /// Run the global integration gate over the staging root (Gate AA):
    /// realize the combined state in one eager-copy integration workspace,
    /// run the full suite and the immutable oracle, and promote atomically
    /// only on a global hard pass. `None` means integration failed and the
    /// prior root is restored (nothing reached the user workspace).
    pub(super) async fn run_integration_gate(
        &self,
        recorder: &Psp9Recorder,
        session_id: &str,
        graph: &perspt_sdk::WorkGraphRevision,
        staging: &StagingRoot,
    ) -> Result<Option<Vec<String>>> {
        let staging_digest = staging.digest();
        if let Some(conflict) = staging.conflict() {
            recorder.record_custom(
                "integration_failed",
                serde_json::json!({
                    "staging_root": staging_digest,
                    "reason": format!("merge conflict: {conflict}"),
                }),
            )?;
            return Ok(None);
        }
        // Realize the combined state in an isolated integration workspace.
        let mut workspace = CandidateWorkspace::create_with_policy(
            &self.working_dir,
            "integration",
            0,
            &graph.revision_id,
            self.config.allow_unisolated_verifiers,
        )?;
        workspace.set_tool_handlers(self.tool_handlers.clone());
        workspace.restore_exported(&staging.merged_files())?;
        let measured = CodingCandidateMeasurer::new(&workspace, "integration", 0)
            .with_domain(self.domain.clone())
            .with_max_parallel(self.config.max_parallel_verifiers)
            .with_require_format(self.config.require_format)
            .measure()
            .await?;
        recorder.record_custom(
            "integration_measured",
            serde_json::json!({
                "staging_root": staging_digest,
                "energy": measured.energy,
                "hard_pass": measured.hard_pass,
                "residuals": measured.residuals.len(),
            }),
        )?;
        if !measured.hard_pass {
            recorder.record_custom(
                "integration_failed",
                serde_json::json!({
                    "staging_root": staging_digest,
                    "reason": "combined state failed the global verifier gate",
                }),
            )?;
            return Ok(None);
        }
        self.promote_integration_root(recorder, session_id, graph, &workspace, &staging_digest)
            .await
    }

    /// Atomic promotion of the verified integration root through the
    /// hardened path, under the ordinary grant/kernel discipline.
    async fn promote_integration_root(
        &self,
        recorder: &Psp9Recorder,
        session_id: &str,
        graph: &perspt_sdk::WorkGraphRevision,
        workspace: &CandidateWorkspace,
        staging_digest: &str,
    ) -> Result<Option<Vec<String>>> {
        let assembly = {
            let external_entries = self.external_tool_entries(recorder).await?;
            self.assemble_node(
                recorder,
                session_id,
                graph,
                "integration",
                0,
                &external_entries,
            )?
        };
        let kernel_state =
            super::node::loop_kernel_state(&assembly.grant_policy, &graph.revision_id);
        let promoted = self
            .promote_certified(
                recorder,
                workspace,
                "integration",
                &assembly.grant_policy,
                &assembly.capability,
                &assembly.contract,
                &assembly.barrier,
                &kernel_state,
                &assembly.calibration,
            )
            .await?;
        match promoted {
            Some(paths) => {
                recorder.record_custom(
                    "integration_promoted",
                    serde_json::json!({
                        "staging_root": staging_digest,
                        "paths": paths,
                    }),
                )?;
                Ok(Some(paths))
            }
            None => {
                recorder.record_custom(
                    "integration_failed",
                    serde_json::json!({
                        "staging_root": staging_digest,
                        "reason": "promotion was not certified; prior root restored",
                    }),
                )?;
                Ok(None)
            }
        }
    }

    /// The seed downstream nodes fork from: the latest staging root's
    /// merged files, realized once so the seed's state root is verifiable.
    pub(super) async fn staging_seed(
        &self,
        graph: &perspt_sdk::WorkGraphRevision,
        staging: &StagingRoot,
    ) -> Result<Option<super::node::CandidateSeed>> {
        if staging.is_empty() {
            return Ok(None);
        }
        let scratch =
            CandidateWorkspace::create(&self.working_dir, "staging-seed", 0, &graph.revision_id)?;
        let files = staging.merged_files();
        scratch.restore_exported(&files)?;
        let checkpoint = scratch.checkpoint(&[]).await?;
        Ok(Some(super::node::CandidateSeed {
            expected_state_root: checkpoint.witness.state_root,
            canonical_scope: checkpoint.witness.canonical_scope.clone(),
            files,
            conversation: perspt_sdk::Conversation::default(),
            activated_tools: Vec::new(),
        }))
    }
}
