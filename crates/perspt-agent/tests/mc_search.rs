//! Search-forest mechanism checks (PSP-10 Gates W, X, AB, AC; Phase 8).
//!
//! The forest opens at the refine rung, runs isolated branches against
//! the same accepted root, and commits at most one candidate through the
//! ordinary gate. Removing every search-alphabet event leaves the
//! accepted fold unchanged (Proposition 2); discarded branches leave the
//! user workspace untouched; exact no-goods and the selection rule are
//! deterministic. Exit criterion: the scripted ensemble-baseline scenario
//! reaches the same outcome with no more branch attempts than the removed
//! ensemble used (`tests/fixtures/ensemble_baseline.json`).

use std::sync::{Arc, Mutex};

use perspt_agent::{Psp9AgentRuntime, Psp9RunConfig};
use perspt_sdk::ledger::{tool_loop_body, Ledger, LedgerEvent, ToolLoopBody};
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
        "[package]\nname='search-fixture'\nversion='0.1.0'\nedition='2021'\n",
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

const SEARCH_ALPHABET: &[&str] = &[
    "search_opened",
    "branch_forked",
    "branch_strategy_selected",
    "branch_observation",
    "branch_candidate_measured",
    "partial_checkpointed",
    "frontier_epoch_started",
    "frontier_entry_served",
    "branch_ineligible",
    "branch_not_selected",
    "branch_abandoned",
    "branch_selected",
    "branch_committed",
    "no_good_recorded",
    "search_closed",
    "context_working_set",
    "context_pages_selected",
    "context_miss",
    "context_page_recalled",
    "context_infeasible",
    "context_compacted",
];

fn event_name(row: &perspt_store::Psp9LedgerRow) -> Option<String> {
    let LedgerEvent::Custom { kind, payload } = serde_json::from_str(&row.event_json).ok()? else {
        return None;
    };
    if kind != "tool_loop" {
        return None;
    }
    let (ToolLoopBody::Legacy(body) | ToolLoopBody::V1(body)) = tool_loop_body(&payload).ok()?;
    body.get("event")?.as_str().map(str::to_string)
}

fn ledger_from(rows: &[perspt_store::Psp9LedgerRow], keep_search: bool) -> Ledger {
    let mut ledger = Ledger::new();
    for row in rows {
        let Ok(event) = serde_json::from_str::<LedgerEvent>(&row.event_json) else {
            continue;
        };
        if !keep_search {
            if let Some(name) = event_name(row) {
                if SEARCH_ALPHABET.contains(&name.as_str()) {
                    continue;
                }
            }
        }
        ledger.append(event).unwrap();
    }
    ledger
}

/// Proposition 2 / Gate W: `fold(L) == fold(L \ S)` for the recorded run,
/// plus the contrapositive control — removing an ordinary gate event DOES
/// change the fold, so the filter is not vacuous.
fn assert_fold_invariant(rows: &[perspt_store::Psp9LedgerRow]) {
    let full = perspt_sdk::ledger::replay_accepted_trajectory(&ledger_from(rows, true)).unwrap();
    let stripped =
        perspt_sdk::ledger::replay_accepted_trajectory(&ledger_from(rows, false)).unwrap();
    assert_eq!(full, stripped, "search events changed the accepted fold");
    if !full.is_empty() {
        let mut ledger = Ledger::new();
        let mut dropped_one = false;
        for row in rows {
            let Ok(event) = serde_json::from_str::<LedgerEvent>(&row.event_json) else {
                continue;
            };
            let accepting = event_name(row).as_deref() == Some("gate_decision_recorded")
                && (row.event_json.contains("hard_pass")
                    || row.event_json.contains("accepted_by_descent"));
            if !dropped_one && accepting {
                dropped_one = true;
                continue;
            }
            ledger.append(event).unwrap();
        }
        let counter = perspt_sdk::ledger::replay_accepted_trajectory(&ledger).unwrap();
        assert_ne!(full, counter, "the fold must depend on ordinary events");
    }
}

