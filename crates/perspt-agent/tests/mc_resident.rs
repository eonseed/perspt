//! Definition 6 mechanism checks: live resident-context paging on the
//! wire (in-place tombstones), the governed `context_recall` tool, and
//! infrastructure-gap steering (PSP-10 Gate AF).

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

/// A measurer that replays scripted (hard_pass, energy) readings.
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
        self.measure().await
    }
}

struct PassContract;

impl ContractEvaluator for PassContract {
    fn evaluate(&self, _transition: &CandidateTransition) -> ContractWitness {
        ContractWitness {
            ok: true,
            policy_version: "test-policy-v1".into(),
            evidence_refs: vec![],
        }
    }
}

struct ZeroBarrier;

impl BarrierEvaluator for ZeroBarrier {
    fn evaluate(
        &self,
        _transition: &CandidateTransition,
    ) -> std::result::Result<BarrierWitness, SdkError> {
        Ok(BarrierWitness {
            h_before: 0.0,
            expected_h_after_upper: 0.0,
            certified_increment: 0.0,
            unsafe_threshold: 1.0,
            evidence_refs: vec![],
        })
    }
}

static PASS_CONTRACT: PassContract = PassContract;
static ZERO_BARRIER: ZeroBarrier = ZeroBarrier;

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
            EffectKind::DataRead,
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
            text: "governed test worker".into(),
            ..Default::default()
        },
        recorder: None,
        partial_seed_root: None,
    }
}

fn budgets() -> LoopBudgets {
    LoopBudgets {
        max_turns: 8,
        max_calls_per_turn: 4,
        rejection_budget: 2,
        rho_gate: 0.5,
        declared_energy_floor: None,
        context_soft_limit_chars: 240_000,
        recovery_budget: 2,
        turn_deadline_secs: 120,
        resident: perspt_agent::toolloop::ResidentReserves::default(),
    }
}

/// A transport that records every conversation it is asked to send.
struct Capturing {
    inner: Scripted,
    sent: Mutex<Vec<Conversation>>,
}

/// `Capturing` with a configurable context window.
struct CapturingWindow {
    inner: Capturing,
    window: u32,
}

impl ModelTransport for CapturingWindow {
    fn chat_turn<'a>(
        &'a self,
        model: &'a ModelId,
        conversation: &'a Conversation,
        tools: &'a [ToolSpec],
        choice: ToolChoicePolicy,
    ) -> TransportFuture<'a, TurnOutput> {
        self.inner.chat_turn(model, conversation, tools, choice)
    }
    fn capabilities(&self, _model: &ModelId) -> ProviderCapabilities {
        ProviderCapabilities::text_only(self.window)
    }
    fn family_of(&self, model: &ModelId) -> ModelFamily {
        ModelFamily::from_model_name(&model.model)
    }
    fn adapter_kind(&self) -> &'static str {
        "scripted"
    }
}

impl ModelTransport for Capturing {
    fn chat_turn<'a>(
        &'a self,
        model: &'a ModelId,
        conversation: &'a Conversation,
        tools: &'a [ToolSpec],
        choice: ToolChoicePolicy,
    ) -> TransportFuture<'a, TurnOutput> {
        self.sent.lock().unwrap().push(conversation.clone());
        self.inner.chat_turn(model, conversation, tools, choice)
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

/// An executor whose `r1` read returns a large payload (other reads return
/// "ok"); mutations advance the state root like `ApplyAll`.
struct PayloadReads {
    payload: String,
    state: AtomicU32,
}

