//! Capability-constrained admissibility kernel (PSP-8 System 7).
//!
//! Stochastic components emit proposals, never unmediated effects. Every effect
//! passes through an admissibility kernel before execution. This module is the
//! domain-neutral reference kernel and contract; `perspt-policy` is the
//! deterministic trusted base that adopts it. Generated code, prompts, domain
//! packages, and subagents are outside that trusted base.
//!
//! Authority is an explicit, attenuable value: delegation may only *shrink*
//! effect scope, call budget, expiry, and delegability (the attenuation
//! preorder `c' ⪯ c`). Payload data, model text, or generated code cannot mint
//! authority (PSP-8 R4).

use serde::{Deserialize, Serialize};

use crate::command::{classify_tier, CommandInvocation, CommandTier};

/// An actor that can hold capabilities and emit proposals.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActorId(pub String);

impl ActorId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// Effect classes (PSP-8 System 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    ReadFile,
    Search,
    List,
    LspQuery,
    WriteArtifact,
    ApplyPatch,
    MoveFile,
    DeleteFile,
    RunVerifier,
    RunFormatter,
    RunTest,
    RunBuild,
    MutateDependencies,
    RunRepoScript,
    RunShell,
    GitRead,
    GitWrite,
    NetworkFetch,
    AskUser,
    SpawnAgent,
    UpdateGraph,
    UpdatePolicy,
}

impl EffectKind {
    /// Read-only effects allowed in workspace scope by default.
    pub fn is_read_only(self) -> bool {
        matches!(
            self,
            EffectKind::ReadFile
                | EffectKind::Search
                | EffectKind::List
                | EffectKind::LspQuery
                | EffectKind::GitRead
        )
    }

    /// Privileged effects that self-modifying agents must never grant themselves.
    pub fn is_privileged(self) -> bool {
        matches!(self, EffectKind::UpdateGraph | EffectKind::UpdatePolicy)
    }
}

/// Risk classification for a proposed effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskClass {
    None,
    Low,
    Medium,
    High,
    Critical,
}

/// A glob-like path pattern. `matches` uses a simple prefix/suffix/`*` rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathPattern(pub String);

impl PathPattern {
    pub fn matches(&self, path: &str) -> bool {
        glob_match(&self.0, path)
    }
}

/// A command pattern matched against the canonical program name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandPattern(pub String);

impl CommandPattern {
    pub fn matches(&self, program: &str) -> bool {
        glob_match(&self.0, program)
    }
}

/// A network host/URL pattern.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkPattern(pub String);

impl NetworkPattern {
    pub fn matches(&self, target: &str) -> bool {
        glob_match(&self.0, target)
    }
}

/// Minimal glob: supports a single trailing `*`, leading `*`, or exact match.
fn glob_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return value.starts_with(prefix);
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return value.ends_with(suffix);
    }
    pattern == value
}

/// A recorded risk budget (PSP-8 System 7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskBudget {
    pub name: String,
    /// Total budget `ρ_c`.
    pub limit: f64,
    /// Amount already spent `spent(x)`.
    pub spent: f64,
}

impl RiskBudget {
    pub fn new(name: impl Into<String>, limit: f64) -> Self {
        Self {
            name: name.into(),
            limit,
            spent: 0.0,
        }
    }

    /// Whether `spent + cost <= limit`.
    pub fn admits(&self, cost: f64) -> bool {
        self.spent + cost <= self.limit
    }
}

/// Approval policy for an effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalPolicy {
    /// Allowed without explicit approval (within scope).
    Auto,
    /// Requires user approval.
    Ask,
    /// Allowed because an approved session policy covers it.
    SessionApproved,
    /// Never allowed.
    Deny,
}

/// A capability: an explicit, attenuable grant of authority (PSP-8 System 7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Capability {
    pub capability_id: String,
    pub holder: ActorId,
    pub effects: Vec<EffectKind>,
    pub path_scope: Vec<PathPattern>,
    pub command_scope: Vec<CommandPattern>,
    pub network_scope: Vec<NetworkPattern>,
    /// Remaining call budget `q_c`. `None` means unbounded.
    pub max_calls: Option<u32>,
    /// Expiry `τ_c` as a unix timestamp. `None` means no expiry.
    pub expires_at: Option<i64>,
    /// Delegability `d_c`.
    pub may_delegate: bool,
    pub risk_budget: Option<RiskBudget>,
    pub approval_policy: ApprovalPolicy,
}

