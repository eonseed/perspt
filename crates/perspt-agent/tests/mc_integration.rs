//! Integration-gate mechanism checks (PSP-10 system 22, Gate AA; Phase 10).
//!
//! Two nodes may pass separately and fail together: winners stage, one
//! global gate measures the combined state, and only a hard-passing
//! integration root is promoted — atomically. Integration failure leaves
//! the user workspace byte-identical (no partial promotion exists).

use std::sync::{Arc, Mutex};

use perspt_agent::{Psp9AgentRuntime, Psp9RunConfig};
use perspt_sdk::{
    ApprovalPolicy, Conversation, ModelFamily, ModelId, ModelTransport, NodeTerminalOutcome,
    ProviderCapabilities, ProviderToolCall, ToolChoicePolicy, ToolSpec, TransportFuture,
    TurnOutput,
};

/// Routes turns by goal marker (the mc_dispatch pattern).
struct GoalRouted {
    plan: Mutex<Option<TurnOutput>>,
    alpha: Mutex<Vec<TurnOutput>>,
    beta: Mutex<Vec<TurnOutput>>,
}

impl ModelTransport for GoalRouted {
    fn chat_turn<'a>(
        &'a self,
        _model: &'a ModelId,
        conversation: &'a Conversation,
        _tools: &'a [ToolSpec],
        _choice: ToolChoicePolicy,
    ) -> TransportFuture<'a, TurnOutput> {
        let text = conversation
            .messages()
            .iter()
            .map(|message| format!("{message:?}"))
            .collect::<Vec<_>>()
            .join("\n");
        let output = if text.contains("planning architect") {
            self.plan
                .lock()
                .unwrap()
                .take()
                .unwrap_or(TurnOutput::Text("no plan".into()))
        } else {
            let script = if text.contains("alpha-part") {
                &self.alpha
            } else {
                &self.beta
            };
            let mut turns = script.lock().unwrap();
            if turns.is_empty() {
                TurnOutput::Text("done".into())
            } else {
                turns.remove(0)
            }
        };
        Box::pin(async move { Ok(output) })
    }

    fn capabilities(&self, _model: &ModelId) -> ProviderCapabilities {
        ProviderCapabilities {
            tool_calling: true,
            strict_schema: true,
            parallel_tool_calls: false,
            streaming_tool_calls: false,
            prompt_caching: false,
            structured_output: true,
            max_context_tokens: 32_000,
        }
    }

    fn family_of(&self, _model: &ModelId) -> ModelFamily {
        ModelFamily::Other("scripted".into())
    }

    fn adapter_kind(&self) -> &'static str {
        "scripted"
    }
}

/// A coupled fixture: the test passes with EITHER single change but fails
/// with both — pass separately, fail together.
fn write_coupled_project(project: &std::path::Path) {
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join("Cargo.toml"),
        "[package]\nname='integration-fixture'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    std::fs::write(
        project.join("src/lib.rs"),
        "pub mod a;\npub mod b;\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn coupled() {\n        \
         assert!(crate::a::alpha() == 1 || crate::b::beta() == 2);\n    }\n}\n",
    )
    .unwrap();
    std::fs::write(project.join("src/a.rs"), "pub fn alpha() -> u32 { 1 }\n").unwrap();
    std::fs::write(project.join("src/b.rs"), "pub fn beta() -> u32 { 2 }\n").unwrap();
}

fn plan_turn() -> TurnOutput {
    let revision = serde_json::json!({
        "nodes": [
            {"node_id": "node-a", "goal": "alpha-part: change alpha to 2",
             "output_targets": ["src/a.rs"]},
            {"node_id": "node-b", "goal": "beta-part: change beta to 3",
             "output_targets": ["src/b.rs"]},
        ],
        "edges": [],
    });
    TurnOutput::ToolCalls(vec![ProviderToolCall {
        call_id: "plan-1".into(),
        name: "update_graph".into(),
        arguments: serde_json::json!({"revision": revision.to_string()}),
    }])
}

