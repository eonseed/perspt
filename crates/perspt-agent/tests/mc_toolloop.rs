//! Tool-loop mechanism checks (PSP-9 Gates J, M, V).
//!
//! Each test falsifies a named claim, in the trilogy's discipline. The
//! scripted transport stands in for every provider, which is exactly what
//! makes Gate S falsifiable: the loop cannot tell the difference.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use perspt_agent::toolloop::{
    CandidateCheckpoint, CandidateMeasurer, EffectExecutor, EffectOutcome, LoopBudgets, LoopEvent,
    LoopRecorder, Measured, ToolLoop,
};
use perspt_sdk::{
    ActorId, BarrierEvaluator, BarrierWitness, CandidateStateWitness, CandidateTransition,
    Capability, ContractEvaluator, ContractWitness, Conversation, EffectKind, ModelFamily, ModelId,
    ModelTransport, NodeTerminalOutcome, ProviderCapabilities, ProviderToolCall, RiskBudget,
    SdkError, StaticCatalog, ToolChoicePolicy, ToolEntry, ToolSpec, TransportFuture, TurnOutput,
    VerificationCadence,
};

/// A transport that replays a fixed script of turns.
struct Scripted {
    turns: Mutex<Vec<TurnOutput>>,
}

#[derive(Default)]
struct Recording {
    events: Mutex<Vec<LoopEvent>>,
}

impl LoopRecorder for Recording {
    fn record(&self, event: &LoopEvent) -> anyhow::Result<()> {
        self.events.lock().unwrap().push(event.clone());
        Ok(())
    }
}

impl Scripted {
    fn new(turns: Vec<TurnOutput>) -> Self {
        Self {
            turns: Mutex::new(turns),
        }
    }
}

impl ModelTransport for Scripted {
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
        ProviderCapabilities::text_only(100_000)
    }

    fn family_of(&self, model: &ModelId) -> ModelFamily {
        ModelFamily::from_model_name(&model.model)
    }

    fn adapter_kind(&self) -> &'static str {
        "scripted"
    }
}

/// Executor that applies everything and reports mutation.
struct ApplyAll {
    applied: AtomicU32,
    state: AtomicU32,
}

#[async_trait::async_trait]
impl EffectExecutor for ApplyAll {
    async fn checkpoint(&self, _scope: &[String]) -> anyhow::Result<CandidateCheckpoint> {
        let state = self.state.load(Ordering::SeqCst);
        Ok(CandidateCheckpoint {
            id: state.to_string(),
            witness: CandidateStateWitness {
                state_root: state.to_string(),
                node_id: "toolloop".into(),
                canonical_scope: vec!["src/lib.rs".into()],
                ..CandidateStateWitness::default()
            },
        })
    }

    async fn apply(
        &self,
        _call: &ProviderToolCall,
        entry: &ToolEntry,
    ) -> anyhow::Result<EffectOutcome> {
        self.applied.fetch_add(1, Ordering::SeqCst);
        if !entry.effect.is_read_only() {
            self.state.fetch_add(1, Ordering::SeqCst);
        }
        Ok(EffectOutcome {
            output: "ok".into(),
            mutated: !entry.effect.is_read_only(),
        })
    }

    async fn restore(&self, checkpoint: &CandidateCheckpoint) -> anyhow::Result<()> {
        self.state
            .store(checkpoint.id.parse().unwrap(), Ordering::SeqCst);
        Ok(())
    }

    async fn state_witness(&self) -> anyhow::Result<CandidateStateWitness> {
        Ok(self.checkpoint(&[]).await?.witness)
    }
}

/// Measurer that replays a fixed script of energies.
struct EnergyScript {
    readings: Mutex<Vec<(bool, f64)>>,
}

impl EnergyScript {
    fn new(readings: Vec<(bool, f64)>) -> Self {
        Self {
            readings: Mutex::new(readings),
        }
    }
}

