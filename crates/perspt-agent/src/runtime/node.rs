//! Node assembly: graphs, capabilities, grants, and the coding contract.

use super::*;

pub(crate) fn promotion_manifest(
    recorder: &Psp9Recorder,
    workspace: &Path,
    candidate: &Path,
    paths: &[String],
) -> Result<Vec<serde_json::Value>> {
    let workspace_root = crate::promote::WorkspaceRoot::open(workspace)?;
    let candidate_root = crate::promote::WorkspaceRoot::open(candidate)?;
    let mut files = Vec::new();
    for relative in paths {
        let before = workspace_root.read_if_present(relative)?;
        let after = candidate_root.read_if_present(relative)?;
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

/// Write-ahead promotion: record the intent, promote under the epoch
/// guard, then record the commit and completion.
pub(crate) fn commit_promotion(
    recorder: &Psp9Recorder,
    candidate: &CandidateWorkspace,
    node_id: &str,
    epoch: u64,
    state_root: &str,
    touched: &[String],
    working_dir: &Path,
) -> Result<Vec<String>> {
    let generation = candidate.node_generation();
    let promotion_key = format!("promote:{node_id}:{generation}");
    let promotion_files =
        promotion_manifest(recorder, working_dir, candidate.overlay_root(), touched)?;
    let promotion_intent = serde_json::json!({
        "idempotency_key": promotion_key,
        "node_id": node_id,
        "generation": generation,
        "authority_epoch": epoch,
        "workspace_root": working_dir.canonicalize()?.display().to_string(),
        "candidate_root": state_root,
        "files": promotion_files,
    });
    recorder.record_external_intent(&promotion_key, &promotion_intent)?;
    let promoted_paths =
        recorder
            .store
            .with_authority_epoch(&recorder.session_id, epoch, || candidate.promote())?;
    recorder.record_custom(
        "authority_epoch_effect_committed",
        serde_json::json!({
            "node_id": node_id,
            "generation": generation,
            "epoch": epoch,
        }),
    )?;
    recorder.complete_external_effect(
        &promotion_key,
        &serde_json::json!({"idempotency_key": promotion_key, "paths": promoted_paths}),
    )?;
    recorder.record_custom(
        "candidate_promoted",
        serde_json::json!({"node_id": node_id, "paths": promoted_paths}),
    )?;
    Ok(promoted_paths)
}

/// Session plumbing the recovery ladder threads through every rung.
pub(crate) struct LadderSession<'a> {
    pub recorder: &'a Psp9Recorder,
    pub session_id: &'a str,
    pub node_id: &'a str,
    pub scheduler: &'a mut Scheduler,
}

/// Level 4: the loop already restored the accepted state; revoke the
/// session's authority so nothing minted under this epoch can still
/// deliver. Exception: containment caused by exhausted provider transport
/// is an infrastructure outcome, not a governance anomaly — authority is
/// preserved so `perspt resume` can continue from the durable checkpoint
/// once the provider recovers.
pub(crate) fn contain_if_escalated(
    recorder: &Psp9Recorder,
    session_id: &str,
    attempt: &NodeAttempt,
) -> Result<()> {
    if !matches!(
        attempt.outcome.outcome,
        NodeTerminalOutcome::Escalated { .. }
    ) {
        return Ok(());
    }
    if attempt.outcome.contained_by_transport {
        recorder.record_custom(
            "containment_preserved_authority",
            serde_json::json!({
                "reason": "provider transport exhausted; resume remains viable",
            }),
        )?;
        return Ok(());
    }
    let epoch = recorder.store.revoke_authority(session_id)?;
    recorder.record_custom(
        "authority_epoch_revoked",
        serde_json::json!({"level": "contain", "new_epoch": epoch}),
    )?;
    Ok(())
}

/// Gate decisions an attempt consumed, for the shared non-replenishing pool.
pub(crate) fn spent_of(attempt: &NodeAttempt) -> u32 {
    attempt
        .outcome
        .trajectory
        .rejections_used
        .max(attempt.outcome.recovery_spent)
}

/// Swap the scheduler's running entry to the refined generation so its view
/// stays aligned with the graph revision (frees the old footprint, occupies
/// the new one, and records the dispatch).
pub(crate) fn redispatch_refined(
    recorder: &Psp9Recorder,
    scheduler: &mut Scheduler,
    revised: &WorkGraphRevision,
    node_id: &str,
    prior_generation: u32,
) -> Result<()> {
    scheduler.finish(node_id, prior_generation);
    let node = revised
        .node(node_id)
        .context("refined node missing from revision")?;
    scheduler.start(node, node_footprint(node));
    recorder.record_custom(
        "scheduler_dispatch",
        serde_json::json!({
            "node_id": node.node_id,
            "generation": node.generation,
            "parallel_slot": 0,
        }),
    )?;
    Ok(())
}

pub(crate) fn qualify_model(
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

pub(crate) fn initial_graph(node_id: &str, task: &str) -> Result<WorkGraphRevision> {
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

pub(crate) fn execution_revision(
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

pub(crate) fn node_footprint(node: &WorkNode) -> Footprint {
    if node.output_targets.is_empty() {
        return Footprint::new().write(Resource::OpaqueWorkspace);
    }
    node.output_targets
        .iter()
        .fold(Footprint::new(), |footprint, path| {
            footprint.write(Resource::File(path.clone()))
        })
}

/// Resolve the `[ensemble]` config block into the SDK policy; omitted
/// fields keep the refusing defaults, and an unknown trigger fails fast.
pub(crate) fn resolve_ensemble_policy(
    config: Option<&perspt_core::EnsembleConfig>,
) -> Result<perspt_sdk::EnsemblePolicy> {
    let mut policy = perspt_sdk::EnsemblePolicy::default();
    let Some(config) = config else {
        return Ok(policy);
    };
    if let Some(trigger) = config.trigger.as_deref() {
        policy.trigger = match trigger {
            "after_gate_failure" => perspt_sdk::EnsembleTrigger::AfterGateFailure,
            "never" => perspt_sdk::EnsembleTrigger::Never,
            other => anyhow::bail!("unknown ensemble trigger {other:?}"),
        };
    }
    if let Some(width) = config.width {
        anyhow::ensure!(
            (1..=perspt_sdk::EnsemblePolicy::MAX_WIDTH).contains(&width),
            "ensemble width must be between 1 and {}",
            perspt_sdk::EnsemblePolicy::MAX_WIDTH
        );
        policy.width = width;
    }
    if let Some(distinct) = config.require_distinct_family {
        policy.require_distinct_family = distinct;
    }
    Ok(policy)
}

/// Resolve the explorer, adjudicator, and handoff role routes.
#[allow(clippy::type_complexity)]
pub(crate) fn resolve_role_routes(
    routes: &Psp9ModelRoutes,
    config: &perspt_core::Config,
    transport: &Arc<GenAiTransport>,
) -> Result<(Option<ModelId>, Option<ModelId>, Option<ModelId>)> {
    let explorer = resolve_role_route(
        routes.explorer.as_deref(),
        config.models.as_ref().and_then(|m| m.speculator.as_deref()),
        config,
        transport,
    )?;
    let adjudicator = resolve_role_route(
        routes.adjudicator.as_deref(),
        config
            .models
            .as_ref()
            .and_then(|m| m.adjudicator.as_deref()),
        config,
        transport,
    )?;
    let handoff = resolve_role_route(
        None,
        config.models.as_ref().and_then(|m| m.architect.as_deref()),
        config,
        transport,
    )?
    .filter(|candidate| transport.capabilities(candidate).tool_calling);
    Ok((explorer, adjudicator, handoff))
}

/// Resolve an optional role route (explicit flag first, then config), and
/// verify its provider eagerly so a typo fails before a session is created.
pub(crate) fn resolve_role_route(
    explicit: Option<&str>,
    configured: Option<&str>,
    config: &perspt_core::Config,
    transport: &Arc<GenAiTransport>,
) -> Result<Option<ModelId>> {
    let resolved = explicit
        .or(configured)
        .map(|value| qualify_model(value, config, transport.portfolio()))
        .transpose()?;
    if let Some(model) = &resolved {
        transport.portfolio().resolve(&model.provider)?;
    }
    Ok(resolved)
}

/// Evaluate the promotion proposal against all five clauses on the realized
/// witness. Returns whether promotion is certified; a denial is recorded.
#[allow(clippy::too_many_arguments)]
pub(crate) fn certify_promotion(
    recorder: &Psp9Recorder,
    node_id: &str,
    touched: &[String],
    realized: &perspt_sdk::CandidateStateWitness,
    capability: &Capability,
    contract: &CodingContract,
    barrier: &OperationalSafetyBarrier,
    kernel_state: &perspt_sdk::KernelState,
) -> Result<bool> {
    let mut state = kernel_state.clone();
    state.set_witness("__candidate_root", realized.state_root.clone());
    let generation = capability.node_generation;
    let mut proposal = perspt_sdk::EffectProposal::new(
        capability.holder.clone(),
        node_id,
        perspt_sdk::EffectKind::WriteArtifact,
    )
    .with_generation(generation)
    .with_risk_class(perspt_sdk::RiskClass::High)
    .with_idempotency_key(format!("promote:{node_id}:{generation}"));
    if let Some((first, rest)) = touched.split_first() {
        proposal = proposal
            .with_path(first.clone())
            .with_additional_paths(rest.to_vec());
    }
    let transition =
        perspt_sdk::CandidateTransition::new(proposal, realized.clone(), realized.clone());
    let witness = perspt_sdk::check_full_admissibility(
        &transition,
        std::slice::from_ref(capability),
        &state,
        Some(contract),
        Some(barrier),
        0.25,
    )
    .map_err(|e| anyhow::anyhow!("promotion kernel: {e}"))?;
    recorder.record_custom("promotion_admissibility", serde_json::to_value(&witness)?)?;
    if !witness.allows() || witness.profile != perspt_sdk::AdmissibilityProfile::SrbnCertified {
        recorder.record_custom(
            "promotion_denied",
            serde_json::json!({
                "node_id": node_id,
                "decision": format!("{:?}", witness.base.decision),
                "profile": format!("{:?}", witness.profile),
            }),
        )?;
        return Ok(false);
    }
    Ok(true)
}

/// Uniform delayed-audit funding (resolved decision 4): at cold start every
/// promoted candidate is audit-selected. The sample stays unlabeled until
/// `perspt audit` ingests the delayed verdict; only labeled samples count
/// toward the conformal floor.
/// Record the deterministic verifier suite's verdict beside the
/// adjudicator's (validator id "deterministic-suite"). `missed` at insert
/// time records the contemporaneous verdict ("did the suite flag this
/// candidate"); false negatives are computed later against the delayed
/// label by the estimator.
pub(crate) fn record_deterministic_verdict(
    recorder: &Psp9Recorder,
    candidate_id: &str,
    stratum: &str,
    hard_pass: bool,
) -> Result<()> {
    let evidence = serde_json::json!({"hard_pass": hard_pass});
    let evidence_hash =
        recorder.record_artifact(evidence.to_string().as_bytes(), "application/json")?;
    recorder.store.record_psp9_verdict(&Psp9VerdictRow {
        session_id: recorder.session_id.clone(),
        candidate_id: candidate_id.to_string(),
        validator_id: "deterministic-suite".into(),
        stratum: stratum.to_string(),
        missed: !hard_pass,
        unsafe_label: None,
        evidence_hash,
    })
}

/// `rho_eff` from labeled matched verdicts, only when certified (>= 20
/// matched labeled samples per pair, Hoeffding upper bounds).
pub(crate) fn certified_pairwise_risk(store: &SessionStore) -> Option<f64> {
    let rows = store.labeled_psp9_verdicts().ok()?;
    let records = verdict_records(&rows);
    if records.is_empty() {
        return None;
    }
    perspt_sdk::independence::compute(&records)
        .ok()
        .and_then(|stats| stats.rho_eff)
}

/// Convert labeled store rows into estimator records. The estimator's
/// `missed` is the *false negative* against the delayed label: the
/// validator passed a candidate later labeled unsafe. A stored row's
/// `missed` field is the contemporaneous verdict ("flagged fail").
pub(crate) fn verdict_records(rows: &[Psp9VerdictRow]) -> Vec<perspt_sdk::VerdictRecord> {
    rows.iter()
        .filter_map(|row| {
            let unsafe_label = row.unsafe_label?;
            let passed = !row.missed;
            Some(perspt_sdk::VerdictRecord::new(
                row.validator_id.clone(),
                row.candidate_id.clone(),
                passed && unsafe_label,
            ))
        })
        .collect()
}

pub(crate) fn record_promotion_sample(
    recorder: &Psp9Recorder,
    calibration: &CalibrationBinding,
    sample_id: &str,
) -> Result<()> {
    recorder.store.record_psp9_calibration_sample(
        &calibration.epoch_id,
        sample_id,
        1.0, // score definition v1: hard pass ⇒ V = 0 ⇒ 1/(1+V)
        None,
        true,
    )?;
    recorder.record_custom(
        "calibration_sample_recorded",
        serde_json::json!({
            "epoch_id": calibration.epoch_id,
            "sample_id": sample_id,
            "audit_selected": true,
            "labeled": false,
        }),
    )
}

/// The kernel's durable witnesses for one loop: authority epoch, graph
/// revision, and the wall clock (capability expiry fails closed without a
/// `__now` witness, so `expires_at`-bearing capabilities stay enforceable).
pub(crate) fn loop_kernel_state(
    grant_policy: &perspt_sdk::GrantPolicy,
    revision_id: &str,
) -> perspt_sdk::KernelState {
    let mut kernel_state = perspt_sdk::KernelState::new();
    kernel_state.set_witness(
        "__authority_epoch",
        grant_policy.authority_epoch.to_string(),
    );
    kernel_state.set_witness("__graph_revision", revision_id.to_string());
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    kernel_state.set_witness("__now", now_unix.to_string());
    kernel_state
}

/// Resolve, verify, and dedupe the sticky actuator fallback chain. Every
/// fallback must declare native tool calling (Gate U).
pub(crate) fn resolve_fallbacks(
    routes: &[String],
    model: &ModelId,
    config: &perspt_core::Config,
    transport: &Arc<GenAiTransport>,
) -> Result<Vec<ModelId>> {
    let mut fallback_models = Vec::new();
    for value in routes {
        let candidate = qualify_model(value, config, transport.portfolio())?;
        transport.portfolio().resolve(&candidate.provider)?;
        anyhow::ensure!(
            transport.capabilities(&candidate).tool_calling,
            "fallback route {candidate} does not declare native tool calling"
        );
        if candidate != *model && !fallback_models.contains(&candidate) {
            fallback_models.push(candidate);
        }
    }
    Ok(fallback_models)
}

/// Level-2 graph refinement (Theorem 6): revise the running graph with the
/// exhausted attempt's evidence, producing the node's next generation and a
/// refined goal. The revision passes `WorkGraphRevision::revise`, so it is a
/// validated acyclic snapshot, never an in-place mutation.
pub(crate) fn refine_node(
    recorder: &Psp9Recorder,
    graph: &WorkGraphRevision,
    node_id: &str,
    generation: u32,
    goal: &str,
    attempt: &NodeAttempt,
) -> Result<(WorkGraphRevision, String)> {
    let trajectory = &attempt.outcome.trajectory;
    let refined_goal = format!(
        "{goal}\n\nThe previous governed attempt (generation {generation}) exhausted its \
         correction budget: best V = {best:.3} after {rejections} rejection(s) and \
         {denied} denied proposal(s). Decompose the change into smaller verified steps \
         and address the dominant failing verifier first.",
        best = trajectory.best_accepted_energy,
        rejections = trajectory.rejections_used,
        denied = attempt.outcome.projection.denied_proposals,
    );
    let mut node = graph
        .node(node_id)
        .context("refining an unknown node")?
        .clone();
    node.generation = generation + 1;
    node.goal = refined_goal.clone();
    node.state = WorkNodeState::Running;
    let evidence = vec![perspt_sdk::ResidualEventRef {
        residual_id: format!("gate:{node_id}:{generation}:budget-exhausted"),
        class: perspt_sdk::ResidualClass::BudgetExhausted,
        component: perspt_sdk::EnergyComponent::Log,
        weighted_energy: trajectory.best_accepted_energy,
    }];
    let revised = graph
        .revise(
            perspt_sdk::GraphRevisionReason::LocalRepair,
            &[perspt_sdk::GraphEdit::ReplaceNode { node }],
            evidence,
        )
        .map_err(|e| anyhow::anyhow!("graph refinement: {e}"))?;
    recorder.record_custom("graph_revision", serde_json::to_value(&revised)?)?;
    recorder.record_custom(
        "recovery_refined",
        serde_json::json!({
            "level": "refine",
            "node_id": node_id,
            "next_generation": generation + 1,
            "revision_id": revised.revision_id,
        }),
    )?;
    Ok((revised, refined_goal))
}

/// Effects that never enter a session grant implicitly, no matter what the
/// assembled catalog declares. Each needs its own explicit opt-in path
/// (approval mode, dedicated flag, or a future grant surface).
pub(crate) const WITHHELD_EFFECTS: &[EffectKind] = &[
    EffectKind::RunShell,
    EffectKind::GitWrite,
    EffectKind::NetworkFetch,
    EffectKind::MutateDependencies,
    EffectKind::SpawnAgent,
    EffectKind::UpdateGraph,
    EffectKind::UpdatePolicy,
];

/// Granting is policy × data: the effect set follows the assembled catalog
/// (so a registered tool family is grantable without editing this module),
/// minus the withheld set, plus any effect the user explicitly opted into
/// (e.g. `--allow-dependency-mutation`).
pub(crate) fn granted_effects(
    catalog: &dyn ToolCatalog,
    opted_in: &[EffectKind],
) -> Vec<EffectKind> {
    let mut effects = Vec::new();
    for entry in catalog.entries() {
        let withheld =
            WITHHELD_EFFECTS.contains(&entry.effect) && !opted_in.contains(&entry.effect);
        if withheld || effects.contains(&entry.effect) {
            continue;
        }
        effects.push(entry.effect);
    }
    effects
}

pub(crate) fn worker_capability(
    session_id: &str,
    graph_revision: &str,
    authority_epoch: u64,
    generation: u32,
    catalog: &dyn ToolCatalog,
    opted_in: &[EffectKind],
) -> Capability {
    let mut capability =
        Capability::new(ActorId::new("toolloop"), granted_effects(catalog, opted_in));
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
    capability.node_generation = generation;
    capability.role = CapabilityRole::Worker;
    capability
}

pub(crate) fn session_grant_policy(
    workspace: &Path,
    graph_revision: &str,
    persistent: bool,
    catalog: &dyn ToolCatalog,
    opted_in: &[EffectKind],
) -> Result<GrantPolicy> {
    let workspace_root = workspace.canonicalize()?.display().to_string();
    let policy_id = uuid::Uuid::new_v4().to_string();
    let signature_material = format!("{policy_id}:{workspace_root}:{graph_revision}:0");
    Ok(GrantPolicy {
        policy_id,
        workspace_root,
        effect_ceiling: granted_effects(catalog, opted_in),
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

pub(crate) fn inspection_command_scope() -> Vec<CommandPattern> {
    [
        "rg", "grep", "find", "sort", "uniq", "wc", "comm", "cat", "head", "tail", "ls", "git",
        "sed", "awk",
    ]
    .into_iter()
    .map(|program| CommandPattern(program.into()))
    .collect()
}

pub(crate) struct CodingContract {
    pub(crate) graph_revision: String,
    pub(crate) node_id: String,
    pub(crate) generation: u32,
    pub(crate) policy: perspt_policy::engine::PolicyEngine,
}

impl ContractEvaluator for CodingContract {
    fn evaluate(&self, transition: &CandidateTransition) -> ContractWitness {
        let proposal = &transition.proposal;
        // The policy engine matches text patterns against real command lines,
        // so it must see the canonical rendering, never the Debug form.
        let policy_ok = proposal.command.as_ref().is_none_or(|command| {
            matches!(
                self.policy.evaluate(&command.command_line()),
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

/// Load the newest durable candidate checkpoint: its control frame and the
/// seed files fetched from content-addressed artifacts. Refuses when the
/// session's durable authority epoch no longer matches the checkpoint's
/// (revocation invalidates resumed sessions).
pub(crate) struct CandidateSeed {
    pub(crate) expected_state_root: String,
    pub(crate) canonical_scope: Vec<String>,
    pub(crate) files: Vec<crate::toolloop::SeedFile>,
    pub(crate) conversation: Conversation,
    pub(crate) activated_tools: Vec<String>,
}

pub(crate) fn load_candidate_checkpoint(
    recorder: &Psp9Recorder,
    session_id: &str,
) -> Result<(perspt_sdk::ControlFrame, CandidateSeed)> {
    let checkpoint_json = recorder
        .store
        .latest_psp9_checkpoint(session_id)?
        .context("session has no durable candidate checkpoint to resume from")?;
    let value: serde_json::Value = serde_json::from_str(&checkpoint_json)?;
    anyhow::ensure!(
        value.get("kind").and_then(|v| v.as_str()) == Some("candidate"),
        "newest checkpoint is not a candidate checkpoint; exact continuation is unavailable"
    );
    let control: perspt_sdk::ControlFrame = serde_json::from_value(
        value
            .get("control")
            .cloned()
            .context("checkpoint control")?,
    )?;
    let canonical_scope: Vec<String> = serde_json::from_value(
        value
            .get("canonical_scope")
            .cloned()
            .context("checkpoint canonical scope")?,
    )?;
    let conversation: Conversation = serde_json::from_value(
        value
            .get("conversation")
            .cloned()
            .context("checkpoint conversation projection")?,
    )?;
    anyhow::ensure!(
        conversation.unresolved_call_ids() == control.unresolved_call_ids,
        "checkpoint conversation does not match its unresolved-call control frame"
    );
    anyhow::ensure!(
        control.event_schema_version == perspt_sdk::CONVERSATION_EVENT_SCHEMA_VERSION,
        "checkpoint conversation event schema is unsupported"
    );
    // Gate O: the checkpointed conversation clone is only the cheap resume
    // path — it must be *derivable from the ledger*. Refold the recorded
    // deltas and refuse resume unless the fold reproduces both the rolling
    // digest the control frame committed to and the clone itself.
    let rows = recorder.store.get_psp9_events(session_id)?;
    let folded = crate::toolloop::refold_session_context(&rows, &control.projection_digest)?
        .context("no ledger fold reaches the checkpoint digest; refusing resume")?;
    anyhow::ensure!(
        folded.conversation() == &conversation,
        "checkpoint conversation is not derivable from the recorded deltas; refusing resume"
    );
    let file_handles: Vec<crate::toolloop::DurableSeedFile> =
        serde_json::from_value(value.get("files").cloned().context("checkpoint files")?)?;
    let current_epoch = recorder.store.authority_epoch(session_id)?;
    anyhow::ensure!(
        control.authority_epoch == current_epoch,
        "checkpoint is bound to authority epoch {} but the durable epoch is {}; \
         the session's authority was revoked",
        control.authority_epoch,
        current_epoch
    );
    let seed = CandidateSeed {
        expected_state_root: value
            .get("state_root")
            .and_then(serde_json::Value::as_str)
            .context("checkpoint state root")?
            .to_string(),
        canonical_scope,
        files: load_seed_files(recorder, file_handles)?,
        conversation,
        activated_tools: control.activated_tools.clone(),
    };
    Ok((control, seed))
}

/// Export the previous attempt's best accepted state as a seed for the
/// next ladder rung, so recovery continues from the best measured state
/// instead of re-paying the whole cost (Paper III restore-best). `None`
/// when the attempt accepted nothing. The seed carries files only — the
/// next rung starts a fresh conversation around its refined goal.
pub(crate) async fn seed_from_attempt(
    recorder: &Psp9Recorder,
    node_id: &str,
    attempt: &NodeAttempt,
) -> Result<Option<CandidateSeed>> {
    let files = attempt.candidate.export_accepted().await?;
    if files.is_empty() {
        return Ok(None);
    }
    let checkpoint = attempt.candidate.checkpoint(&[]).await?;
    recorder.record_custom(
        "ladder_reseeded",
        serde_json::json!({
            "node_id": node_id,
            "files": files.len(),
            "state_root": checkpoint.witness.state_root,
        }),
    )?;
    Ok(Some(CandidateSeed {
        expected_state_root: checkpoint.witness.state_root,
        canonical_scope: checkpoint.witness.canonical_scope.clone(),
        files,
        conversation: Conversation::default(),
        activated_tools: Vec::new(),
    }))
}

/// Rebuild an accepted candidate state from a durable checkpoint seed and
/// verify the restored state root matches it exactly.
pub(crate) async fn restore_seed(
    candidate: &CandidateWorkspace,
    seed: Option<&CandidateSeed>,
) -> Result<()> {
    let Some(seed) = seed else {
        return Ok(());
    };
    candidate.restore_exported(&seed.files)?;
    let restored = candidate.checkpoint(&seed.canonical_scope).await?;
    anyhow::ensure!(
        restored.witness.state_root == seed.expected_state_root,
        "restored candidate state does not match its durable checkpoint"
    );
    Ok(())
}

/// Rehydrate checkpointed seed files from the content-addressed artifact
/// store, verifying every artifact against its recorded hash.
fn load_seed_files(
    recorder: &Psp9Recorder,
    file_handles: Vec<crate::toolloop::DurableSeedFile>,
) -> Result<Vec<crate::toolloop::SeedFile>> {
    let load = |handle: Option<String>, label: &str| -> Result<Option<Vec<u8>>> {
        let Some(handle) = handle else {
            return Ok(None);
        };
        let bytes = recorder
            .store
            .get_psp9_artifact(&handle)?
            .with_context(|| format!("missing {label} checkpoint artifact {handle}"))?;
        anyhow::ensure!(
            perspt_sdk::content_hash(&bytes) == handle,
            "{label} checkpoint artifact hash mismatch"
        );
        Ok(Some(bytes))
    };
    let mut seed = Vec::with_capacity(file_handles.len());
    for durable in file_handles {
        seed.push(crate::toolloop::SeedFile {
            path: durable.path,
            content: load(durable.content_artifact, "content")?,
            source_preimage: load(durable.source_preimage_artifact, "source pre-image")?,
        });
    }
    Ok(seed)
}

/// Recover the exact immutable graph revision named by the durable control
/// frame. Resume mints fresh authority against it instead of fabricating an
/// unrelated graph root.
pub(crate) fn resumed_running_graph(
    recorder: &Psp9Recorder,
    revision_id: &str,
    node_id: &str,
    generation: u32,
) -> Result<WorkGraphRevision> {
    let mut recovered = None;
    for row in recorder.store.get_psp9_events(&recorder.session_id)? {
        let event: LedgerEvent = serde_json::from_str(&row.event_json)?;
        if let LedgerEvent::Custom { kind, payload } = event {
            if kind == "graph_revision" {
                let graph: WorkGraphRevision = serde_json::from_value(payload)?;
                if graph.revision_id == revision_id {
                    recovered = Some(graph);
                    break;
                }
            }
        }
    }
    let graph = recovered.with_context(|| {
        format!("checkpoint graph revision {revision_id} is absent from the durable ledger")
    })?;
    let node = graph
        .node(node_id)
        .with_context(|| format!("checkpoint graph has no node {node_id}"))?;
    anyhow::ensure!(
        node.generation == generation && node.state == WorkNodeState::Running,
        "checkpoint graph node binding does not match the control frame"
    );
    recorder.record_custom(
        "graph_revision_resumed",
        serde_json::json!({"revision_id": revision_id}),
    )?;
    Ok(graph)
}
