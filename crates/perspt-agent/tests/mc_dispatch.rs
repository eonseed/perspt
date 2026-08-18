//! Gate P mechanism checks for bounded multi-node dispatch.
//!
//! * Disjoint output targets at `max_parallel_nodes = 2`: both nodes run,
//!   both promote, and the second dispatch happens while the first node is
//!   still running (proof of concurrency).
//! * Identical output targets: the scheduler refuses the second slot, so
//!   the dispatch spans never interleave.
//! * Two failing nodes share ONE non-replenishing recovery pool — the
//!   ledgered claims never exceed the session budget.

use std::sync::{Arc, Mutex};

use perspt_agent::{Psp9AgentRuntime, Psp9RunConfig};
use perspt_sdk::{
    ApprovalPolicy, Conversation, ModelFamily, ModelId, ModelTransport, NodeTerminalOutcome,
    ProviderCapabilities, ProviderToolCall, ToolChoicePolicy, ToolSpec, TransportFuture,
    TurnOutput,
};

/// Routes turns by conversation content: the architect planning turn gets
/// the graph plan; each node's loop pops from its own script, keyed by a
/// marker in the node goal. Concurrency-safe and deterministic.
struct GoalRouted {
    plan: Mutex<Option<TurnOutput>>,
    alpha: Mutex<Vec<TurnOutput>>,
    beta: Mutex<Vec<TurnOutput>>,
}

