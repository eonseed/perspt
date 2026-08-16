//! Authoritative PSP-9 agent runtime.

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use perspt_coding::{CodingDomain, OperationalSafetyBarrier};
use perspt_sdk::capability::CommandPattern;
use perspt_sdk::capability::PathPattern;
use perspt_sdk::{
    ActorId, AgentDomainPackage, ApprovalPolicy, CandidateTransition, Capability, CapabilityRole,
    ContractEvaluator, ContractWitness, Conversation, EffectKind, Footprint, GrantPolicy, Ledger,
    LedgerEvent, ModelId, ModelTransport, NodeClass, NodeTerminalOutcome, Resource, RiskBudget,
    Scheduler, StaticCatalog, ToolCatalog, ToolChoicePolicy, TurnOutput, WorkGraphRevision,
    WorkNode, WorkNodeState,
};
use perspt_store::{Psp9LedgerRow, Psp9VerdictRow, SessionRecord, SessionStore};

use crate::candidate::{CandidateWorkspace, CodingCandidateMeasurer};
use crate::toolloop::{EffectExecutor, LoopBudgets, LoopEvent, LoopRecorder, ToolLoop};
use crate::transport::GenAiTransport;

/// Finite settings for one PSP-9 run.
#[derive(Debug, Clone)]
pub struct Psp9RunConfig {
    pub max_turns: u32,
    pub max_calls_per_turn: u32,
    pub rejection_budget: u32,
    pub rho_gate: f64,
    pub approval_policy: ApprovalPolicy,
    /// Embedders that already isolate the entire process may opt out of the
    /// nested verifier sandbox. The CLI never enables this.
    pub allow_unisolated_verifiers: bool,
    pub max_parallel_verifiers: usize,
    /// Persist signed grant intent across sessions. Disabled by default;
    /// resume still re-mints fresh, epoch-bound capabilities.
    pub persistent_grants: bool,
}