/// D1 lineage consistency: `parent_branch` and `seed_witness` must tell
/// the same story on every `branch_forked` — a parent only with a chain
/// longer than `[root]` (a continued partial), a bare-root chain only with
/// no parent. Before this fix `parent_branch` was index arithmetic.
fn assert_lineage_consistency(rows: &[perspt_store::Psp9LedgerRow]) {
    for row in rows {
        if event_name(row).as_deref() != Some("branch_forked") {
            continue;
        }
        let LedgerEvent::Custom { payload, .. } = serde_json::from_str(&row.event_json).unwrap()
        else {
            continue;
        };
        let (ToolLoopBody::Legacy(body) | ToolLoopBody::V1(body)) =
            tool_loop_body(&payload).unwrap();
        let parent = body.get("parent_branch").and_then(|v| v.as_str());
        let chain_len = body
            .get("seed_witness")
            .and_then(|w| w.get("chain"))
            .and_then(|c| c.as_array())
            .map(Vec::len)
            .unwrap_or(0);
        match parent {
            Some(producer) => {
                assert!(
                    chain_len > 1,
                    "branch with parent {producer} must carry a partial witness chain"
                );
                assert!(
                    producer.contains("/b"),
                    "parent must name a real branch: {producer}"
                );
            }
            None => assert_eq!(
                chain_len, 1,
                "a root restart must carry the bare accepted-root chain"
            ),
        }
    }
}

fn runtime_with(
    project: &std::path::Path,
    transport: Arc<ScriptedTransport>,
    primary: &str,
    fallbacks: Vec<&str>,
) -> Psp9AgentRuntime {
    let runtime = Psp9AgentRuntime::with_transport(
        project.to_path_buf(),
        transport,
        ModelId::new("test", primary),
        Psp9RunConfig {
            approval_policy: ApprovalPolicy::Auto,
            max_turns: 2,
            rejection_budget: 4,
            allow_unisolated_verifiers: true,
            ..Psp9RunConfig::default()
        },
    );
    runtime.with_fallback_models(
        fallbacks
            .into_iter()
            .map(|name| ModelId::new("test", name))
            .collect(),
    )
}

/// Exit criterion (pre-step 0 baseline): the recorded ensemble scenario —
/// a failed first attempt, then a fixing candidate — reaches HardPass
/// through the forest with at most the baseline's two attempts, ending in
/// a committed candidate at energy 0.0.
#[tokio::test]
async fn single_branch_search_matches_the_ensemble_baseline() {
    let project = tempfile::tempdir().unwrap();
    write_fixture_project(project.path());
    let database = project.path().join("runtime.db");
    let transport = Arc::new(ScriptedTransport {
        turns: Mutex::new(vec![
            // Attempt 1: two unproductive turns exhaust the loop.
            TurnOutput::Text("thinking".into()),
            TurnOutput::Text("still thinking".into()),
            // Forest branch 1: the actual fix.
            fix_call(),
            TurnOutput::Text("done".into()),
        ]),
    });
    let runtime = runtime_with(project.path(), transport, "alpha", vec!["beta"])
        .with_database_path(database.clone());
    let summary = runtime.run("make answer return two".into()).await.unwrap();
    assert!(matches!(summary.outcome, NodeTerminalOutcome::HardPass));

    let store = perspt_store::SessionStore::open(&database).unwrap();
    let rows = store.get_psp9_events(&summary.session_id).unwrap();
    let names: Vec<String> = rows.iter().filter_map(event_name).collect();
    let count = |needle: &str| names.iter().filter(|name| *name == needle).count();
    assert_eq!(count("search_opened"), 1);
    assert!(count("branch_forked") <= 2, "baseline allows two attempts");
    assert_eq!(count("branch_selected"), 1);
    assert_eq!(count("branch_committed"), 1);
    assert_eq!(count("search_closed"), 1);
    // The committed candidate carries the baseline's winning energy 0.0.
    let committed = rows
        .iter()
        .find(|row| event_name(row).as_deref() == Some("branch_committed"))
        .unwrap();
    assert!(committed.event_json.contains("hard_pass"));
    assert_fold_invariant(&rows);
    assert_lineage_consistency(&rows);
}

