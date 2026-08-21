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

use crate::candidate::CandidateWorkspace;
use crate::toolloop::{EffectExecutor, LoopBudgets, LoopEvent, LoopRecorder};
use crate::transport::GenAiTransport;

mod adjudicate;
mod attempt;
mod dispatch;
mod explore;
mod external;
mod integrate;
mod node;
mod plan;
mod prompt_bind;
mod recorder;
mod resume;
mod search;
mod settings;

use external::{external_runtime_from, registry_with_external};
use node::*;
pub use recorder::Psp9Recorder;
pub use settings::{Psp9ModelRoutes, Psp9RunConfig, Psp9RunSummary};

/// The production runtime for one coding task.
pub struct Psp9AgentRuntime {
    working_dir: PathBuf,
    transport: Arc<dyn ModelTransport>,
    model: ModelId,
    fallback_models: Vec<ModelId>,
    explorer_model: Option<ModelId>,
    adjudicator_model: Option<ModelId>,
    /// Higher-capability route for the recovery ladder's level-3 handoff.
    handoff_model: Option<ModelId>,
    config: Psp9RunConfig,
    event_sender: Option<perspt_core::events::channel::EventSender>,
    action_receiver: tokio::sync::Mutex<Option<perspt_core::events::channel::ActionReceiver>>,
    database_path: Option<PathBuf>,
    shared_store: Option<Arc<SessionStore>>,
    /// The open execution plane shared by every candidate this runtime
    /// creates. The composition root may extend it with registered families.
    tool_handlers: Arc<crate::tools::handlers::CandidateHandlerRegistry>,
    /// Catalog entries for registered tool families, appended to the
    /// domain's entries at node assembly.
    extra_tool_entries: Vec<perspt_sdk::ToolEntry>,
    /// The selected domain package. Defaults to coding; the composition
    /// root selects from a `DomainRegistry` (explicit `--domain` or
    /// detection).
    domain: Arc<dyn AgentDomainPackage>,
    /// Optional MCP edge adapter (system 13). Servers come from
    /// `[[external_tools]]`; nothing changes when none are configured.
    external: Option<Arc<tokio::sync::Mutex<crate::external_tools::ExternalToolRuntime>>>,
    /// Admitted external entries, discovered once per session.
    external_entries: Mutex<Option<Vec<perspt_sdk::ToolEntry>>>,
    /// Bounded-search settings from `[exploration]` (PSP-10 system 20).
    search: search::SearchSettings,
    /// Prior search state folded from the ledger at resume. The folded
    /// no-goods seed every forest in the session; the interrupted
    /// forest's consumption is claimed once, by its exact owner.
    search_seed: Mutex<Option<search::SearchSeed>>,
    /// Experimental platform-section overrides from validated
    /// `[prompts].bundles`, loaded by the composition root only under
    /// `--allow-experimental-prompts` (PSP-10 system 25, Gate AE).
    prompt_overrides: perspt_core::prompts::SectionOverrides,
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
            .unwrap_or_else(|| perspt_core::ModelTier::default_model_name());
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
        let (explorer_model, adjudicator_model, handoff_model) =
            resolve_role_routes(&routes, config, &transport)?;
        let fallback_models = resolve_fallbacks(&routes.fallbacks, &model, config, &transport)?;
        let external = external_runtime_from(config)?;
        let handlers = registry_with_external(&external);
        let run_config = settings::apply_config_overrides(run_config, config);
        Ok(Self {
            working_dir,
            transport,
            model,
            fallback_models,
            explorer_model,
            adjudicator_model,
            handoff_model,
            config: run_config,
            event_sender: None,
            action_receiver: tokio::sync::Mutex::new(None),
            database_path: None,
            shared_store: None,
            tool_handlers: Arc::new(handlers),
            extra_tool_entries: Vec::new(),
            domain: Arc::new(CodingDomain::new()),
            external,
            external_entries: Mutex::new(None),
            search: search::SearchSettings::from_config(config.exploration.as_ref()),
            search_seed: Mutex::new(None),
            prompt_overrides: Default::default(),
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
            handoff_model: None,
            config: run_config,
            event_sender: None,
            action_receiver: tokio::sync::Mutex::new(None),
            database_path: None,
            shared_store: None,
            tool_handlers: Arc::new(
                crate::tools::handlers::CandidateHandlerRegistry::with_builtins(),
            ),
            extra_tool_entries: Vec::new(),
            domain: Arc::new(CodingDomain::new()),
            external: None,
            external_entries: Mutex::new(None),
            search: search::SearchSettings::default(),
            search_seed: Mutex::new(None),
            prompt_overrides: Default::default(),
        }
    }