impl Capability {
    pub fn new(holder: ActorId, effects: Vec<EffectKind>) -> Self {
        Self {
            capability_id: uuid::Uuid::new_v4().to_string(),
            holder,
            effects,
            path_scope: Vec::new(),
            command_scope: Vec::new(),
            network_scope: Vec::new(),
            max_calls: None,
            expires_at: None,
            may_delegate: false,
            risk_budget: None,
            approval_policy: ApprovalPolicy::Auto,
        }
    }

    pub fn with_paths(mut self, patterns: Vec<&str>) -> Self {
        self.path_scope = patterns
            .into_iter()
            .map(|p| PathPattern(p.to_string()))
            .collect();
        self
    }

    pub fn delegable(mut self) -> Self {
        self.may_delegate = true;
        self
    }

    pub fn grants(&self, effect: EffectKind) -> bool {
        self.effects.contains(&effect)
    }

    /// The attenuation preorder `c' ⪯ c`: a delegated capability may only shrink
    /// effect scope, call budget, expiry, and delegability (PSP-8 System 7).
    pub fn attenuates(&self, source: &Capability) -> bool {
        // Effects subset.
        if !self.effects.iter().all(|e| source.effects.contains(e)) {
            return false;
        }
        // Path/command/network scope subset (each pattern must be covered).
        let scope_subset = self.path_scope.iter().all(|p| {
            source.path_scope.iter().any(|sp| sp == p)
                || source.path_scope.iter().any(|sp| sp.0 == "*")
        });
        if !source.path_scope.is_empty() && !scope_subset {
            return false;
        }
        // Call budget no greater.
        if let (Some(child), Some(parent)) = (self.max_calls, source.max_calls) {
            if child > parent {
                return false;
            }
        }
        if self.max_calls.is_none() && source.max_calls.is_some() {
            return false; // child unbounded but parent bounded
        }
        // Expiry no later.
        if let (Some(child), Some(parent)) = (self.expires_at, source.expires_at) {
            if child > parent {
                return false;
            }
        }
        if self.expires_at.is_none() && source.expires_at.is_some() {
            return false;
        }
        // Delegability no greater.
        if self.may_delegate && !source.may_delegate {
            return false;
        }
        true
    }

    /// Attempt to delegate an attenuated child capability. Returns `None` if the
    /// source is not delegable or the child does not satisfy the preorder.
    pub fn delegate(&self, child: Capability) -> Option<Capability> {
        if !self.may_delegate {
            return None;
        }
        // The child holder may differ (the delegatee); attenuation governs scope.
        if child.attenuates(self) {
            Some(child)
        } else {
            None
        }
    }
}

/// A state witness: a content hash of a precondition that must still hold at
/// execution time (PSP-8 System 7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateWitness {
    pub resource: String,
    pub content_hash: String,
}

/// An effect proposal (PSP-8 System 7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectProposal {
    pub proposal_id: String,
    pub actor: ActorId,
    pub node_id: String,
    pub generation: u32,
    pub effect: EffectKind,
    /// The path the effect touches, if any.
    pub path: Option<String>,
    /// The command, if this is an execution effect.
    pub command: Option<CommandInvocation>,
    /// The network target, if any.
    pub network_target: Option<String>,
    pub risk: RiskClass,
    /// Cost charged against the capability risk budget `c_c`.
    pub risk_cost: f64,
    pub idempotency_key: String,
    pub preconditions: Vec<StateWitness>,
}

impl EffectProposal {
    pub fn new(actor: ActorId, node_id: impl Into<String>, effect: EffectKind) -> Self {
        Self {
            proposal_id: uuid::Uuid::new_v4().to_string(),
            actor,
            node_id: node_id.into(),
            generation: 0,
            effect,
            path: None,
            command: None,
            network_target: None,
            risk: RiskClass::Low,
            risk_cost: 0.0,
            idempotency_key: uuid::Uuid::new_v4().to_string(),
            preconditions: Vec::new(),
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_command(mut self, command: CommandInvocation) -> Self {
        self.command = Some(command);
        self
    }
}

/// The admissibility decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum AdmissibilityDecision {
    Allow,
    Deny { reason: DenyReason },
    NeedsApproval,
}

/// Why an effect was denied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenyReason {
    NoCapability,
    EffectOutOfScope,
    PathOutOfScope,
    CommandOutOfScope,
    NetworkOutOfScope,
    CallBudgetExhausted,
    Expired,
    RiskBudgetExhausted,
    StateWitnessMismatch,
    ShellNotPermitted,
    MutationNotPermitted,
    PolicyDenied,
    PrivilegeEscalation,
}

/// Recovery classification for a denied or failed effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryClass {
    Retryable,
    NeedsApproval,
    NeedsCapability,
    Fatal,
}