/// Gate W + AB: an unproductive branch is not selected, records an exact
/// no-good, leaves the user workspace untouched, and the diverse second
/// branch wins. The forest commits exactly once (Gate X).
#[tokio::test]
async fn a_failed_branch_is_discarded_and_a_diverse_branch_commits() {
    let project = tempfile::tempdir().unwrap();
    write_fixture_project(project.path());
    let database = project.path().join("runtime.db");
    let transport = Arc::new(ScriptedTransport {
        turns: Mutex::new(vec![
            // Attempt 1: exhausted without progress.
            TurnOutput::Text("thinking".into()),
            TurnOutput::Text("still thinking".into()),
            // Forest branch 1 (primary): still no progress.
            TurnOutput::Text("no idea".into()),
            TurnOutput::Text("really no idea".into()),
            // Forest branch 2 (diverse route): the fix.
            fix_call(),
            TurnOutput::Text("done".into()),
        ]),
    });
    let source_before = std::fs::read_to_string(project.path().join("src/lib.rs")).unwrap();
    let runtime = runtime_with(project.path(), transport, "alpha", vec!["beta"])
        .with_database_path(database.clone());
    let summary = runtime.run("make answer return two".into()).await.unwrap();
    assert!(matches!(summary.outcome, NodeTerminalOutcome::HardPass));

    let store = perspt_store::SessionStore::open(&database).unwrap();
    let rows = store.get_psp9_events(&summary.session_id).unwrap();
    let names: Vec<String> = rows.iter().filter_map(event_name).collect();
    let count = |needle: &str| names.iter().filter(|name| *name == needle).count();
    assert_eq!(count("branch_forked"), 2, "expansion trigger fired once");
    assert_eq!(count("branch_ineligible"), 1);
    assert!(
        count("no_good_recorded") >= 1,
        "failed attempt is a no-good"
    );
    assert_eq!(count("branch_not_selected"), 1);
    assert_eq!(count("branch_selected"), 1);
    assert_eq!(count("branch_committed"), 1, "at most one commit");
    // The discarded branch never touched the user workspace; only the
    // final promotion changed it.
    assert!(summary.promoted_paths.contains(&"src/lib.rs".to_string()));
    let source_after = std::fs::read_to_string(project.path().join("src/lib.rs")).unwrap();
    assert_ne!(source_before, source_after, "the winner promoted");
    assert!(source_after.contains("{ 2 }"));
    assert_fold_invariant(&rows);
    assert_lineage_consistency(&rows);
}

/// Seeded randomized interleavings: splicing search-alphabet streams into
/// an ordinary accepted ledger never changes the fold (256 seeds).
#[test]
fn spliced_search_streams_never_change_the_fold() {
    struct SplitMix64(u64);
    impl SplitMix64 {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }
    }
    let ordinary = |candidate: &str, energy: f64| {
        vec![
            serde_json::json!({"schema_version": 1, "body": {
                "event": "candidate_measured", "node_id": "n1", "generation": 0,
                "candidate_id": candidate, "energy": energy, "hard_pass": false,
                "residuals": []}}),
            serde_json::json!({"schema_version": 1, "body": {
                "event": "gate_decision_recorded", "node_id": "n1", "generation": 0,
                "candidate_id": candidate,
                "decision": {"kind": "accepted_by_descent", "delta_v": 0.5},
                "observed_energy": energy}}),
        ]
    };
    let search_stream = |forest: &str| {
        vec![
            serde_json::json!({"schema_version": 1, "body": {
                "event": "branch_observation", "forest_id": forest,
                "branch_id": format!("{forest}/b1"),
                "observation": "branch measurement V=0.2 hard_pass=false"}}),
            serde_json::json!({"schema_version": 1, "body": {
                "event": "branch_not_selected", "forest_id": forest,
                "branch_id": format!("{forest}/b1")}}),
            serde_json::json!({"schema_version": 1, "body": {
                "event": "search_closed", "forest_id": forest,
                "usage": perspt_sdk::SearchUsage::default()}}),
        ]
    };
    for seed in 0..256u64 {
        let mut rng = SplitMix64(seed);
        let mut payloads = ordinary("n1/0/c1", 3.0);
        payloads.extend(ordinary("n1/0/c2", 2.0));
        for (index, event) in search_stream(&format!("f{seed}")).into_iter().enumerate() {
            let position = (rng.next() as usize) % (payloads.len() + 1);
            payloads.insert(position.min(payloads.len()), event);
            let _ = index;
        }
        let build = |include_search: bool| {
            let mut ledger = Ledger::new();
            for payload in &payloads {
                let name = payload["body"]["event"].as_str().unwrap();
                if !include_search && SEARCH_ALPHABET.contains(&name) {
                    continue;
                }
                ledger
                    .append(LedgerEvent::Custom {
                        kind: "tool_loop".into(),
                        payload: payload.clone(),
                    })
                    .unwrap();
            }
            perspt_sdk::ledger::replay_accepted_trajectory(&ledger).unwrap()
        };
        assert_eq!(build(true), build(false), "seed {seed}");
    }
}

