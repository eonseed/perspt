//! End-to-end tests for the `perspt` `"Omni"` facade across coding and
//! non-coding domains, custom tools, async `ask_user`, research citation
//! self-healing, subagent delegation, and interactive human corrections.

use std::sync::{Arc, Mutex};

use perspt::core::events::channel::{action_channel, event_channel};
use perspt::core::{AgentAction, AgentEvent};
use perspt::prelude::*;
use perspt::sdk::{
    AccessMode, Conversation, CorrectionDirection, DomainDetection, DomainId, DomainScope,
    EnergyComponent, EnergyModel, FootprintSpec, HardGatePolicy, IndependenceRoute,
    ProviderCapabilities, ProviderToolCall, ResidualClass, ResidualEvent, ResidualSchema,
    ResidualSeverity, ResidualWeight, ResourceSelector, SafetyBarrierDisposition, SensorRef,
    ToolChoicePolicy, ToolOrigin, ToolSpec, TransportFuture, TurnOutput, VerifierSuiteSpec,
};

struct RecordedTransport {
    turns: Mutex<Vec<TurnOutput>>,
    calls: Mutex<Vec<(String, Vec<String>)>>,
}

impl RecordedTransport {
    fn new(turns: Vec<TurnOutput>) -> Arc<Self> {
        Arc::new(Self {
            turns: Mutex::new(turns),
            calls: Mutex::new(Vec::new()),
        })
    }

    fn calls(&self) -> Vec<(String, Vec<String>)> {
        self.calls.lock().unwrap().clone()
    }
}

impl ModelTransport for RecordedTransport {
    fn chat_turn<'a>(
        &'a self,
        _model: &'a ModelId,
        conversation: &'a Conversation,
        tools: &'a [ToolSpec],
        _choice: ToolChoicePolicy,
    ) -> TransportFuture<'a, TurnOutput> {
        let conv_json = serde_json::to_string(conversation).unwrap_or_default();
        let tool_names = tools.iter().map(|t| t.name.clone()).collect();
        self.calls.lock().unwrap().push((conv_json, tool_names));
        let mut queue = self.turns.lock().unwrap();
        let out = if queue.is_empty() {
            TurnOutput::Text("complete".into())
        } else {
            queue.remove(0)
        };
        Box::pin(async move { Ok(out) })
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

    fn family_of(&self, _model: &ModelId) -> perspt::sdk::ModelFamily {
        perspt::sdk::ModelFamily::Other("scripted".into())
    }

    fn adapter_kind(&self) -> &'static str {
        "scripted"
    }
}

struct SpecDomain {
    required_file: &'static str,
    required_marker: &'static str,
}

impl SpecDomain {
    fn new(required_file: &'static str, required_marker: &'static str) -> Self {
        Self {
            required_file,
            required_marker,
        }
    }
}

impl AgentDomainPackage for SpecDomain {
    fn domain_id(&self) -> DomainId {
        DomainId::new("spec-omni")
    }

    fn detect(&self, _: &WorkspaceSnapshot) -> DomainDetection {
        DomainDetection {
            domain: self.domain_id(),
            activated: true,
            confidence: 1.0,
            evidence: vec!["explicit omni spec domain".into()],
        }
    }

    fn residual_schema(&self, _: &DomainScope) -> ResidualSchema {
        ResidualSchema::new(vec![
            ResidualClass::TestFailure,
            ResidualClass::SensorUnavailable,
        ])
    }

    fn energy_model(&self, _: &DomainScope) -> EnergyModel {
        let mut model = EnergyModel::new("spec-omni", 0.5).with_correction_budget(4);
        model.residual_weights = spec_weights();
        model
    }

    fn correction_directions(&self, residuals: &[ResidualEvent]) -> Vec<CorrectionDirection> {
        residuals
            .iter()
            .map(|r| {
                let mut d = CorrectionDirection::new(r.class, "satisfy spec invariant");
                d.target_paths = r.affected_paths.clone();
                d.rationale = r.evidence.summary.clone();
                d
            })
            .collect()
    }

    fn tool_entries(&self, _: &DomainScope) -> Vec<ToolEntry> {
        Vec::new()
    }

    fn hard_gate_policy(&self, _: &DomainScope) -> HardGatePolicy {
        HardGatePolicy {
            required_stages: vec!["spec-check".into()],
        }
    }

    fn verifier_suite(&self, _: &DomainScope) -> VerifierSuiteSpec {
        VerifierSuiteSpec {
            stages: vec!["spec-check".into()],
            ..VerifierSuiteSpec::default()
        }
    }

    fn safety_barrier(&self, _: &DomainScope) -> SafetyBarrierDisposition {
        SafetyBarrierDisposition::NotClaimed {
            reason: "test spec domain".into(),
        }
    }