#[async_trait::async_trait]
impl CandidateMeasurer for EnergyScript {
    async fn measure(&self) -> anyhow::Result<Measured> {
        let mut readings = self.readings.lock().unwrap();
        let (hard_pass, energy) = if readings.is_empty() {
            (false, 10.0)
        } else {
            readings.remove(0)
        };
        Ok(Measured {
            hard_pass,
            energy,
            residuals: vec![],
            correction: None,
            packet: None,
        })
    }

    async fn measure_incremental(&self) -> anyhow::Result<Measured> {
        let readings = self.readings.lock().unwrap();
        let (hard_pass, energy) = readings.first().copied().unwrap_or((false, 10.0));
        Ok(Measured {
            hard_pass,
            energy,
            residuals: vec![],
            correction: None,
            packet: None,
        })
    }
}

fn call(id: &str, name: &str, args: serde_json::Value) -> ProviderToolCall {
    ProviderToolCall {
        call_id: id.into(),
        name: name.into(),
        arguments: args,
    }
}

fn worker_capability() -> Capability {
    let mut cap = Capability::new(
        ActorId::new("toolloop"),
        vec![
            EffectKind::ReadFile,
            EffectKind::ToolProgram,
            EffectKind::Search,
            EffectKind::List,
            EffectKind::ApplyPatch,
            EffectKind::WriteArtifact,
        ],
    );
    cap.max_calls = Some(100);
    cap.risk_budget = Some(RiskBudget {
        name: "workspace".into(),
        limit: 1.0,
        spent: 0.0,
    });
    cap
}

fn budgets() -> LoopBudgets {
    LoopBudgets {
        max_turns: 6,
        max_calls_per_turn: 4,
        rejection_budget: 2,
        rho_gate: 0.5,
        declared_energy_floor: None,
        context_soft_limit_chars: 240_000,
        recovery_budget: 2,
        turn_deadline_secs: 120,
    }
}

struct PassContract;

impl ContractEvaluator for PassContract {
    fn evaluate(&self, _transition: &CandidateTransition) -> ContractWitness {
        ContractWitness {
            ok: true,
            policy_version: "test-policy-v1".into(),
            evidence_refs: vec!["test-contract".into()],
        }
    }
}

struct ZeroBarrier;

impl BarrierEvaluator for ZeroBarrier {
    fn evaluate(&self, _transition: &CandidateTransition) -> Result<BarrierWitness, SdkError> {
        Ok(BarrierWitness {
            h_before: 0.0,
            expected_h_after_upper: 0.0,
            certified_increment: 0.0,
            unsafe_threshold: 1.0,
            evidence_refs: vec!["test-barrier".into()],
        })
    }
}

static PASS_CONTRACT: PassContract = PassContract;
static ZERO_BARRIER: ZeroBarrier = ZeroBarrier;

fn loop_with<'a>(
    transport: &'a dyn ModelTransport,
    catalog: &'a StaticCatalog,
    executor: &'a ApplyAll,
    measurer: &'a EnergyScript,
) -> ToolLoop<'a> {
    ToolLoop {
        transport,
        model: ModelId::new("test", "scripted"),
        fallback_models: Vec::new(),
        catalog,
        capabilities: vec![worker_capability()],
        contract: Some(&PASS_CONTRACT),
        barrier: Some(&ZERO_BARRIER),
        c_c_max: 0.0,
        executor,
        measurer,
        budgets: budgets(),
        cadence: VerificationCadence::default(),
        kernel_state: perspt_sdk::KernelState::new(),
        node_id: "toolloop".into(),
        generation: 0,
        system_prompt: perspt_agent::toolloop::PromptEnvelope {
            text: "You are a governed coding agent. Propose tool calls; \
                every effect is mediated."
                .into(),
            ..Default::default()
        },
        recorder: None,
    }
}

struct FailPrimary {
    calls: Mutex<Vec<ModelId>>,
}

