//! Node assembly: graphs, capabilities, grants, and the coding contract.

use super::*;

pub(crate) fn promotion_manifest(
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
    let mut proposal = perspt_sdk::EffectProposal::new(
        capability.holder.clone(),
        node_id,
        perspt_sdk::EffectKind::WriteArtifact,
    )
    .with_risk_class(perspt_sdk::RiskClass::High)
    .with_idempotency_key(format!("promote:{node_id}:0"));
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

pub(crate) fn worker_capability(
    session_id: &str,
    graph_revision: &str,
    authority_epoch: u64,
) -> Capability {
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

pub(crate) fn session_grant_policy(
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
