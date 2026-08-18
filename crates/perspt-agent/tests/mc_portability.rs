//! MC-S (Gate S): provider portability. The same governed loop, catalog, and
//! budgets driven through three different provider "families" must produce
//! identical classifications, identical gate decision sequences, and
//! identical denial behavior — the loop cannot tell the difference, because
//! nothing in it may read a vendor type.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use perspt_agent::toolloop::{
    CandidateCheckpoint, CandidateMeasurer, EffectExecutor, EffectOutcome, LoopBudgets, Measured,
    ToolLoop,
};
use perspt_sdk::{
    ActorId, CandidateStateWitness, Capability, Conversation, EffectKind, GateDecisionRef,
    ModelFamily, ModelId, ModelTransport, NodeTerminalOutcome, ProviderCapabilities,
    ProviderToolCall, RiskBudget, StaticCatalog, ToolChoicePolicy, ToolEntry, ToolSpec,
    TransportFuture, TurnOutput, VerificationCadence,
};

/// A scripted transport that impersonates one provider family.
struct FamilyScripted {
    family: ModelFamily,
    turns: Mutex<Vec<TurnOutput>>,
}

impl ModelTransport for FamilyScripted {
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

    fn family_of(&self, _model: &ModelId) -> ModelFamily {
        self.family.clone()
    }

    fn adapter_kind(&self) -> &'static str {
        "scripted"
    }
}

struct ApplyAll {
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
            packet: None,
        })
    }
}

fn script() -> Vec<TurnOutput> {
    vec![
        TurnOutput::ToolCalls(vec![
            ProviderToolCall {
                call_id: "c1".into(),
                name: "edit_file".into(),
                arguments: serde_json::json!({
                    "path": "src/lib.rs", "old_string": "a", "new_string": "b"
                }),
            },
            // An unknown tool: denial behavior must be provider-invariant.
            ProviderToolCall {
                call_id: "c2".into(),
                name: "launch_missiles".into(),
                arguments: serde_json::json!({}),
            },
        ]),
        TurnOutput::Text("finished".into()),
    ]
}

fn capability() -> Capability {
    let mut cap = Capability::new(
        ActorId::new("toolloop"),
        vec![
            EffectKind::ReadFile,
            EffectKind::ApplyPatch,
            EffectKind::WriteArtifact,
        ],
    );
    cap.max_calls = Some(100);
    cap.risk_budget = Some(RiskBudget::new("workspace", 1.0));
    cap
}

async fn run_family(family: ModelFamily) -> (NodeTerminalOutcome, Vec<GateDecisionRef>, u64) {
    let transport = FamilyScripted {
        family,
        turns: Mutex::new(script()),
    };
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = ApplyAll {
        state: AtomicU32::new(0),
    };
    let measurer = EnergyScript {
        readings: Mutex::new(vec![(false, 10.0), (true, 0.0)]),
    };
    let tool_loop = ToolLoop {
        transport: &transport,
        model: ModelId::new("test", "scripted"),
        fallback_models: Vec::new(),
        catalog: &catalog,
        capabilities: vec![capability()],
        contract: None,
        barrier: None,
        c_c_max: 0.0,
        executor: &executor,
        measurer: &measurer,
        budgets: LoopBudgets {
            max_turns: 6,
            max_calls_per_turn: 4,
            rejection_budget: 2,
            rho_gate: 0.5,
            declared_energy_floor: None,
            context_soft_limit_chars: 240_000,
            recovery_budget: 2,
            turn_deadline_secs: 120,
        },
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
    };
    let outcome = tool_loop.run("portability fixture").await.unwrap();
    let denials = outcome
        .events
        .iter()
        .filter(|event| {
            matches!(
                event,
                perspt_agent::toolloop::LoopEvent::EffectDenied { .. }
            )
        })
        .count() as u64;
    (outcome.outcome, outcome.trajectory.gate_decisions, denials)
}

