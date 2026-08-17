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
        match self.architect_plan(recorder, task).await {
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
        let mut conversation = Conversation::with_system(
            "You are a planning architect. Decompose the task into independent \
             work-graph nodes ONLY when parts genuinely touch disjoint files. \
             Call update_graph exactly once; its `revision` argument is JSON: \
             {\"nodes\":[{\"node_id\":str,\"goal\":str,\"output_targets\":[str]}],\
             \"edges\":[[src,dst]]}. Declare output_targets precisely; a node \
             without them serializes against everything. Prefer one node when \
             in doubt.",
        );
        conversation.push_user(task.to_string());
        let output = self
            .transport
            .chat_turn(
                &route,
                &conversation,
                &[spec],
                ToolChoicePolicy::Specific("update_graph".into()),
            )
            .await?;
        let TurnOutput::ToolCalls(calls) = output else {
            return Ok(None);
        };
        let Some(call) = calls.iter().find(|call| call.name == "update_graph") else {
            return Ok(None);
        };
        let revision = call
            .arguments
            .get("revision")
            .and_then(serde_json::Value::as_str)
            .context("update_graph call had no revision argument")?;
        let spec: PlanSpec = serde_json::from_str(revision).context("parsing plan revision")?;
        if spec.nodes.len() < 2 {
            return Ok(None);
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
