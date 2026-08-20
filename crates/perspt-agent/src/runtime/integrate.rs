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

use std::collections::{BTreeMap, BTreeSet};

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

    /// Dependency-aware conflict detection: two nodes writing the same
    /// path with different content conflict only when they are
    /// **topologically incomparable** (true siblings). A downstream node
    /// legitimately refines its predecessor's file — edge order defines
    /// precedence, so that pair is a refinement, never a divergence.
    pub fn conflict(&self, graph: &perspt_sdk::WorkGraphRevision) -> Option<String> {
        let ancestors: BTreeMap<&str, BTreeSet<String>> = self
            .contributions
            .keys()
            .map(|node_id| (node_id.as_str(), transitive_predecessors(graph, node_id)))
            .collect();
        let comparable = |a: &str, b: &str| {
            ancestors.get(a).is_some_and(|set| set.contains(b))
                || ancestors.get(b).is_some_and(|set| set.contains(a))
        };
        let mut owners: BTreeMap<&str, (&str, Option<&[u8]>)> = BTreeMap::new();
        for (node_id, winner) in &self.contributions {
            for file in &winner.files {
                let content = file.content.as_deref();
                if let Some((other_node, other_content)) = owners.get(file.path.as_str()) {
                    if *other_content != content && !comparable(other_node, node_id) {
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

    /// The merged file set restricted to `contributors`, in topological
    /// order so a downstream contribution overwrites its predecessor's
    /// (deterministic downstream precedence); path order within.
    pub fn merged_files_for(
        &self,
        graph: &perspt_sdk::WorkGraphRevision,
        contributors: &BTreeSet<String>,
    ) -> Vec<SeedFile> {
        let topo: BTreeMap<&str, usize> = graph
            .validation
            .topo_order
            .iter()
            .enumerate()
            .map(|(index, node_id)| (node_id.as_str(), index))
            .collect();
        let mut ordered: Vec<&String> = self
            .contributions
            .keys()
            .filter(|node_id| contributors.contains(*node_id))
            .collect();
        ordered.sort_by_key(|node_id| topo.get(node_id.as_str()).copied().unwrap_or(usize::MAX));
        let mut merged: BTreeMap<String, SeedFile> = BTreeMap::new();
        for node_id in ordered {
            for file in &self.contributions[node_id].files {
                merged.insert(file.path.clone(), file.clone());
            }
        }
        merged.into_values().collect()
    }

    /// The full merged set (the global integration gate's combined state).
    pub fn merged_files(&self, graph: &perspt_sdk::WorkGraphRevision) -> Vec<SeedFile> {
        self.merged_files_for(graph, &self.contributions.keys().cloned().collect())
    }
}

/// The transitive dependency closure of one node over the graph's edges.
pub(super) fn transitive_predecessors(
    graph: &perspt_sdk::WorkGraphRevision,
    node_id: &str,
) -> BTreeSet<String> {
    let mut wanted = BTreeSet::new();
    let mut frontier = vec![node_id.to_string()];
    while let Some(current) = frontier.pop() {
        for dependency in graph.dependencies_of(&current) {
            if wanted.insert(dependency.to_string()) {
                frontier.push(dependency.to_string());
            }
        }
    }
    wanted
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
        if let Some(conflict) = staging.conflict(graph) {
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
        workspace.set_verifier_timeouts(self.config.verifier_timeouts);
        workspace.restore_exported(&staging.merged_files(graph))?;
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

    /// The seed one downstream node forks from: only its **transitive
    /// predecessors'** staged contributions (system 22) — an unrelated
    /// sibling's state never leaks in — realized once so the seed's state
    /// root is verifiable. The seed is inherited: it restores without
    /// entering the node's promotable set.
    pub(super) async fn staging_seed(
        &self,
        graph: &perspt_sdk::WorkGraphRevision,
        staging: &StagingRoot,
        node_id: &str,
    ) -> Result<Option<super::node::CandidateSeed>> {
        let predecessors = transitive_predecessors(graph, node_id);
        let contributors: BTreeSet<String> = staging
            .contributions
            .keys()
            .filter(|contributor| predecessors.contains(*contributor))
            .cloned()
            .collect();
        if contributors.is_empty() {
            return Ok(None);
        }
        let scratch =
            CandidateWorkspace::create(&self.working_dir, "staging-seed", 0, &graph.revision_id)?;
        let files = staging.merged_files_for(graph, &contributors);
        scratch.restore_seeded(&files)?;
        let checkpoint = scratch.checkpoint(&[]).await?;
        Ok(Some(super::node::CandidateSeed {
            expected_state_root: checkpoint.witness.state_root,
            canonical_scope: checkpoint.witness.canonical_scope.clone(),
            files,
            conversation: perspt_sdk::Conversation::default(),
            activated_tools: Vec::new(),
            inherited: true,
        }))
    }
}
