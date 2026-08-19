//! Governed graph planning (PSP-9 system 15).
//!
//! Multi-node graphs are never fabricated by the host: one forced-tool-
//! choice architect turn, restricted to the privileged `update_graph`
//! entry, proposes nodes **with declared output targets** and edges. The
//! proposal is validated by `WorkGraphRevision::build` (acyclic, complete);
//! anything invalid or empty falls back to today's single-node graph and
//! the fallback is recorded. A node without output targets keeps the
//! opaque-workspace footprint and simply serializes — a safe default.

use super::*;

/// The single source of the `revision` payload shape shown to the model —
/// co-located with `PlanSpec` so the prompt cannot drift from the parser
/// (PSP-10 Phase 5; the historic inline JSON existed in three copies).
pub(crate) const REVISION_SHAPE: &str = "{\"nodes\":[{\"node_id\":str,\"goal\":str,\
\"output_targets\":[str]}],\"edges\":[[src,dst]]}";

/// The architect's proposal, parsed from the `update_graph` call argument.
#[derive(Debug, serde::Deserialize)]
struct PlanSpec {
    nodes: Vec<PlanNode>,
    #[serde(default)]
    edges: Vec<(String, String)>,
}

#[derive(Debug, serde::Deserialize)]
struct PlanNode {
    node_id: String,
    goal: String,
    #[serde(default)]
    output_targets: Vec<String>,
}

impl Psp9AgentRuntime {
    /// Plan the session's work graph. Single-node unless multi-node
    /// dispatch is enabled; with it, one governed architect turn may
    /// decompose the task.
    pub(super) async fn plan_initial_graph(
        &self,
        recorder: &Psp9Recorder,
        node_id: &str,
        task: &str,
    ) -> Result<WorkGraphRevision> {
        if self.config.max_parallel_nodes <= 1 {
            return initial_graph(node_id, task);
        }
        match self.architect_plan(recorder, node_id, task).await {
            Ok(Some(graph)) => Ok(graph),
            Ok(None) => {
                recorder.record_custom(
                    "graph_plan_fallback",
                    serde_json::json!({"reason": "no plan produced"}),
                )?;
                initial_graph(node_id, task)
            }
            Err(error) => {
                recorder.record_custom(
                    "graph_plan_fallback",
                    serde_json::json!({"reason": error.to_string()}),
                )?;
                initial_graph(node_id, task)
            }
        }
    }

    /// One forced-tool-choice turn restricted to `update_graph`.
    async fn architect_plan(
        &self,
        recorder: &Psp9Recorder,
        node_id: &str,
        task: &str,
    ) -> Result<Option<WorkGraphRevision>> {
        let route = self.handoff_model.as_ref().unwrap_or(&self.model).clone();
        let entry = perspt_sdk::base_entries()
            .into_iter()
            .find(|entry| entry.name == "update_graph")
            .context("base catalog lost update_graph")?;
        let spec = perspt_sdk::ToolSpec {
            name: entry.name.clone(),
            description: entry.description.clone(),
            schema: entry.schema.clone(),
            strict: false,
        };
        let stage = perspt_core::prompts::PlatformPromptLibrary::graph_plan(
            REVISION_SHAPE,
            &self.prompt_overrides,
        )
        .map_err(|e| anyhow::anyhow!("graph plan prompt: {e}"))?;
        let invocation = self.actor_invocation(
            recorder,
            "architect",
            &stage,
            &route,
            std::slice::from_ref(&spec),
        )?;
        let mut conversation = Conversation::with_system(invocation.platform.system_text());
        conversation.push_user(task.to_string());
        let mut runner = crate::turn::ActorTurnRunner {
            transport: self.transport.as_ref(),
            model: route.clone(),
            fallbacks: self.fallback_models.clone(),
            recorder: Some(recorder),
            actor: crate::turn::ActorKind::Architect,
            deadline_secs: self.config.turn_deadline_secs,
            turn: 1,
        };
        let output = runner
            .run_turn(
                &conversation,
                std::slice::from_ref(&spec),
                ToolChoicePolicy::Specific("update_graph".into()),
            )
            .await?;
        self.fold_plan_output(recorder, node_id, &route, &entry, output)
    }