    fn evaluate_workspace(
        &self,
        snapshot: &WorkspaceSnapshot,
        node_id: &str,
        generation: u32,
    ) -> Option<DomainEvaluation> {
        let ok = snapshot
            .read_text(self.required_file)
            .is_some_and(|body| body.contains(self.required_marker));
        let residuals = if ok {
            Vec::new()
        } else {
            let mut ev = ResidualEvent::new(
                node_id,
                generation,
                ResidualClass::TestFailure,
                ResidualSeverity::Error,
                1.0,
                SensorRef::new("spec-check", IndependenceRoute::DeterministicTool),
            )
            .ok()?;
            ev.affected_paths = vec![self.required_file.into()];
            ev.evidence.summary = format!(
                "{} missing required marker {:?}",
                self.required_file, self.required_marker
            );
            vec![ev]
        };
        Some(DomainEvaluation {
            ran_stages: vec!["spec-check".into()],
            residuals,
            advisory: Vec::new(),
        })
    }
}

fn spec_weights() -> Vec<ResidualWeight> {
    use EnergyComponent::{Boot, Log, Str};
    use ResidualClass::*;
    vec![
        ResidualWeight::new(TestFailure, Log, 3.0),
        ResidualWeight::new(SensorUnavailable, Boot, 1.0),
        ResidualWeight::new(ToolFailure, Boot, 1.0),
        ResidualWeight::new(ToolArgumentInvalid, Str, 1.0),
        ResidualWeight::new(WitnessStale, Str, 1.0),
        ResidualWeight::new(ToolThrash, Str, 1.0),
        ResidualWeight::new(ToolBatchConflict, Str, 1.0),
        ResidualWeight::new(UnreadEdit, Str, 1.0),
        ResidualWeight::new(BudgetPressure, Str, 1.0),
        ResidualWeight::new(AdjudicatorObjection, Log, 2.0),
        ResidualWeight::new(ProviderUnavailable, Boot, 1.0),
    ]
}

struct EchoProbeTool;

#[async_trait::async_trait]
impl CandidateToolHandler for EchoProbeTool {
    async fn apply(
        &self,
        _workspace: &perspt::agent::CandidateWorkspace,
        call: &ProviderToolCall,
        _entry: &ToolEntry,
    ) -> anyhow::Result<perspt::agent::toolloop::EffectOutcome> {
        let topic = call
            .arguments
            .get("topic")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        Ok(perspt::agent::toolloop::EffectOutcome {
            output: format!("echo:{topic}"),
            mutated: false,
            completed: true,
        })
    }
}

fn echo_probe_entry() -> ToolEntry {
    ToolEntry {
        name: "echo_probe".into(),
        description: "Omni read-only probe".into(),
        discovery_summary: String::new(),
        description_templates: None,
        effect: EffectKind::SystemProbe,
        risk: RiskClass::Low,
        schema: serde_json::json!({
            "type": "object",
            "properties": {"topic": {"type": "string", "description": "Topic to probe"}},
            "required": ["topic"],
            "additionalProperties": false
        }),
        footprint: FootprintSpec::new(vec![ResourceSelector::ScopedArgument {
            family: "omni".into(),
            field: "topic".into(),
            access: AccessMode::Read,
        }]),
        proposal_bindings: Vec::new(),
        durable: false,
        origin: ToolOrigin::Builtin,
        hot: false,
    }
}

fn call_turn(id: &str, name: &str, arguments: serde_json::Value) -> TurnOutput {
    TurnOutput::ToolCalls(vec![ProviderToolCall {
        call_id: id.into(),
        name: name.into(),
        arguments,
    }])
}

#[tokio::test]
async fn custom_non_coding_domain_and_custom_tool_hard_pass() {
    let dir = tempfile::tempdir().unwrap();
    let transport = RecordedTransport::new(vec![
        call_turn("t1", "echo_probe", serde_json::json!({"topic": "workflow"})),
        call_turn(
            "t2",
            "ask_user",
            serde_json::json!({"question": "confirmed"}),
        ),
        call_turn(
            "t3",
            "write_file",
            serde_json::json!({"path": "spec.txt", "content": "echo:workflow|async:confirmed\n"}),
        ),
        TurnOutput::Text("done".into()),
    ]);
    let runtime = AgentBuilder::new(dir.path())
        .model("scripted", "omni")
        .transport(transport.clone())
        .domain(Arc::new(SpecDomain::new(
            "spec.txt",
            "echo:workflow|async:confirmed",
        )))
        .register_tool(echo_probe_entry(), Arc::new(EchoProbeTool))
        .on_ask_user_async(|q| async move { Ok(format!("async:{q}")) })
        .approval_policy(ApprovalPolicy::Auto)
        .allow_unisolated_verifiers(true)
        .max_turns(6)
        .database_path(dir.path().join("runtime.db"))
        .build()
        .unwrap();
    let summary = runtime.run("produce verified spec".into()).await.unwrap();
    assert!(matches!(summary.outcome, NodeTerminalOutcome::HardPass));
    let calls = transport.calls();
    assert!(calls[1].0.contains("echo:workflow"));
    assert!(calls[2].0.contains("async:confirmed"));
    let promoted = std::fs::read_to_string(dir.path().join("spec.txt")).unwrap();
    assert!(promoted.contains("echo:workflow|async:confirmed"));
}

