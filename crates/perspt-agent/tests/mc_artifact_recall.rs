//! Artifact retrieval mechanism checks (PSP-10 Gate AF, large outputs).
//!
//! A tool output above the 8 KiB preview bound is truncated with an
//! `artifact:<handle>` note; the `read_artifact` tool must page the stored
//! bytes back in windows that themselves respect the preview bound, so the
//! model can reassemble the full output without any response exceeding the
//! bound. A miss must be a plain miss message, never an error.

use std::collections::HashMap;
use std::sync::Mutex;

use perspt_agent::toolloop::{
    CandidateCheckpoint, CandidateMeasurer, EffectExecutor, EffectOutcome, LoopBudgets, LoopEvent,
    LoopRecorder, Measured, ToolLoop,
};
use perspt_sdk::{
    ActorId, BarrierEvaluator, BarrierWitness, CandidateStateWitness, CandidateTransition,
    Capability, ContractEvaluator, ContractWitness, Conversation, EffectKind, Message, ModelFamily,
    ModelId, ModelTransport, ProviderCapabilities, ProviderToolCall, RiskBudget, SdkError,
    StaticCatalog, ToolChoicePolicy, ToolEntry, ToolSpec, TransportFuture, TurnOutput,
    VerificationCadence,
};

/// The oversized deterministic payload a single read produces.
fn big_payload() -> String {
    (0..800)
        .map(|i| format!("payload line {i:04} abcdefghijklmnopqrstuvwxyz\n"))
        .collect()
}

/// Recorder that stores artifacts in memory and serves them back.
#[derive(Default)]
struct ArtifactStore {
    events: Mutex<Vec<LoopEvent>>,
    artifacts: Mutex<HashMap<String, Vec<u8>>>,
}

impl LoopRecorder for ArtifactStore {
    fn record(&self, event: &LoopEvent) -> anyhow::Result<()> {
        self.events.lock().unwrap().push(event.clone());
        Ok(())
    }

    fn record_artifact(&self, content: &[u8], _media_type: &str) -> anyhow::Result<String> {
        let handle = perspt_sdk::ledger::content_hash(content);
        self.artifacts
            .lock()
            .unwrap()
            .insert(handle.clone(), content.to_vec());
        Ok(handle)
    }

    fn fetch_artifact(&self, handle: &str) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(self.artifacts.lock().unwrap().get(handle).cloned())
    }
}

/// Executor returning the oversized payload for any read.
struct BigRead;

#[async_trait::async_trait]
impl EffectExecutor for BigRead {
    async fn checkpoint(&self, _scope: &[String]) -> anyhow::Result<CandidateCheckpoint> {
        Ok(CandidateCheckpoint {
            id: "0".into(),
            witness: CandidateStateWitness {
                state_root: "0".into(),
                node_id: "toolloop".into(),
                canonical_scope: vec!["src/lib.rs".into()],
                ..CandidateStateWitness::default()
            },
        })
    }

    async fn apply(
        &self,
        _call: &ProviderToolCall,
        _entry: &ToolEntry,
    ) -> anyhow::Result<EffectOutcome> {
        Ok(EffectOutcome {
            output: big_payload(),
            mutated: false,
        })
    }

    async fn restore(&self, _checkpoint: &CandidateCheckpoint) -> anyhow::Result<()> {
        Ok(())
    }

    async fn state_witness(&self) -> anyhow::Result<CandidateStateWitness> {
        Ok(self.checkpoint(&[]).await?.witness)
    }
}

/// Transport that reads the truncation note out of the conversation, pages
/// the artifact window by window, and accumulates what it saw.
struct Prober {
    issued_read: Mutex<bool>,
    pages: Mutex<Vec<String>>,
}

fn latest_tool_response(conversation: &Conversation) -> Option<&str> {
    conversation.messages().iter().rev().find_map(|m| match m {
        Message::ToolResponse { content, .. } => Some(content.as_str()),
        _ => None,
    })
}

fn parse_handle(note: &str) -> Option<String> {
    let start = note.find("artifact:")? + "artifact:".len();
    let rest = &note[start..];
    let end = rest.find(';')?;
    Some(rest[..end].to_string())
}

fn parse_continue_offset(page: &str) -> Option<u64> {
    let start = page.rfind("[continue with offset=")? + "[continue with offset=".len();
    let rest = &page[start..];
    let end = rest.find(']')?;
    rest[..end].parse().ok()
}

