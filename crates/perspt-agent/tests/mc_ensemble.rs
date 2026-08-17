//! Mechanism check: live proposal ensembles (Gates M, T).
//!
//! After the first attempt exhausts its gate budget, the refine rung draws
//! one width-2 distinct-family round. Each candidate runs the ordinary
//! governed loop with exactly one gate decision; selection is strictly by
//! measured energy; the round and every candidate are ledgered.

use std::sync::{Arc, Mutex};

use perspt_agent::{Psp9AgentRuntime, Psp9RunConfig};
use perspt_sdk::{
    ApprovalPolicy, Conversation, EnsemblePolicy, EnsembleTrigger, ModelFamily, ModelId,
    ModelTransport, NodeTerminalOutcome, ProviderCapabilities, ProviderToolCall, ToolChoicePolicy,
    ToolSpec, TransportFuture, TurnOutput,
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

    /// Each scripted route is its own family, so the distinct-family rule
    /// is exercised for real.
    fn family_of(&self, model: &ModelId) -> ModelFamily {
        ModelFamily::Other(model.model.clone())
    }
}

fn write_fixture_project(project: &std::path::Path) {
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join("Cargo.toml"),
        "[package]\nname='ensemble-fixture'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    std::fs::write(
        project.join("src/lib.rs"),
        "pub fn answer() -> u32 { 1 }\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    \
         fn answer_is_two() { assert_eq!(super::answer(), 2); }\n}\n",
    )
    .unwrap();
}

fn fix_call() -> TurnOutput {
    TurnOutput::ToolCalls(vec![ProviderToolCall {
        call_id: "fix-1".into(),
        name: "edit_file".into(),
        arguments: serde_json::json!({
            "path": "src/lib.rs",
            "old_string": "pub fn answer() -> u32 { 1 }",
            "new_string": "pub fn answer() -> u32 { 2 }"
        }),
    }])
}

#[tokio::test]
async fn gate_failure_triggers_a_distinct_family_round_selected_by_energy() {
    let project = tempfile::tempdir().unwrap();
    write_fixture_project(project.path());
    let database = project.path().join("runtime.db");
    let transport = Arc::new(ScriptedTransport {
        turns: Mutex::new(vec![
            // Attempt 1 (route alpha): two unproductive turns exhaust it.
            TurnOutput::Text("thinking".into()),
            TurnOutput::Text("still thinking".into()),
            // Ensemble candidate alpha: one unproductive turn, one gate decision.
            TurnOutput::Text("no idea".into()),
            // Ensemble candidate beta: the actual fix.
            fix_call(),
            TurnOutput::Text("done".into()),
        ]),
    });
    let runtime = Psp9AgentRuntime::with_transport(
        project.path().to_path_buf(),
        transport,
        ModelId::new("test", "alpha"),
        Psp9RunConfig {
            approval_policy: ApprovalPolicy::Auto,
            max_turns: 2,
            rejection_budget: 4,
            allow_unisolated_verifiers: true,
            ..Psp9RunConfig::default()
        },
    )
    .with_fallback_models(vec![ModelId::new("test", "beta")])
    .with_ensemble_policy(EnsemblePolicy {
        trigger: EnsembleTrigger::AfterGateFailure,
        width: 2,
        require_distinct_family: true,
    })
    .with_database_path(database.clone());

    let summary = runtime.run("make answer return two".into()).await.unwrap();
    assert!(matches!(summary.outcome, NodeTerminalOutcome::HardPass));

    let store = perspt_store::SessionStore::open(&database).unwrap();
    let rows = store.get_psp9_events(&summary.session_id).unwrap();
    let count = |needle: &str| {
        rows.iter()
            .filter(|row| row.event_json.contains(needle))
            .count()
    };
    assert_eq!(count("\"ensemble_round\""), 1, "one round drawn");
    assert_eq!(
        count("\"ensemble_candidate\""),
        2,
        "each proposer is one ledgered gate decision"
    );
    assert_eq!(count("\"ensemble_selected\""), 1);
    let selected = rows
        .iter()
        .find(|row| row.event_json.contains("\"ensemble_selected\""))
        .unwrap();
    assert!(
        selected.event_json.contains("\"hard_pass\":true"),
        "selection is by measured energy, and the fix won: {}",
        selected.event_json
    );
}
