//! Gate AC mechanism checks: reservations precede actions inside the
//! branch tool loop, a refused reservation abandons the branch before the
//! action executes, the closed forest's ledgered usage never sits above a
//! limit, and nothing follows `search_closed`.

use std::sync::{Arc, Mutex};

use perspt_agent::{Psp9AgentRuntime, Psp9RunConfig};
use perspt_sdk::ledger::{tool_loop_body, LedgerEvent, ToolLoopBody};
use perspt_sdk::{
    ApprovalPolicy, Conversation, ModelFamily, ModelId, ModelTransport, ProviderCapabilities,
    SearchLimits, ToolChoicePolicy, ToolSpec, TransportFuture, TurnOutput,
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

    fn family_of(&self, model: &ModelId) -> ModelFamily {
        ModelFamily::Other(model.model.clone())
    }

    fn adapter_kind(&self) -> &'static str {
        "scripted"
    }
}

fn write_fixture_project(project: &std::path::Path) {
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join("Cargo.toml"),
        "[package]\nname='budget-fixture'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    std::fs::write(
        project.join("src/lib.rs"),
        "pub fn answer() -> u32 { 1 }\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    \
         fn answer_is_two() { assert_eq!(super::answer(), 2); }\n}\n",
    )
    .unwrap();
}

fn event_body(row: &perspt_store::Psp9LedgerRow) -> Option<serde_json::Value> {
    let LedgerEvent::Custom { kind, payload } = serde_json::from_str(&row.event_json).ok()? else {
        return None;
    };
    if kind != "tool_loop" {
        return None;
    }
    let (ToolLoopBody::Legacy(body) | ToolLoopBody::V1(body)) = tool_loop_body(&payload).ok()?;
    Some(body.clone())
}

fn event_name(body: &serde_json::Value) -> Option<&str> {
    body.get("event")?.as_str()
}

/// A model-turn limit of one: the branch's second turn is refused before
/// any transport call, the forest ledgers `branch_abandoned`, the closed
/// usage sits at or under every limit, and no tool-loop event follows
/// `search_closed`.
#[tokio::test]
async fn a_refused_turn_reservation_abandons_the_branch_within_limits() {
    let project = tempfile::tempdir().unwrap();
    write_fixture_project(project.path());
    let database = project.path().join("runtime.db");
    let transport = Arc::new(ScriptedTransport {
        turns: Mutex::new(vec![
            // Attempt 1 (outside the forest): exhausted without progress.
            TurnOutput::Text("thinking".into()),
            TurnOutput::Text("still thinking".into()),
            TurnOutput::Text("out of ideas".into()),
            // Forest branch: one reserved turn (a harmless mutation that
            // triggers no boundary); the second turn's reservation is
            // refused before any transport call.
            TurnOutput::ToolCalls(vec![perspt_sdk::ProviderToolCall {
                call_id: "t1".into(),
                name: "edit_file".into(),
                arguments: serde_json::json!({
                    "path": "src/lib.rs",
                    "old_string": "pub fn answer() -> u32 { 1 }",
                    "new_string": "pub fn answer() -> u32 { 1 } // note"
                }),
            }]),
        ]),
    });
    let limits = SearchLimits {
        model_turns: 1,
        ..SearchLimits::release_default()
    };
    let runtime = Psp9AgentRuntime::with_transport(
        project.path().to_path_buf(),
        transport,
        ModelId::new("test", "alpha"),
        Psp9RunConfig {
            approval_policy: ApprovalPolicy::Auto,
            max_turns: 3,
            rejection_budget: 4,
            allow_unisolated_verifiers: true,
            ..Psp9RunConfig::default()
        },
    )
    .with_search_limits(limits.clone())
    .with_database_path(database.clone());

    let summary = runtime.run("make answer return two".into()).await.unwrap();

    let store = perspt_store::SessionStore::open(&database).unwrap();
    let rows = store.get_psp9_events(&summary.session_id).unwrap();
    let bodies: Vec<serde_json::Value> = rows.iter().filter_map(event_body).collect();

    assert_abandoned_and_bounded(&bodies, &limits);
}

/// The Gate AC assertions: the refusal is ledgered, closed usage sits at
/// or under every limit, and nothing follows `search_closed`.
fn assert_abandoned_and_bounded(bodies: &[serde_json::Value], limits: &SearchLimits) {
    let abandoned = bodies.iter().any(|body| {
        event_name(body) == Some("branch_abandoned")
            && body
                .get("reason")
                .and_then(|r| r.as_str())
                .is_some_and(|r| r.contains("refused"))
    });
    assert!(abandoned, "the refused reservation must be ledgered");

    // The closed forest's usage never sits above any limit (Gate AC's
    // falsifier is "usage above a limit").
    let closed = bodies
        .iter()
        .find(|body| event_name(body) == Some("search_closed"))
        .expect("the forest closed");
    let usage = closed.get("usage").expect("closed usage");
    let field = |name: &str| usage.get(name).and_then(|v| v.as_u64()).unwrap_or(0);
    assert!(field("model_turns") <= u64::from(limits.model_turns));
    assert!(field("tool_calls") <= u64::from(limits.tool_calls));
    assert!(field("verifier_runs") <= u64::from(limits.verifier_runs));
    assert!(field("tokens") <= limits.tokens);
    assert!(field("result_bytes") <= limits.result_bytes);

    // Nothing search-scoped follows search_closed for that forest.
    let forest_id = closed
        .get("forest_id")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();
    let closed_at = bodies
        .iter()
        .position(|body| event_name(body) == Some("search_closed"))
        .unwrap();
    for body in &bodies[closed_at + 1..] {
        let same_forest = body.get("forest_id").and_then(|v| v.as_str()) == Some(&forest_id);
        assert!(
            !same_forest,
            "no event may follow search_closed for the closed forest: {body}"
        );
    }
}