    /// Extend or replace the execution plane with a registry assembled at
    /// the composition root (registered tool families beyond the builtins).
    /// A configured MCP dispatcher is preserved as the fallback.
    pub fn with_tool_handlers(
        mut self,
        mut handlers: crate::tools::handlers::CandidateHandlerRegistry,
    ) -> Self {
        if let (Some(external), false) = (&self.external, handlers.has_fallback()) {
            handlers.set_fallback(Arc::new(
                crate::tools::handlers::external::ExternalDispatcher::new(external.clone()),
            ));
        }
        self.tool_handlers = Arc::new(handlers);
        self
    }

    /// Add a registered tool family's catalog entries. Pair with
    /// [`Self::with_tool_handlers`]; the entries enter every node's
    /// assembled catalog and therefore its derived grant.
    pub fn with_tool_family(mut self, entries: Vec<perspt_sdk::ToolEntry>) -> Self {
        self.extra_tool_entries.extend(entries);
        self
    }

    /// Effects the user explicitly opted into beyond the default grant.
    fn opted_in_effects(&self) -> Vec<EffectKind> {
        if self.config.allow_dependency_mutation {
            vec![EffectKind::MutateDependencies]
        } else {
            Vec::new()
        }
    }

    /// Select the domain package (from a `DomainRegistry` at the
    /// composition root). Defaults to the coding domain.
    pub fn with_domain(mut self, domain: Arc<dyn AgentDomainPackage>) -> Self {
        self.domain = domain;
        self
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

    /// Install experimental platform-section overrides (validated bundle
    /// replacements). The composition root gates this behind
    /// `--allow-experimental-prompts`; active overrides are ledgered at
    /// session start.
    pub fn with_prompt_overrides(
        mut self,
        overrides: perspt_core::prompts::SectionOverrides,
    ) -> Self {
        self.prompt_overrides = overrides;
        self
    }

    /// Override the forest's Definition 5 limit vector (tests, embedders).
    pub fn with_search_limits(mut self, limits: perspt_sdk::SearchLimits) -> Self {
        self.search.limits = limits;
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
        self.action_receiver = tokio::sync::Mutex::new(Some(receiver));
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
                "test_policy": self.config.test_policy.as_str(),
            }),
        )?;
        self.record_route_capabilities(&recorder)?;
        self.record_prompt_overrides(&recorder)?;

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

        let graph = self.plan_initial_graph(&recorder, node_id, task).await?;
        recorder.record_custom("graph_revision", serde_json::to_value(&graph)?)?;
        if self.config.max_parallel_nodes > 1 {
            // The dispatcher owns scheduling; this scheduler is unused.
            return Ok((recorder, agent_goal, Scheduler::new(1), graph));
        }
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

    /// Gate AE: experimental section overrides are ledgered at session
    /// start, never silent.
    fn record_prompt_overrides(&self, recorder: &Psp9Recorder) -> Result<()> {
        if self.prompt_overrides.is_empty() {
            return Ok(());
        }
        recorder.record_custom(
            "prompt_overrides_active",
            serde_json::json!({
                "experimental": true,
                "sections": self
                    .prompt_overrides
                    .provenance()
                    .iter()
                    .map(|(id, hash)| serde_json::json!({"id": id, "content_hash": hash}))
                    .collect::<Vec<_>>(),
            }),
        )
    }

    /// Mint (and for persistent grants, sign and verify) the session grant.
    fn mint_grant(
        &self,
        recorder: &Psp9Recorder,
        revision_id: &str,
        catalog: &dyn ToolCatalog,
    ) -> Result<perspt_sdk::GrantPolicy> {
        let grant_policy = session_grant_policy(
            &self.working_dir,
            revision_id,
            self.config.persistent_grants,
            catalog,
            &self.opted_in_effects(),
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

    fn open_candidate(
        &self,
        node_id: &str,
        generation: u32,
        revision_id: &str,
    ) -> Result<CandidateWorkspace> {
        let mut candidate = CandidateWorkspace::create_with_policy(
            &self.working_dir,
            node_id,
            generation,
            revision_id,
            self.config.allow_unisolated_verifiers,
        )?;
        candidate.set_tool_handlers(self.tool_handlers.clone());
        candidate.set_verifier_timeouts(self.config.verifier_timeouts);
        Ok(candidate)
    }

    /// Recovery ladder (Theorem 6): the loop already consumed its retry and
    /// fallback levels; the runtime holds the higher rungs. Level 2 refines
    /// the work graph with the attempt's evidence and re-runs at a new
    /// generation; level 3 hands the node to the configured higher-capability
    /// route; level 4 revokes the session's authority epoch after
    /// restore-best containment.
    async fn recovery_ladder(
        &self,
        session: LadderSession<'_>,
        initial_budget: u32,
        mut graph: WorkGraphRevision,
        mut goal: String,
        mut attempt: NodeAttempt,
    ) -> Result<(NodeAttempt, WorkGraphRevision, u32)> {
        let LadderSession {
            recorder,
            session_id,
            node_id,
            scheduler,
        } = session;
        // The ladder continues the node's recorded generation (a node
        // dispatched at generation >= 1 after a graph resume must refine to
        // generation + 1, or `WorkGraphRevision::revise` rejects it stale).
        let mut generation = graph.node(node_id).map_or(0, |node| node.generation);
        let mut model = self.model.clone();
        let mut remaining_budget = initial_budget.saturating_sub(spent_of(&attempt));
        for level in [
            perspt_sdk::CascadeLevel::Refine,
            perspt_sdk::CascadeLevel::Escalate,
        ] {
            if !matches!(
                attempt.outcome.outcome,
                NodeTerminalOutcome::Escalated { .. }
            ) {
                break;
            }
            match level {
                perspt_sdk::CascadeLevel::Refine => {
                    let refined = refine_rung(
                        recorder, scheduler, &graph, node_id, generation, &goal, &attempt,
                    )?;
                    let Some((revised, refined_goal)) = refined else {
                        continue;
                    };
                    graph = revised;
                    generation += 1;
                    goal = refined_goal;
                }
                perspt_sdk::CascadeLevel::Escalate => {
                    let Some(handoff) = self.escalation_handoff(recorder, &model)? else {
                        continue;
                    };
                    model = handoff;
                }
                _ => unreachable!("ladder iterates refine and escalate only"),
            }
            let next = self
                .rung_forest(
                    recorder,
                    session_id,
                    &goal,
                    node_id,
                    generation,
                    &model,
                    &graph,
                    &attempt,
                    remaining_budget,
                )
                .await?;
            let spent = spent_of(&next);
            attempt = next;
            remaining_budget = remaining_budget.saturating_sub(spent);
        }
        contain_if_escalated(recorder, session_id, &attempt)?;
        Ok((attempt, graph, generation))
    }

    /// One rung's re-attempt: restore-best, then open a bounded search
    /// forest (PSP-10 system 20) — isolated branches against the same
    /// accepted root, one candidate committed through the ordinary gate
    /// by the deterministic rule.
    #[allow(clippy::too_many_arguments)]
    async fn rung_forest(
        &self,
        recorder: &Psp9Recorder,
        session_id: &str,
        goal: &str,
        node_id: &str,
        generation: u32,
        model: &ModelId,
        graph: &WorkGraphRevision,
        attempt: &NodeAttempt,
        remaining_budget: u32,
    ) -> Result<NodeAttempt> {
        let seed = seed_from_attempt(recorder, node_id, attempt).await?;
        let baseline_energy = attempt.outcome.trajectory.best_accepted_energy;
        let rho_gate = attempt.assembly.energy.rho_gate;
        self.run_search_forest(
            recorder,
            session_id,
            goal,
            node_id,
            generation,
            model,
            graph,
            seed.as_ref(),
            remaining_budget,
            baseline_energy,
            rho_gate,
        )
        .await
    }

    /// Multi-node session: dispatch the planned graph, then close the
    /// session with the aggregate outcome.
    async fn run_graph_session(
        &self,
        recorder: &Psp9Recorder,
        session_id: &str,
        graph: WorkGraphRevision,
    ) -> Result<Psp9RunSummary> {
        let dispatched = match self.run_dispatched(recorder, session_id, graph).await {
            Ok(dispatched) => dispatched,
            Err(error) => {
                recorder.finish("FAILED_PSP9").ok();
                return Err(error);
            }
        };
        recorder.finish(dispatched.status)?;
        self.emit(perspt_core::AgentEvent::Complete {
            success: matches!(dispatched.outcome, NodeTerminalOutcome::HardPass),
            message: format!("PSP-9 outcome: {:?}", dispatched.outcome),
        });
        Ok(Psp9RunSummary {
            session_id: session_id.to_string(),
            node_id: "graph".into(),
            outcome: dispatched.outcome,
            turns_used: dispatched.turns_used,
            ledger_head: recorder.head(),
            promoted_paths: dispatched.promoted_paths,
        })
    }

    /// Assemble the conclusion context for a finished attempt and decide the
    /// node's terminal fate.
    async fn conclude_attempt(
        &self,
        recorder: &Psp9Recorder,
        attempt: &NodeAttempt,
        node_id: &str,
        task: &str,
    ) -> Result<(NodeTerminalOutcome, &'static str, Vec<String>)> {
        self.conclude_run(ConcludeContext {
            recorder,
            candidate: &attempt.candidate,
            node_id,
            task,
            grant_policy: &attempt.assembly.grant_policy,
            capability: &attempt.assembly.capability,
            contract: &attempt.assembly.contract,
            barrier: &attempt.assembly.barrier,
            kernel_state: &attempt.kernel_state,
            loop_outcome: &attempt.outcome.outcome,
            calibration: &attempt.assembly.calibration,
        })
        .await
    }

    /// Adjudicate and approve one attempt without promoting it — the
    /// staging path's validation half (PSP-10 system 22).
    async fn validate_and_approve_attempt(
        &self,
        recorder: &Psp9Recorder,
        attempt: &NodeAttempt,
        node_id: &str,
        task: &str,
    ) -> Result<bool> {
        let ctx = ConcludeContext {
            recorder,
            candidate: &attempt.candidate,
            node_id,
            task,
            grant_policy: &attempt.assembly.grant_policy,
            capability: &attempt.assembly.capability,
            contract: &attempt.assembly.contract,
            barrier: &attempt.assembly.barrier,
            kernel_state: &attempt.kernel_state,
            loop_outcome: &attempt.outcome.outcome,
            calibration: &attempt.assembly.calibration,
        };
        self.validate_and_approve(&ctx, true).await
    }

    /// Level-3 handoff: returns the higher-capability route when one is
    /// configured and distinct from the current model.
    fn escalation_handoff(
        &self,
        recorder: &Psp9Recorder,
        current: &perspt_sdk::ModelId,
    ) -> Result<Option<perspt_sdk::ModelId>> {
        let Some(handoff) = self.handoff_model.clone() else {
            return Ok(None);
        };
        if handoff == *current {
            return Ok(None);
        }
        recorder.record_custom(
            "recovery_handoff",
            serde_json::json!({
                "level": "escalate",
                "from_model": current,
                "to_model": handoff,
            }),
        )?;
        Ok(Some(handoff))
    }

    pub async fn run(mut self, task: String) -> Result<Psp9RunSummary> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let node_id = "implement-1".to_string();
        let (recorder, agent_goal, mut scheduler, running_graph) =
            self.open_session(&task, &session_id, &node_id).await?;

        if self.config.max_parallel_nodes > 1 {
            return self
                .run_graph_session(&recorder, &session_id, running_graph)
                .await;
        }

        let first = match self
            .attempt_node(
                &recorder,
                &session_id,
                &agent_goal,
                &node_id,
                0,
                &self.model.clone(),
                &running_graph,
                None,
                self.config.rejection_budget,
            )
            .await
        {
            Ok(attempt) => attempt,
            Err(error) => return Err(self.fail_session(&recorder, error)),
        };
        let (attempt, graph, final_generation) = match self
            .recovery_ladder(
                LadderSession {
                    recorder: &recorder,
                    session_id: &session_id,
                    node_id: &node_id,
                    scheduler: &mut scheduler,
                },
                self.config.rejection_budget,
                running_graph,
                agent_goal,
                first,
            )
            .await
        {
            Ok(result) => result,
            Err(error) => return Err(self.fail_session(&recorder, error)),
        };

        let verdict = match self
            .conclude_attempt(&recorder, &attempt, &node_id, &task)
            .await
        {
            Ok(verdict) => verdict,
            Err(error) => return Err(self.fail_session(&recorder, error)),
        };
        let (final_outcome, status, promoted_paths) = verdict;
        scheduler.finish(&node_id, final_generation);
        if let Err(error) = self.finish_node(&recorder, &graph, &node_id, &final_outcome, status) {
            return Err(self.fail_session(&recorder, error));
        }

        Ok(Psp9RunSummary {
            session_id,
            node_id,
            outcome: final_outcome,
            turns_used: attempt.outcome.turns_used,
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

    fn loop_budgets(&self, domain_rho_gate: f64, shared_recovery_budget: u32) -> LoopBudgets {
        LoopBudgets {
            max_turns: self.config.max_turns,
            max_calls_per_turn: self.config.max_calls_per_turn,
            rejection_budget: shared_recovery_budget,
            rho_gate: self.config.rho_gate.max(domain_rho_gate),
            declared_energy_floor: None,
            context_soft_limit_chars: 240_000,
            recovery_budget: shared_recovery_budget,
            turn_deadline_secs: self.config.turn_deadline_secs,
            resident: crate::toolloop::ResidentReserves {
                paging_enabled: self.config.resident.paging_enabled
                    && !self.config.ablate_context_paging,
                ..self.config.resident
            },
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
    #[allow(clippy::too_many_arguments)]
    fn assemble_node(
        &self,
        recorder: &Psp9Recorder,
        session_id: &str,
        running_graph: &WorkGraphRevision,
        node_id: &str,
        generation: u32,
        external_entries: &[perspt_sdk::ToolEntry],
    ) -> Result<NodeAssembly> {
        let domain = self.domain.clone();
        let scope = perspt_sdk::DomainScope {
            label: node_id.to_string(),
            paths: Vec::new(),
        };
        let catalog = self.assemble_catalog(node_id, external_entries)?;
        let grant_policy = self.mint_grant(recorder, &running_graph.revision_id, &catalog)?;
        // Every live capability is the intersection of the worker template
        // with the grant ceilings — the ceilings are enforced, not decorative.
        let mut capability = grant_policy
            .mint(worker_capability(
                session_id,
                &running_graph.revision_id,
                grant_policy.authority_epoch,
                generation,
                &catalog,
                &self.opted_in_effects(),
            ))
            .map_err(|e| anyhow::anyhow!("grant intersection: {e}"))?;
        // PSP-10 system 22: a node's declared output_targets become its
        // enforced write ceiling — mutating proposals outside the declared
        // footprint are denied while reads stay workspace-wide. Undeclared
        // targets keep unrestricted writes (the opaque scheduler footprint
        // still serializes such nodes).
        if let Some(node) = running_graph.node(node_id) {
            capability.write_scope = write_scope_from_targets(&node.output_targets);
        }
        let contract = CodingContract {
            graph_revision: running_graph.revision_id.clone(),
            node_id: node_id.to_string(),
            generation,
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
        &self,
        ctx: ConcludeContext<'_>,
    ) -> Result<(NodeTerminalOutcome, &'static str, Vec<String>)> {
        let hard_pass = matches!(ctx.loop_outcome, NodeTerminalOutcome::HardPass);
        let promotion_approved = self.validate_and_approve(&ctx, hard_pass).await?;
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

    /// Record the deterministic-suite verdict (system 8's second live
    /// validator, joined to the adjudicator's row by candidate id and
    /// stratum), then run adjudication and approval for a hard pass.
    async fn validate_and_approve(
        &self,
        ctx: &ConcludeContext<'_>,
        hard_pass: bool,
    ) -> Result<bool> {
        let candidate_id = ctx.candidate.checkpoint(&[]).await?.witness.state_root;
        record_deterministic_verdict(
            ctx.recorder,
            &candidate_id,
            &ctx.calibration.stratum,
            hard_pass,
        )?;
        if !hard_pass {
            return Ok(false);
        }
        let adjudicated = self
            .adjudicate_candidate(
                ctx.recorder,
                ctx.candidate,
                ctx.task,
                &ctx.calibration.stratum,
            )
            .await?;
        Ok(adjudicated && self.conformal_or_human_approval(ctx).await?)
    }

    /// Autonomous commitment (PSP-9 Gate Q): with an *activated* calibration
    /// epoch for this exact stratum — reached only at the finite sample floor
    /// through delayed audit labels — a hard-pass candidate above the
    /// conformal threshold commits without a human prompt, and the certified
    /// accept is ledgered. Any other state (shadow, insufficient samples,
    /// stale) backs off to the configured approval policy.
    async fn conformal_or_human_approval(&self, ctx: &ConcludeContext<'_>) -> Result<bool> {
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
                "generation": capability.node_generation,
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

        let promoted_paths = commit_promotion(
            recorder,
            candidate,
            node_id,
            grant_policy.authority_epoch,
            &realized.state_root,
            &touched,
            &self.working_dir,
        )?;
        record_promotion_sample(recorder, calibration, &realized.state_root)?;
        Ok(Some(promoted_paths))
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

    async fn approve_promotion(
        &self,
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
        let mut receiver_slot = self.action_receiver.lock().await;
        let Some(receiver) = receiver_slot.as_mut() else {
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
