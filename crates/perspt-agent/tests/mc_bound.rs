//! Finite-decision bound mechanism checks (PSP-10 Gate X, Phase 1).
//!
//! The falsifying predicate: a preview changes `gate_decisions.len()`, a
//! commit proceeds after the bound, or the loop's trajectory carries a
//! fabricated identity. The bound is enforced at two layers — the SDK
//! refuses the `(N_gate + 1)`-th submission outright, and the tool loop
//! refuses to submit once the bound is reached.

use std::sync::Mutex;

use perspt_agent::toolloop::{
    CandidateCheckpoint, CandidateMeasurer, EffectExecutor, EffectOutcome, LoopBudgets, LoopEvent,
    Measured, ToolLoop,
};
use perspt_sdk::{
    AcceptedTrajectory, CandidateStateWitness, Conversation, GateDecision, ModelFamily, ModelId,
    ModelTransport, ProviderCapabilities, ProviderToolCall, StaticCatalog, ToolChoicePolicy,
    ToolEntry, ToolSpec, TransportFuture, TurnOutput, VerificationCadence,
};

struct Scripted {
    turns: Mutex<Vec<TurnOutput>>,
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

struct Idle;

#[async_trait::async_trait]
impl EffectExecutor for Idle {
    async fn checkpoint(&self, _scope: &[String]) -> anyhow::Result<CandidateCheckpoint> {
        Ok(CandidateCheckpoint {
            id: "0".into(),
            witness: CandidateStateWitness::default(),
        })
    }

    async fn apply(
        &self,
        _call: &ProviderToolCall,
        _entry: &ToolEntry,
    ) -> anyhow::Result<EffectOutcome> {
        Ok(EffectOutcome {
            output: "ok".into(),
            mutated: false,
        })
    }

    async fn restore(&self, _checkpoint: &CandidateCheckpoint) -> anyhow::Result<()> {
        Ok(())
    }

    async fn state_witness(&self) -> anyhow::Result<CandidateStateWitness> {
        Ok(CandidateStateWitness::default())
    }
}

struct EnergyScript {
    readings: Mutex<Vec<(bool, f64)>>,
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

    async fn measure_incremental(&self) -> anyhow::Result<Measured> {
        self.measure().await
    }
}

fn budgets(max_turns: u32, rejection_budget: u32) -> LoopBudgets {
    LoopBudgets {
        max_turns,
        max_calls_per_turn: 4,
        rejection_budget,
        rho_gate: 0.5,
        declared_energy_floor: None,
        context_soft_limit_chars: 240_000,
        recovery_budget: rejection_budget,
    }
}

/// The SDK layer refuses the `(N_gate + 1)`-th submission. Repeated hard
/// passes are the reachable overshoot path: each appends a decision and
/// nothing in the trajectory itself is terminal, so without the guard the
/// decision count would grow without bound.
#[test]
fn sdk_refuses_submission_past_the_finite_bound() {
    let mut trajectory = AcceptedTrajectory::new("n1", 0, 1.0, 0.5, 1).unwrap();
    let bound = trajectory.decision_bound().unwrap();
    assert_eq!(bound, 4, "floor(1.0/0.5) + 1 + 1");
    for _ in 0..bound {
        let decision = trajectory.submit_with_floor(true, 0.0, None).unwrap();
        assert_eq!(decision, GateDecision::HardPass);
    }
    let refused = trajectory.submit_with_floor(true, 0.0, None);
    assert!(
        refused.is_err(),
        "submission past the bound must be refused"
    );
    assert_eq!(trajectory.gate_decisions.len() as u64, bound);
}

/// Under an adversarial script mixing descents and rejections the realized
/// decision count stays at or under the bound and the loop classifies a
/// terminal outcome; the enforcement never has to fire on the ordinary path.
#[tokio::test]
async fn loop_decisions_never_exceed_the_bound() {
    let turns: Vec<TurnOutput> = (0..40)
        .map(|i| TurnOutput::Text(format!("attempt {i}")))
        .collect();
    let transport = Scripted {
        turns: Mutex::new(turns),
    };
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = Idle;
    // Baseline 2.0, then two real descents and endless non-descents.
    let mut readings = vec![(false, 2.0), (false, 1.4), (false, 0.8)];
    readings.extend(std::iter::repeat_n((false, 0.8), 30));
    let measurer = EnergyScript {
        readings: Mutex::new(readings),
    };
    let toolloop = ToolLoop {
        transport: &transport,
        model: ModelId::new("test", "scripted"),
        fallback_models: Vec::new(),
        catalog: &catalog,
        capabilities: vec![],
        contract: None,
        barrier: None,
        c_c_max: 0.0,
        executor: &executor,
        measurer: &measurer,
        budgets: budgets(40, 3),
        cadence: VerificationCadence::default(),
        kernel_state: perspt_sdk::KernelState::new(),
        node_id: "n-bound".into(),
        generation: 0,
        recorder: None,
    };
    let bound = perspt_agent::toolloop::loop_decision_bound(2.0, &budgets(40, 3)).unwrap();
    let outcome = toolloop.run("impossible goal").await.unwrap();
    let decisions = outcome
        .events
        .iter()
        .filter(|event| matches!(event, LoopEvent::GateDecisionRecorded { .. }))
        .count() as u64;
    assert!(decisions <= bound, "{decisions} decisions > bound {bound}");
    assert_eq!(outcome.trajectory.gate_decisions.len() as u64, decisions);
}

/// The loop's trajectory carries the real node identity, not the historic
/// hardcoded `("toolloop", 0)` — Phase 2 keys candidates by this identity
/// and Phase 8 measures branches against it.
#[tokio::test]
async fn loop_trajectory_carries_the_real_node_identity() {
    let transport = Scripted {
        turns: Mutex::new(vec![TurnOutput::Text("thinking".into())]),
    };
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = Idle;
    let measurer = EnergyScript {
        readings: Mutex::new(vec![(false, 1.0), (true, 0.0)]),
    };
    let toolloop = ToolLoop {
        transport: &transport,
        model: ModelId::new("test", "scripted"),
        fallback_models: Vec::new(),
        catalog: &catalog,
        capabilities: vec![],
        contract: None,
        barrier: None,
        c_c_max: 0.0,
        executor: &executor,
        measurer: &measurer,
        budgets: budgets(4, 2),
        cadence: VerificationCadence::default(),
        kernel_state: perspt_sdk::KernelState::new(),
        node_id: "node-7".into(),
        generation: 3,
        recorder: None,
    };
    let outcome = toolloop.run("do it").await.unwrap();
    assert_eq!(outcome.trajectory.node_id, "node-7");
    assert_eq!(outcome.trajectory.generation, 3);
}
