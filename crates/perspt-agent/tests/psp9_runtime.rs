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

/// Mid-loop resume (PSP-9 resolved decision 6): a session killed after an
/// accepted durable checkpoint — here, while waiting for promotion approval —
/// is continued by rebuilding the accepted candidate from content-addressed
/// artifacts and re-entering the loop, and the resumed session promotes.
#[tokio::test]
async fn interrupted_session_resumes_from_its_durable_candidate_checkpoint() {
    let project = tempfile::tempdir().unwrap();
    write_fixture_project(project.path());
    let database = project.path().join("runtime.db");
    // One shared handle for runtime, poller, and the resumed runtime: DuckDB
    // permits one live handle per file per process.
    let store = Arc::new(perspt_store::SessionStore::open(&database).unwrap());

    // Phase 1: run with Ask approval and a connected-but-silent approver, so
    // the task hangs right after the durable checkpoint; abort simulates the
    // crash (status stays RUNNING_PSP9, epoch untouched).
    let (event_sender, _event_receiver) = perspt_core::events::channel::event_channel();
    let (_action_sender, action_receiver) = perspt_core::events::channel::action_channel();
    let runtime = Psp9AgentRuntime::with_transport(
        project.path().to_path_buf(),
        scripted_edit_transport(),
        ModelId::new("test", "scripted"),
        Psp9RunConfig {
            approval_policy: ApprovalPolicy::Ask,
            max_turns: 4,
            allow_unisolated_verifiers: true,
            ..Psp9RunConfig::default()
        },
    )
    .with_session_store(store.clone())
    .connect_tui(event_sender, action_receiver);
    let handle = tokio::spawn(async move { runtime.run("make answer return two".into()).await });

    // Wait for the durable candidate checkpoint to land, then kill the task.
    let mut session_id = None;
    for _ in 0..600 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let Some(session) = store.list_recent_sessions(1).unwrap_or_default().pop() else {
            continue;
        };
        let checkpoint = store
            .latest_psp9_checkpoint(&session.session_id)
            .unwrap_or(None);
        let is_candidate = checkpoint
            .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
            .is_some_and(|v| v.get("kind").and_then(|k| k.as_str()) == Some("candidate"));
        if is_candidate {
            session_id = Some(session.session_id.clone());
            break;
        }
    }
    let session_id = session_id.expect("durable candidate checkpoint recorded before approval");
    let checkpoint: serde_json::Value = serde_json::from_str(
        &store
            .latest_psp9_checkpoint(&session_id)
            .unwrap()
            .expect("candidate checkpoint"),
    )
    .unwrap();
    let control: perspt_sdk::ControlFrame =
        serde_json::from_value(checkpoint["control"].clone()).unwrap();
    let conversation: Conversation =
        serde_json::from_value(checkpoint["conversation"].clone()).unwrap();
    assert_eq!(control.active_model, ModelId::new("test", "scripted"));
    assert!(control.remaining_fallback_models.is_empty());
    assert!(conversation.messages().iter().any(|message| matches!(
        message,
        perspt_sdk::Message::AssistantToolCalls { calls }
            if calls.iter().any(|call| call.call_id == "edit-1")
    )));
    let graph_revisions_before_resume = store
        .get_psp9_events(&session_id)
        .unwrap()
        .iter()
        .filter(|row| row.event_json.contains("\"kind\":\"graph_revision\""))
        .count();
    handle.abort();
    let _ = handle.await;
    assert_eq!(
        store.get_session(&session_id).unwrap().unwrap().status,
        "RUNNING_PSP9",
        "a killed task leaves the session running — the crash fixture"
    );

    // Phase 2: resume with a fresh runtime; the seeded candidate already
    // passes, so the loop hard-passes at baseline and promotes.
    let resume_transport = Arc::new(ScriptedTransport {
        turns: Mutex::new(vec![]),
    });
    let resumed = Psp9AgentRuntime::with_transport(
        project.path().to_path_buf(),
        resume_transport,
        ModelId::new("test", "scripted"),
        Psp9RunConfig {
            approval_policy: ApprovalPolicy::Auto,
            max_turns: 2,
            allow_unisolated_verifiers: true,
            ..Psp9RunConfig::default()
        },
    )
    .with_session_store(store.clone());
    let summary = resumed.resume_session(session_id.clone()).await.unwrap();
    assert!(matches!(summary.outcome, NodeTerminalOutcome::HardPass));
    assert_eq!(summary.promoted_paths, ["src/lib.rs"]);
    let promoted = std::fs::read_to_string(project.path().join("src/lib.rs")).unwrap();
    let status = store.get_session(&session_id).unwrap().unwrap().status;
    assert!(promoted.contains("{ 2 }") && status == "COMPLETED_PSP9");
    let resumed_rows = store.get_psp9_events(&session_id).unwrap();
    let graph_events: Vec<_> = resumed_rows
        .iter()
        .filter_map(|row| {
            let event: perspt_sdk::LedgerEvent = serde_json::from_str(&row.event_json).ok()?;
            let perspt_sdk::LedgerEvent::Custom { kind, payload } = event else {
                return None;
            };
            (kind == "graph_revision")
                .then(|| serde_json::from_value::<perspt_sdk::WorkGraphRevision>(payload).unwrap())
        })
        .collect();
    assert_eq!(
        graph_events.len(),
        graph_revisions_before_resume + 1,
        "resume may append the terminal revision but must not fabricate a new root"
    );
    assert_eq!(
        graph_events.last().unwrap().parent_revision_id.as_deref(),
        Some(control.graph_revision.as_str())
    );
    assert!(resumed_rows
        .iter()
        .any(|row| row.event_json.contains("graph_revision_resumed")));
}
