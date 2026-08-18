//! Mechanism checks for governed dependency mutation (Gate J).
//!
//! Default posture: `MutateDependencies` is withheld from every grant, so a
//! model-proposed dependency change is a recorded capability denial. With
//! the explicit opt-in, the effect resolves through the language plugin's
//! dependency commands and is bracketed in the external-effect log (the
//! live network fixture is `#[ignore]`d for offline CI).

use std::sync::{Arc, Mutex};

use perspt_agent::{Psp9AgentRuntime, Psp9RunConfig};
use perspt_sdk::{
    ApprovalPolicy, Conversation, ModelFamily, ModelId, ModelTransport, ProviderCapabilities,
    ProviderToolCall, ToolChoicePolicy, ToolSpec, TransportFuture, TurnOutput,
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
        let mut turns = self.turns.lock().unwrap();
        let output = if turns.is_empty() {
            TurnOutput::Text("no further actions".into())
        } else {
            turns.remove(0)
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

fn write_rust_fixture(project: &std::path::Path) {
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join("Cargo.toml"),
        "[package]\nname='dep-fixture'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    std::fs::write(project.join("src/lib.rs"), "pub fn answer() -> u32 { 2 }\n").unwrap();
}

fn dependency_call() -> TurnOutput {
    TurnOutput::ToolCalls(vec![ProviderToolCall {
        call_id: "dep-1".into(),
        name: "mutate_dependencies".into(),
        arguments: serde_json::json!({"action": "add", "packages": ["itertools"]}),
    }])
}

#[tokio::test]
async fn dependency_mutation_is_denied_without_the_explicit_grant() {
    let project = tempfile::tempdir().unwrap();
    write_rust_fixture(project.path());
    let database = project.path().join("runtime.db");
    let transport = Arc::new(ScriptedTransport {
        turns: Mutex::new(vec![
            dependency_call(),
            TurnOutput::Text("stopping".into()),
            TurnOutput::Text("stopping".into()),
            TurnOutput::Text("stopping".into()),
        ]),
    });
    let runtime = Psp9AgentRuntime::with_transport(
        project.path().to_path_buf(),
        transport,
        ModelId::new("test", "scripted"),
        Psp9RunConfig {
            approval_policy: ApprovalPolicy::Auto,
            max_turns: 3,
            allow_unisolated_verifiers: true,
            ..Psp9RunConfig::default()
        },
    )
    .with_database_path(database.clone());

    let summary = runtime.run("add itertools".into()).await.unwrap();
    let store = perspt_store::SessionStore::open(&database).unwrap();
    let rows = store.get_psp9_events(&summary.session_id).unwrap();
    let denied = rows.iter().any(|row| {
        row.event_json.contains("effect_denied")
            && row.event_json.contains("\"call_id\":\"dep-1\"")
            && row.event_json.contains("capability_denied")
    });
    assert!(
        denied,
        "an ungranted dependency mutation must be a recorded capability denial"
    );
    assert!(
        !project.path().join("Cargo.lock").exists(),
        "the denied mutation must not have touched the workspace"
    );
}

/// Live fixture (network required): `cargo add itertools` resolves through
/// the governed handler and both manifest and lockfile promote with the
/// bracket visible. Run with `cargo test -- --ignored` when online.
#[tokio::test]
#[ignore = "requires network access for cargo add"]
async fn governed_dependency_mutation_promotes_manifest_and_lockfile() {
    let project = tempfile::tempdir().unwrap();
    write_rust_fixture(project.path());
    let database = project.path().join("runtime.db");
    let transport = Arc::new(ScriptedTransport {
        turns: Mutex::new(vec![dependency_call(), TurnOutput::Text("done".into())]),
    });
    let runtime = Psp9AgentRuntime::with_transport(
        project.path().to_path_buf(),
        transport,
        ModelId::new("test", "scripted"),
        Psp9RunConfig {
            approval_policy: ApprovalPolicy::Auto,
            max_turns: 3,
            allow_unisolated_verifiers: true,
            allow_dependency_mutation: true,
            ..Psp9RunConfig::default()
        },
    )
    .with_database_path(database.clone());

    let summary = runtime.run("add itertools".into()).await.unwrap();
    assert!(summary.promoted_paths.contains(&"Cargo.toml".to_string()));
    assert!(summary.promoted_paths.contains(&"Cargo.lock".to_string()));
    assert!(std::fs::read_to_string(project.path().join("Cargo.toml"))
        .unwrap()
        .contains("itertools"));

    let store = perspt_store::SessionStore::open(&database).unwrap();
    let effects = store.pending_external_effects(&summary.session_id).unwrap();
    assert!(
        effects.is_empty(),
        "every external-effect bracket must be closed"
    );
}