#[tokio::test]
async fn research_domain_heals_unresolved_citation_to_hard_pass() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("refs.bib"),
        "@article{known2026,\n  title={Grounded Research}\n}\n",
    )
    .unwrap();
    let transport = RecordedTransport::new(vec![
        call_turn(
            "w1",
            "write_file",
            serde_json::json!({"path": "draft.md", "content": "Claim [@missing2026].\n"}),
        ),
        TurnOutput::Text("first pass".into()),
        call_turn(
            "w2",
            "write_file",
            serde_json::json!({"path": "draft.md", "content": "Claim [@known2026].\n"}),
        ),
        TurnOutput::Text("fixed pass".into()),
    ]);
    let runtime = AgentBuilder::new(dir.path())
        .research_domain()
        .transport(transport.clone())
        .approval_policy(ApprovalPolicy::Auto)
        .allow_unisolated_verifiers(true)
        .max_turns(6)
        .database_path(dir.path().join("runtime.db"))
        .build()
        .unwrap();
    let summary = runtime
        .run("verify citations in draft.md".into())
        .await
        .unwrap();
    assert!(matches!(summary.outcome, NodeTerminalOutcome::HardPass));
    let calls = transport.calls();
    assert!(calls.len() >= 3 && calls[2].0.contains("missing2026"));
    let draft = std::fs::read_to_string(dir.path().join("draft.md")).unwrap();
    assert_eq!(draft, "Claim [@known2026].\n");
}

#[tokio::test]
async fn subagent_delegation_imports_worker_mutations_without_recursive_privilege() {
    let dir = tempfile::tempdir().unwrap();
    let transport = RecordedTransport::new(vec![
        call_turn(
            "p1",
            "spawn_agent",
            serde_json::json!({"role": "worker", "goal": "write worker_note.txt"}),
        ),
        call_turn(
            "c1",
            "write_file",
            serde_json::json!({"path": "worker_note.txt", "content": "delegated-ok\n"}),
        ),
        TurnOutput::Text("worker finished".into()),
        TurnOutput::Text("parent finished".into()),
    ]);
    let runtime = AgentBuilder::new(dir.path())
        .transport(transport.clone())
        .domain(Arc::new(SpecDomain::new("worker_note.txt", "delegated-ok")))
        .allow_subagents(true)
        .approval_policy(ApprovalPolicy::Auto)
        .allow_unisolated_verifiers(true)
        .max_turns(6)
        .database_path(dir.path().join("runtime.db"))
        .build()
        .unwrap();
    let summary = runtime.run("delegate note creation".into()).await.unwrap();
    assert!(matches!(summary.outcome, NodeTerminalOutcome::HardPass));
    let calls = transport.calls();
    assert_eq!(calls.len(), 4);
    assert!(!calls[1].1.iter().any(|name| name == "spawn_agent"));
    let promoted = std::fs::read_to_string(dir.path().join("worker_note.txt")).unwrap();
    assert_eq!(promoted, "delegated-ok\n");
}

#[tokio::test]
async fn interactive_correction_refines_candidate_before_promotion() {
    let dir = tempfile::tempdir().unwrap();
    let (ev_tx, mut ev_rx) = event_channel();
    let (act_tx, act_rx) = action_channel();
    let reviewer = tokio::spawn(async move {
        let mut requests = 0u32;
        while let Some(ev) = ev_rx.recv().await {
            if let AgentEvent::ApprovalRequest { request_id, .. } = ev {
                let action = if requests == 0 {
                    AgentAction::RequestCorrection {
                        request_id,
                        feedback: "Include APPROVED-V2 marker".into(),
                    }
                } else {
                    AgentAction::Approve { request_id }
                };
                requests += 1;
                let _ = act_tx.send(action);
                if requests >= 2 {
                    break;
                }
            }
        }
    });
    let transport = RecordedTransport::new(vec![
        call_turn(
            "a1",
            "write_file",
            serde_json::json!({"path": "plan.md", "content": "PLAN: v1\n"}),
        ),
        TurnOutput::Text("initial draft".into()),
        call_turn(
            "a2",
            "write_file",
            serde_json::json!({"path": "plan.md", "content": "PLAN: APPROVED-V2\n"}),
        ),
        TurnOutput::Text("revised draft".into()),
    ]);
    let runtime = AgentBuilder::new(dir.path())
        .transport(transport.clone())
        .domain(Arc::new(SpecDomain::new("plan.md", "PLAN:")))
        .approval_policy(ApprovalPolicy::Ask)
        .allow_unisolated_verifiers(true)
        .connect_channels(ev_tx, act_rx)
        .max_turns(6)
        .database_path(dir.path().join("runtime.db"))
        .build()
        .unwrap();
    let summary = runtime.run("draft plan".into()).await.unwrap();
    reviewer.await.unwrap();
    assert!(matches!(summary.outcome, NodeTerminalOutcome::HardPass));
    assert!(transport.calls()[2].0.contains("APPROVED-V2"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("plan.md")).unwrap(),
        "PLAN: APPROVED-V2\n"
    );
}