impl ModelTransport for FailPrimary {
    fn chat_turn<'a>(
        &'a self,
        model: &'a ModelId,
        _conversation: &'a Conversation,
        _tools: &'a [ToolSpec],
        _choice: ToolChoicePolicy,
    ) -> TransportFuture<'a, TurnOutput> {
        self.calls.lock().unwrap().push(model.clone());
        let is_primary = model.provider == "primary";
        Box::pin(async move {
            if is_primary {
                Err(SdkError::Domain("primary unavailable".into()))
            } else {
                Ok(TurnOutput::Text("recovered".into()))
            }
        })
    }

    fn capabilities(&self, _model: &ModelId) -> ProviderCapabilities {
        ProviderCapabilities::text_only(100_000)
    }

    fn family_of(&self, model: &ModelId) -> ModelFamily {
        ModelFamily::from_model_name(&model.model)
    }

    fn adapter_kind(&self) -> &'static str {
        "scripted"
    }
}

#[tokio::test]
async fn provider_failure_uses_recorded_sticky_failover() {
    let transport = FailPrimary {
        calls: Mutex::new(Vec::new()),
    };
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    let measurer = EnergyScript::new(vec![(false, 1.0), (true, 0.0)]);
    let mut tool_loop = loop_with(&transport, &catalog, &executor, &measurer);
    tool_loop.model = ModelId::new("primary", "one");
    tool_loop.fallback_models = vec![ModelId::new("fallback", "two")];

    let outcome = tool_loop.run("recover").await.unwrap();
    assert!(matches!(outcome.outcome, NodeTerminalOutcome::HardPass));
    assert!(outcome.events.iter().any(|event| matches!(
        event,
        LoopEvent::RouteFailover { from_model, to_model, .. }
            if from_model.provider == "primary" && to_model.provider == "fallback"
    )));
    assert_eq!(
        transport
            .calls
            .lock()
            .unwrap()
            .iter()
            .map(|model| model.provider.as_str())
            .collect::<Vec<_>>(),
        ["primary", "fallback"]
    );
}

#[tokio::test]
async fn context_projection_records_control_frame_before_reuse() {
    let transport = Scripted::new(vec![
        TurnOutput::ToolCalls(vec![call(
            "edit",
            "edit_file",
            serde_json::json!({
                "path": "src/lib.rs", "old_string": "a", "new_string": "b"
            }),
        )]),
        TurnOutput::Text("done".into()),
    ]);
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    let measurer = EnergyScript::new(vec![(false, 10.0), (false, 9.0), (true, 0.0)]);
    let mut tool_loop = loop_with(&transport, &catalog, &executor, &measurer);
    tool_loop.budgets.context_soft_limit_chars = 1;

    let outcome = tool_loop.run("edit the file").await.unwrap();
    let checkpoint = outcome.events.iter().find_map(|event| match event {
        LoopEvent::ContextCheckpointCreated { checkpoint } => Some(checkpoint),
        _ => None,
    });
    let checkpoint = checkpoint.expect("context limit must create a checkpoint");
    assert_eq!(checkpoint.control.goal, "edit the file");
    assert_eq!(checkpoint.control.accepted_state_root, "1");
    assert!(checkpoint.control.unresolved_call_ids.is_empty());
    assert!(checkpoint.narrative_observation.is_none());
}

