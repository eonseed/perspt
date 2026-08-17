//! Mechanism checks for recovery continuity.
//!
//! * Ladder rungs continue from the previous attempt's best accepted state
//!   (Paper III restore-best): work accepted in generation 0 survives into
//!   the refine attempt instead of being re-paid from scratch.
//! * Containment caused by exhausted provider transport preserves the
//!   session's authority: an infrastructure outage must not revoke the
//!   epoch and brick `perspt resume`.

use std::sync::{Arc, Mutex};

use perspt_agent::{Psp9AgentRuntime, Psp9RunConfig};
use perspt_sdk::{
    ApprovalPolicy, Conversation, ModelFamily, ModelId, ModelTransport, NodeTerminalOutcome,
    ProviderCapabilities, ProviderToolCall, SdkError, ToolChoicePolicy, ToolSpec, TransportFuture,
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
            TurnOutput::Text("no further actions".into())
        } else {
            turns.remove(0)
        };
        Box::pin(async move { Ok(output) })
    }

    fn capabilities(&self, _model: &ModelId) -> ProviderCapabilities {
        scripted_capabilities()
    }

    fn family_of(&self, _model: &ModelId) -> ModelFamily {
        ModelFamily::Other("scripted".into())
    }
}

/// A provider that is hard-down: every turn fails like an infrastructure
/// outage (already past the transport's internal retries).
struct DownTransport;

impl ModelTransport for DownTransport {
    fn chat_turn<'a>(
        &'a self,
        _model: &'a ModelId,
        _conversation: &'a Conversation,
        _tools: &'a [ToolSpec],
        _choice: ToolChoicePolicy,
    ) -> TransportFuture<'a, TurnOutput> {
        Box::pin(async move {
            Err(SdkError::Domain(
                "chat turn failed: status code '504 Gateway Timeout'".into(),
            ))
        })
    }

    fn capabilities(&self, _model: &ModelId) -> ProviderCapabilities {
        scripted_capabilities()
    }

    fn family_of(&self, _model: &ModelId) -> ModelFamily {
        ModelFamily::Other("scripted".into())
    }
}

fn scripted_capabilities() -> ProviderCapabilities {
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

fn write_two_bug_project(project: &std::path::Path) {
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join("Cargo.toml"),
        "[package]\nname='continuity-fixture'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    std::fs::write(
        project.join("src/lib.rs"),
        "pub fn alpha() -> u32 { 1 }\npub fn beta() -> u32 { 1 }\n\n\
         #[cfg(test)]\nmod tests {\n    #[test]\n    \
         fn alpha_is_two() { assert_eq!(super::alpha(), 2); }\n    #[test]\n    \
         fn beta_is_two() { assert_eq!(super::beta(), 2); }\n}\n",
    )
    .unwrap();
}

fn edit(id: &str, old: &str, new: &str) -> TurnOutput {
    TurnOutput::ToolCalls(vec![ProviderToolCall {
        call_id: id.into(),
        name: "edit_file".into(),
        arguments: serde_json::json!({
            "path": "src/lib.rs", "old_string": old, "new_string": new
        }),
    }])
}

#[tokio::test]
async fn refine_rung_continues_from_the_best_accepted_state() {
    let project = tempfile::tempdir().unwrap();
    write_two_bug_project(project.path());
    let database = project.path().join("runtime.db");
    let transport = Arc::new(ScriptedTransport {
        turns: Mutex::new(vec![
            // Attempt 1 (gen 0): fix alpha only, then stall out.
            edit(
                "a-1",
                "pub fn alpha() -> u32 { 1 }",
                "pub fn alpha() -> u32 { 2 }",
            ),
            TurnOutput::Text("half done".into()),
            // Attempt 2 (gen 1, seeded): only beta remains.
            edit(
                "b-1",
                "pub fn beta() -> u32 { 1 }",
                "pub fn beta() -> u32 { 2 }",
            ),
            TurnOutput::Text("done".into()),
        ]),
    });
    let runtime = Psp9AgentRuntime::with_transport(
        project.path().to_path_buf(),
        transport,
        ModelId::new("test", "scripted"),
        Psp9RunConfig {
            approval_policy: ApprovalPolicy::Auto,
            max_turns: 2,
            allow_unisolated_verifiers: true,
            ..Psp9RunConfig::default()
        },
    )
    .with_database_path(database.clone());

    let summary = runtime.run("make both tests pass".into()).await.unwrap();
    assert!(matches!(summary.outcome, NodeTerminalOutcome::HardPass));

    // Both fixes are present: the alpha fix from generation 0 survived
    // into the refined attempt instead of being re-implemented.
    let promoted = std::fs::read_to_string(project.path().join("src/lib.rs")).unwrap();
    assert!(promoted.contains("pub fn alpha() -> u32 { 2 }"));
    assert!(promoted.contains("pub fn beta() -> u32 { 2 }"));

    let store = perspt_store::SessionStore::open(&database).unwrap();
    let rows = store.get_psp9_events(&summary.session_id).unwrap();
    assert!(
        rows.iter()
            .any(|row| row.event_json.contains("\"kind\":\"ladder_reseeded\"")),
        "the refine rung must record the restore-best seed"
    );

    // The seeded attempt's baseline is strictly better than the original
    // baseline: measured continuity, not a fresh overlay.
    let baselines: Vec<(u64, f64)> = rows
        .iter()
        .filter_map(|row| {
            let value: serde_json::Value = serde_json::from_str(&row.event_json).ok()?;
            let payload = value.get("payload")?;
            if payload.get("event")?.as_str()? != "candidate_measured" {
                return None;
            }
            Some((
                payload.get("generation")?.as_u64()?,
                payload.get("energy")?.as_f64()?,
            ))
        })
        .collect();
    let gen0_baseline = baselines.iter().find(|(gen, _)| *gen == 0).unwrap().1;
    let gen1_baseline = baselines.iter().find(|(gen, _)| *gen == 1).unwrap().1;
    assert!(
        gen1_baseline < gen0_baseline,
        "seeded baseline {gen1_baseline} must improve on the original {gen0_baseline}"
    );
}

#[tokio::test]
async fn transport_containment_preserves_authority_for_resume() {
    let project = tempfile::tempdir().unwrap();
    write_two_bug_project(project.path());
    let database = project.path().join("runtime.db");
    let runtime = Psp9AgentRuntime::with_transport(
        project.path().to_path_buf(),
        Arc::new(DownTransport),
        ModelId::new("test", "down"),
        Psp9RunConfig {
            approval_policy: ApprovalPolicy::Auto,
            max_turns: 2,
            allow_unisolated_verifiers: true,
            ..Psp9RunConfig::default()
        },
    )
    .with_database_path(database.clone());

    let summary = runtime.run("cannot even start".into()).await.unwrap();
    assert!(matches!(
        summary.outcome,
        NodeTerminalOutcome::Escalated { .. }
    ));

    let store = perspt_store::SessionStore::open(&database).unwrap();
    let rows = store.get_psp9_events(&summary.session_id).unwrap();
    assert!(
        !rows
            .iter()
            .any(|row| row.event_json.contains("authority_epoch_revoked")),
        "an infrastructure outage must not revoke the session's authority"
    );
    assert!(rows
        .iter()
        .any(|row| row.event_json.contains("containment_preserved_authority")));
    assert_eq!(
        store.authority_epoch(&summary.session_id).unwrap(),
        0,
        "the epoch is untouched, so perspt resume stays viable"
    );
}