impl Default for Psp9RunConfig {
    fn default() -> Self {
        Self {
            max_turns: 12,
            max_calls_per_turn: 8,
            rejection_budget: 4,
            rho_gate: 0.5,
            approval_policy: ApprovalPolicy::Ask,
            allow_unisolated_verifiers: false,
            max_parallel_verifiers: 4,
            persistent_grants: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Psp9RunSummary {
    pub session_id: String,
    pub node_id: String,
    pub outcome: NodeTerminalOutcome,
    pub turns_used: u32,
    pub ledger_head: String,
    pub promoted_paths: Vec<String>,
}

/// Explicit model-plane routes for one PSP-9 session. Role routes are not
/// interchangeable: only `fallbacks` may replace the actuator after a recorded
/// transport failure.
#[derive(Debug, Clone, Default)]
pub struct Psp9ModelRoutes {
    pub primary: Option<String>,
    pub actuator: Option<String>,
    pub explorer: Option<String>,
    pub adjudicator: Option<String>,
    pub fallbacks: Vec<String>,
}

/// The governed surfaces assembled for one node run.
struct NodeAssembly {
    catalog: StaticCatalog,
    grant_policy: GrantPolicy,
    capability: Capability,
    contract: CodingContract,
    barrier: OperationalSafetyBarrier,
    cadence: perspt_sdk::VerificationCadence,
    energy: perspt_sdk::EnergyModel,
    calibration: CalibrationBinding,
}

/// The immutable calibration epoch this run is bound to.
#[derive(Debug, Clone)]
struct CalibrationBinding {
    epoch_id: String,
    stratum: String,
    state: String,
    threshold: Option<f64>,
}

/// Everything `conclude_run` needs to decide a node's terminal fate.
struct ConcludeContext<'a> {
    recorder: &'a Psp9Recorder,
    candidate: &'a CandidateWorkspace,
    node_id: &'a str,
    task: &'a str,
    grant_policy: &'a GrantPolicy,
    capability: &'a Capability,
    contract: &'a CodingContract,
    barrier: &'a OperationalSafetyBarrier,
    kernel_state: &'a perspt_sdk::KernelState,
    loop_outcome: &'a NodeTerminalOutcome,
    calibration: &'a CalibrationBinding,
}

mod node;
mod recorder;

use node::*;
pub use recorder::Psp9Recorder;

/// The production runtime for one coding task.
pub struct Psp9AgentRuntime {
    working_dir: PathBuf,
    transport: Arc<dyn ModelTransport>,
    model: ModelId,
    fallback_models: Vec<ModelId>,
    explorer_model: Option<ModelId>,
    adjudicator_model: Option<ModelId>,
    config: Psp9RunConfig,
    event_sender: Option<perspt_core::events::channel::EventSender>,
    action_receiver: Option<perspt_core::events::channel::ActionReceiver>,
    database_path: Option<PathBuf>,
    shared_store: Option<Arc<SessionStore>>,
}

impl std::fmt::Debug for Psp9AgentRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Psp9AgentRuntime")
            .field("working_dir", &self.working_dir)
            .field("model", &self.model)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl Psp9AgentRuntime {
    pub fn from_config(
        working_dir: PathBuf,
        config: &perspt_core::Config,
        routes: Psp9ModelRoutes,
        run_config: Psp9RunConfig,
    ) -> Result<Self> {
        let portfolio = Arc::new(perspt_core::ModelPortfolio::from_config(config)?);
        let route = routes
            .primary
            .as_deref()
            .or(routes.actuator.as_deref())
            .or_else(|| config.models.as_ref().and_then(|m| m.actuator.as_deref()))
            .or(config.actuator_model.as_deref())
            .or(config.model.as_deref())
            .unwrap_or_else(|| crate::ModelTier::default_model_name());
        let model = qualify_model(route, config, &portfolio)?;
        // Resolve eagerly so a typo fails before a session is created.
        portfolio.resolve(&model.provider)?;
        let transport = Arc::new(GenAiTransport::new(portfolio));
        let primary_capabilities = transport.capabilities(&model);
        anyhow::ensure!(
            primary_capabilities.tool_calling,
            "route {model} does not declare native tool calling; PSP-9 \
             tool-loop mode never emulates tool calls in text"
        );
        let explorer_model = resolve_role_route(
            routes.explorer.as_deref(),
            config.models.as_ref().and_then(|m| m.speculator.as_deref()),
            config,
            &transport,
        )?;
        let adjudicator_model = resolve_role_route(
            routes.adjudicator.as_deref(),
            config
                .models
                .as_ref()
                .and_then(|m| m.adjudicator.as_deref()),
            config,
            &transport,
        )?;
        let mut fallback_models = Vec::new();
        for value in &routes.fallbacks {
            let candidate = qualify_model(value, config, transport.portfolio())?;
            transport.portfolio().resolve(&candidate.provider)?;
            anyhow::ensure!(
                transport.capabilities(&candidate).tool_calling,
                "fallback route {candidate} does not declare native tool calling"
            );
            if candidate != model && !fallback_models.contains(&candidate) {
                fallback_models.push(candidate);
            }
        }
        Ok(Self {
            working_dir,
            transport,
            model,
            fallback_models,
            explorer_model,
            adjudicator_model,
            config: run_config,
            event_sender: None,
            action_receiver: None,
            database_path: None,
            shared_store: None,
        })
    }

    /// Assemble the same runtime with an injected provider-neutral transport.
    /// Used by conformance tests and embedders that supply their own model plane.
    pub fn with_transport(
        working_dir: PathBuf,
        transport: Arc<dyn ModelTransport>,
        model: ModelId,
        run_config: Psp9RunConfig,
    ) -> Self {
        Self {
            working_dir,
            transport,
            model,
            fallback_models: Vec::new(),
            explorer_model: None,
            adjudicator_model: None,
            config: run_config,
            event_sender: None,
            action_receiver: None,
            database_path: None,
            shared_store: None,
        }
    }

    pub fn with_database_path(mut self, path: PathBuf) -> Self {
        self.database_path = Some(path);
        self
    }

    /// Record into an existing store handle instead of opening a second live
    /// handle on the same database file (used when an in-process dashboard
    /// already holds one).
    pub fn with_session_store(mut self, store: Arc<SessionStore>) -> Self {
        self.shared_store = Some(store);
        self
    }

    pub fn with_fallback_models(mut self, models: Vec<ModelId>) -> Self {
        self.fallback_models = models;
        self
    }

    pub fn with_explorer_model(mut self, model: ModelId) -> Self {
        self.explorer_model = Some(model);
        self
    }

    pub fn with_adjudicator_model(mut self, model: ModelId) -> Self {
        self.adjudicator_model = Some(model);
        self
    }

    pub fn connect_tui(
        mut self,
        sender: perspt_core::events::channel::EventSender,
        receiver: perspt_core::events::channel::ActionReceiver,
    ) -> Self {
        self.event_sender = Some(sender);
        self.action_receiver = Some(receiver);
        self
    }

    /// Open the durable session, run read-only exploration, and dispatch the
    /// initial work graph. Returns the recorder, the compiled goal, and the
    /// running graph revision.
    async fn open_session(
        &mut self,
        task: &str,
        session_id: &str,
        node_id: &str,
    ) -> Result<(Psp9Recorder, String, Scheduler, WorkGraphRevision)> {
        let recorder = Psp9Recorder::create(
            session_id,
            task,
            &self.working_dir,
            self.database_path.as_deref(),
            self.shared_store.clone(),
            self.event_sender.clone(),
        )?;
        recorder.record_custom(
            "session_started",
            serde_json::json!({
                "task": task,
                "model": self.model,
                "fallback_models": self.fallback_models,
                "mode": "tool_loop",
            }),
        )?;
        self.record_route_capabilities(&recorder)?;

        let agent_goal = self.explore(&recorder, task).await?;

        self.emit(perspt_core::AgentEvent::Log(format!(
            "PSP-9 session {} using {}",
            &session_id[..8],
            self.model
        )));
        self.emit(perspt_core::AgentEvent::TaskStatusChanged {
            node_id: node_id.to_string(),
            status: perspt_core::NodeStatus::Coding,
        });

        let graph = initial_graph(node_id, task)?;
        recorder.record_custom("graph_revision", serde_json::to_value(&graph)?)?;
        let mut scheduler = Scheduler::new(1);
        let ready = scheduler.ready_nodes(&graph, node_footprint);
        let selected = ready
            .first()
            .context("initial work graph has no schedulable node")?;
        scheduler.start(selected, node_footprint(selected));
        recorder.record_custom(
            "scheduler_dispatch",
            serde_json::json!({
                "node_id": selected.node_id,
                "generation": selected.generation,
                "parallel_slot": 0,
            }),
        )?;
        let running_graph = execution_revision(&graph, node_id, WorkNodeState::Running)?;
        recorder.record_custom("graph_revision", serde_json::to_value(&running_graph)?)?;
        Ok((recorder, agent_goal, scheduler, running_graph))
    }

    /// Mint (and for persistent grants, sign and verify) the session grant.
    fn mint_grant(
        &self,
        recorder: &Psp9Recorder,
        revision_id: &str,
    ) -> Result<perspt_sdk::GrantPolicy> {
        let grant_policy = session_grant_policy(
            &self.working_dir,
            revision_id,
            self.config.persistent_grants,
        )?;
        let signed_grant = if self.config.persistent_grants {
            let key = crate::grant::GrantSigningKey::resolve()?;
            let signed = perspt_sdk::SignedGrantPolicy::sign(grant_policy.clone(), &key.bytes)?;
            signed.verify_against(&key.public_key())?;
            recorder.record_custom(
                "grant_signing_key_resolved",
                serde_json::json!({"source": format!("{:?}", key.source)}),
            )?;
            Some(signed)
        } else {
            None
        };
        recorder.record_grant_policy(&grant_policy, signed_grant.as_ref())?;
        Ok(grant_policy)
    }

    /// Exploration-only session (PSP-9 phase 7): the deterministic repository
    /// map plus an interactive read-only explorer tool loop. The explorer
    /// holds only read capabilities; every tool call still passes the kernel,
    /// so a mutation attempt is denied and recorded rather than prevented by
    /// convention. Nothing is measured, gated, or promoted.
    pub async fn run_exploration(mut self, task: String) -> Result<Psp9RunSummary> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let node_id = "explore-1".to_string();
        let (recorder, _goal, mut scheduler, running_graph) =
            self.open_session(&task, &session_id, &node_id).await?;
        let candidate = self.open_candidate(&node_id, &running_graph.revision_id)?;
        let explorer = perspt_sdk::exploration_capability(perspt_sdk::ActorId::new("toolloop"));
        debug_assert!(perspt_sdk::is_read_only_capability(&explorer));
        let mut capability = explorer;
        capability.session_id = session_id.clone();
        capability.graph_revision = running_graph.revision_id.clone();
        capability.role = CapabilityRole::Explorer;
        let domain = CodingDomain::new();
        let scope = perspt_sdk::DomainScope {
            label: node_id.clone(),
            paths: Vec::new(),
        };
        let catalog = StaticCatalog::with_base(domain.tool_entries(&scope))
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let mut kernel_state = perspt_sdk::KernelState::new();
        kernel_state.set_witness("__graph_revision", running_graph.revision_id.clone());
        kernel_state.set_witness("__authority_epoch", capability.authority_epoch.to_string());
        let outcome = self
            .explorer_loop(
                &recorder,
                &candidate,
                &catalog,
                capability,
                kernel_state,
                &task,
            )
            .await;
        scheduler.finish(&node_id, 0);
        let status = if outcome.is_ok() {
            "COMPLETED_PSP9"
        } else {
            "FAILED_PSP9"
        };
        recorder.finish(status)?;
        outcome?;
        Ok(Psp9RunSummary {
            session_id,
            node_id,
            outcome: NodeTerminalOutcome::HardPass,
            turns_used: 0,
            ledger_head: recorder.head(),
            promoted_paths: Vec::new(),
        })
    }

    /// The interactive explorer: read-only calls execute after a kernel
    /// check; denials (including any mutation attempt) are returned to the
    /// model and ledgered.
    async fn explorer_loop(
        &self,
        recorder: &Psp9Recorder,
        candidate: &CandidateWorkspace,
        catalog: &StaticCatalog,
        capability: Capability,
        kernel_state: perspt_sdk::KernelState,
        task: &str,
    ) -> Result<()> {
        let model = self.explorer_model.as_ref().unwrap_or(&self.model);
        let budget = perspt_sdk::ExplorationBudget::default();
        let specs = catalog.specs_for(std::slice::from_ref(&capability), false);
        let mut conversation = Conversation::with_system(
            "You are a read-only repository explorer. Inspect with the provided \
             tools, then answer. You cannot modify anything.",
        );
        conversation.push_user(task.to_string());
        let mut tool_calls = 0u32;
        for _turn in 0..16u32 {
            let output = self
                .transport
                .chat_turn(model, &conversation, &specs, ToolChoicePolicy::Auto)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            match output {
                TurnOutput::Text(text) => {
                    recorder
                        .record_custom("exploration_answer", serde_json::json!({"text": text}))?;
                    self.emit(perspt_core::AgentEvent::Log(format!("Explorer: {text}")));
                    println!("{text}");
                    return Ok(());
                }
                TurnOutput::ToolCalls(calls) => {
                    conversation.push_tool_calls(calls.clone());
                    for call in calls {
                        tool_calls += 1;
                        let response = if tool_calls > budget.max_tool_calls {
                            "denied: exploration tool-call budget exhausted".to_string()
                        } else {
                            self.explorer_call(
                                recorder,
                                candidate,
                                catalog,
                                &capability,
                                &kernel_state,
                                &call,
                            )
                            .await?
                        };
                        conversation.push_tool_response(call.call_id.clone(), response);
                    }
                }
            }
        }
        anyhow::bail!("explorer did not produce a final answer within the turn budget")
    }

    /// One explorer call through the kernel; the read-only capability makes
    /// every mutation a recorded denial.
    async fn explorer_call(
        &self,
        recorder: &Psp9Recorder,
        candidate: &CandidateWorkspace,
        catalog: &StaticCatalog,
        capability: &Capability,
        kernel_state: &perspt_sdk::KernelState,
        call: &perspt_sdk::ProviderToolCall,
    ) -> Result<String> {
        use crate::toolloop::EffectExecutor as _;
        let Some(entry) = catalog.lookup(&call.name).cloned() else {
            return Ok(format!("denied: unknown tool {:?}", call.name));
        };
        let mut proposal =
            perspt_sdk::EffectProposal::new(capability.holder.clone(), "explore-1", entry.effect);
        if let Some(path) = call.arguments.get("path").and_then(|v| v.as_str()) {
            proposal = proposal.with_path(path);
        }
        let witness = perspt_sdk::check_admissibility(
            &proposal,
            std::slice::from_ref(capability),
            kernel_state,
        );
        if !matches!(witness.decision, perspt_sdk::AdmissibilityDecision::Allow) {
            recorder.record_custom(
                "exploration_denied",
                serde_json::json!({
                    "tool": call.name,
                    "decision": format!("{:?}", witness.decision),
                }),
            )?;
            return Ok(format!("denied: {:?}", witness.decision));
        }
        let outcome = candidate.apply(call, &entry).await?;
        recorder.record_custom(
            "exploration_tool_call",
            serde_json::json!({"tool": call.name, "mutated": outcome.mutated}),
        )?;
        Ok(outcome.output)
    }

    fn open_candidate(&self, node_id: &str, revision_id: &str) -> Result<CandidateWorkspace> {
        CandidateWorkspace::create_with_policy(
            &self.working_dir,
            node_id,
            0,
            revision_id,
            self.config.allow_unisolated_verifiers,
        )
    }

    pub async fn run(mut self, task: String) -> Result<Psp9RunSummary> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let node_id = "implement-1".to_string();
        let (recorder, agent_goal, mut scheduler, running_graph) =
            self.open_session(&task, &session_id, &node_id).await?;
        let candidate = self.open_candidate(&node_id, &running_graph.revision_id)?;
        let measurer = CodingCandidateMeasurer::new(&candidate, &node_id, 0)
            .with_max_parallel(self.config.max_parallel_verifiers);
        let NodeAssembly {
            catalog,
            grant_policy,
            capability,
            contract,
            barrier,
            cadence,
            energy,
            calibration,
        } = self.assemble_node(&recorder, &session_id, &running_graph, &node_id)?;
        let kernel_state = loop_kernel_state(&grant_policy, &running_graph.revision_id);
        let tool_loop = ToolLoop {
            transport: self.transport.as_ref(),
            model: self.model.clone(),
            fallback_models: self.fallback_models.clone(),
            catalog: &catalog,
            capabilities: vec![capability.clone()],
            contract: Some(&contract),
            barrier: Some(&barrier),
            c_c_max: 0.25,
            executor: &candidate,
            measurer: &measurer,
            budgets: self.loop_budgets(energy.rho_gate),
            cadence,
            kernel_state: kernel_state.clone(),
            node_id: node_id.clone(),
            generation: 0,
            recorder: Some(&recorder),
        };

        let outcome = match tool_loop.run(&agent_goal).await {
            Ok(outcome) => outcome,
            Err(error) => return Err(self.fail_session(&recorder, error)),
        };

        let verdict = self
            .conclude_run(ConcludeContext {
                recorder: &recorder,
                candidate: &candidate,
                node_id: &node_id,
                task: &task,
                grant_policy: &grant_policy,
                capability: &capability,
                contract: &contract,
                barrier: &barrier,
                kernel_state: &kernel_state,
                loop_outcome: &outcome.outcome,
                calibration: &calibration,
            })
            .await?;
        let (final_outcome, status, promoted_paths) = verdict;
        scheduler.finish(&node_id, 0);
        self.finish_node(&recorder, &running_graph, &node_id, &final_outcome, status)?;

        Ok(Psp9RunSummary {
            session_id,
            node_id,
            outcome: final_outcome,
            turns_used: outcome.turns_used,
            ledger_head: recorder.head(),
            promoted_paths,
        })
    }