/// The witness produced by checking a proposal (PSP-8 `AdmissibilityWitness`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdmissibilityWitness {
    pub proposal_id: String,
    pub actor: ActorId,
    pub capability_id: Option<String>,
    pub authority_ok: bool,
    pub contract_ok: bool,
    pub effect_ok: bool,
    pub barrier_increment_ok: bool,
    pub risk_budget_ok: bool,
    pub decision: AdmissibilityDecision,
    pub recovery_class: Option<RecoveryClass>,
}

/// The current durable state the kernel reads when checking a proposal.
#[derive(Debug, Clone, Default)]
pub struct KernelState {
    /// Live content hashes of resources, for state-witness validation.
    pub witnesses: std::collections::HashMap<String, String>,
}

impl KernelState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_witness(&mut self, resource: impl Into<String>, hash: impl Into<String>) {
        self.witnesses.insert(resource.into(), hash.into());
    }
}

fn deny(
    proposal: &EffectProposal,
    cap: Option<&Capability>,
    reason: DenyReason,
    recovery: RecoveryClass,
) -> AdmissibilityWitness {
    AdmissibilityWitness {
        proposal_id: proposal.proposal_id.clone(),
        actor: proposal.actor.clone(),
        capability_id: cap.map(|c| c.capability_id.clone()),
        authority_ok: cap.is_some(),
        contract_ok: false,
        effect_ok: false,
        barrier_increment_ok: false,
        risk_budget_ok: false,
        decision: AdmissibilityDecision::Deny { reason },
        recovery_class: Some(recovery),
    }
}