    /// Parse, validate, and kernel-admit the architect's proposal.
    fn fold_plan_output(
        &self,
        recorder: &Psp9Recorder,
        node_id: &str,
        route: &ModelId,
        entry: &perspt_sdk::ToolEntry,
        output: TurnOutput,
    ) -> Result<Option<WorkGraphRevision>> {
        let TurnOutput::ToolCalls(calls) = output else {
            return Ok(None);
        };
        let Some(call) = calls.iter().find(|call| call.name == "update_graph") else {
            return Ok(None);
        };
        entry
            .validate_arguments(&call.arguments)
            .context("update_graph arguments rejected by the catalog schema")?;
        let revision = call
            .arguments
            .get("revision")
            .and_then(serde_json::Value::as_str)
            .context("update_graph call had no revision argument")?;
        let spec: PlanSpec = serde_json::from_str(revision).context("parsing plan revision")?;
        if spec.nodes.len() < 2 {
            return Ok(None);
        }
        if !self.admit_graph_update(recorder, node_id, revision)? {
            anyhow::bail!("admissibility kernel refused the update_graph proposal");
        }
        let graph = build_planned_graph(&spec)?;
        recorder.record_custom(
            "graph_planned",
            serde_json::json!({
                "architect": route,
                "nodes": graph.nodes.len(),
                "edges": graph.edges.len(),
            }),
        )?;
        Ok(Some(graph))
    }

    /// Kernel mediation for the architect's graph update (PSP-10 Phase 1,
    /// hardened). Authority derives from the session composition-root
    /// grant — the same `session_grant_policy` construction every worker
    /// grant attenuates from, with `UpdateGraph` as the one architect-role
    /// opt-in — never a locally invented policy. The proposal runs the full
    /// kernel with a real plan contract (graph validity) and a real plan
    /// barrier (shape bounds), and only an `SrbnCertified` allow admits the
    /// revision; a `ConfinementOnly` witness refuses.
    fn admit_graph_update(
        &self,
        recorder: &Psp9Recorder,
        node_id: &str,
        revision_json: &str,
    ) -> Result<bool> {
        let catalog = perspt_sdk::StaticCatalog::with_base(Vec::new())
            .map_err(|e| anyhow::anyhow!("plan catalog: {e}"))?;
        let mut policy = session_grant_policy(
            &self.working_dir,
            "initial-plan",
            false,
            &catalog,
            &[perspt_sdk::EffectKind::UpdateGraph],
        )?;
        // The architect is a Session-role actor, so the ceiling's approval
        // policy binds directly; the operator's configured session approval
        // is the authority here (multi-node dispatch already requires an
        // auto-approved session).
        policy.approval_ceiling = self.config.approval_policy;
        let mut requested = perspt_sdk::Capability::new(
            perspt_sdk::ActorId::new("architect"),
            vec![perspt_sdk::EffectKind::UpdateGraph],
        );
        requested.max_calls = Some(1);
        requested.role = perspt_sdk::CapabilityRole::Session;
        let capability = policy
            .mint(requested)
            .map_err(|e| anyhow::anyhow!("architect grant: {e}"))?;
        let proposal = perspt_sdk::EffectProposal::new(
            capability.holder.clone(),
            node_id,
            perspt_sdk::EffectKind::UpdateGraph,
        )
        .with_risk_class(perspt_sdk::RiskClass::Critical)
        .with_idempotency_key(format!("plan:{node_id}"));
        let transition = perspt_sdk::CandidateTransition::new(
            proposal,
            graph_state_witness(node_id, "unplanned"),
            graph_state_witness(node_id, revision_json),
        );
        let contract = PlanContract {
            revision_json: revision_json.to_string(),
        };
        let barrier = PlanBarrier {
            revision_json: revision_json.to_string(),
            max_nodes: self.config.max_parallel_nodes.saturating_mul(4).max(4),
        };
        let witness = perspt_sdk::check_full_admissibility(
            &transition,
            std::slice::from_ref(&capability),
            &perspt_sdk::KernelState::new(),
            Some(&contract),
            Some(&barrier),
            0.25,
        )
        .map_err(|e| anyhow::anyhow!("plan kernel: {e}"))?;
        recorder.record_custom("graph_plan_admissibility", serde_json::to_value(&witness)?)?;
        Ok(witness.allows() && witness.profile == perspt_sdk::AdmissibilityProfile::SrbnCertified)
    }
}

/// The plan contract: the proposed revision must parse against the
/// advertised shape and build a valid (acyclic, complete) work graph with
/// nonempty goals.
struct PlanContract {
    revision_json: String,
}

