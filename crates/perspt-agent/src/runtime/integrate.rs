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
        type Writers<'a> = BTreeMap<&'a str, Vec<(&'a str, Option<&'a [u8]>)>>;
        // The full writer set per path: the pairwise rule must compare
        // every unordered pair, not each writer against one retained
        // owner — a superseded upstream write can still diverge from an
        // incomparable sibling's.
        let mut writers: Writers = BTreeMap::new();
        for (node_id, winner) in &self.contributions {
            for file in &winner.files {
                writers
                    .entry(file.path.as_str())
                    .or_default()
                    .push((node_id, file.content.as_deref()));
            }
        }
        for (path, entries) in &writers {
            for (index, (first, first_content)) in entries.iter().enumerate() {
                for (second, second_content) in &entries[index + 1..] {
                    if first_content != second_content && !comparable(first, second) {
                        return Some(format!(
                            "path {path} written divergently by {first} and {second}"
                        ));
                    }
                }
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

/// Rebuild the staging root from the durable ledger: the newest
/// `staging_root_updated` event per node, its files rehydrated from the
/// content-addressed artifact store. Fails closed on a staged
/// contribution that predates durable staging (no `files` field) — an
/// unreconstructible staging root must refuse resume, never promote a
/// partial one (Gate AA).
pub(super) fn fold_staging(
    recorder: &Psp9Recorder,
    session_id: &str,
    graph: &perspt_sdk::WorkGraphRevision,
) -> Result<StagingRoot> {
    use anyhow::Context;
    let mut staging = StagingRoot::default();
    for row in recorder.store.get_psp9_events(session_id)? {
        let Ok(perspt_sdk::LedgerEvent::Custom { kind, payload }) =
            serde_json::from_str::<perspt_sdk::LedgerEvent>(&row.event_json)
        else {
            continue;
        };
        if kind != "staging_root_updated" {
            continue;
        }
        let node_id = payload
            .get("node_id")
            .and_then(|value| value.as_str())
            .context("staged contribution names no node")?;
        if graph.node(node_id).is_none() {
            continue;
        }
        let files_value = payload.get("files").with_context(|| {
            format!(
                "staged contribution for {node_id} predates durable staging; \
                 the staging root cannot be reconstructed"
            )
        })?;
        let handles: Vec<crate::toolloop::DurableSeedFile> =
            serde_json::from_value(files_value.clone())?;
        let state_root = payload
            .get("state_root")
            .and_then(|value| value.as_str())
            .context("staged contribution carries no state root")?
            .to_string();
        staging.contributions.insert(
            node_id.to_string(),
            StagedWinner {
                state_root,
                files: super::node::load_seed_files(recorder, handles)?,
            },
        );
    }
    Ok(staging)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolloop::SeedFile;

    fn graph_a_to_b_with_sibling_z() -> perspt_sdk::WorkGraphRevision {
        let node = |id: &str| {
            perspt_sdk::WorkNode::new(id, format!("goal {id}"), perspt_sdk::NodeClass::Implement)
        };
        perspt_sdk::WorkGraphRevision::build(
            1,
            None,
            perspt_sdk::GraphRevisionReason::InitialPlan,
            vec![node("a"), node("b"), node("z")],
            vec![perspt_sdk::WorkEdge::new(
                "a",
                "b",
                perspt_sdk::EdgeKind::RequiresArtifact,
            )],
        )
        .unwrap()
    }

    fn winner(content: &[u8]) -> StagedWinner {
        StagedWinner {
            state_root: format!("root-{}", perspt_sdk::ledger::content_hash(content)),
            files: vec![SeedFile {
                path: "src/shared.rs".into(),
                content: Some(content.to_vec()),
                source_preimage: None,
            }],
        }
    }

    /// The pairwise rule over the full writer set: with `a → b` and an
    /// unrelated sibling `z`, `a` writing X, `b` refining it to Y, and `z`
    /// independently writing Y must conflict on the incomparable divergent
    /// pair (a, z) — even though the retained-owner walk (a/b comparable,
    /// then b/z equal-content) sees nothing.
    #[test]
    fn a_superseded_upstream_write_still_conflicts_with_a_divergent_sibling() {
        let graph = graph_a_to_b_with_sibling_z();
        let mut staging = StagingRoot::default();
        staging.contributions.insert("a".into(), winner(b"X"));
        staging.contributions.insert("b".into(), winner(b"Y"));
        staging.contributions.insert("z".into(), winner(b"Y"));
        let conflict = staging.conflict(&graph).expect("a and z diverge");
        assert!(
            conflict.contains("a") && conflict.contains("z"),
            "the incomparable divergent pair is named: {conflict}"
        );
    }

    /// Refinement along an edge stays conflict-free, and an agreeing
    /// sibling does not manufacture one.
    #[test]
    fn comparable_refinement_and_agreeing_siblings_do_not_conflict() {
        let graph = graph_a_to_b_with_sibling_z();
        let mut staging = StagingRoot::default();
        staging.contributions.insert("a".into(), winner(b"X"));
        staging.contributions.insert("b".into(), winner(b"Y"));
        assert!(staging.conflict(&graph).is_none(), "a → b is a refinement");
        staging.contributions.insert("z".into(), winner(b"X"));
        let conflict = staging.conflict(&graph).expect("b and z diverge");
        assert!(conflict.contains("b") && conflict.contains("z"));
    }
}
