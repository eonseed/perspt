//! Mechanism check: live pairwise independence rows (Gates R, T).
//!
//! A governed run records the deterministic verifier suite's verdict beside
//! the configured adjudicator's for the same candidate and stratum; the
//! delayed audit label reaches both rows; and the estimator refuses to
//! certify below the matched-sample floor.

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
        "[package]\nname='indep-fixture'\nversion='0.1.0'\nedition='2021'\n",
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
async fn both_validators_record_matched_verdicts_and_labels_reach_them() {
    let project = tempfile::tempdir().unwrap();
    write_fixture_project(project.path());
    let database = project.path().join("runtime.db");
    let transport = Arc::new(ScriptedTransport {
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
            // The adjudicator's tool-free review turn.
            TurnOutput::Text(r#"{"pass": true, "reason": "diff is exactly the fix"}"#.into()),
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
    .with_adjudicator_model(ModelId::new("test", "adjudicator"))
    .with_database_path(database.clone());

    let summary = runtime.run("make answer return two".into()).await.unwrap();
    assert!(matches!(summary.outcome, NodeTerminalOutcome::HardPass));

    let store = perspt_store::SessionStore::open(&database).unwrap();
    let verdicts = store.get_psp9_verdicts(&summary.session_id).unwrap();
    let validators: Vec<&str> = verdicts
        .iter()
        .map(|row| row.validator_id.as_str())
        .collect();
    assert!(
        validators.contains(&"deterministic-suite"),
        "the verifier suite must record a verdict row: {validators:?}"
    );
    assert!(
        validators.iter().any(|v| v.contains("adjudicator")),
        "the adjudicator must record a verdict row: {validators:?}"
    );
    let candidate_ids: std::collections::BTreeSet<&str> = verdicts
        .iter()
        .map(|row| row.candidate_id.as_str())
        .collect();
    assert_eq!(
        candidate_ids.len(),
        1,
        "both verdicts join on the same realized candidate id"
    );

    let candidate_id = candidate_ids.iter().next().unwrap().to_string();
    assert_labels_and_floor(&store, &candidate_id);
}

/// The delayed oracle labels both rows in one single-assignment pass, and
/// one matched labeled pair is far below the certification floor: the
/// estimator refuses to certify rather than fabricate a bound.
fn assert_labels_and_floor(store: &perspt_store::SessionStore, candidate_id: &str) {
    let labeled = store.label_psp9_verdicts(candidate_id, false).unwrap();
    assert_eq!(labeled, 2);
    assert_eq!(store.label_psp9_verdicts(candidate_id, true).unwrap(), 0);

    let rows = store.labeled_psp9_verdicts().unwrap();
    assert_eq!(rows.len(), 2);
    let records: Vec<perspt_sdk::VerdictRecord> = rows
        .iter()
        .map(|row| {
            perspt_sdk::VerdictRecord::new(
                row.validator_id.clone(),
                row.candidate_id.clone(),
                !row.missed && row.unsafe_label.unwrap_or(false),
            )
        })
        .collect();
    let stats = perspt_sdk::independence::compute(&records).unwrap();
    assert_eq!(
        stats.certification,
        perspt_sdk::EnsembleCertification::InsufficientEvidence
    );
    assert!(stats.rho_eff.is_none());
}
