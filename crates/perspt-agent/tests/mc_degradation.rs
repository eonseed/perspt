//! MC-U (Gate U): every absent provider capability is recorded as an explicit
//! degradation with its mitigation named — never silently emulated. The
//! ladder is read back from the durable ledger, not from the transport.

use std::sync::{Arc, Mutex};

use perspt_agent::{Psp9AgentRuntime, Psp9RunConfig};
use perspt_sdk::{
    ApprovalPolicy, Conversation, ModelFamily, ModelId, ModelTransport, ProviderCapabilities,
    ToolChoicePolicy, ToolSpec, TransportFuture, TurnOutput,
};

/// Declares tool calling only; every optional capability is absent.
struct DegradedTransport {
    turns: Mutex<Vec<TurnOutput>>,
}

impl ModelTransport for DegradedTransport {
    fn chat_turn<'a>(
        &'a self,
        _model: &'a ModelId,
        _conversation: &'a Conversation,
        _tools: &'a [ToolSpec],
        _choice: ToolChoicePolicy,
    ) -> TransportFuture<'a, TurnOutput> {
        let mut turns = self.turns.lock().unwrap();
        let output = if turns.is_empty() {
            TurnOutput::Text("done".into())
        } else {
            turns.remove(0)
        };
        Box::pin(async move { Ok(output) })
    }

    fn capabilities(&self, _model: &ModelId) -> ProviderCapabilities {
        ProviderCapabilities {
            tool_calling: true,
            strict_schema: false,
            parallel_tool_calls: false,
            streaming_tool_calls: false,
            prompt_caching: false,
            structured_output: false,
            max_context_tokens: 32_000,
        }
    }

    fn family_of(&self, _model: &ModelId) -> ModelFamily {
        ModelFamily::Other("degraded".into())
    }
}

#[tokio::test]
async fn mc_u_absent_capabilities_are_recorded_with_named_mitigations() {
    let project = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(project.path().join("src")).unwrap();
    std::fs::write(
        project.path().join("Cargo.toml"),
        "[package]\nname='degradation-fixture'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    std::fs::write(project.path().join("src/lib.rs"), "pub fn ok() {}\n").unwrap();

    let transport = Arc::new(DegradedTransport {
        turns: Mutex::new(vec![TurnOutput::Text("nothing to do".into())]),
    });
    let database = project.path().join("runtime.db");
    let runtime = Psp9AgentRuntime::with_transport(
        project.path().to_path_buf(),
        transport,
        ModelId::new("test", "degraded"),
        Psp9RunConfig {
            approval_policy: ApprovalPolicy::Auto,
            max_turns: 2,
            allow_unisolated_verifiers: true,
            ..Psp9RunConfig::default()
        },
    )
    .with_database_path(database.clone());

    let summary = runtime.run("record the ladder".into()).await.unwrap();

    let store = perspt_store::SessionStore::open(&database).unwrap();
    let rows = store.get_psp9_events(&summary.session_id).unwrap();
    let evidence = rows
        .iter()
        .find_map(|row| {
            let value: serde_json::Value = serde_json::from_str(&row.event_json).ok()?;
            (value.get("kind")?.as_str()? == "provider_capability_evidence")
                .then(|| value.get("payload").cloned())?
        })
        .expect("every route records capability evidence at session start");

    let degradations: Vec<String> = evidence
        .get("degradations")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .expect("degradations list present");
    for expected in [
        "strict_schema:local_validation",
        "parallel_tool_calls:sequential_execution",
        "streaming_tool_calls:turn_granular_progress",
        "prompt_caching:cache_cold_accounting",
        "structured_output:local_parse_and_validation",
    ] {
        assert!(
            degradations.iter().any(|entry| entry == expected),
            "absent capability must record its mitigation: {expected} (got {degradations:?})"
        );
    }
    assert_eq!(
        evidence.get("source").and_then(|v| v.as_str()),
        Some("declared"),
        "unprobed evidence must say so"
    );
}
