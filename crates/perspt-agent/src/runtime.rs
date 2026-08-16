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

/// Durable sink for all live PSP-9 events.
pub struct Psp9Recorder {
    session_id: String,
    store: SessionStore,
    ledger: Mutex<Ledger>,
    event_sender: Option<perspt_core::events::channel::EventSender>,
}

impl std::fmt::Debug for Psp9Recorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Psp9Recorder")
            .field("session_id", &self.session_id)
            .field("head", &self.head())
            .finish_non_exhaustive()
    }
}

impl Psp9Recorder {
    fn create(
        session_id: &str,
        task: &str,
        working_dir: &Path,
        database_path: Option<&Path>,
        event_sender: Option<perspt_core::events::channel::EventSender>,
    ) -> Result<Self> {
        let store = match database_path {
            Some(path) => SessionStore::open(&path.to_path_buf())?,
            None => SessionStore::new()?,
        };
        store.create_session(&SessionRecord {
            session_id: session_id.into(),
            task: task.into(),
            working_dir: working_dir.display().to_string(),
            merkle_root: None,
            detected_toolchain: None,
            status: "RUNNING_PSP9".into(),
        })?;
        store.initialize_authority_epoch(session_id, 0)?;
        Ok(Self {
            session_id: session_id.into(),
            store,
            ledger: Mutex::new(Ledger::new()),
            event_sender,
        })
    }

    pub fn record_custom(&self, kind: &str, payload: serde_json::Value) -> Result<()> {
        self.append(LedgerEvent::Custom {
            kind: kind.into(),
            payload,
        })
    }

    fn append(&self, event: LedgerEvent) -> Result<()> {
        let mut guard = self.ledger.lock().unwrap();
        let mut candidate = guard.clone();
        candidate
            .append(event)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let record = candidate
            .records()
            .last()
            .context("missing appended record")?;
        self.store.record_psp9_event(&Psp9LedgerRow {
            session_id: self.session_id.clone(),
            sequence: record.sequence as i64,
            event_json: serde_json::to_string(&record.event)?,
            prev_hash: record.prev_hash.clone(),
            hash: record.hash.clone(),
        })?;
        *guard = candidate;
        Ok(())
    }

    pub fn head(&self) -> String {
        self.ledger.lock().unwrap().head()
    }

    fn finish(&self, status: &str) -> Result<()> {
        self.store.update_session_status(&self.session_id, status)
    }

    fn authority_epoch(&self) -> Result<u64> {
        self.store.authority_epoch(&self.session_id)
    }

    fn record_grant_policy(
        &self,
        policy: &GrantPolicy,
        signed: Option<&perspt_sdk::SignedGrantPolicy>,
    ) -> Result<()> {
        let durable = signed
            .map(serde_json::to_string)
            .transpose()?
            .unwrap_or_else(|| serde_json::to_string(policy).expect("serializable grant policy"));
        self.store
            .record_grant_policy(&self.session_id, &policy.policy_id, &durable)?;
        self.record_custom(
            "grant_policy",
            serde_json::json!({
                "policy": policy,
                "signed": signed.is_some(),
            }),
        )
    }

    fn record_external_intent(&self, key: &str, intent: &serde_json::Value) -> Result<()> {
        let bytes = serde_json::to_vec(intent)?;
        self.store.record_external_effect_intent(
            &self.session_id,
            key,
            &perspt_sdk::ledger::content_hash(&bytes),
            &String::from_utf8(bytes)?,
        )?;
        self.record_custom("external_effect_intent", intent.clone())
    }

    fn complete_external_effect(&self, key: &str, result: &serde_json::Value) -> Result<()> {
        self.store.complete_external_effect(
            &self.session_id,
            key,
            &serde_json::to_string(result)?,
        )?;
        self.record_custom("external_effect_completed", result.clone())
    }
}

