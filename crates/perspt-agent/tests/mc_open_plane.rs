//! Mechanism check: the open execution plane's exit criterion.
//!
//! A tool family registers from OUTSIDE perspt-agent — catalog entries via
//! `Psp9AgentRuntime::with_tool_family`, a handler via
//! `CandidateHandlerRegistry::register` — and a scripted-transport run calls
//! it end to end through the kernel: the entry enters the assembled catalog,
//! the derived grant covers its effect, admission validates its scoped
//! footprint, the handler executes, and the call is ledgered. No edit to the
//! loop, candidate, or node modules is involved.

use std::sync::{Arc, Mutex};

use perspt_agent::{
    CandidateHandlerRegistry, CandidateToolHandler, Psp9AgentRuntime, Psp9RunConfig,
};
use perspt_sdk::{
    AccessMode, ApprovalPolicy, Conversation, EffectKind, FootprintSpec, ModelFamily, ModelId,
    ModelTransport, NodeTerminalOutcome, ProviderCapabilities, ProviderToolCall, ResourceSelector,
    RiskClass, ToolChoicePolicy, ToolEntry, ToolOrigin, ToolSpec, TransportFuture, TurnOutput,
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

    fn adapter_kind(&self) -> &'static str {
        "scripted"
    }
}

/// The fixture family: one read-only probe with a scoped (non-file)
/// footprint, implemented entirely in this test crate.
struct FixtureProbe;

#[async_trait::async_trait]
impl CandidateToolHandler for FixtureProbe {
    async fn apply(
        &self,
        _workspace: &perspt_agent::CandidateWorkspace,
        call: &ProviderToolCall,
        _entry: &ToolEntry,
    ) -> anyhow::Result<perspt_agent::toolloop::EffectOutcome> {
        let target = call
            .arguments
            .get("target")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        Ok(perspt_agent::toolloop::EffectOutcome {
            output: format!("probe-ok: {target}"),
            mutated: false,
        })
    }
}

fn fixture_entry() -> ToolEntry {
    ToolEntry {
        name: "fixture_probe".into(),
        description: "Read-only fixture probe registered by the test crate".into(),
        discovery_summary: String::new(),
        description_templates: None,
        effect: EffectKind::SystemProbe,
        risk: RiskClass::Low,
        schema: serde_json::json!({
            "type": "object",
            "properties": {
                "target": {"type": "string", "description": "Probe target"}
            },
            "required": ["target"],
            "additionalProperties": false
        }),
        footprint: FootprintSpec::new(vec![ResourceSelector::ScopedArgument {
            family: "fixture".into(),
            field: "target".into(),
            access: AccessMode::Read,
        }]),
        proposal_bindings: Vec::new(),
        durable: false,
        origin: ToolOrigin::Builtin,
    }
}

fn write_fixture_project(project: &std::path::Path) {
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join("Cargo.toml"),
        "[package]\nname='open-plane-fixture'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    std::fs::write(
        project.join("src/lib.rs"),
        "pub fn answer() -> u32 { 1 }\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    \
         fn answer_is_two() { assert_eq!(super::answer(), 2); }\n}\n",
    )
    .unwrap();
}

#[tokio::test]
async fn registered_family_executes_through_the_kernel_without_agent_edits() {
    let project = tempfile::tempdir().unwrap();
    write_fixture_project(project.path());
    let database = project.path().join("runtime.db");

    let mut handlers = CandidateHandlerRegistry::with_builtins();
    handlers
        .register("fixture_probe", Arc::new(FixtureProbe))
        .unwrap();

    let transport = Arc::new(ScriptedTransport {
        turns: Mutex::new(vec![
            TurnOutput::ToolCalls(vec![ProviderToolCall {
                call_id: "probe-1".into(),
                name: "fixture_probe".into(),
                arguments: serde_json::json!({"target": "toolchain"}),
            }]),
            TurnOutput::ToolCalls(vec![ProviderToolCall {
                call_id: "edit-1".into(),
                name: "edit_file".into(),
                arguments: serde_json::json!({
                    "path": "src/lib.rs",
                    "old_string": "pub fn answer() -> u32 { 1 }",
                    "new_string": "pub fn answer() -> u32 { 2 }"
                }),
            }]),
            TurnOutput::Text("done".into()),
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
            ..Psp9RunConfig::default()
        },
    )
    .with_database_path(database.clone())
    .with_tool_family(vec![fixture_entry()])
    .with_tool_handlers(handlers);

    let summary = runtime.run("probe then fix".into()).await.unwrap();
    assert!(matches!(summary.outcome, NodeTerminalOutcome::HardPass));

    let store = perspt_store::SessionStore::open(&database).unwrap();
    let rows = store.get_psp9_events(&summary.session_id).unwrap();
    let probe_observed = rows.iter().any(|row| {
        row.event_json.contains("tool_call_observed") && row.event_json.contains("fixture_probe")
    });
    let probe_applied = rows.iter().any(|row| {
        row.event_json.contains("effect_applied")
            && row.event_json.contains("\"call_id\":\"probe-1\"")
            && row.event_json.contains("probe-ok: toolchain")
    });
    assert!(
        probe_observed && probe_applied,
        "the registered family's call must be observed and ledgered as an applied effect"
    );
}