    fn emit(&self, event: perspt_core::AgentEvent) {
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(event);
        }
    }

    /// Record a loop failure durably and close the session as failed.
    fn fail_session(&self, recorder: &Psp9Recorder, error: anyhow::Error) -> anyhow::Error {
        let recording = recorder
            .record_custom(
                "node_terminal",
                serde_json::json!({"class": "failed", "error": error.to_string()}),
            )
            .and_then(|()| recorder.finish("FAILED_PSP9"));
        if let Err(recording_error) = recording {
            log::warn!("failed to record session failure: {recording_error}");
        }
        self.emit(perspt_core::AgentEvent::Error(error.to_string()));
        error
    }

    fn loop_budgets(&self, domain_rho_gate: f64) -> LoopBudgets {
        LoopBudgets {
            max_turns: self.config.max_turns,
            max_calls_per_turn: self.config.max_calls_per_turn,
            rejection_budget: self.config.rejection_budget,
            rho_gate: self.config.rho_gate.max(domain_rho_gate),
            declared_energy_floor: None,
            context_soft_limit_chars: 240_000,
            recovery_budget: self.config.rejection_budget,
        }
    }

    /// Record the terminal graph revision and outcome, close the session.
    fn finish_node(
        &self,
        recorder: &Psp9Recorder,
        running_graph: &WorkGraphRevision,
        node_id: &str,
        final_outcome: &NodeTerminalOutcome,
        status: &str,
    ) -> Result<()> {
        let terminal_state = if matches!(final_outcome, NodeTerminalOutcome::HardPass) {
            WorkNodeState::Stable
        } else {
            WorkNodeState::Stopped {
                certificate_id: match final_outcome {
                    NodeTerminalOutcome::Escalated { certificate_id } => certificate_id.clone(),
                    _ => uuid::Uuid::new_v4().to_string(),
                },
            }
        };
        let terminal_graph = execution_revision(running_graph, node_id, terminal_state)?;
        recorder.record_custom("graph_revision", serde_json::to_value(&terminal_graph)?)?;
        recorder.record_custom(
            "node_terminal",
            serde_json::json!({"node_id": node_id, "outcome": final_outcome}),
        )?;
        recorder.finish(status)?;
        self.emit(perspt_core::AgentEvent::Complete {
            success: matches!(final_outcome, NodeTerminalOutcome::HardPass),
            message: format!("PSP-9 outcome: {final_outcome:?}"),
        });
        Ok(())
    }

    /// Build the node's governed surfaces: catalog, grant, capability,
    /// contract, barrier, cadence, and energy model.
    fn assemble_node(
        &self,
        recorder: &Psp9Recorder,
        session_id: &str,
        running_graph: &WorkGraphRevision,
        node_id: &str,
    ) -> Result<NodeAssembly> {
        let domain = CodingDomain::new();
        let scope = perspt_sdk::DomainScope {
            label: node_id.to_string(),
            paths: Vec::new(),
        };
        let catalog = StaticCatalog::with_base(domain.tool_entries(&scope))
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let grant_policy = self.mint_grant(recorder, &running_graph.revision_id)?;
        // Every live capability is the intersection of the worker template
        // with the grant ceilings — the ceilings are enforced, not decorative.
        let capability = grant_policy
            .mint(worker_capability(
                session_id,
                &running_graph.revision_id,
                grant_policy.authority_epoch,
            ))
            .map_err(|e| anyhow::anyhow!("grant intersection: {e}"))?;
        let contract = CodingContract {
            graph_revision: running_graph.revision_id.clone(),
            node_id: node_id.to_string(),
            generation: 0,
            policy: perspt_policy::engine::PolicyEngine::new()?,
        };
        let barrier = OperationalSafetyBarrier::default();
        let cadence = domain.verifier_suite(&scope).cadence;
        let energy = domain.energy_model(&scope);
        let calibration =
            self.record_calibration_readiness(recorder, &catalog, &capability, &cadence)?;
        Ok(NodeAssembly {
            catalog,
            grant_policy,
            capability,
            contract,
            barrier,
            cadence,
            energy,
            calibration,
        })
    }

    /// Decide the node's terminal fate after the loop: adjudication and
    /// human approval first, then the kernel-certified promotion. Anything
    /// short of a fully certified promotion escalates.
    async fn conclude_run(
        &mut self,
        ctx: ConcludeContext<'_>,
    ) -> Result<(NodeTerminalOutcome, &'static str, Vec<String>)> {
        let hard_pass = matches!(ctx.loop_outcome, NodeTerminalOutcome::HardPass);
        let promotion_approved = if hard_pass {
            let adjudicated = self
                .adjudicate_candidate(
                    ctx.recorder,
                    ctx.candidate,
                    ctx.task,
                    &ctx.calibration.stratum,
                )
                .await?;
            adjudicated && self.conformal_or_human_approval(&ctx).await?
        } else {
            false
        };
        let mut final_outcome = if hard_pass && !promotion_approved {
            NodeTerminalOutcome::Escalated {
                certificate_id: uuid::Uuid::new_v4().to_string(),
            }
        } else {
            ctx.loop_outcome.clone()
        };
        let mut promoted_paths = Vec::new();
        let status = if matches!(final_outcome, NodeTerminalOutcome::HardPass) {
            let certified = self
                .promote_certified(
                    ctx.recorder,
                    ctx.candidate,
                    ctx.node_id,
                    ctx.grant_policy,
                    ctx.capability,
                    ctx.contract,
                    ctx.barrier,
                    ctx.kernel_state,
                    ctx.calibration,
                )
                .await?;
            match certified {
                Some(paths) => {
                    promoted_paths = paths;
                    self.emit(perspt_core::AgentEvent::NodeCompleted {
                        node_id: ctx.node_id.to_string(),
                        goal: ctx.task.to_string(),
                    });
                    "COMPLETED_PSP9"
                }
                None => {
                    final_outcome = NodeTerminalOutcome::Escalated {
                        certificate_id: uuid::Uuid::new_v4().to_string(),
                    };
                    self.emit(perspt_core::AgentEvent::TaskStatusChanged {
                        node_id: ctx.node_id.to_string(),
                        status: perspt_core::NodeStatus::Escalated,
                    });
                    "ESCALATED_PSP9"
                }
            }
        } else {
            self.emit(perspt_core::AgentEvent::TaskStatusChanged {
                node_id: ctx.node_id.to_string(),
                status: perspt_core::NodeStatus::Escalated,
            });
            "ESCALATED_PSP9"
        };
        Ok((final_outcome, status, promoted_paths))
    }

    /// Autonomous commitment (PSP-9 Gate Q): with an *activated* calibration
    /// epoch for this exact stratum — reached only at the finite sample floor
    /// through delayed audit labels — a hard-pass candidate above the
    /// conformal threshold commits without a human prompt, and the certified
    /// accept is ledgered. Any other state (shadow, insufficient samples,
    /// stale) backs off to the configured approval policy.
    async fn conformal_or_human_approval(&mut self, ctx: &ConcludeContext<'_>) -> Result<bool> {
        // Score definition v1: hard pass ⇒ V = 0 ⇒ score 1/(1+V) = 1.0.
        const HARD_PASS_SCORE: f64 = 1.0;
        let certified = self.config.approval_policy == ApprovalPolicy::Ask
            && ctx.calibration.state == "active"
            && ctx
                .calibration
                .threshold
                .is_some_and(|theta| HARD_PASS_SCORE > theta);
        if certified {
            ctx.recorder.record_custom(
                "conformal_certified_accept",
                serde_json::json!({
                    "epoch_id": ctx.calibration.epoch_id,
                    "threshold": ctx.calibration.threshold,
                    "score": HARD_PASS_SCORE,
                    "node_id": ctx.node_id,
                }),
            )?;
            return Ok(true);
        }
        self.approve_promotion(
            ctx.recorder,
            ctx.node_id,
            ctx.candidate.touched_paths(),
            ctx.candidate.realized_diff().ok(),
        )
        .await
    }

    /// Certify and perform the run's one durable effect. Promotion passes
    /// through the same five-clause kernel as every candidate mutation: a
    /// `WriteArtifact` proposal over the realized mutated paths, evaluated
    /// with the contract and barrier on the realized witness. A kernel denial
    /// escalates instead of promoting (`None`).
    #[allow(clippy::too_many_arguments)]
    async fn promote_certified(
        &self,
        recorder: &Psp9Recorder,
        candidate: &CandidateWorkspace,
        node_id: &str,
        grant_policy: &perspt_sdk::GrantPolicy,
        capability: &Capability,
        contract: &CodingContract,
        barrier: &OperationalSafetyBarrier,
        kernel_state: &perspt_sdk::KernelState,
        calibration: &CalibrationBinding,
    ) -> Result<Option<Vec<String>>> {
        anyhow::ensure!(
            recorder.authority_epoch()? == grant_policy.authority_epoch,
            "authority epoch changed before promotion"
        );
        recorder.record_custom(
            "authority_epoch_rechecked",
            serde_json::json!({
                "node_id": node_id,
                "generation": 0,
                "epoch": grant_policy.authority_epoch,
            }),
        )?;
        let touched = candidate.touched_paths();
        let realized = candidate.checkpoint(&[]).await?.witness;
        let certified = certify_promotion(
            recorder,
            node_id,
            &touched,
            &realized,
            capability,
            contract,
            barrier,
            kernel_state,
        )?;
        if !certified {
            return Ok(None);
        }

        let promotion_key = format!("promote:{node_id}:0");
        let promotion_files = promotion_manifest(
            recorder,
            &self.working_dir,
            candidate.overlay_root(),
            &touched,
        )?;
        let promotion_intent = serde_json::json!({
            "idempotency_key": promotion_key,
            "node_id": node_id,
            "generation": 0,
            "authority_epoch": grant_policy.authority_epoch,
            "workspace_root": self.working_dir.canonicalize()?.display().to_string(),
            "candidate_root": realized.state_root,
            "files": promotion_files,
        });
        recorder.record_external_intent(&promotion_key, &promotion_intent)?;
        let promoted_paths = candidate.promote()?;
        recorder.complete_external_effect(
            &promotion_key,
            &serde_json::json!({"idempotency_key": promotion_key, "paths": promoted_paths}),
        )?;
        recorder.record_custom(
            "candidate_promoted",
            serde_json::json!({"node_id": node_id, "paths": promoted_paths}),
        )?;
        record_promotion_sample(recorder, calibration, &realized.state_root)?;
        Ok(Some(promoted_paths))
    }

    fn record_route_capabilities(&self, recorder: &Psp9Recorder) -> Result<()> {
        for (position, model) in std::iter::once(&self.model)
            .chain(self.fallback_models.iter())
            .enumerate()
        {
            let capabilities = self.transport.capabilities(model);
            let mut degradations = Vec::new();
            if !capabilities.strict_schema {
                degradations.push("strict_schema:local_validation");
            }
            if !capabilities.parallel_tool_calls {
                degradations.push("parallel_tool_calls:sequential_execution");
            }
            if !capabilities.streaming_tool_calls {
                degradations.push("streaming_tool_calls:turn_granular_progress");
            }
            if !capabilities.prompt_caching {
                degradations.push("prompt_caching:cache_cold_accounting");
            }
            if !capabilities.structured_output {
                degradations.push("structured_output:local_parse_and_validation");
            }
            recorder.record_custom(
                "provider_capability_evidence",
                serde_json::json!({
                    "model": model,
                    "route_position": position,
                    "source": "declared",
                    "capabilities": capabilities,
                    "degradations": degradations,
                }),
            )?;
        }
        Ok(())
    }

    /// The exact fingerprinted stratum this run calibrates under: model
    /// route, verifier suite, catalog, policy, and score definition.
    fn calibration_stratum(
        &self,
        catalog: &StaticCatalog,
        capability: &Capability,
        cadence: &perspt_sdk::VerificationCadence,
    ) -> Result<perspt_sdk::CalibrationStratum> {
        let verifier_suite_fingerprint =
            perspt_sdk::content_hash(&serde_json::to_vec(&serde_json::json!({
                "cadence": cadence,
                "plugins": perspt_core::PluginRegistry::new()
                    .detect_all(&self.working_dir)
                    .into_iter()
                    .map(|plugin| plugin.name().to_string())
                    .collect::<Vec<_>>(),
            }))?);
        let tool_catalog_fingerprint = perspt_sdk::content_hash(&serde_json::to_vec(
            &catalog.specs_for(std::slice::from_ref(capability), true),
        )?);
        Ok(perspt_sdk::CalibrationStratum {
            domain_package: "perspt-coding".into(),
            domain_version: env!("CARGO_PKG_VERSION").into(),
            effect_kind: "candidate_promotion".into(),
            risk_class: "workspace_mutation".into(),
            model_route: self.model.to_string(),
            verifier_suite_fingerprint,
            tool_catalog_fingerprint,
            policy_version: "coding-contract-v1".into(),
            score_definition: "hard-gate-plus-quadratic-energy-v1".into(),
        })
    }

    fn record_calibration_readiness(
        &self,
        recorder: &Psp9Recorder,
        catalog: &StaticCatalog,
        capability: &Capability,
        cadence: &perspt_sdk::VerificationCadence,
    ) -> Result<CalibrationBinding> {
        const TARGET_RHO: f64 = 0.05;
        let stratum = self.calibration_stratum(catalog, capability, cadence)?;
        let serialized = serde_json::to_string(&stratum)?;
        let epoch = match recorder.store.latest_psp9_calibration_epoch(&serialized)? {
            Some(epoch) => epoch,
            None => {
                let epoch = perspt_store::Psp9CalibrationEpochRow {
                    epoch_id: uuid::Uuid::new_v4().to_string(),
                    stratum: serialized,
                    target_rho: TARGET_RHO,
                    threshold: None,
                    state: "insufficient_samples".into(),
                    sample_count: 0,
                };
                recorder.store.record_psp9_calibration_epoch(&epoch)?;
                epoch
            }
        };
        let need = perspt_sdk::sample_floor(epoch.target_rho)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let certified = epoch.state == "active" && epoch.threshold.is_some();
        recorder.record_custom(
            "calibration_readiness",
            serde_json::json!({
                "epoch_id": epoch.epoch_id,
                "stratum": stratum,
                "state": epoch.state,
                "sample_count": epoch.sample_count,
                "sample_floor": need,
                "target_rho": epoch.target_rho,
                "threshold": epoch.threshold,
                "certified_for_promotion": certified,
                "reason": if certified {
                    "an activated epoch at the finite sample floor backs the \
                     conformal accepted-unsafe bound for this stratum"
                } else {
                    "coding promotion currently relies on deterministic contract, \
                     barrier, and verifier evidence; no probabilistic claim is used"
                },
            }),
        )?;
        Ok(CalibrationBinding {
            epoch_id: epoch.epoch_id,
            stratum: epoch.stratum,
            state: epoch.state,
            threshold: epoch.threshold,
        })
    }

    async fn explore(&self, recorder: &Psp9Recorder, task: &str) -> Result<String> {
        let root = self.working_dir.clone();
        let report = tokio::task::spawn_blocking(move || crate::exploration::map_workspace(&root))
            .await
            .context("repository exploration worker panicked")??;
        recorder.record_custom("exploration_report", serde_json::to_value(&report)?)?;
        self.emit(perspt_core::AgentEvent::Log(format!(
            "Exploration mapped {} language groups and {} package roots",
            report.project_map.languages.len(),
            report.project_map.package_roots.len()
        )));

        let mut advisory = None;
        if let Some(model) = &self.explorer_model {
            recorder.record_custom(
                "route_resolved",
                serde_json::json!({
                    "phase": "explore",
                    "model": model,
                    "reason": "configured speculator/explorer route",
                    "authority": "no_tools",
                }),
            )?;
            let mut conversation = Conversation::with_system(
                "Summarize the deterministic repository map for a coding worker. \
                 You have no tools and no authority. Do not claim facts absent \
                 from the map.",
            );
            conversation.push_user(format!(
                "Task: {task}\nRepository map:\n{}",
                serde_json::to_string_pretty(&report.project_map)?
            ));
            match self
                .transport
                .chat_turn(model, &conversation, &[], ToolChoicePolicy::None)
                .await
            {
                Ok(output) => {
                    recorder.record_custom(
                        "exploration_model_observation",
                        serde_json::json!({"model": model, "output": output}),
                    )?;
                    if let TurnOutput::Text(text) = output {
                        advisory = Some(text);
                    }
                }
                Err(error) => {
                    recorder.record_custom(
                        "exploration_model_unavailable",
                        serde_json::json!({"model": model, "error": error.to_string()}),
                    )?;
                }
            }
        }

        Ok(format!(
            "{task}\n\nDeterministic repository map (advisory orientation, not acceptance evidence):\n{}{}",
            serde_json::to_string_pretty(&report.project_map)?,
            advisory
                .map(|summary| format!("\n\nUntrusted explorer summary:\n{summary}"))
                .unwrap_or_default()
        ))
    }

    async fn adjudicate_candidate(
        &self,
        recorder: &Psp9Recorder,
        candidate: &CandidateWorkspace,
        task: &str,
        stratum: &str,
    ) -> Result<bool> {
        let Some(model) = &self.adjudicator_model else {
            return Ok(true);
        };
        let diff = candidate.realized_diff()?;
        let diff_handle = recorder.record_artifact(diff.as_bytes(), "text/x-diff")?;
        let mut boundary = diff.len().min(100_000);
        while !diff.is_char_boundary(boundary) {
            boundary -= 1;
        }
        let mut conversation = Conversation::with_system(
            "You are a conjunctive coding validator with no tools or authority. \
             Review only the realized diff. Return strict JSON: \
             {\"pass\":bool,\"reason\":string}. Reject uncertainty; do not \
             propose edits.",
        );
        conversation.push_user(format!(
            "Task: {task}\nDiff artifact: {diff_handle}\nRealized diff:\n{}",
            &diff[..boundary]
        ));
        recorder.record_custom(
            "adjudication_requested",
            serde_json::json!({"model": model, "diff_artifact": diff_handle}),
        )?;
        let output = self
            .transport
            .chat_turn(model, &conversation, &[], ToolChoicePolicy::None)
            .await?;
        let TurnOutput::Text(text) = output else {
            anyhow::bail!("adjudicator returned tool calls despite having no tools");
        };
        #[derive(serde::Deserialize)]
        struct Verdict {
            pass: bool,
            reason: String,
        }
        let verdict: Verdict = serde_json::from_str(text.trim())
            .context("adjudicator did not return strict verdict JSON")?;
        let evidence_hash = recorder.record_artifact(text.as_bytes(), "application/json")?;
        let candidate_id = candidate.checkpoint(&[]).await?.witness.state_root;
        // The verdict shares the epoch's serialized stratum so verdicts and
        // calibration samples can be joined during delayed-label ingestion.
        recorder.store.record_psp9_verdict(&Psp9VerdictRow {
            session_id: recorder.session_id.clone(),
            candidate_id,
            validator_id: model.to_string(),
            stratum: stratum.to_string(),
            missed: !verdict.pass,
            unsafe_label: None,
            evidence_hash,
        })?;
        recorder.record_custom(
            "adjudication_verdict",
            serde_json::json!({
                "model": model,
                "pass": verdict.pass,
                "reason": verdict.reason,
                "certified_risk": null,
            }),
        )?;
        Ok(verdict.pass)
    }

    async fn approve_promotion(
        &mut self,
        recorder: &Psp9Recorder,
        node_id: &str,
        paths: Vec<String>,
        diff: Option<String>,
    ) -> Result<bool> {
        match self.config.approval_policy {
            ApprovalPolicy::Auto | ApprovalPolicy::SessionApproved => return Ok(true),
            ApprovalPolicy::Deny => return Ok(false),
            ApprovalPolicy::Ask => {}
        }
        let request_id = uuid::Uuid::new_v4().to_string();
        recorder.record_custom(
            "approval_requested",
            serde_json::json!({"request_id": request_id, "node_id": node_id, "paths": paths}),
        )?;
        self.emit(perspt_core::AgentEvent::ApprovalRequest {
            request_id: request_id.clone(),
            node_id: node_id.into(),
            action_type: perspt_core::ActionType::BundleWrite {
                node_id: node_id.into(),
                files: paths,
            },
            description: "Promote the verified PSP-9 candidate to the workspace".into(),
            diff,
        });
        let Some(receiver) = self.action_receiver.as_mut() else {
            recorder.record_custom(
                "approval_unavailable",
                serde_json::json!({"request_id": request_id}),
            )?;
            return Ok(false);
        };
        while let Some(action) = receiver.recv().await {
            let approved = match action {
                perspt_core::AgentAction::Approve { request_id: id }
                | perspt_core::AgentAction::ApproveWithEdit { request_id: id, .. }
                    if id == request_id =>
                {
                    Some(true)
                }
                perspt_core::AgentAction::Reject { request_id: id, .. } if id == request_id => {
                    Some(false)
                }
                perspt_core::AgentAction::Abort => Some(false),
                _ => None,
            };
            if let Some(approved) = approved {
                recorder.record_custom(
                    "approval_resolved",
                    serde_json::json!({"request_id": request_id, "approved": approved}),
                )?;
                return Ok(approved);
            }
        }
        Ok(false)
    }
}