#[tokio::test]
async fn mc_s_three_provider_families_yield_identical_classifications() {
    let families = [
        ModelFamily::from_model_name("gpt-5"),
        ModelFamily::from_model_name("claude-sonnet"),
        ModelFamily::from_model_name("gemini-3.7-flash"),
    ];
    let mut results = Vec::new();
    for family in families {
        results.push(run_family(family).await);
    }
    let (first_outcome, first_gates, first_denials) = &results[0];
    assert!(matches!(first_outcome, NodeTerminalOutcome::HardPass));
    assert!(*first_denials > 0, "the unknown tool must be denied");
    for (outcome, gates, denials) in &results[1..] {
        assert_eq!(
            format!("{outcome:?}"),
            format!("{first_outcome:?}"),
            "terminal classification must not depend on the provider family"
        );
        assert_eq!(
            gates.len(),
            first_gates.len(),
            "gate decision sequence must not depend on the provider family"
        );
        assert_eq!(denials, first_denials, "denials must be provider-invariant");
    }
}

/// PSP-10 Phase 4 (Gate Y): the prompt route is (adapter kind, family,
/// exact model). Scripted transports report the fixed adapter identity
/// `"scripted"`; a model served through any endpoint keeps its own family.
#[test]
fn prompt_route_composes_adapter_family_and_exact_model() {
    let transport = FamilyScripted {
        family: ModelFamily::Qwen,
        turns: Mutex::new(Vec::new()),
    };
    assert_eq!(transport.adapter_kind(), "scripted");
    let model = ModelId::new("some-gateway", "qwen-3.8-27b");
    let route = transport.prompt_route(&model);
    assert_eq!(route.adapter, "scripted");
    assert_eq!(
        route.family,
        ModelFamily::Qwen,
        "family survives the gateway"
    );
    assert_eq!(route.exact_model.as_deref(), Some("qwen-3.8-27b"));
    // Stability: the adapter identity is a constant of the transport.
    assert_eq!(transport.adapter_kind(), transport.adapter_kind());
}

/// PSP-10 Phase 4: with defaults, the route-compiled tool spec is
/// byte-identical to the neutral one for every base entry — the extension
/// is behavior-preserving until a description library exists.
#[test]
fn route_specs_default_to_the_neutral_spec() {
    let route = perspt_sdk::prompt::PromptRoute {
        adapter: "scripted".into(),
        family: ModelFamily::Qwen,
        exact_model: Some("qwen-3.8".into()),
    };
    for entry in perspt_sdk::base_entries() {
        let neutral = entry.to_spec(false);
        let routed = entry.to_spec_for_route(&route, false);
        assert_eq!(neutral, routed, "tool {}", entry.name);
    }
}

/// PSP-10 Phase 4: a description library changes presentation only, by the
/// first matching selector, and discovery ranks by the trusted summary.
#[test]
fn description_library_and_discovery_summary_are_presentation_only() {
    let mut entry = perspt_sdk::base_entries()
        .into_iter()
        .find(|entry| entry.name == "read_file")
        .unwrap();
    entry.description_templates = Some(perspt_sdk::ToolDescriptionLibrary {
        base: "neutral text".into(),
        overrides: vec![("scripted/Qwen".into(), "qwen text".into())],
    });
    entry.discovery_summary = "grep-like file inspection".into();
    let route = |family: ModelFamily| perspt_sdk::prompt::PromptRoute {
        adapter: "scripted".into(),
        family,
        exact_model: None,
    };
    let qwen = entry.to_spec_for_route(&route(ModelFamily::Qwen), false);
    assert_eq!(qwen.description, "qwen text");
    let other = entry.to_spec_for_route(&route(ModelFamily::Mistral), false);
    assert_eq!(other.description, "neutral text");
    // Authority fields never move.
    assert_eq!(qwen.schema, entry.schema);
    assert_eq!(entry.discovery_text(), "grep-like file inspection");
}