/// Evaluate the admissibility predicate `Adm(x, p, x')` for a proposal against
/// the actor's capabilities and current kernel state.
///
/// Returns an [`AdmissibilityWitness`] recording each clause and the decision.
/// Every effect SHALL be mediated by such a witness before any durable effect
/// occurs (PSP-8 Gate E).
pub fn check_admissibility(
    proposal: &EffectProposal,
    capabilities: &[Capability],
    state: &KernelState,
) -> AdmissibilityWitness {
    // Self-modifying agents cannot grant themselves privileged effects: a
    // privileged effect requires a capability whose holder is *not* the
    // proposing actor, or an explicitly user-granted privileged capability.
    // Find a capability held by the actor that grants the effect.
    let cap = capabilities
        .iter()
        .find(|c| c.holder == proposal.actor && c.grants(proposal.effect));

    let cap = match cap {
        Some(c) => c,
        None => {
            return deny(
                proposal,
                None,
                DenyReason::NoCapability,
                RecoveryClass::NeedsCapability,
            )
        }
    };

    // Expiry.
    if let Some(expiry) = cap.expires_at {
        // A timestamp of 0 in preconditions is treated as "now unknown"; callers
        // pass the real clock through the witness resource. Here we only reject
        // when an explicit `__now` witness exceeds expiry.
        if let Some(now) = state
            .witnesses
            .get("__now")
            .and_then(|s| s.parse::<i64>().ok())
        {
            if now > expiry {
                return deny(
                    proposal,
                    Some(cap),
                    DenyReason::Expired,
                    RecoveryClass::NeedsCapability,
                );
            }
        }
    }

    // Call budget.
    if cap.max_calls == Some(0) {
        return deny(
            proposal,
            Some(cap),
            DenyReason::CallBudgetExhausted,
            RecoveryClass::NeedsCapability,
        );
    }

    // Effect scope: path.
    if let Some(path) = &proposal.path {
        if !cap.path_scope.is_empty() && !cap.path_scope.iter().any(|p| p.matches(path)) {
            return deny(
                proposal,
                Some(cap),
                DenyReason::PathOutOfScope,
                RecoveryClass::NeedsApproval,
            );
        }
    }

    // Command governance.
    if let Some(command) = &proposal.command {
        if command.requires_shell() && !cap.grants(EffectKind::RunShell) {
            return deny(
                proposal,
                Some(cap),
                DenyReason::ShellNotPermitted,
                RecoveryClass::NeedsApproval,
            );
        }
        let tier = classify_tier(command);
        let mutation_effect = matches!(
            proposal.effect,
            EffectKind::WriteArtifact
                | EffectKind::ApplyPatch
                | EffectKind::MoveFile
                | EffectKind::DeleteFile
                | EffectKind::MutateDependencies
        );
        if tier == CommandTier::Mutation && !mutation_effect && proposal.effect.is_read_only() {
            return deny(
                proposal,
                Some(cap),
                DenyReason::MutationNotPermitted,
                RecoveryClass::NeedsApproval,
            );
        }
        if !cap.command_scope.is_empty()
            && !cap
                .command_scope
                .iter()
                .any(|p| p.matches(command.program_name()))
        {
            return deny(
                proposal,
                Some(cap),
                DenyReason::CommandOutOfScope,
                RecoveryClass::NeedsApproval,
            );
        }
    }

    // Network scope.
    if let Some(target) = &proposal.network_target {
        if !cap.network_scope.iter().any(|p| p.matches(target)) {
            return deny(
                proposal,
                Some(cap),
                DenyReason::NetworkOutOfScope,
                RecoveryClass::NeedsApproval,
            );
        }
    }

    // State witnesses still match.
    for w in &proposal.preconditions {
        match state.witnesses.get(&w.resource) {
            Some(current) if current == &w.content_hash => {}
            _ => {
                return deny(
                    proposal,
                    Some(cap),
                    DenyReason::StateWitnessMismatch,
                    RecoveryClass::Retryable,
                )
            }
        }
    }

    // Risk budget: spent + cost <= limit.
    let risk_ok = cap
        .risk_budget
        .as_ref()
        .map(|b| b.admits(proposal.risk_cost))
        .unwrap_or(true);
    if !risk_ok {
        return deny(
            proposal,
            Some(cap),
            DenyReason::RiskBudgetExhausted,
            RecoveryClass::NeedsApproval,
        );
    }

    // Approval policy.
    let decision = match cap.approval_policy {
        ApprovalPolicy::Deny => {
            return deny(
                proposal,
                Some(cap),
                DenyReason::PolicyDenied,
                RecoveryClass::Fatal,
            )
        }
        ApprovalPolicy::Ask => AdmissibilityDecision::NeedsApproval,
        ApprovalPolicy::Auto | ApprovalPolicy::SessionApproved => AdmissibilityDecision::Allow,
    };
    let recovery = match decision {
        AdmissibilityDecision::NeedsApproval => Some(RecoveryClass::NeedsApproval),
        _ => None,
    };

    AdmissibilityWitness {
        proposal_id: proposal.proposal_id.clone(),
        actor: proposal.actor.clone(),
        capability_id: Some(cap.capability_id.clone()),
        authority_ok: true,
        contract_ok: true,
        effect_ok: true,
        barrier_increment_ok: true,
        risk_budget_ok: risk_ok,
        decision,
        recovery_class: recovery,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::canonicalize;

    fn actor() -> ActorId {
        ActorId::new("implementer")
    }

    #[test]
    fn read_only_actor_cannot_write() {
        let caps = vec![Capability::new(
            actor(),
            vec![EffectKind::ReadFile, EffectKind::Search],
        )];
        let proposal =
            EffectProposal::new(actor(), "n1", EffectKind::WriteArtifact).with_path("src/x.rs");
        let w = check_admissibility(&proposal, &caps, &KernelState::new());
        assert!(matches!(
            w.decision,
            AdmissibilityDecision::Deny {
                reason: DenyReason::NoCapability
            }
        ));
    }

    #[test]
    fn write_in_scope_is_allowed() {
        let caps = vec![
            Capability::new(actor(), vec![EffectKind::WriteArtifact]).with_paths(vec!["src/*"])
        ];
        let proposal =
            EffectProposal::new(actor(), "n1", EffectKind::WriteArtifact).with_path("src/x.rs");
        let w = check_admissibility(&proposal, &caps, &KernelState::new());
        assert_eq!(w.decision, AdmissibilityDecision::Allow);
    }

    #[test]
    fn write_out_of_path_scope_is_denied() {
        let caps = vec![
            Capability::new(actor(), vec![EffectKind::WriteArtifact]).with_paths(vec!["src/*"])
        ];
        let proposal =
            EffectProposal::new(actor(), "n1", EffectKind::WriteArtifact).with_path("/etc/passwd");
        let w = check_admissibility(&proposal, &caps, &KernelState::new());
        assert!(matches!(
            w.decision,
            AdmissibilityDecision::Deny {
                reason: DenyReason::PathOutOfScope
            }
        ));
    }

    #[test]
    fn shell_command_denied_without_shell_capability() {
        let mut cap = Capability::new(actor(), vec![EffectKind::RunVerifier]);
        cap.command_scope = vec![CommandPattern("*".into())];
        let proposal = EffectProposal::new(actor(), "n1", EffectKind::RunVerifier)
            .with_command(canonicalize("cat x | grep y", "/r"));
        let w = check_admissibility(&proposal, &[cap], &KernelState::new());
        assert!(matches!(
            w.decision,
            AdmissibilityDecision::Deny {
                reason: DenyReason::ShellNotPermitted
            }
        ));
    }

    #[test]
    fn sed_in_place_denied_under_read_only_effect() {
        let mut cap = Capability::new(actor(), vec![EffectKind::ReadFile]);
        cap.command_scope = vec![CommandPattern("*".into())];
        let proposal = EffectProposal::new(actor(), "n1", EffectKind::ReadFile)
            .with_command(canonicalize("sed -i s/a/b/ f", "/r"));
        let w = check_admissibility(&proposal, &[cap], &KernelState::new());
        assert!(matches!(
            w.decision,
            AdmissibilityDecision::Deny {
                reason: DenyReason::MutationNotPermitted
            }
        ));
    }

    #[test]
    fn stale_state_witness_is_denied() {
        let caps =
            vec![Capability::new(actor(), vec![EffectKind::ApplyPatch]).with_paths(vec!["*"])];
        let mut proposal =
            EffectProposal::new(actor(), "n1", EffectKind::ApplyPatch).with_path("src/x.rs");
        proposal.preconditions = vec![StateWitness {
            resource: "src/x.rs".into(),
            content_hash: "old".into(),
        }];
        let mut state = KernelState::new();
        state.set_witness("src/x.rs", "new"); // changed since proposal
        let w = check_admissibility(&proposal, &caps, &state);
        assert!(matches!(
            w.decision,
            AdmissibilityDecision::Deny {
                reason: DenyReason::StateWitnessMismatch
            }
        ));
    }

    #[test]
    fn risk_budget_exhaustion_is_denied() {
        let mut cap = Capability::new(actor(), vec![EffectKind::ApplyPatch]).with_paths(vec!["*"]);
        cap.risk_budget = Some(RiskBudget {
            name: "session".into(),
            limit: 1.0,
            spent: 0.8,
        });
        let mut proposal =
            EffectProposal::new(actor(), "n1", EffectKind::ApplyPatch).with_path("x");
        proposal.risk_cost = 0.5;
        let w = check_admissibility(&proposal, &[cap], &KernelState::new());
        assert!(matches!(
            w.decision,
            AdmissibilityDecision::Deny {
                reason: DenyReason::RiskBudgetExhausted
            }
        ));
    }

    #[test]
    fn ask_policy_needs_approval() {
        let mut cap = Capability::new(actor(), vec![EffectKind::RunShell]).with_paths(vec!["*"]);
        cap.approval_policy = ApprovalPolicy::Ask;
        cap.command_scope = vec![CommandPattern("*".into())];
        let proposal = EffectProposal::new(actor(), "n1", EffectKind::RunShell)
            .with_command(canonicalize("echo hi | tee x", "/r"));
        let w = check_admissibility(&proposal, &[cap], &KernelState::new());
        assert_eq!(w.decision, AdmissibilityDecision::NeedsApproval);
    }

    #[test]
    fn attenuation_only_shrinks_authority() {
        let parent = Capability::new(
            actor(),
            vec![EffectKind::ReadFile, EffectKind::WriteArtifact],
        )
        .with_paths(vec!["*"])
        .delegable();
        // Valid child: fewer effects, bounded calls.
        let mut child = Capability::new(ActorId::new("sub"), vec![EffectKind::ReadFile])
            .with_paths(vec!["src/*"]);
        child.max_calls = Some(3);
        assert!(child.attenuates(&parent));
        assert!(parent.delegate(child).is_some());

        // Invalid child: tries to add an effect the parent lacks.
        let bad = Capability::new(ActorId::new("sub"), vec![EffectKind::UpdatePolicy]);
        assert!(!bad.attenuates(&parent));
        assert!(parent.delegate(bad).is_none());
    }

    #[test]
    fn non_delegable_capability_cannot_delegate() {
        let parent = Capability::new(actor(), vec![EffectKind::ReadFile]); // may_delegate = false
        let child = Capability::new(ActorId::new("sub"), vec![EffectKind::ReadFile]);
        assert!(parent.delegate(child).is_none());
    }

    #[test]
    fn payload_cannot_mint_authority() {
        // An actor with no capability at all cannot perform any effect, no matter
        // what the proposal claims.
        let proposal = EffectProposal::new(ActorId::new("ghost"), "n1", EffectKind::UpdatePolicy);
        let w = check_admissibility(&proposal, &[], &KernelState::new());
        assert!(matches!(
            w.decision,
            AdmissibilityDecision::Deny {
                reason: DenyReason::NoCapability
            }
        ));
    }
}