#[tokio::test]
async fn durable_checkpoint_preserves_the_next_turn_projection() {
    let transport = Scripted::new(vec![
        TurnOutput::ToolCalls(vec![call(
            "edit",
            "edit_file",
            serde_json::json!({
                "path": "src/lib.rs", "old_string": "a", "new_string": "b"
            }),
        )]),
        TurnOutput::Text("done".into()),
    ]);
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    let measurer = EnergyScript::new(vec![(false, 10.0), (false, 9.0), (true, 0.0)]);
    let recorder = Recording::default();
    let mut tool_loop = loop_with(&transport, &catalog, &executor, &measurer);
    tool_loop.recorder = Some(&recorder);

    let outcome = tool_loop.run("edit the file").await.unwrap();
    assert!(matches!(outcome.outcome, NodeTerminalOutcome::HardPass));
    let events = recorder.events.lock().unwrap();
    let (control, conversation) = events
        .iter()
        .find_map(|event| match event {
            LoopEvent::DurableCandidateCheckpoint {
                control,
                conversation,
                ..
            } if control.remaining_turns > 0 => Some((control, conversation)),
            _ => None,
        })
        .expect("descent acceptance must persist a resumable projection");
    assert_eq!(control.active_model, ModelId::new("test", "scripted"));
    assert!(control.unresolved_call_ids.is_empty());
    assert!(matches!(
        conversation.messages().last(),
        Some(perspt_sdk::Message::User { content })
            if content.contains("descended to V = 9.000")
    ));
}

/// MC-J: every executed effect traces to a model-issued call, and every
/// returned call — including one above `M` — has a recorded proposal/result.
#[tokio::test]
async fn mc_j_every_effect_traces_to_a_recorded_model_call() {
    let transport = Scripted::new(vec![
        TurnOutput::ToolCalls(vec![
            call("c1", "read_file", serde_json::json!({"path": "src/lib.rs"})),
            call(
                "c2",
                "edit_file",
                serde_json::json!({
                    "path": "src/lib.rs", "old_string": "a", "new_string": "b"
                }),
            ),
        ]),
        TurnOutput::Text("finished".into()),
    ]);
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    let measurer = EnergyScript::new(vec![(false, 10.0), (false, 9.0), (true, 0.0)]);

    let outcome = loop_with(&transport, &catalog, &executor, &measurer)
        .run("fix it")
        .await
        .unwrap();

    let applied: Vec<&str> = outcome
        .events
        .iter()
        .filter_map(|e| match e {
            LoopEvent::EffectApplied { call_id, .. } => Some(call_id.as_str()),
            _ => None,
        })
        .collect();
    let checked: Vec<&str> = outcome
        .events
        .iter()
        .filter_map(|e| match e {
            LoopEvent::ProposalChecked { call_id, .. } => Some(call_id.as_str()),
            _ => None,
        })
        .collect();
    // No effect without a checked proposal from a model-issued call.
    for id in &applied {
        assert!(checked.contains(id), "effect {id} lacks a proposal record");
    }
    assert_eq!(
        executor.applied.load(Ordering::SeqCst) as usize,
        applied.len()
    );
}

/// A tool program is only a compact proposal generator. Its emitted calls are
/// independently observed and checked by the ordinary kernel path.
#[tokio::test]
async fn tool_program_nested_calls_return_to_the_five_clause_kernel() {
    let source = r#"
def main():
    return '[{"tool":"read_file","arguments":{"path":"src/lib.rs"}}]'
main
"#;
    let transport = Scripted::new(vec![
        TurnOutput::ToolCalls(vec![call(
            "program",
            "tool_program",
            serde_json::json!({"source": source}),
        )]),
        TurnOutput::Text("done".into()),
    ]);
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    let measurer = EnergyScript::new(vec![(false, 1.0), (true, 0.0)]);

    let outcome = loop_with(&transport, &catalog, &executor, &measurer)
        .run("inspect through a bounded program")
        .await
        .unwrap();

    assert_eq!(executor.applied.load(Ordering::SeqCst), 1);
    for call_id in ["program", "program:0"] {
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            LoopEvent::ToolCallObserved { call } if call.call_id == call_id
        )));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            LoopEvent::ProposalChecked { call_id: checked, witness, .. }
                if checked == call_id && witness.allows()
        )));
    }
}