impl LoopRecorder for Psp9Recorder {
    fn record(&self, event: &LoopEvent) -> Result<()> {
        self.record_custom("tool_loop", serde_json::to_value(event)?)?;
        if let LoopEvent::ContextCheckpointCreated { checkpoint } = event {
            self.store.record_psp9_checkpoint(
                &self.session_id,
                &checkpoint.covered_event_root,
                &serde_json::to_string(checkpoint)?,
            )?;
        }
        if let Some(sender) = &self.event_sender {
            let message = match event {
                LoopEvent::CandidateMeasured { node_id, generation, energy, hard_pass, residuals } =>
                    Some(format!(
                        "Measured {node_id} generation {generation}: V={energy:.3}, hard_pass={hard_pass}, residuals={}",
                        residuals.len()
                    )),
                LoopEvent::GateDecisionRecorded { node_id, generation, decision } => Some(format!(
                    "Gate {node_id} generation {generation}: {decision:?}"
                )),
                LoopEvent::EffectDenied { call_id, reason } =>
                    Some(format!("Effect {call_id} denied: {reason}")),
                LoopEvent::EffectApplied { call_id, mutated, .. } => Some(format!(
                    "Effect {call_id} applied to candidate (mutated={mutated})"
                )),
                LoopEvent::RouteFailover { from_model, to_model, cause } => Some(format!(
                    "Route failover {from_model} -> {to_model}: {cause}"
                )),
                LoopEvent::ContextCheckpointCreated { checkpoint } => Some(format!(
                    "Context checkpoint {} covers events {}..{}",
                    &checkpoint.covered_event_root[..12.min(checkpoint.covered_event_root.len())],
                    checkpoint.covered_from,
                    checkpoint.covered_to
                )),
                LoopEvent::RecoveryContained { reason, .. } =>
                    Some(format!("Recovery contained the node: {reason}")),
                _ => None,
            };
            if let Some(message) = message {
                let _ = sender.send(perspt_core::AgentEvent::Log(message));
            }
        }
        Ok(())
    }

