//! Tool-loop mechanism checks (PSP-9 Gates J, M, V).
//!
//! Each test falsifies a named claim, in the trilogy's discipline. The
//! scripted transport stands in for every provider, which is exactly what
//! makes Gate S falsifiable: the loop cannot tell the difference.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use perspt_agent::toolloop::{
    CandidateMeasurer, EffectExecutor, EffectOutcome, LoopBudgets, LoopEvent, Measured, ToolLoop,
};
use perspt_sdk::{
    ActorId, Capability, Conversation, EffectKind, ModelFamily, ModelId, ModelTransport,
    NodeTerminalOutcome, ProviderCapabilities, ProviderToolCall, RiskBudget, StaticCatalog,
    ToolChoicePolicy, ToolEntry, ToolSpec, TransportFuture, TurnOutput, VerificationCadence,
};

/// A transport that replays a fixed script of turns.
struct Scripted {
    turns: Mutex<Vec<TurnOutput>>,
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
}

/// Executor that applies everything and reports mutation.
struct ApplyAll {
    applied: AtomicU32,
}

#[async_trait::async_trait]
impl EffectExecutor for ApplyAll {
    async fn apply(
        &self,
        _call: &ProviderToolCall,
        entry: &ToolEntry,
    ) -> anyhow::Result<EffectOutcome> {
        self.applied.fetch_add(1, Ordering::SeqCst);
        Ok(EffectOutcome {
            output: "ok".into(),
            mutated: !entry.effect.is_read_only(),
        })
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
    }
}

fn loop_with<'a>(
    transport: &'a Scripted,
    catalog: &'a StaticCatalog,
    executor: &'a ApplyAll,
    measurer: &'a EnergyScript,
) -> ToolLoop<'a> {
    ToolLoop {
        transport,
        model: ModelId::new("test", "scripted"),
        catalog,
        capabilities: vec![worker_capability()],
        contract: None,
        barrier: None,
        c_c_max: 0.0,
        executor,
        measurer,
        budgets: budgets(),
        cadence: VerificationCadence::default(),
    }
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
            LoopEvent::EffectDenied { call_id, reason } if reason.contains("Budget") => {
                Some(call_id.as_str())
            }
            _ => None,
        })
        .collect();
    // M = 4, six calls returned: two denied over budget.
    assert_eq!(denied, ["c4", "c5"]);
    assert_eq!(executor.applied.load(Ordering::SeqCst), 4);
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
    };
    let measurer = EnergyScript::new(vec![]);

    let mut toolloop = loop_with(&transport, &catalog, &executor, &measurer);
    toolloop.budgets.max_turns = 0;
    assert!(toolloop.run("anything").await.is_err());
}