impl GoalRouted {
    fn text_of(conversation: &Conversation) -> String {
        conversation
            .messages()
            .iter()
            .map(|message| format!("{message:?}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl ModelTransport for GoalRouted {
    fn chat_turn<'a>(
        &'a self,
        _model: &'a ModelId,
        conversation: &'a Conversation,
        _tools: &'a [ToolSpec],
        _choice: ToolChoicePolicy,
    ) -> TransportFuture<'a, TurnOutput> {
        let text = Self::text_of(conversation);
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
}

fn write_fixture_project(project: &std::path::Path) {
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join("Cargo.toml"),
        "[package]\nname='dispatch-fixture'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    std::fs::write(project.join("src/lib.rs"), "pub mod a;\npub mod b;\n").unwrap();
    std::fs::write(project.join("src/a.rs"), "pub fn alpha() -> u32 { 1 }\n").unwrap();
    std::fs::write(project.join("src/b.rs"), "pub fn beta() -> u32 { 2 }\n").unwrap();
}

fn plan_turn(a_target: &str, b_target: &str) -> TurnOutput {
    let revision = serde_json::json!({
        "nodes": [
            {"node_id": "node-a", "goal": "alpha-part: annotate src/a.rs",
             "output_targets": [a_target]},
            {"node_id": "node-b", "goal": "beta-part: annotate src/b.rs",
             "output_targets": [b_target]},
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

fn runtime_with(
    project: &std::path::Path,
    transport: Arc<GoalRouted>,
    parallel: usize,
) -> Psp9AgentRuntime {
    Psp9AgentRuntime::with_transport(
        project.to_path_buf(),
        transport,
        ModelId::new("test", "scripted"),
        Psp9RunConfig {
            approval_policy: ApprovalPolicy::Auto,
            max_turns: 3,
            allow_unisolated_verifiers: true,
            max_parallel_nodes: parallel,
            ..Psp9RunConfig::default()
        },
    )
}

#[tokio::test]
async fn disjoint_nodes_run_concurrently_and_both_promote() {
    let project = tempfile::tempdir().unwrap();
    write_fixture_project(project.path());
    let database = project.path().join("runtime.db");
    let transport = Arc::new(GoalRouted {
        plan: Mutex::new(Some(plan_turn("src/a.rs", "src/b.rs"))),
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
    let runtime = runtime_with(project.path(), transport, 2).with_database_path(database.clone());

    let summary = runtime.run("annotate both modules".into()).await.unwrap();
    assert!(matches!(summary.outcome, NodeTerminalOutcome::HardPass));
    let mut promoted = summary.promoted_paths.clone();
    promoted.sort();
    assert_eq!(promoted, ["src/a.rs", "src/b.rs"]);

    let store = perspt_store::SessionStore::open(&database).unwrap();
    let rows = store.get_psp9_events(&summary.session_id).unwrap();
    let dispatch_sequences: Vec<i64> = rows
        .iter()
        .filter(|row| row.event_json.contains("\"kind\":\"scheduler_dispatch\""))
        .map(|row| row.sequence)
        .collect();
    let first_terminal = rows
        .iter()
        .find(|row| row.event_json.contains("\"kind\":\"node_terminal\""))
        .map(|row| row.sequence)
        .unwrap();
    assert_eq!(dispatch_sequences.len(), 2);
    assert!(
        dispatch_sequences[1] < first_terminal,
        "the second node must dispatch while the first is still running"
    );
    // PSP-10 Phase 1: the architect's update_graph proposal passed the
    // admissibility kernel — the witness is ledgered before the plan lands.
    let admissibility = rows
        .iter()
        .find(|row| {
            row.event_json
                .contains("\"kind\":\"graph_plan_admissibility\"")
        })
        .map(|row| row.sequence)
        .expect("architect turn must ledger its admissibility witness");
    let planned = rows
        .iter()
        .find(|row| row.event_json.contains("\"kind\":\"graph_planned\""))
        .map(|row| row.sequence)
        .unwrap();
    assert!(admissibility < planned, "kernel check precedes the plan");
    // Replay folds: the recorded chain verifies end to end.
    assert!(rows
        .windows(2)
        .all(|pair| pair[1].prev_hash == pair[0].hash));
}

/// PSP-10 Phase 1: an architect call whose arguments do not satisfy the real
/// catalog schema is refused before any graph is built — the session falls
/// back to the deterministic single-node graph and records why.
#[tokio::test]
async fn schema_invalid_plan_falls_back_to_the_initial_graph() {
    let project = tempfile::tempdir().unwrap();
    write_fixture_project(project.path());
    let database = project.path().join("runtime.db");
    let transport = Arc::new(GoalRouted {
        // `revision` must be a string per the catalog schema; an object is
        // schema-invalid even though serde could still reach inside it.
        plan: Mutex::new(Some(TurnOutput::ToolCalls(vec![ProviderToolCall {
            call_id: "plan-bad".into(),
            name: "update_graph".into(),
            arguments: serde_json::json!({"revision": {"nodes": []}}),
        }]))),
        alpha: Mutex::new(vec![]),
        beta: Mutex::new(vec![]),
    });
    let runtime = runtime_with(project.path(), transport, 2).with_database_path(database.clone());
    let summary = runtime.run("annotate both modules".into()).await.unwrap();
    let store = perspt_store::SessionStore::open(&database).unwrap();
    let rows = store.get_psp9_events(&summary.session_id).unwrap();
    assert!(
        rows.iter()
            .any(|row| row.event_json.contains("\"kind\":\"graph_plan_fallback\"")),
        "schema-invalid plan must record its fallback"
    );
    assert!(
        !rows
            .iter()
            .any(|row| row.event_json.contains("\"kind\":\"graph_planned\"")),
        "no multi-node graph may land from a schema-invalid call"
    );
}

#[tokio::test]
async fn conflicting_output_targets_never_run_concurrently() {
    let project = tempfile::tempdir().unwrap();
    write_fixture_project(project.path());
    let database = project.path().join("runtime.db");
    let transport = Arc::new(GoalRouted {
        plan: Mutex::new(Some(plan_turn("src/a.rs", "src/a.rs"))),
        alpha: Mutex::new(vec![edit_turn(
            "a-1",
            "src/a.rs",
            "pub fn alpha() -> u32 { 1 }",
            "/// Alpha.\npub fn alpha() -> u32 { 1 }",
        )]),
        beta: Mutex::new(vec![]),
    });
    let runtime = runtime_with(project.path(), transport, 2).with_database_path(database.clone());

    let summary = runtime
        .run("annotate one module twice".into())
        .await
        .unwrap();
    let store = perspt_store::SessionStore::open(&database).unwrap();
    let rows = store.get_psp9_events(&summary.session_id).unwrap();
    let first_b_dispatch = rows
        .iter()
        .find(|row| {
            row.event_json.contains("\"kind\":\"scheduler_dispatch\"")
                && row.event_json.contains("node-b")
        })
        .map(|row| row.sequence)
        .unwrap();
    let first_terminal = rows
        .iter()
        .find(|row| row.event_json.contains("\"kind\":\"node_terminal\""))
        .map(|row| row.sequence)
        .unwrap();
    assert!(
        first_b_dispatch > first_terminal,
        "conflicting footprints must serialize: the second node dispatches \
         only after the first node's terminal fold"
    );
}

#[tokio::test]
async fn failing_nodes_share_one_recovery_pool() {
    let project = tempfile::tempdir().unwrap();
    write_fixture_project(project.path());
    // A failing test makes every node's baseline fail; useless turns burn
    // gate decisions from the SHARED pool.
    std::fs::write(
        project.path().join("src/b.rs"),
        "pub fn beta() -> u32 { 2 }\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    \
         fn beta_is_three() { assert_eq!(super::beta(), 3); }\n}\n",
    )
    .unwrap();
    let database = project.path().join("runtime.db");
    let transport = Arc::new(GoalRouted {
        plan: Mutex::new(Some(plan_turn("src/a.rs", "src/b.rs"))),
        alpha: Mutex::new(vec![]),
        beta: Mutex::new(vec![]),
    });
    let mut config = Psp9RunConfig {
        approval_policy: ApprovalPolicy::Auto,
        max_turns: 2,
        allow_unisolated_verifiers: true,
        max_parallel_nodes: 2,
        ..Psp9RunConfig::default()
    };
    config.rejection_budget = 2;
    let runtime = Psp9AgentRuntime::with_transport(
        project.path().to_path_buf(),
        transport,
        ModelId::new("test", "scripted"),
        config,
    )
    .with_database_path(database.clone());

    let summary = runtime.run("cannot succeed".into()).await.unwrap();
    assert!(!matches!(summary.outcome, NodeTerminalOutcome::HardPass));

    let store = perspt_store::SessionStore::open(&database).unwrap();
    let rows = store.get_psp9_events(&summary.session_id).unwrap();
    let claims: Vec<u64> = rows
        .iter()
        .filter(|row| row.event_json.contains("\"kind\":\"recovery_pool_claim\""))
        .filter_map(|row| {
            serde_json::from_str::<serde_json::Value>(&row.event_json)
                .ok()?
                .pointer("/payload/claimed")?
                .as_u64()
        })
        .collect();
    assert!(!claims.is_empty());
    assert!(
        claims.iter().sum::<u64>() <= 2 + 2,
        "initial claims plus refunded re-claims never exceed the shared \
         budget in flight: {claims:?}"
    );
}