#[tokio::test]
async fn nested_tool_program_calls_share_the_top_level_turn_budget() {
    let nested: Vec<String> = ["a", "b", "c", "d", "e"]
        .iter()
        .map(|path| format!(r#"{{"tool":"read_file","arguments":{{"path":"{path}"}}}}"#))
        .collect();
    let source = format!("\ndef main():\n    return '[{}]'\nmain\n", nested.join(","));
    let transport = Scripted::new(vec![TurnOutput::ToolCalls(vec![call(
        "program",
        "tool_program",
        serde_json::json!({"source": source}),
    )])]);
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    let measurer = EnergyScript::new(vec![(false, 1.0), (true, 0.0)]);

    let outcome = loop_with(&transport, &catalog, &executor, &measurer)
        .run("inspect")
        .await
        .unwrap();
    // M=4 includes the top-level tool_program call, leaving three nested calls.
    assert_eq!(executor.applied.load(Ordering::SeqCst), 3);
    let denied = outcome
        .events
        .iter()
        .filter(|event| {
            matches!(
                event,
                LoopEvent::EffectDenied { class, .. }
                    if *class == perspt_sdk::ResidualClass::BudgetExhausted
            )
        })
        .count();
    assert_eq!(denied, 2);
}

/// MC-J (budget half): a call above `M` is denied with a recorded pair, not
/// silently dropped.
#[tokio::test]
async fn mc_j_calls_above_m_are_denied_and_recorded() {
    let calls: Vec<ProviderToolCall> = (0..6)
        .map(|i| {
            call(
                &format!("c{i}"),
                "read_file",
                serde_json::json!({"path": "x"}),
            )
        })
        .collect();
    let transport = Scripted::new(vec![TurnOutput::ToolCalls(calls)]);
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    let measurer = EnergyScript::new(vec![(false, 10.0), (true, 0.0)]);

    let outcome = loop_with(&transport, &catalog, &executor, &measurer)
        .run("look around")
        .await
        .unwrap();

    let denied: Vec<&str> = outcome
        .events
        .iter()
        .filter_map(|e| match e {
            LoopEvent::EffectDenied { call_id, class, .. }
                if *class == perspt_sdk::ResidualClass::BudgetExhausted =>
            {
                Some(call_id.as_str())
            }
            _ => None,
        })
        .collect();
    // M = 4, six calls returned: two denied over budget.
    assert_eq!(denied, ["c4", "c5"]);
    for call_id in ["c4", "c5"] {
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            LoopEvent::ProposalObserved { call_id: observed, .. } if observed == call_id
        )));
    }
    assert_eq!(executor.applied.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn malformed_tool_arguments_are_locally_denied_and_returned_to_the_model() {
    let transport = Scripted::new(vec![TurnOutput::ToolCalls(vec![call(
        "bad-edit",
        "edit_file",
        serde_json::json!({"path": "src/lib.rs", "old_string": 7}),
    )])]);
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    let measurer = EnergyScript::new(vec![(false, 1.0), (true, 0.0)]);

    let outcome = loop_with(&transport, &catalog, &executor, &measurer)
        .run("fix it")
        .await
        .unwrap();
    assert_eq!(executor.applied.load(Ordering::SeqCst), 0);
    assert!(outcome.events.iter().any(|event| matches!(
        event,
        LoopEvent::EffectDenied { call_id, class, .. }
            if call_id == "bad-edit"
                && *class == perspt_sdk::ResidualClass::ToolArgumentInvalid
    )));
}

/// MC-M: the realized decision count never exceeds
/// `floor(V_0/rho_gate) + B + 1`, and the loop always classifies.
#[tokio::test]
async fn mc_m_decision_count_respects_the_finite_bound() {
    // A model that never stops proposing and never descends.
    let turns: Vec<TurnOutput> = (0..20)
        .map(|i| TurnOutput::Text(format!("attempt {i}")))
        .collect();
    let transport = Scripted::new(turns);
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    // Energy never improves: every submission is a rejection.
    let measurer = EnergyScript::new(vec![(false, 2.0); 30]);

    let mut budgets = budgets();
    budgets.max_turns = 30;
    let bound = perspt_agent::toolloop::loop_decision_bound(2.0, &budgets).unwrap();

    let mut toolloop = loop_with(&transport, &catalog, &executor, &measurer);
    toolloop.budgets = budgets;
    let outcome = toolloop.run("impossible goal").await.unwrap();

    let decisions = outcome
        .events
        .iter()
        .filter(|e| matches!(e, LoopEvent::GateDecisionRecorded { .. }))
        .count() as u64;
    assert!(decisions <= bound, "{decisions} decisions > bound {bound}");
    assert!(matches!(
        outcome.outcome,
        NodeTerminalOutcome::Escalated { .. }
    ));
}

/// MC-V: an adversarial model that reports successful edits while every
/// effect is denied must never reach a hard pass — the gate re-reads the
/// unchanged overlay and the node escalates.
#[tokio::test]
async fn mc_v_a_lying_model_never_reaches_hard_pass() {
    // The model calls run_shell (no capability held) and then claims success.
    let turns = vec![
        TurnOutput::ToolCalls(vec![call(
            "c1",
            "run_shell",
            serde_json::json!({"command": "echo hacked > src/lib.rs"}),
        )]),
        TurnOutput::Text("I fixed everything; all tests pass now.".into()),
        TurnOutput::Text("Definitely fixed.".into()),
        TurnOutput::Text("Trust me.".into()),
    ];
    let transport = Scripted::new(turns);
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    // The overlay never changes, so measurement never improves.
    let measurer = EnergyScript::new(vec![(false, 10.0); 20]);

    let outcome = loop_with(&transport, &catalog, &executor, &measurer)
        .run("fix the build")
        .await
        .unwrap();

    assert!(
        matches!(outcome.outcome, NodeTerminalOutcome::Escalated { .. }),
        "a lying model must escalate, got {:?}",
        outcome.outcome
    );
    // The denied call is recorded, nothing was executed, and the projection
    // telemetry is nonzero.
    assert_eq!(executor.applied.load(Ordering::SeqCst), 0);
    assert!(outcome.projection.denied_proposals > 0);
}

/// A hard-passing baseline terminates before any model turn is spent.
#[tokio::test]
async fn a_passing_baseline_costs_no_turns() {
    let transport = Scripted::new(vec![]);
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    let measurer = EnergyScript::new(vec![(true, 0.0)]);

    let outcome = loop_with(&transport, &catalog, &executor, &measurer)
        .run("already done")
        .await
        .unwrap();
    assert!(matches!(outcome.outcome, NodeTerminalOutcome::HardPass));
    assert_eq!(outcome.turns_used, 0);
}

/// Descent continues the node rather than terminating it (Paper II Lemma 1).
#[tokio::test]
async fn descent_is_a_non_terminal_decision() {
    let transport = Scripted::new(vec![
        TurnOutput::Text("first attempt".into()),
        TurnOutput::Text("second attempt".into()),
    ]);
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    // Baseline 10, then descent to 8 (accepted, continues), then hard pass.
    let measurer = EnergyScript::new(vec![(false, 10.0), (false, 8.0), (true, 0.0)]);

    let outcome = loop_with(&transport, &catalog, &executor, &measurer)
        .run("iterate")
        .await
        .unwrap();
    assert!(matches!(outcome.outcome, NodeTerminalOutcome::HardPass));
    assert_eq!(outcome.turns_used, 2, "descent continued the loop");
}

/// An unbounded configuration is rejected at startup (Paper II Theorem 3).
#[tokio::test]
async fn unbounded_configurations_are_rejected_at_startup() {
    let transport = Scripted::new(vec![]);
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    let measurer = EnergyScript::new(vec![]);

    let mut toolloop = loop_with(&transport, &catalog, &executor, &measurer);
    toolloop.budgets.max_turns = 0;
    assert!(toolloop.run("anything").await.is_err());
}

/// MC-M: H counts admitted mutations across turns, not independently inside
/// each provider response.
#[tokio::test]
async fn cadence_accumulates_mutations_across_turns() {
    let edit = |id: &str| {
        TurnOutput::ToolCalls(vec![call(
            id,
            "edit_file",
            serde_json::json!({
                "path": "src/lib.rs", "old_string": "a", "new_string": "b"
            }),
        )])
    };
    let transport = Scripted::new(vec![edit("c1"), edit("c2"), edit("c3")]);
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    let measurer = EnergyScript::new(vec![(false, 10.0), (true, 0.0)]);

    let mut toolloop = loop_with(&transport, &catalog, &executor, &measurer);
    toolloop.cadence.max_mutations_between_checks = 2;
    let outcome = toolloop.run("edit twice").await.unwrap();

    assert!(matches!(outcome.outcome, NodeTerminalOutcome::HardPass));
    assert_eq!(outcome.turns_used, 2);
    assert_eq!(executor.applied.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn rejected_candidate_restores_the_last_accepted_workspace_state() {
    let transport = Scripted::new(vec![
        TurnOutput::ToolCalls(vec![call(
            "c1",
            "edit_file",
            serde_json::json!({
                "path": "src/lib.rs", "old_string": "a", "new_string": "b"
            }),
        )]),
        TurnOutput::Text("done".into()),
    ]);
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    let measurer = EnergyScript::new(vec![(false, 10.0), (false, 10.0)]);
    let mut toolloop = loop_with(&transport, &catalog, &executor, &measurer);
    toolloop.budgets.rejection_budget = 0;

    let outcome = toolloop.run("attempt a bad edit").await.unwrap();
    assert!(matches!(
        outcome.outcome,
        NodeTerminalOutcome::Escalated { .. }
    ));
    assert_eq!(executor.state.load(Ordering::SeqCst), 0);
    assert!(outcome
        .events
        .iter()
        .any(|event| matches!(event, LoopEvent::CandidateRestored { .. })));
}

#[tokio::test]
async fn one_provider_turn_cannot_exceed_the_cadence_bound() {
    // Disjoint paths: the edits commute (Gate P admits all three), so the
    // cadence bound alone must stop the second and third.
    let calls = (0..3)
        .map(|index| {
            call(
                &format!("c{index}"),
                "edit_file",
                serde_json::json!({
                    "path": format!("src/file{index}.rs"), "old_string": "a", "new_string": "b"
                }),
            )
        })
        .collect();
    let transport = Scripted::new(vec![TurnOutput::ToolCalls(calls)]);
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    let measurer = EnergyScript::new(vec![(false, 10.0), (true, 0.0)]);
    let mut toolloop = loop_with(&transport, &catalog, &executor, &measurer);
    toolloop.cadence.max_mutations_between_checks = 1;

    let outcome = toolloop.run("bounded edits").await.unwrap();
    assert!(matches!(outcome.outcome, NodeTerminalOutcome::HardPass));
    assert_eq!(executor.applied.load(Ordering::SeqCst), 1);
    let boundary_denials = outcome
        .events
        .iter()
        .filter(|event| {
            matches!(
                event,
                LoopEvent::EffectDenied { reason, .. }
                    if reason.contains("verification boundary required")
            )
        })
        .count();
    assert_eq!(boundary_denials, 2);
}

// ---------------------------------------------------------------------------
// Gate P: intra-turn commuting batches (PSP-9 system 15).
// ---------------------------------------------------------------------------

fn edit(id: &str, path: &str) -> ProviderToolCall {
    call(
        id,
        "edit_file",
        serde_json::json!({"path": path, "old_string": "a", "new_string": "b"}),
    )
}

#[tokio::test]
async fn same_path_double_edit_is_returned_as_a_conflict_then_resequenced() {
    let transport = Scripted::new(vec![
        TurnOutput::ToolCalls(vec![edit("e1", "src/a.rs"), edit("e2", "src/a.rs")]),
        TurnOutput::ToolCalls(vec![edit("e2-retry", "src/a.rs")]),
    ]);
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    let measurer = EnergyScript::new(vec![(false, 10.0), (true, 2.0), (true, 0.0)]);
    let toolloop = loop_with(&transport, &catalog, &executor, &measurer);

    let outcome = toolloop.run("edit the same file twice").await.unwrap();
    let conflict = outcome.events.iter().find_map(|event| match event {
        LoopEvent::ToolBatchConflict {
            call_id,
            conflicts_with,
            ..
        } => Some((call_id.clone(), conflicts_with.clone())),
        _ => None,
    });
    assert_eq!(
        conflict,
        Some(("e2".into(), "e1".into())),
        "the second same-path edit must be a recorded conflict observation"
    );
    // e1 applied in turn 1, the re-issued edit applied in turn 2; the
    // conflicted e2 never executed.
    assert_eq!(executor.applied.load(Ordering::SeqCst), 2);
    assert!(matches!(outcome.outcome, NodeTerminalOutcome::HardPass));
}

#[tokio::test]
async fn disjoint_path_edits_both_apply_in_one_turn() {
    let transport = Scripted::new(vec![TurnOutput::ToolCalls(vec![
        edit("e1", "src/a.rs"),
        edit("e2", "src/b.rs"),
    ])]);
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    let measurer = EnergyScript::new(vec![(false, 10.0), (true, 0.0)]);
    let toolloop = loop_with(&transport, &catalog, &executor, &measurer);

    let outcome = toolloop.run("edit two files").await.unwrap();
    assert_eq!(
        executor.applied.load(Ordering::SeqCst),
        2,
        "commuting mutators run in arrival order in one turn"
    );
    assert!(!outcome
        .events
        .iter()
        .any(|event| matches!(event, LoopEvent::ToolBatchConflict { .. })));
    assert!(matches!(outcome.outcome, NodeTerminalOutcome::HardPass));
}

#[tokio::test]
async fn read_after_same_path_write_is_a_conflict() {
    let transport = Scripted::new(vec![TurnOutput::ToolCalls(vec![
        edit("w1", "src/a.rs"),
        call("r1", "read_file", serde_json::json!({"path": "src/a.rs"})),
    ])]);
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    let measurer = EnergyScript::new(vec![(false, 10.0), (true, 0.0)]);
    let toolloop = loop_with(&transport, &catalog, &executor, &measurer);

    let outcome = toolloop.run("write then read").await.unwrap();
    assert!(outcome.events.iter().any(|event| matches!(
        event,
        LoopEvent::ToolBatchConflict { call_id, .. } if call_id == "r1"
    )));
    assert_eq!(executor.applied.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn three_reads_apply_concurrently_with_a_deterministic_chain() {
    let transport = Scripted::new(vec![TurnOutput::ToolCalls(vec![
        call("r1", "read_file", serde_json::json!({"path": "src/a.rs"})),
        call("r2", "read_file", serde_json::json!({"path": "src/b.rs"})),
        call("r3", "read_file", serde_json::json!({"path": "src/a.rs"})),
    ])]);
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    let measurer = EnergyScript::new(vec![(false, 10.0), (true, 0.0)]);
    let toolloop = loop_with(&transport, &catalog, &executor, &measurer);

    let outcome = toolloop.run("read three views").await.unwrap();
    assert_eq!(executor.applied.load(Ordering::SeqCst), 3);
    assert!(!outcome
        .events
        .iter()
        .any(|event| matches!(event, LoopEvent::ToolBatchConflict { .. })));
    // Responses (and their recorded EffectApplied events) stay in arrival
    // order even though execution was concurrent.
    let applied_order: Vec<String> = outcome
        .events
        .iter()
        .filter_map(|event| match event {
            LoopEvent::EffectApplied { call_id, .. } => Some(call_id.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(applied_order, ["r1", "r2", "r3"]);
}
