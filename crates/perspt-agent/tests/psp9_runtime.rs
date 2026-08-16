use std::sync::{Arc, Mutex};

use perspt_agent::{Psp9AgentRuntime, Psp9RunConfig};
use perspt_sdk::{
    ApprovalPolicy, Conversation, ModelFamily, ModelId, ModelTransport, NodeTerminalOutcome,
    ProviderCapabilities, ProviderToolCall, ToolChoicePolicy, ToolSpec, TransportFuture,
    TurnOutput,
};

struct ScriptedTransport {
    turns: Mutex<Vec<TurnOutput>>,
}

impl ModelTransport for ScriptedTransport {
    fn chat_turn<'a>(
        &'a self,
        _model: &'a ModelId,
        _conversation: &'a Conversation,
        _tools: &'a [ToolSpec],
        _choice: ToolChoicePolicy,
    ) -> TransportFuture<'a, TurnOutput> {
        let output = self.turns.lock().unwrap().remove(0);
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
        "[package]\nname='runtime-fixture'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    std::fs::write(
        project.join("src/lib.rs"),
        "pub fn answer() -> u32 { 1 }\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    \
         fn answer_is_two() { assert_eq!(super::answer(), 2); }\n}\n",
    )
    .unwrap();
}

fn scripted_edit_transport() -> Arc<ScriptedTransport> {
    Arc::new(ScriptedTransport {
        turns: Mutex::new(vec![
            TurnOutput::ToolCalls(vec![ProviderToolCall {
                call_id: "edit-1".into(),
                name: "edit_file".into(),
                arguments: serde_json::json!({
                    "path": "src/lib.rs",
                    "old_string": "pub fn answer() -> u32 { 1 }",
                    "new_string": "pub fn answer() -> u32 { 2 }"
                }),
            }]),
            TurnOutput::Text("verification requested".into()),
        ]),
    })
}

#[tokio::test]
async fn production_runtime_edits_verifies_promotes_and_records() {
    let project = tempfile::tempdir().unwrap();
    write_fixture_project(project.path());
    let database = project.path().join("runtime.db");
    let runtime = Psp9AgentRuntime::with_transport(
        project.path().to_path_buf(),
        scripted_edit_transport(),
        ModelId::new("test", "scripted"),
        Psp9RunConfig {
            approval_policy: ApprovalPolicy::Auto,
            max_turns: 4,
            allow_unisolated_verifiers: true,
            ..Psp9RunConfig::default()
        },
    )
    .with_database_path(database.clone());

    let summary = runtime.run("make answer return two".into()).await.unwrap();
    assert!(matches!(summary.outcome, NodeTerminalOutcome::HardPass));
    assert_eq!(summary.promoted_paths, ["src/lib.rs"]);
    assert!(std::fs::read_to_string(project.path().join("src/lib.rs"))
        .unwrap()
        .contains("{ 2 }"));

    let store = perspt_store::SessionStore::open(&database).unwrap();
    let rows = store.get_psp9_events(&summary.session_id).unwrap();
    assert!(
        rows.len() >= 10,
        "live run must emit a complete event trace"
    );
    assert!(rows
        .windows(2)
        .all(|pair| pair[1].prev_hash == pair[0].hash));
    assert!(rows
        .iter()
        .any(|row| row.event_json.contains("turn_observed")));
    assert!(rows
        .iter()
        .any(|row| row.event_json.contains("provider_capability_evidence")));
    assert!(rows.iter().any(|row| {
        row.event_json.contains("calibration_readiness")
            && row.event_json.contains("\"certified_for_promotion\":false")
    }));
    assert!(rows
        .iter()
        .any(|row| row.event_json.contains("candidate_promoted")));
    let mut ledger = perspt_sdk::Ledger::new();
    for row in rows {
        let event = serde_json::from_str(&row.event_json).unwrap();
        ledger.append(event).unwrap();
    }
    let replay = perspt_sdk::audit_replay(&ledger);
    assert!(replay.chain_ok);
    assert_eq!(replay.accepted, [("implement-1".into(), 0, 0.0)]);
}