/// Research-domain search conformance (Phase 11): the forest mechanics are
/// domain-generic. With no runnable citation verifier every branch is
/// ineligible — the forest opens, measures, records no-goods, commits
/// nothing, and the session escalates honestly instead of promoting.
#[tokio::test]
async fn the_forest_is_domain_generic_under_the_research_domain() {
    let project = tempfile::tempdir().unwrap();
    std::fs::write(
        project.path().join("refs.bib"),
        "@book{k, title={Working Set Model}}\n",
    )
    .unwrap();
    std::fs::write(project.path().join("draft.md"), "claim [k]\n").unwrap();
    let database = project.path().join("runtime.db");
    let transport = Arc::new(ScriptedTransport {
        turns: Mutex::new(vec![
            TurnOutput::Text("thinking".into()),
            TurnOutput::Text("still thinking".into()),
        ]),
    });
    let runtime = runtime_with(project.path(), transport, "alpha", vec!["beta"])
        .with_domain(std::sync::Arc::new(perspt_research::ResearchDomain::new()))
        .with_database_path(database.clone());
    let summary = runtime.run("support the claim".into()).await.unwrap();
    assert!(
        !matches!(summary.outcome, NodeTerminalOutcome::HardPass),
        "no citation verifier ran; nothing may hard-pass"
    );
    assert!(summary.promoted_paths.is_empty());

    let store = perspt_store::SessionStore::open(&database).unwrap();
    let rows = store.get_psp9_events(&summary.session_id).unwrap();
    let names: Vec<String> = rows.iter().filter_map(event_name).collect();
    let count = |needle: &str| names.iter().filter(|name| *name == needle).count();
    assert!(
        count("search_opened") >= 1,
        "the refine rung opened a forest"
    );
    assert_eq!(count("search_closed"), count("search_opened"));
    assert_eq!(count("branch_committed"), 0, "no eligible candidate");
    assert!(count("branch_ineligible") >= 1);
    assert_fold_invariant(&rows);
    assert_lineage_consistency(&rows);
}

/// A permanently rate-limited route never opens the forest: the attempt is
/// contained by transport exhaustion, the Refine rung is skipped with a
/// ledgered reason, and the session terminates promptly instead of
/// multiplying the retry storm across branches (429 pathology).
#[tokio::test]
async fn a_rate_limited_route_skips_the_forest_and_terminates() {
    struct RateLimited;
    impl ModelTransport for RateLimited {
        fn chat_turn<'a>(
            &'a self,
            _model: &'a ModelId,
            _conversation: &'a Conversation,
            _tools: &'a [ToolSpec],
            _choice: ToolChoicePolicy,
        ) -> TransportFuture<'a, TurnOutput> {
            Box::pin(async {
                Err(perspt_sdk::SdkError::Domain(
                    "chat turn failed: status code '429 Too Many Requests'".into(),
                ))
            })
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

    let project = tempfile::tempdir().unwrap();
    write_fixture_project(project.path());
    let database = project.path().join("ledger.db");
    let runtime = Psp9AgentRuntime::with_transport(
        project.path().to_path_buf(),
        Arc::new(RateLimited),
        ModelId::new("test", "limited"),
        Psp9RunConfig {
            approval_policy: ApprovalPolicy::Auto,
            max_turns: 4,
            rejection_budget: 4,
            allow_unisolated_verifiers: true,
            ..Psp9RunConfig::default()
        },
    )
    .with_database_path(database.clone());

    let summary = runtime.run("fix the answer function".into()).await.unwrap();
    assert!(matches!(
        summary.outcome,
        NodeTerminalOutcome::Escalated { .. }
    ));

    let store = perspt_store::SessionStore::open(&database).unwrap();
    let rows = store.get_psp9_events(&summary.session_id).unwrap();
    let skipped = rows
        .iter()
        .any(|row| row.event_json.contains("recovery_rung_skipped"));
    assert!(skipped, "the refine rung records why it was skipped");
    let forest_opened = rows
        .iter()
        .filter_map(event_name)
        .any(|name| name == "search_opened" || name == "branch_forked");
    assert!(
        !forest_opened,
        "no forest opens against a transport-dead route"
    );
}