impl perspt_sdk::ContractEvaluator for PlanContract {
    fn evaluate(&self, _transition: &perspt_sdk::CandidateTransition) -> ContractWitness {
        let ok = serde_json::from_str::<PlanSpec>(&self.revision_json)
            .ok()
            .filter(|spec| spec.nodes.iter().all(|node| !node.goal.trim().is_empty()))
            .and_then(|spec| build_planned_graph(&spec).ok())
            .is_some();
        ContractWitness {
            ok,
            policy_version: "plan-contract-v1".into(),
            evidence_refs: vec![perspt_sdk::ledger::content_hash(
                self.revision_json.as_bytes(),
            )],
        }
    }
}

/// The plan barrier: graph shape stays inside declared bounds. Oversized
/// or reference-broken revisions certify an unsafe increment.
struct PlanBarrier {
    revision_json: String,
    max_nodes: usize,
}

impl perspt_sdk::BarrierEvaluator for PlanBarrier {
    fn evaluate(
        &self,
        _transition: &perspt_sdk::CandidateTransition,
    ) -> std::result::Result<perspt_sdk::BarrierWitness, perspt_sdk::SdkError> {
        let shape_safe = serde_json::from_str::<PlanSpec>(&self.revision_json)
            .map(|spec| {
                let ids: std::collections::BTreeSet<&str> = spec
                    .nodes
                    .iter()
                    .map(|node| node.node_id.as_str())
                    .collect();
                spec.nodes.len() <= self.max_nodes
                    && spec
                        .edges
                        .iter()
                        .all(|(src, dst)| ids.contains(src.as_str()) && ids.contains(dst.as_str()))
            })
            .unwrap_or(false);
        Ok(perspt_sdk::BarrierWitness {
            h_before: 0.0,
            expected_h_after_upper: if shape_safe { 0.0 } else { 1.0 },
            certified_increment: if shape_safe { 0.0 } else { 1.0 },
            unsafe_threshold: 0.5,
            evidence_refs: vec!["plan-barrier-v1".into()],
        })
    }
}

/// The work-graph state witness for the architect transition: the mutated
/// "state" is the serialized revision itself, not a workspace root.
fn graph_state_witness(node_id: &str, revision: &str) -> perspt_sdk::CandidateStateWitness {
    perspt_sdk::CandidateStateWitness {
        state_root: perspt_sdk::ledger::content_hash(revision.as_bytes()),
        graph_revision: "initial-plan".into(),
        node_id: node_id.into(),
        node_generation: 0,
        canonical_scope: Vec::new(),
        barrier_channels: std::collections::BTreeMap::new(),
    }
}

/// Validate and build the proposed revision (acyclicity and edge
/// completeness are `WorkGraphRevision::build`'s job).
fn build_planned_graph(spec: &PlanSpec) -> Result<WorkGraphRevision> {
    let nodes: Vec<WorkNode> = spec
        .nodes
        .iter()
        .map(|planned| {
            let mut node = WorkNode::new(&planned.node_id, &planned.goal, NodeClass::Implement);
            node.owner_domains = vec!["coding".into()];
            node.output_targets = planned.output_targets.clone();
            node.state = WorkNodeState::Ready;
            node
        })
        .collect();
    let edges: Vec<perspt_sdk::WorkEdge> = spec
        .edges
        .iter()
        .map(|(src, dst)| {
            perspt_sdk::WorkEdge::new(src, dst, perspt_sdk::EdgeKind::RequiresArtifact)
        })
        .collect();
    WorkGraphRevision::build(
        0,
        None,
        perspt_sdk::GraphRevisionReason::InitialPlan,
        nodes,
        edges,
    )
    .map_err(|error| anyhow::anyhow!("planned graph rejected: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An instance matching the advertised shape parses as `PlanSpec`, so
    /// the model-facing shape and the parser cannot drift apart.
    #[test]
    fn the_advertised_revision_shape_matches_the_parser() {
        let instance = serde_json::json!({
            "nodes": [{"node_id": "n1", "goal": "g", "output_targets": ["src/a.rs"]}],
            "edges": [["n1", "n1"]],
        })
        .to_string();
        let parsed: PlanSpec = serde_json::from_str(&instance).unwrap();
        assert_eq!(parsed.nodes.len(), 1);
        // The shape names exactly the fields the parser reads.
        for field in ["nodes", "node_id", "goal", "output_targets", "edges"] {
            assert!(REVISION_SHAPE.contains(field), "shape lost {field}");
        }
    }
}