fn edit_turn(id: &str, path: &str, old: &str, new: &str) -> TurnOutput {
    TurnOutput::ToolCalls(vec![ProviderToolCall {
        call_id: id.into(),
        name: "edit_file".into(),
        arguments: serde_json::json!({
            "path": path, "old_string": old, "new_string": new
        }),
    }])
}

fn event_names(rows: &[perspt_store::Psp9LedgerRow]) -> Vec<String> {
    rows.iter()
        .filter_map(|row| {
            let value: serde_json::Value = serde_json::from_str(&row.event_json).ok()?;
            value
                .get("kind")
                .and_then(|kind| kind.as_str())
                .map(str::to_string)
        })
        .collect()
}

/// Decode a tool-loop row's event name (the search/loop alphabet).
fn tool_event_names(rows: &[perspt_store::Psp9LedgerRow]) -> Vec<String> {
    use perspt_sdk::ledger::{tool_loop_body, LedgerEvent, ToolLoopBody};
    rows.iter()
        .filter_map(|row| {
            let LedgerEvent::Custom { kind, payload } =
                serde_json::from_str(&row.event_json).ok()?
            else {
                return None;
            };
            if kind != "tool_loop" {
                return None;
            }
            let (ToolLoopBody::Legacy(body) | ToolLoopBody::V1(body)) =
                tool_loop_body(&payload).ok()?;
            body.get("event")
                .and_then(|e| e.as_str())
                .map(str::to_string)
        })
        .collect()
}