    fn record_artifact(&self, content: &[u8], media_type: &str) -> Result<String> {
        let handle = perspt_sdk::ledger::content_hash(content);
        self.store.put_psp9_artifact(&handle, content, media_type)?;
        Ok(handle)
    }
}

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
            "route {model} does not declare native tool calling; PSP-9 tool-loop mode never emulates tool calls in text"
        );
        let explorer_model = routes
            .explorer
            .as_deref()
            .or_else(|| {
                config
                    .models
                    .as_ref()
                    .and_then(|models| models.speculator.as_deref())
            })
            .map(|value| qualify_model(value, config, transport.portfolio()))
            .transpose()?;
        if let Some(model) = &explorer_model {
            transport.portfolio().resolve(&model.provider)?;
        }
        let adjudicator_model = routes
            .adjudicator
            .as_deref()
            .or_else(|| {
                config
                    .models
                    .as_ref()
                    .and_then(|models| models.adjudicator.as_deref())
            })
            .map(|value| qualify_model(value, config, transport.portfolio()))
            .transpose()?;
        if let Some(model) = &adjudicator_model {
            transport.portfolio().resolve(&model.provider)?;
        }
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
        }
    }

    pub fn with_database_path(mut self, path: PathBuf) -> Self {
        self.database_path = Some(path);
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

    pub async fn run(mut self, task: String) -> Result<Psp9RunSummary> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let node_id = "implement-1".to_string();
        let recorder = Psp9Recorder::create(
            &session_id,
            &task,
            &self.working_dir,
            self.database_path.as_deref(),
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

        let agent_goal = self.explore(&recorder, &task).await?;

        self.emit(perspt_core::AgentEvent::Log(format!(
            "PSP-9 session {} using {}",
            &session_id[..8],
            self.model
        )));
        self.emit(perspt_core::AgentEvent::TaskStatusChanged {
            node_id: node_id.clone(),
            status: perspt_core::NodeStatus::Coding,
        });

        let graph = initial_graph(&node_id, &task)?;
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
        let running_graph = execution_revision(&graph, &node_id, WorkNodeState::Running)?;
        recorder.record_custom("graph_revision", serde_json::to_value(&running_graph)?)?;
        let candidate = CandidateWorkspace::create_with_policy(
            &self.working_dir,
            &node_id,
            0,
            &running_graph.revision_id,
            self.config.allow_unisolated_verifiers,
        )?;
        let measurer = CodingCandidateMeasurer::new(&candidate, &node_id, 0)
            .with_max_parallel(self.config.max_parallel_verifiers);
        let domain = CodingDomain::new();
        let scope = perspt_sdk::DomainScope {
            label: node_id.clone(),
            paths: Vec::new(),
        };
        let catalog = StaticCatalog::with_base(domain.tool_entries(&scope))
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let grant_policy = session_grant_policy(
            &self.working_dir,
            &running_graph.revision_id,
            self.config.persistent_grants,
        )?;
        let signed_grant = if self.config.persistent_grants {
            let key = crate::grant::GrantSigningKey::resolve()?;
            let signed = perspt_sdk::SignedGrantPolicy::sign(grant_policy.clone(), &key.bytes)
                .map_err(anyhow::Error::msg)?;
            signed.verify().map_err(anyhow::Error::msg)?;
            recorder.record_custom(
                "grant_signing_key_resolved",
                serde_json::json!({"source": format!("{:?}", key.source)}),
            )?;
            Some(signed)
        } else {
            None
        };
        recorder.record_grant_policy(&grant_policy, signed_grant.as_ref())?;
        let capability = worker_capability(
            &session_id,
            &running_graph.revision_id,
            grant_policy.authority_epoch,
        );
        let contract = CodingContract {
            graph_revision: running_graph.revision_id.clone(),
            node_id: node_id.clone(),
            generation: 0,
            policy: perspt_policy::engine::PolicyEngine::new()?,
        };
        let barrier = OperationalSafetyBarrier::default();
        let cadence = domain.verifier_suite(&scope).cadence;
        let energy = domain.energy_model(&scope);
        self.record_calibration_readiness(&recorder, &catalog, &capability, &cadence)?;

        let mut kernel_state = perspt_sdk::KernelState::new();
        kernel_state.set_witness(
            "__authority_epoch",
            grant_policy.authority_epoch.to_string(),
        );
        kernel_state.set_witness("__graph_revision", running_graph.revision_id.clone());
        let tool_loop = ToolLoop {
            transport: self.transport.as_ref(),
            model: self.model.clone(),
            fallback_models: self.fallback_models.clone(),
            catalog: &catalog,
            capabilities: vec![capability],
            contract: Some(&contract),
            barrier: Some(&barrier),
            c_c_max: 0.25,
            executor: &candidate,
            measurer: &measurer,
            budgets: LoopBudgets {
                max_turns: self.config.max_turns,
                max_calls_per_turn: self.config.max_calls_per_turn,
                rejection_budget: self.config.rejection_budget,
                rho_gate: self.config.rho_gate.max(energy.rho_gate),
                declared_energy_floor: None,
                context_soft_limit_chars: 240_000,
                recovery_budget: self.config.rejection_budget,
            },
            cadence,
            kernel_state,
            node_id: node_id.clone(),
            generation: 0,
            recorder: Some(&recorder),
        };

        let result = tool_loop.run(&agent_goal).await;
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(error) => {
                recorder.record_custom(
                    "node_terminal",
                    serde_json::json!({"class": "failed", "error": error.to_string()}),
                )?;
                recorder.finish("FAILED_PSP9")?;
                self.emit(perspt_core::AgentEvent::Error(error.to_string()));
                return Err(error);
            }
        };

        let mut promoted_paths = Vec::new();
        let promotion_approved = if matches!(outcome.outcome, NodeTerminalOutcome::HardPass) {
            let adjudicated = self
                .adjudicate_candidate(&recorder, &candidate, &task, &node_id)
                .await?;
            adjudicated
                && self
                    .approve_promotion(&recorder, &node_id, candidate.touched_paths())
                    .await?
        } else {
            false
        };
        let final_outcome =
            if matches!(outcome.outcome, NodeTerminalOutcome::HardPass) && !promotion_approved {
                NodeTerminalOutcome::Escalated {
                    certificate_id: uuid::Uuid::new_v4().to_string(),
                }
            } else {
                outcome.outcome.clone()
            };
        let status = if matches!(final_outcome, NodeTerminalOutcome::HardPass) {
            anyhow::ensure!(
                recorder.authority_epoch()? == grant_policy.authority_epoch,
                "authority epoch changed before promotion"
            );
            recorder.record_custom(
                "authority_epoch_rechecked",
                serde_json::json!({"node_id": node_id, "generation": 0, "epoch": grant_policy.authority_epoch}),
            )?;
            let promotion_key = format!("promote:{node_id}:0");
            let promotion_files = promotion_manifest(
                &recorder,
                &self.working_dir,
                candidate.overlay_root(),
                &candidate.touched_paths(),
            )?;
            let promotion_intent = serde_json::json!({
                "idempotency_key": promotion_key,
                "node_id": node_id,
                "generation": 0,
                "authority_epoch": grant_policy.authority_epoch,
                "workspace_root": self.working_dir.canonicalize()?.display().to_string(),
                "candidate_root": candidate.checkpoint(&[]).await?.witness.state_root,
                "files": promotion_files,
            });
            recorder.record_external_intent(&promotion_key, &promotion_intent)?;
            promoted_paths = candidate.promote()?;
            recorder.complete_external_effect(
                &promotion_key,
                &serde_json::json!({"idempotency_key": promotion_key, "paths": promoted_paths}),
            )?;
            recorder.record_custom(
                "candidate_promoted",
                serde_json::json!({"node_id": node_id, "paths": promoted_paths}),
            )?;
            self.emit(perspt_core::AgentEvent::NodeCompleted {
                node_id: node_id.clone(),
                goal: task.clone(),
            });
            "COMPLETED_PSP9"
        } else {
            self.emit(perspt_core::AgentEvent::TaskStatusChanged {
                node_id: node_id.clone(),
                status: perspt_core::NodeStatus::Escalated,
            });
            "ESCALATED_PSP9"
        };
        scheduler.finish(&node_id, 0);
        let terminal_state = if matches!(final_outcome, NodeTerminalOutcome::HardPass) {
            WorkNodeState::Stable
        } else {
            WorkNodeState::Stopped {
                certificate_id: match &final_outcome {
                    NodeTerminalOutcome::Escalated { certificate_id } => certificate_id.clone(),
                    _ => uuid::Uuid::new_v4().to_string(),
                },
            }
        };
        let terminal_graph = execution_revision(&running_graph, &node_id, terminal_state)?;
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

    fn record_calibration_readiness(
        &self,
        recorder: &Psp9Recorder,
        catalog: &StaticCatalog,
        capability: &Capability,
        cadence: &perspt_sdk::VerificationCadence,
    ) -> Result<()> {
        const TARGET_RHO: f64 = 0.05;
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
        let stratum = perspt_sdk::CalibrationStratum {
            domain_package: "perspt-coding".into(),
            domain_version: env!("CARGO_PKG_VERSION").into(),
            effect_kind: "candidate_promotion".into(),
            risk_class: "workspace_mutation".into(),
            model_route: self.model.to_string(),
            verifier_suite_fingerprint,
            tool_catalog_fingerprint,
            policy_version: "coding-contract-v1".into(),
            score_definition: "hard-gate-plus-quadratic-energy-v1".into(),
        };
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
                "certified_for_promotion": false,
                "reason": "coding promotion currently relies on deterministic contract, barrier, and verifier evidence; no probabilistic claim is used",
            }),
        )
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
                "Summarize the deterministic repository map for a coding worker. You have no tools and no authority. Do not claim facts absent from the map.",
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
        node_id: &str,
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
            "You are a conjunctive coding validator with no tools or authority. Review only the realized diff. Return strict JSON: {\"pass\":bool,\"reason\":string}. Reject uncertainty; do not propose edits.",
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
        recorder.store.record_psp9_verdict(&Psp9VerdictRow {
            session_id: recorder.session_id.clone(),
            candidate_id,
            validator_id: model.to_string(),
            stratum: format!("coding:{node_id}:adjudicator:{model}:uncalibrated"),
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
            diff: None,
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

fn promotion_manifest(
    recorder: &Psp9Recorder,
    workspace: &Path,
    candidate: &Path,
    paths: &[String],
) -> Result<Vec<serde_json::Value>> {
    let mut files = Vec::new();
    for relative in paths {
        let before = std::fs::read(workspace.join(relative)).ok();
        let after = std::fs::read(candidate.join(relative)).ok();
        let before_hash = before
            .as_deref()
            .map(|bytes| recorder.record_artifact(bytes, "application/octet-stream"))
            .transpose()?;
        let after_hash = after
            .as_deref()
            .map(|bytes| recorder.record_artifact(bytes, "application/octet-stream"))
            .transpose()?;
        files.push(serde_json::json!({
            "path": relative,
            "before_hash": before_hash,
            "after_hash": after_hash,
        }));
    }
    Ok(files)
}

fn qualify_model(
    value: &str,
    config: &perspt_core::Config,
    portfolio: &perspt_core::ModelPortfolio,
) -> Result<ModelId> {
    if let Ok(model) = ModelId::from_str(value) {
        return Ok(model);
    }
    let ids = portfolio.provider_ids();
    let provider = config
        .provider
        .as_deref()
        .filter(|id| ids.iter().any(|configured| configured == id))
        .map(str::to_string)
        .or_else(|| (ids.len() == 1).then(|| ids[0].clone()))
        .context("bare model name is ambiguous; use provider::model")?;
    Ok(ModelId::new(provider, value))
}

fn initial_graph(node_id: &str, task: &str) -> Result<WorkGraphRevision> {
    let mut node = WorkNode::new(node_id, task, NodeClass::Implement);
    node.owner_domains = vec!["coding".into()];
    node.state = WorkNodeState::Ready;
    WorkGraphRevision::build(
        0,
        None,
        perspt_sdk::GraphRevisionReason::InitialPlan,
        vec![node],
        vec![],
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn execution_revision(
    previous: &WorkGraphRevision,
    node_id: &str,
    state: WorkNodeState,
) -> Result<WorkGraphRevision> {
    let mut nodes = previous.nodes.clone();
    let node = nodes
        .iter_mut()
        .find(|node| node.node_id == node_id)
        .context("execution revision references an unknown node")?;
    node.state = state;
    WorkGraphRevision::build(
        previous.sequence + 1,
        Some(previous.revision_id.clone()),
        perspt_sdk::GraphRevisionReason::ExecutionUpdate,
        nodes,
        previous.edges.clone(),
    )
    .map_err(|error| anyhow::anyhow!(error.to_string()))
}

fn node_footprint(node: &WorkNode) -> Footprint {
    if node.output_targets.is_empty() {
        return Footprint::new().write(Resource::OpaqueWorkspace);
    }
    node.output_targets
        .iter()
        .fold(Footprint::new(), |footprint, path| {
            footprint.write(Resource::File(path.clone()))
        })
}

fn worker_capability(session_id: &str, graph_revision: &str, authority_epoch: u64) -> Capability {
    let mut capability = Capability::new(
        ActorId::new("toolloop"),
        vec![
            EffectKind::ReadFile,
            EffectKind::ToolSearch,
            EffectKind::ToolProgram,
            EffectKind::Search,
            EffectKind::List,
            EffectKind::LspQuery,
            EffectKind::WriteArtifact,
            EffectKind::ApplyPatch,
            EffectKind::MoveFile,
            EffectKind::DeleteFile,
            EffectKind::RunTest,
            EffectKind::RunBuild,
            EffectKind::GitRead,
        ],
    );
    capability.path_scope = vec![PathPattern("*".into())];
    capability.command_scope = inspection_command_scope();
    capability.max_calls = Some(100);
    capability.risk_budget = Some(RiskBudget::new("workspace-barrier", 2.0));
    // This capability authorizes only the disposable candidate overlay. The
    // separate promotion policy controls durable delivery to the workspace.
    capability.approval_policy = ApprovalPolicy::Auto;
    capability.session_id = session_id.into();
    capability.authority_epoch = authority_epoch;
    capability.graph_revision = graph_revision.into();
    capability.node_generation = 0;
    capability.role = CapabilityRole::Worker;
    capability
}

fn session_grant_policy(
    workspace: &Path,
    graph_revision: &str,
    persistent: bool,
) -> Result<GrantPolicy> {
    let workspace_root = workspace.canonicalize()?.display().to_string();
    let policy_id = uuid::Uuid::new_v4().to_string();
    let signature_material = format!("{policy_id}:{workspace_root}:{graph_revision}:0");
    Ok(GrantPolicy {
        policy_id,
        workspace_root,
        effect_ceiling: vec![
            EffectKind::ReadFile,
            EffectKind::ToolSearch,
            EffectKind::ToolProgram,
            EffectKind::Search,
            EffectKind::List,
            EffectKind::LspQuery,
            EffectKind::WriteArtifact,
            EffectKind::ApplyPatch,
            EffectKind::MoveFile,
            EffectKind::DeleteFile,
            EffectKind::RunTest,
            EffectKind::RunBuild,
            EffectKind::GitRead,
        ],
        path_ceiling: vec![PathPattern("*".into())],
        command_ceiling: inspection_command_scope(),
        network_ceiling: Vec::new(),
        approval_ceiling: ApprovalPolicy::Ask,
        authority_epoch: 0,
        persistent,
        integrity_binding: format!(
            "ledger:{}",
            perspt_sdk::ledger::content_hash(signature_material.as_bytes())
        ),
    })
}

fn inspection_command_scope() -> Vec<CommandPattern> {
    [
        "rg", "grep", "find", "sort", "uniq", "wc", "comm", "cat", "head", "tail", "ls", "git",
        "sed", "awk",
    ]
    .into_iter()
    .map(|program| CommandPattern(program.into()))
    .collect()
}

struct CodingContract {
    graph_revision: String,
    node_id: String,
    generation: u32,
    policy: perspt_policy::engine::PolicyEngine,
}

impl ContractEvaluator for CodingContract {
    fn evaluate(&self, transition: &CandidateTransition) -> ContractWitness {
        let proposal = &transition.proposal;
        let policy_ok = proposal.command.as_ref().is_none_or(|command| {
            matches!(
                self.policy.evaluate(&format!("{command:?}")),
                perspt_policy::engine::PolicyDecision::Allow
            )
        });
        let scope_valid = transition
            .after
            .canonical_scope
            .iter()
            .all(|path| !path.starts_with('/') && !path.split('/').any(|part| part == ".."));
        let ok = proposal.node_id == self.node_id
            && proposal.generation == self.generation
            && transition.before.graph_revision == self.graph_revision
            && transition.after.graph_revision == self.graph_revision
            && transition.before.node_generation == self.generation
            && transition.after.node_generation == self.generation
            && scope_valid
            && policy_ok
            && !matches!(
                proposal.effect,
                EffectKind::UpdateGraph | EffectKind::UpdatePolicy
            )
            && !matches!(
                proposal.effect,
                EffectKind::RunShell | EffectKind::NetworkFetch
            );
        ContractWitness {
            ok,
            policy_version: "coding-contract-v1".into(),
            evidence_refs: vec![
                format!("before:{}", transition.before.state_root),
                format!("after:{}", transition.after.state_root),
                format!("revision:{}", self.graph_revision),
            ],
        }
    }
}