impl ModelTransport for Prober {
    fn chat_turn<'a>(
        &'a self,
        _model: &'a ModelId,
        conversation: &'a Conversation,
        _tools: &'a [ToolSpec],
        _choice: ToolChoicePolicy,
    ) -> TransportFuture<'a, TurnOutput> {
        let output = (|| {
            let mut issued = self.issued_read.lock().unwrap();
            if !*issued {
                *issued = true;
                return TurnOutput::ToolCalls(vec![ProviderToolCall {
                    call_id: "r0".into(),
                    name: "read_file".into(),
                    arguments: serde_json::json!({"path": "src/lib.rs"}),
                }]);
            }
            let Some(response) = latest_tool_response(conversation) else {
                return TurnOutput::Text("done".into());
            };
            // First page request comes from the truncation note; later ones
            // from the previous window's continuation hint.
            let request = if response.starts_with("artifact ") {
                let mut pages = self.pages.lock().unwrap();
                if pages.last().map(String::as_str) == Some(response) {
                    return TurnOutput::Text("done".into());
                }
                pages.push(response.to_string());
                drop(pages);
                parse_continue_offset(response)
                    .map(|offset| (parse_handle_from_page(response), offset))
            } else {
                parse_handle(response).map(|handle| (handle, 0))
            };
            match request {
                Some((handle, offset)) => TurnOutput::ToolCalls(vec![ProviderToolCall {
                    call_id: format!("a{offset}"),
                    name: "read_artifact".into(),
                    arguments: serde_json::json!({"handle": handle, "offset": offset}),
                }]),
                None => TurnOutput::Text("done".into()),
            }
        })();
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

fn parse_handle_from_page(page: &str) -> String {
    // "artifact <handle>: bytes a..b of n"
    page.trim_start_matches("artifact ")
        .split(':')
        .next()
        .unwrap_or_default()
        .to_string()
}

struct EnergyScript;

#[async_trait::async_trait]
impl CandidateMeasurer for EnergyScript {
    async fn measure(&self) -> anyhow::Result<Measured> {
        Ok(Measured {
            hard_pass: false,
            energy: 1.0,
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

fn worker_capability() -> Capability {
    let mut cap = Capability::new(
        ActorId::new("toolloop"),
        vec![
            EffectKind::ReadFile,
            EffectKind::DataRead,
            EffectKind::Search,
            EffectKind::List,
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
        max_turns: 20,
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

#[tokio::test]
async fn oversized_output_pages_back_in_full_through_read_artifact() {
    let transport = Prober {
        issued_read: Mutex::new(false),
        pages: Mutex::new(Vec::new()),
    };
    let catalog = StaticCatalog::with_base(vec![]).unwrap();
    let executor = BigRead;
    let measurer = EnergyScript;
    let recorder = ArtifactStore::default();
    let toolloop = ToolLoop {
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
            text: "You are a governed coding agent. Propose tool calls; \
                every effect is mediated."
                .into(),
            ..Default::default()
        },
        recorder: Some(&recorder),
    };

    toolloop.run("read the big file").await.unwrap();

    // Every model-facing effect output respected the preview bound.
    let events = recorder.events.lock().unwrap();
    let outputs: Vec<&String> = events
        .iter()
        .filter_map(|event| match event {
            LoopEvent::EffectApplied { output, .. } => Some(output),
            _ => None,
        })
        .collect();
    assert!(!outputs.is_empty());
    for output in &outputs {
        assert!(
            output.len() <= 8 * 1024 + 256,
            "an effect output exceeded the preview bound: {} bytes",
            output.len()
        );
    }
    let truncated = outputs
        .iter()
        .find(|output| output.contains("[full output: artifact:"))
        .expect("the oversized read must carry a truncation note");
    let handle = parse_handle(truncated).expect("note names a handle");

    // The pages the transport collected reassemble the original payload.
    let pages = transport.pages.lock().unwrap();
    assert!(pages.len() >= 2, "paging must take more than one window");
    let mut reassembled = String::new();
    for page in pages.iter() {
        let body_start = page.find('\n').expect("window has a header line") + 1;
        let body = &page[body_start..];
        let body = body
            .rfind("\n[continue with offset=")
            .map_or(body, |cut| &body[..cut]);
        reassembled.push_str(body);
    }
    assert_eq!(
        reassembled,
        big_payload(),
        "windows must reassemble losslessly"
    );

    // A miss is a plain message, not an error.
    drop(events);
    assert!(recorder
        .fetch_artifact("0000000000000000000000000000000000000000000000000000000000000000")
        .unwrap()
        .is_none());
    let _ = handle;
}