/// PSP-10 system 22, the dependent-graph half: a downstream node is seeded
/// only with its predecessor's staged contribution (its edit anchors on
/// the predecessor's output), legitimately refines the same file without a
/// divergent-writers conflict, exports only its own work, and the
/// downstream bytes win in the promoted integration root. A write outside
/// the node's declared output_targets is denied while reads stay open.
#[tokio::test]
async fn a_downstream_node_refines_its_predecessor_and_wins() {
    let project = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(project.path().join("src")).unwrap();
    std::fs::write(
        project.path().join("Cargo.toml"),
        "[package]\nname='integration-fixture'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    std::fs::write(project.path().join("src/lib.rs"), "pub mod a;\n").unwrap();
    std::fs::write(
        project.path().join("src/a.rs"),
        "pub fn alpha() -> u32 { 1 }\n",
    )
    .unwrap();
    let database = project.path().join("runtime.db");
    let revision = serde_json::json!({
        "nodes": [
            {"node_id": "node-a", "goal": "alpha-part: document alpha",
             "output_targets": ["src/a.rs", "src/notes.rs"]},
            {"node_id": "node-b", "goal": "beta-part: refine the alpha doc",
             "output_targets": ["src/a.rs"]},
        ],
        "edges": [["node-a", "node-b"]],
    });
    let plan = TurnOutput::ToolCalls(vec![ProviderToolCall {
        call_id: "plan-1".into(),
        name: "update_graph".into(),
        arguments: serde_json::json!({"revision": revision.to_string()}),
    }]);
    let transport = Arc::new(GoalRouted {
        plan: Mutex::new(Some(plan)),
        alpha: Mutex::new(vec![
            edit_turn(
                "a-1",
                "src/a.rs",
                "pub fn alpha() -> u32 { 1 }",
                "/// Alpha.\npub fn alpha() -> u32 { 1 }",
            ),
            TurnOutput::ToolCalls(vec![ProviderToolCall {
                call_id: "a-2".into(),
                name: "write_file".into(),
                arguments: serde_json::json!({
                    "path": "src/notes.rs", "content": "// notes\n"
                }),
            }]),
        ]),
        beta: Mutex::new(vec![
            // Outside node-b's declared output_targets: must be denied.
            edit_turn("b-0", "src/lib.rs", "pub mod a;\n", "pub mod a; // no\n"),
            // Anchors on node-a's OUTPUT — only matches if the seed carried
            // the predecessor's staged contribution.
            edit_turn(
                "b-1",
                "src/a.rs",
                "/// Alpha.\npub fn alpha() -> u32 { 1 }",
                "/// Alpha, refined.\npub fn alpha() -> u32 { 1 }",
            ),
        ]),
    });
    let runtime = Psp9AgentRuntime::with_transport(
        project.path().to_path_buf(),
        transport,
        ModelId::new("test", "scripted"),
        Psp9RunConfig {
            approval_policy: ApprovalPolicy::Auto,
            max_turns: 4,
            allow_unisolated_verifiers: true,
            max_parallel_nodes: 2,
            ..Psp9RunConfig::default()
        },
    )
    .with_database_path(database.clone());

    let summary = runtime.run("document then refine".into()).await.unwrap();
    assert!(
        matches!(summary.outcome, NodeTerminalOutcome::HardPass),
        "dependent refinement must integrate: {:?}",
        summary.outcome
    );
    // The downstream refinement won the merged root.
    let final_a = std::fs::read_to_string(project.path().join("src/a.rs")).unwrap();
    assert!(final_a.contains("Alpha, refined."), "{final_a}");

    let store = perspt_store::SessionStore::open(&database).unwrap();
    let rows = store.get_psp9_events(&summary.session_id).unwrap();
    let names = event_names(&rows);
    let count = |needle: &str| names.iter().filter(|name| *name == needle).count();
    assert_eq!(count("integration_promoted"), 1);
    assert_eq!(
        count("integration_failed"),
        0,
        "a downstream refinement is never a divergent-writers conflict"
    );
    // node-b's contribution is its own work only — the unmodified
    // inherited notes file is not re-exported.
    let b_paths = rows
        .iter()
        .filter_map(|row| {
            let value: serde_json::Value = serde_json::from_str(&row.event_json).ok()?;
            (value.get("kind")?.as_str()? == "staging_root_updated"
                && value.get("payload")?.get("node_id")?.as_str()? == "node-b")
                .then(|| value["payload"]["paths"].clone())
        })
        .next_back()
        .expect("node-b staged");
    assert_eq!(
        b_paths,
        serde_json::json!(["src/a.rs"]),
        "inherited unmodified files must not re-export"
    );
    // The out-of-footprint write was denied by the write scope.
    let tool_events = tool_event_names(&rows);
    assert!(
        tool_events.iter().any(|name| name == "effect_denied"),
        "the write outside output_targets must be denied"
    );
}

/// Gate AA falsifier: no user-workspace state may contain one node winner
/// without the rest of its verified integration root. Two individually
/// hard-passing winners whose combination fails are rejected whole.
#[tokio::test]
async fn winners_that_fail_composed_never_reach_the_workspace() {
    let project = tempfile::tempdir().unwrap();
    write_coupled_project(project.path());
    let database = project.path().join("runtime.db");
    let transport = Arc::new(GoalRouted {
        plan: Mutex::new(Some(plan_turn())),
        alpha: Mutex::new(vec![edit_turn(
            "a-1",
            "src/a.rs",
            "pub fn alpha() -> u32 { 1 }",
            "pub fn alpha() -> u32 { 2 }",
        )]),
        beta: Mutex::new(vec![edit_turn(
            "b-1",
            "src/b.rs",
            "pub fn beta() -> u32 { 2 }",
            "pub fn beta() -> u32 { 3 }",
        )]),
    });
    let before_a = std::fs::read_to_string(project.path().join("src/a.rs")).unwrap();
    let before_b = std::fs::read_to_string(project.path().join("src/b.rs")).unwrap();
    let runtime = Psp9AgentRuntime::with_transport(
        project.path().to_path_buf(),
        transport,
        ModelId::new("test", "scripted"),
        Psp9RunConfig {
            approval_policy: ApprovalPolicy::Auto,
            max_turns: 3,
            allow_unisolated_verifiers: true,
            max_parallel_nodes: 2,
            ..Psp9RunConfig::default()
        },
    )
    .with_database_path(database.clone());

    let summary = runtime.run("make the coupled change".into()).await.unwrap();

    // Both winners staged; integration measured the combined state, failed,
    // and nothing was promoted.
    let store = perspt_store::SessionStore::open(&database).unwrap();
    let rows = store.get_psp9_events(&summary.session_id).unwrap();
    let names = event_names(&rows);
    let count = |needle: &str| names.iter().filter(|name| *name == needle).count();
    assert_eq!(count("staging_root_updated"), 2, "both winners staged");
    assert_eq!(count("integration_measured"), 1, "one global gate");
    assert!(count("integration_failed") >= 1, "integration rejected");
    assert_eq!(count("integration_promoted"), 0);
    assert!(
        !matches!(summary.outcome, NodeTerminalOutcome::HardPass),
        "a failed integration is not a hard pass"
    );
    assert!(summary.promoted_paths.is_empty(), "no partial promotion");
    // The user workspace is byte-identical.
    let after_a = std::fs::read_to_string(project.path().join("src/a.rs")).unwrap();
    let after_b = std::fs::read_to_string(project.path().join("src/b.rs")).unwrap();
    assert_eq!(before_a, after_a);
    assert_eq!(before_b, after_b);
}

/// Disjoint compatible winners integrate and promote atomically: the
/// promotion carries every staged path or none.
#[tokio::test]
async fn compatible_winners_promote_through_one_integration_root() {
    let project = tempfile::tempdir().unwrap();
    // Uncoupled fixture: doc-comment edits cannot conflict.
    std::fs::create_dir_all(project.path().join("src")).unwrap();
    std::fs::write(
        project.path().join("Cargo.toml"),
        "[package]\nname='integration-fixture'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    std::fs::write(
        project.path().join("src/lib.rs"),
        "pub mod a;\npub mod b;\n",
    )
    .unwrap();
    std::fs::write(
        project.path().join("src/a.rs"),
        "pub fn alpha() -> u32 { 1 }\n",
    )
    .unwrap();
    std::fs::write(
        project.path().join("src/b.rs"),
        "pub fn beta() -> u32 { 2 }\n",
    )
    .unwrap();
    let database = project.path().join("runtime.db");
    let transport = Arc::new(GoalRouted {
        plan: Mutex::new(Some(plan_turn())),
        alpha: Mutex::new(vec![edit_turn(
            "a-1",
            "src/a.rs",
            "pub fn alpha() -> u32 { 1 }",
            "/// Alpha.\npub fn alpha() -> u32 { 1 }",
        )]),
        beta: Mutex::new(vec![edit_turn(
            "b-1",
            "src/b.rs",
            "pub fn beta() -> u32 { 2 }",
            "/// Beta.\npub fn beta() -> u32 { 2 }",
        )]),
    });
    let runtime = Psp9AgentRuntime::with_transport(
        project.path().to_path_buf(),
        transport,
        ModelId::new("test", "scripted"),
        Psp9RunConfig {
            approval_policy: ApprovalPolicy::Auto,
            max_turns: 3,
            allow_unisolated_verifiers: true,
            max_parallel_nodes: 2,
            ..Psp9RunConfig::default()
        },
    )
    .with_database_path(database.clone());

    let summary = runtime.run("annotate both modules".into()).await.unwrap();
    assert!(matches!(summary.outcome, NodeTerminalOutcome::HardPass));
    let mut promoted = summary.promoted_paths.clone();
    promoted.sort();
    assert_eq!(promoted, ["src/a.rs", "src/b.rs"], "atomic: both or none");

    let store = perspt_store::SessionStore::open(&database).unwrap();
    let rows = store.get_psp9_events(&summary.session_id).unwrap();
    let names = event_names(&rows);
    let count = |needle: &str| names.iter().filter(|name| *name == needle).count();
    assert_eq!(count("staging_root_updated"), 2);
    assert_eq!(count("integration_promoted"), 1);
    assert_eq!(count("integration_failed"), 0);
    assert!(std::fs::read_to_string(project.path().join("src/a.rs"))
        .unwrap()
        .contains("/// Alpha."));
}