#[async_trait::async_trait]
impl EffectExecutor for PayloadReads {
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
        call: &ProviderToolCall,
        entry: &ToolEntry,
    ) -> anyhow::Result<EffectOutcome> {
        if !entry.effect.is_read_only() {
            self.state.fetch_add(1, Ordering::SeqCst);
        }
        Ok(EffectOutcome {
            output: if call.call_id == "r1" {
                self.payload.clone()
            } else {
                "ok".into()
            },
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

/// The eviction/recall assertions over the captured wire traffic and the
/// recorded events.
fn assert_evicted_then_recalled(
    sent: &[Conversation],
    events: &[LoopEvent],
    evicted_page: &str,
    payload: &str,
) {
    let tombstoned = sent.iter().any(|conversation| {
        conversation.messages().iter().any(|message| {
            matches!(message, perspt_sdk::Message::ToolResponse { call_id, content }
                if call_id == "r1" && content.contains("evicted from the resident context")
                   && content.contains(evicted_page))
        })
    });
    assert!(
        tombstoned,
        "the stale old tool result is tombstoned on the wire"
    );
    let recalled = events.iter().any(|event| {
        matches!(event, LoopEvent::ContextPageRecalled { page_id, .. } if page_id == evicted_page)
    });
    assert!(recalled, "the recall is ledgered as ContextPageRecalled");
    let restored = events.iter().any(|event| {
        matches!(event, LoopEvent::EffectApplied { call_id, output, .. }
            if call_id == "recall" && output.contains(payload))
    });
    assert!(restored, "recall returns the page's original content");
    let projection_whole = events.iter().any(|event| {
        matches!(event, LoopEvent::DurableCandidateCheckpoint { conversation, .. }
        if conversation.messages().iter().any(|message| matches!(
            message,
            perspt_sdk::Message::ToolResponse { call_id, content }
                if call_id == "r1" && content == payload
        )))
    });
    assert!(
        projection_whole,
        "the projection (backing store) keeps the evicted page whole"
    );
    assert_pairs_never_split(sent);
}

/// Spec :1431-1432: across every captured request, a tool call and its
/// results are either both verbatim or both tombstoned — never split.
fn assert_pairs_never_split(sent: &[Conversation]) {
    for (turn, conversation) in sent.iter().enumerate() {
        let mut call_state: std::collections::BTreeMap<String, bool> =
            std::collections::BTreeMap::new();
        for message in conversation.messages() {
            match message {
                perspt_sdk::Message::AssistantToolCalls { calls } => {
                    let evicted = calls
                        .iter()
                        .any(|call| call.arguments.get("_evicted_page").is_some());
                    for call in calls {
                        call_state.insert(call.call_id.clone(), evicted);
                    }
                }
                perspt_sdk::Message::ToolResponse { call_id, content } => {
                    let evicted = content.contains("evicted from the resident context");
                    if let Some(call_evicted) = call_state.get(call_id) {
                        // A pair may keep small call arguments verbatim while
                        // the result is tombstoned, but a tombstoned CALL with
                        // a verbatim RESULT would break atomic recall.
                        assert!(
                            !(*call_evicted && !evicted),
                            "turn {turn}: call {call_id} evicted but result verbatim"
                        );
                    }
                }
                _ => {}
            }
        }
    }
}

/// The loop for the eviction scenario: an eight-turn budget, the worker
/// capability widened with `DataRead` for `context_recall`.
fn eviction_loop<'a>(
    transport: &'a dyn ModelTransport,
    catalog: &'a StaticCatalog,
    executor: &'a PayloadReads,
    measurer: &'a EnergyScript,
    recording: &'a Recording,
) -> ToolLoop<'a> {
    let mut capability = worker_capability();
    capability.effects.push(EffectKind::DataRead);
    ToolLoop {
        transport,
        model: ModelId::new("test", "scripted"),
        fallback_models: Vec::new(),
        catalog,
        capabilities: vec![capability],
        contract: Some(&PASS_CONTRACT),
        barrier: Some(&ZERO_BARRIER),
        c_c_max: 0.0,
        executor,
        measurer,
        budgets: LoopBudgets {
            max_turns: 8,
            ..budgets()
        },
        cadence: VerificationCadence::default(),
        kernel_state: perspt_sdk::KernelState::new(),
        node_id: "toolloop".into(),
        generation: 0,
        system_prompt: perspt_agent::toolloop::PromptEnvelope {
            text: "governed test worker".into(),
            ..Default::default()
        },
        recorder: Some(recording),
        partial_seed_root: None,
    }
}

/// Definition 6, transport half: a tool result older than the pinned tail
/// whose birth root went stale after an acceptance is tombstoned in the
/// *sent* conversation while the projection keeps it whole, and
/// `context_recall` restores its content with a ledgered
/// `ContextPageRecalled`.
#[tokio::test]
async fn evicted_pages_are_tombstoned_on_the_wire_and_recallable() {
    let payload = "x".repeat(4_096);
    // The page is the atomic tool pair: the r1 call message plus its result.
    let pair_serialized = format!(
        "{}{}",
        serde_json::to_string(&perspt_sdk::Message::AssistantToolCalls {
            calls: vec![ProviderToolCall {
                call_id: "r1".into(),
                name: "read_file".into(),
                arguments: serde_json::json!({"path": "src/lib.rs"}),
            }],
        })
        .unwrap(),
        serde_json::to_string(&perspt_sdk::Message::ToolResponse {
            call_id: "r1".into(),
            content: payload.clone(),
        })
        .unwrap()
    );
    let evicted_page = perspt_sdk::ledger::content_hash(pair_serialized.as_bytes());
    let read = |id: &str| {
        TurnOutput::ToolCalls(vec![ProviderToolCall {
            call_id: id.into(),
            name: "read_file".into(),
            arguments: serde_json::json!({"path": "src/lib.rs"}),
        }])
    };
    let transport = Capturing {
        inner: Scripted::new(vec![
            read("r1"),
            read("r2"),
            read("r3"),
            read("r4"),
            TurnOutput::ToolCalls(vec![ProviderToolCall {
                call_id: "w1".into(),
                name: "edit_file".into(),
                arguments: serde_json::json!({
                    "path": "src/lib.rs", "old_string": "a", "new_string": "b"
                }),
            }]),
            TurnOutput::Text("checking".into()),
            TurnOutput::ToolCalls(vec![ProviderToolCall {
                call_id: "recall".into(),
                name: "context_recall".into(),
                arguments: serde_json::json!({"page_id": evicted_page}),
            }]),
            TurnOutput::Text("done".into()),
        ]),
        sent: Mutex::new(Vec::new()),
    };
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = PayloadReads {
        payload: payload.clone(),
        state: AtomicU32::new(0),
    };
    // Baseline 10; the w1 incremental consumes 9.9; the first boundary
    // descends (accepted, root 0 -> 1, staling the r1 pair); the final
    // boundary hard-passes.
    let measurer = EnergyScript::new(vec![(false, 10.0), (false, 9.9), (false, 9.0), (true, 0.0)]);
    let recording = Recording::default();
    let tool_loop = eviction_loop(&transport, &catalog, &executor, &measurer, &recording);

    let outcome = tool_loop.run("say done").await.unwrap();
    assert!(matches!(outcome.outcome, NodeTerminalOutcome::HardPass));

    let sent = transport.sent.lock().unwrap();
    let events = recording.events.lock().unwrap();
    assert_evicted_then_recalled(&sent, &events, &evicted_page, &payload);
}

/// Gate AF provenance: pages carry the root they were born under, so an
/// accepted-root change invalidates older optional pages — they are
/// tombstoned on the wire even though they are small enough to stay — while
/// the session-invariant head stays verbatim. Before this fix every page
/// was relabelled with the live root at assembly time, so no page ever went
/// stale.
#[tokio::test]
async fn a_root_change_invalidates_stale_optional_pages() {
    let read = |id: &str| {
        TurnOutput::ToolCalls(vec![ProviderToolCall {
            call_id: id.into(),
            name: "read_file".into(),
            arguments: serde_json::json!({"path": "src/lib.rs"}),
        }])
    };
    let mut turns: Vec<TurnOutput> = (1..=6).map(|i| read(&format!("r{i}"))).collect();
    turns.push(TurnOutput::ToolCalls(vec![ProviderToolCall {
        call_id: "w1".into(),
        name: "edit_file".into(),
        arguments: serde_json::json!({
            "path": "src/lib.rs", "old_string": "a", "new_string": "b"
        }),
    }]));
    turns.push(TurnOutput::Text("checking".into()));
    turns.push(read("r7"));
    turns.push(TurnOutput::Text("done".into()));
    let transport = Capturing {
        inner: Scripted::new(turns),
        sent: Mutex::new(Vec::new()),
    };
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    // Baseline 10; the w1 mutation's incremental pass consumes 9.9; the
    // first boundary descends (accepted, root 0 -> 1); the final boundary
    // hard-passes.
    let measurer = EnergyScript::new(vec![(false, 10.0), (false, 9.9), (false, 9.0), (true, 0.0)]);
    let recording = Recording::default();
    let mut tool_loop = loop_with(&transport, &catalog, &executor, &measurer);
    tool_loop.budgets.max_turns = 12;
    tool_loop.recorder = Some(&recording);

    tool_loop.run("edit the file").await.unwrap();

    let sent = transport.sent.lock().unwrap();
    let last = sent.last().expect("at least one request");
    let stale_tombstoned = last.messages().iter().any(|message| {
        matches!(message, perspt_sdk::Message::ToolResponse { call_id, content }
            if (call_id == "r1" || call_id == "r2")
               && content.contains("evicted from the resident context"))
    });
    assert!(
        stale_tombstoned,
        "small pages born under the old root must go stale after acceptance: {:?}",
        last.messages()
            .iter()
            .map(|m| format!("{m:?}").chars().take(80).collect::<String>())
            .collect::<Vec<_>>()
    );
    // The session-invariant head is mandatory and never tombstoned.
    for message in last.messages().iter().take(2) {
        let text = format!("{message:?}");
        assert!(
            !text.contains("evicted from the resident context"),
            "the instruction/task head must stay verbatim"
        );
    }
    assert_pairs_never_split(&sent);
}

/// Gate AF, the boundedness half: with a small context window and a stream
/// of oversized tool results, **no transport request ever exceeds the
/// input allowance or the dialect byte limit** — the composed request is
/// fitted (or refused with `ContextInfeasible`) before any call. Before
/// this fix tombstones were unbudgeted, so requests grew linearly with
/// conversation length.
#[tokio::test]
async fn no_composed_request_ever_exceeds_the_allowance() {
    let window: u32 = 16_000;
    let read = |i: usize| {
        TurnOutput::ToolCalls(vec![ProviderToolCall {
            call_id: format!("r{i}"),
            name: "read_file".into(),
            arguments: serde_json::json!({"path": format!("src/f{i}.rs")}),
        }])
    };
    let transport = CapturingWindow {
        inner: Capturing {
            inner: Scripted::new((1..=12).map(read).collect()),
            sent: Mutex::new(Vec::new()),
        },
        window,
    };
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = PayloadReads {
        payload: "y".repeat(7_000),
        state: AtomicU32::new(0),
    };
    let measurer = EnergyScript::new(vec![(false, 10.0)]);
    let recording = Recording::default();
    let mut tool_loop = eviction_loop(&transport, &catalog, &executor, &measurer, &recording);
    tool_loop.budgets.max_turns = 12;
    // The payload executor answers every read with 7 KB; whatever terminal
    // class the loop reaches, the bound must have held on every request.
    let _ = tool_loop.run("read many big files").await;

    let accountant = perspt_sdk::prompt::TokenAccountantRef::approx_bytes_v1();
    let sent = transport.inner.sent.lock().unwrap();
    assert!(!sent.is_empty());
    // The allowance ceiling: window minus output/guard reserves (the tool
    // reserve only shrinks it further, so this is the loosest legal bound).
    let ceiling = u64::from(window) - 1_024 - 256;
    for (index, conversation) in sent.iter().enumerate() {
        let mut tokens = 0u64;
        let mut bytes = 0u64;
        for message in conversation.messages() {
            let serialized = serde_json::to_string(message).unwrap();
            tokens += accountant.count_message(&serialized);
            bytes += serialized.len() as u64;
        }
        assert!(
            tokens <= ceiling,
            "request {index}: {tokens} tokens exceed the {ceiling} allowance ceiling"
        );
        assert!(
            bytes <= 262_144,
            "request {index}: over the dialect byte limit"
        );
    }
}

/// A recall of an unknown page id is a typed miss, never an error.
#[tokio::test]
async fn an_unknown_page_recall_is_a_typed_miss() {
    let transport = Scripted::new(vec![
        TurnOutput::ToolCalls(vec![ProviderToolCall {
            call_id: "recall".into(),
            name: "context_recall".into(),
            arguments: serde_json::json!({"page_id": "sha256:nope"}),
        }]),
        TurnOutput::Text("done".into()),
    ]);
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    let measurer = EnergyScript::new(vec![(false, 10.0), (true, 0.0)]);
    let recording = Recording::default();
    let mut tool_loop = loop_with(&transport, &catalog, &executor, &measurer);
    tool_loop.capabilities[0].effects.push(EffectKind::DataRead);
    tool_loop.recorder = Some(&recording);

    tool_loop.run("say done").await.unwrap();
    let events = recording.events.lock().unwrap();
    assert!(events.iter().any(|event| {
        matches!(event, LoopEvent::ContextMiss { key, .. } if key == "sha256:nope")
    }));
}

/// Infrastructure-only residuals steer with the environment gap, never a
/// "fix the code" correction the model can only loop on.
#[tokio::test]
async fn sensor_only_residuals_steer_with_the_environment_gap() {
    struct InfraResiduals;
    #[async_trait::async_trait]
    impl CandidateMeasurer for InfraResiduals {
        async fn measure(&self) -> anyhow::Result<Measured> {
            let mut residual = perspt_sdk::ResidualEvent::new(
                "toolloop",
                0,
                perspt_sdk::ResidualClass::SensorUnavailable,
                perspt_sdk::ResidualSeverity::Error,
                1.0,
                perspt_sdk::SensorRef::new(
                    "governed-verifier",
                    perspt_sdk::IndependenceRoute::DeterministicTool,
                ),
            )
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            residual.evidence.summary = "required sensor unavailable: required-stage:build".into();
            Ok(Measured {
                hard_pass: false,
                energy: 1.0,
                residuals: vec![residual],
                correction: None,
                packet: None,
            })
        }
        async fn measure_incremental(&self) -> anyhow::Result<Measured> {
            self.measure().await
        }
    }

    let transport = Scripted::new(vec![
        TurnOutput::Text("trying".into()),
        TurnOutput::Text("done".into()),
    ]);
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        applied: AtomicU32::new(0),
        state: AtomicU32::new(0),
    };
    let measurer = InfraResiduals;
    let recording = Recording::default();
    let tool_loop = ToolLoop {
        transport: &transport,
        model: ModelId::new("test", "scripted"),
        fallback_models: Vec::new(),
        catalog: &catalog,
        capabilities: vec![worker_capability()],
        contract: Some(&PASS_CONTRACT),
        barrier: Some(&ZERO_BARRIER),
        c_c_max: 0.0,
        executor: &executor,
        measurer: &measurer,
        budgets: budgets(),
        cadence: VerificationCadence::default(),
        kernel_state: perspt_sdk::KernelState::new(),
        node_id: "toolloop".into(),
        generation: 0,
        system_prompt: perspt_agent::toolloop::PromptEnvelope {
            text: "governed test worker".into(),
            ..Default::default()
        },
        recorder: Some(&recording),
        partial_seed_root: None,
    };
    let _ = tool_loop.run("do the task").await.unwrap();

    let events = recording.events.lock().unwrap();
    let steered_with_gap = events.iter().any(|event| {
        matches!(event, LoopEvent::ConversationDelta { record }
            if serde_json::to_string(record).unwrap()
                .contains("blocked by unavailable sensors"))
    });
    assert!(
        steered_with_gap,
        "the steering message names the environment gap"
    );
    let misleading = events.iter().any(|event| {
        matches!(event, LoopEvent::ConversationDelta { record }
            if serde_json::to_string(record).unwrap()
                .contains("Address the dominant residual"))
    });
    assert!(
        !misleading,
        "no code-directed correction is pushed for infra-only residuals"
    );
}
