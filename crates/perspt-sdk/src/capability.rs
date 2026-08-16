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

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::command::{classify_tier, CommandInvocation, CommandTier};
use crate::error::SdkError;

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
    ToolSearch,
    ToolProgram,
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
                | EffectKind::ToolSearch
                | EffectKind::ToolProgram
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

/// Role-scoped authority template. Role is part of attenuation and cannot be
/// changed by proposal payload data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityRole {
    Session,
    Explorer,
    Worker,
    Reviewer,
}

/// Durable authorization intent. A resume re-mints a fresh live capability
/// from this policy and current workspace facts; the capability itself is
/// deliberately not serializable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrantPolicy {
    pub policy_id: String,
    pub workspace_root: String,
    pub effect_ceiling: Vec<EffectKind>,
    pub path_ceiling: Vec<PathPattern>,
    pub command_ceiling: Vec<CommandPattern>,
    pub network_ceiling: Vec<NetworkPattern>,
    pub approval_ceiling: ApprovalPolicy,
    pub authority_epoch: u64,
    pub persistent: bool,
    /// Content binding recorded in the session ledger. Persistent policies
    /// require a separate cryptographic signature before cross-session use.
    pub integrity_binding: String,
}

impl GrantPolicy {
    /// The deterministic byte encoding the grant signature commits to.
    ///
    /// serde output is not canonical (field order, float and escape formatting
    /// may change across versions), so the signature covers this fixed,
    /// length-prefixed encoding instead. Every field participates; list fields
    /// are count-prefixed so no concatenation can alias two policies.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        fn field(out: &mut Vec<u8>, bytes: &[u8]) {
            out.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
            out.extend_from_slice(bytes);
        }
        fn list(out: &mut Vec<u8>, entries: impl ExactSizeIterator<Item = String>) {
            out.extend_from_slice(&(entries.len() as u64).to_be_bytes());
            for entry in entries {
                field(out, entry.as_bytes());
            }
        }
        let mut out = Vec::new();
        field(&mut out, b"perspt-grant-policy-v1");
        field(&mut out, self.policy_id.as_bytes());
        field(&mut out, self.workspace_root.as_bytes());
        list(
            &mut out,
            self.effect_ceiling.iter().map(|e| format!("{e:?}")),
        );
        list(&mut out, self.path_ceiling.iter().map(|p| p.0.clone()));
        list(&mut out, self.command_ceiling.iter().map(|p| p.0.clone()));
        list(&mut out, self.network_ceiling.iter().map(|p| p.0.clone()));
        field(&mut out, format!("{:?}", self.approval_ceiling).as_bytes());
        out.extend_from_slice(&self.authority_epoch.to_be_bytes());
        out.push(u8::from(self.persistent));
        field(&mut out, self.integrity_binding.as_bytes());
        out
    }
}

/// Signed durable grant intent. Verification authenticates policy bytes only;
/// resume must still intersect the policy with the current authority epoch,
/// workspace facts, and configured ceilings before minting a new capability.
///
/// The embedded public key is *not* a trust anchor — anyone can re-sign a
/// rewritten policy with their own key. Callers MUST verify against the key
/// they trust via [`SignedGrantPolicy::verify_against`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignedGrantPolicy {
    pub policy: GrantPolicy,
    pub public_key: String,
    pub signature: String,
}

impl SignedGrantPolicy {
    pub fn sign(policy: GrantPolicy, secret_key: &[u8; 32]) -> crate::error::Result<Self> {
        let signing_key = SigningKey::from_bytes(secret_key);
        let signature = signing_key.sign(&policy.canonical_bytes());
        Ok(Self {
            policy,
            public_key: hex_encode(signing_key.verifying_key().as_bytes()),
            signature: hex_encode(&signature.to_bytes()),
        })
    }

    /// Verify the signature against a caller-supplied trusted public key.
    ///
    /// This is the only verification that authenticates the policy: it fails
    /// both on a bad signature and on a signer other than `trusted_public_key`.
    pub fn verify_against(&self, trusted_public_key: &[u8; 32]) -> crate::error::Result<()> {
        let embedded: [u8; 32] = hex_decode(&self.public_key)?
            .try_into()
            .map_err(|_| SdkError::Signature("grant public key must be 32 bytes".into()))?;
        if &embedded != trusted_public_key {
            return Err(SdkError::Signature(
                "grant signed by an untrusted key".into(),
            ));
        }
        let signature: [u8; 64] = hex_decode(&self.signature)?
            .try_into()
            .map_err(|_| SdkError::Signature("grant signature must be 64 bytes".into()))?;
        let verifying_key = VerifyingKey::from_bytes(&embedded)
            .map_err(|error| SdkError::Signature(error.to_string()))?;
        verifying_key
            .verify(
                &self.policy.canonical_bytes(),
                &Signature::from_bytes(&signature),
            )
            .map_err(|error| SdkError::Signature(error.to_string()))
    }
}

/// Derive the Ed25519 public key for a grant signing key, so callers can
/// anchor [`SignedGrantPolicy::verify_against`] to the key they resolved
/// without taking their own dependency on the signature crate.
pub fn grant_public_key(secret_key: &[u8; 32]) -> [u8; 32] {
    SigningKey::from_bytes(secret_key)
        .verifying_key()
        .to_bytes()
}

/// Lowercase hex encoding, shared by grant signing and key persistence.
pub fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

/// Inverse of [`hex_encode`].
pub fn hex_decode(value: &str) -> crate::error::Result<Vec<u8>> {
    if value.len() & 1 != 0 {
        return Err(SdkError::Signature("hex value has odd length".into()));
    }
    (0..value.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&value[index..index + 2], 16)
                .map_err(|error| SdkError::Signature(format!("invalid hex value: {error}")))
        })
        .collect()
}

impl ApprovalPolicy {
    /// Ordering for the attenuation preorder: a child's policy must be at
    /// least as strict as its parent's (PSP-9 system 12).
    pub fn strictness(self) -> u8 {
        match self {
            ApprovalPolicy::Auto | ApprovalPolicy::SessionApproved => 0,
            ApprovalPolicy::Ask => 1,
            ApprovalPolicy::Deny => 2,
        }
    }
}

/// A capability: an explicit, attenuable grant of authority (PSP-8 System 7).
#[derive(Debug, Clone, PartialEq)]
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
    pub session_id: String,
    pub authority_epoch: u64,
    pub graph_revision: String,
    pub node_generation: u32,
    pub role: CapabilityRole,
    pub parent_capability_id: Option<String>,
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
            session_id: String::new(),
            authority_epoch: 0,
            graph_revision: String::new(),
            node_generation: 0,
            role: CapabilityRole::Session,
            parent_capability_id: None,
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
        // All three scopes are part of Perspt's effective `E_c` and MUST
        // participate in the Def. 4.1 preorder (PSP-9 system 12, Gate L).
        // Empty child scope means unbounded only when the parent is also
        // unbounded; it must never widen a bounded parent.
        fn scope_attenuates<T: PartialEq>(
            child: &[T],
            parent: &[T],
            parent_has_wildcard: bool,
        ) -> bool {
            if parent.is_empty() {
                return true; // unbounded parent admits any child scope
            }
            if child.is_empty() {
                return false; // bounded parent, unbounded child: widening
            }
            child
                .iter()
                .all(|c| parent_has_wildcard || parent.contains(c))
        }
        let path_wild = source.path_scope.iter().any(|sp| sp.0 == "*");
        if !scope_attenuates(&self.path_scope, &source.path_scope, path_wild) {
            return false;
        }
        let cmd_wild = source.command_scope.iter().any(|sp| sp.0 == "*");
        if !scope_attenuates(&self.command_scope, &source.command_scope, cmd_wild) {
            return false;
        }
        // Network scope is deny-by-default at admissibility time (an empty
        // scope grants no network authority), so its preorder must invert:
        // an empty parent scope bounds the child to empty, and a non-empty
        // child must be covered by the parent. Reusing the allow-by-default
        // rule here would let a no-network parent delegate `*` (widening).
        if source.network_scope.is_empty() {
            if !self.network_scope.is_empty() {
                return false;
            }
        } else {
            let net_wild = source.network_scope.iter().any(|sp| sp.0 == "*");
            if !self
                .network_scope
                .iter()
                .all(|c| net_wild || source.network_scope.contains(c))
            {
                return false;
            }
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
        // Risk budget no larger (PSP-9): a bounded parent bounds the child.
        match (&self.risk_budget, &source.risk_budget) {
            (Some(child), Some(parent))
                if child.limit - child.spent > parent.limit - parent.spent =>
            {
                return false;
            }
            (None, Some(_)) => return false, // child unbounded, parent bounded
            _ => {}
        }
        // Approval policy at least as strict.
        if self.approval_policy.strictness() < source.approval_policy.strictness() {
            return false;
        }
        if self.session_id != source.session_id
            || self.authority_epoch != source.authority_epoch
            || self.graph_revision != source.graph_revision
            || self.node_generation != source.node_generation
        {
            return false;
        }
        if self.role == CapabilityRole::Session && source.role != CapabilityRole::Session {
            return false;
        }
        if self.parent_capability_id.as_deref() != Some(source.capability_id.as_str()) {
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
    /// Additional paths touched by a multi-path effect such as a move.
    #[serde(default)]
    pub additional_paths: Vec<String>,
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
            additional_paths: Vec::new(),
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

    pub fn with_additional_paths(mut self, paths: Vec<String>) -> Self {
        self.additional_paths = paths;
        self
    }

    pub fn with_command(mut self, command: CommandInvocation) -> Self {
        self.command = Some(command);
        self
    }

    // --- PSP-9 system 12 builders. The certified increment `c_c` and the
    // budget debit come from the registered capability contract and
    // `BarrierWitness`, never from model arguments, so there is no
    // `with_risk_cost` builder. ---

    pub fn with_generation(mut self, generation: u32) -> Self {
        self.generation = generation;
        self
    }

    pub fn with_risk_class(mut self, risk: RiskClass) -> Self {
        self.risk = risk;
        self
    }

    pub fn with_network_target(mut self, target: impl Into<String>) -> Self {
        self.network_target = Some(target.into());
        self
    }

    pub fn with_idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = key.into();
        self
    }

    pub fn with_preconditions(mut self, preconditions: Vec<StateWitness>) -> Self {
        self.preconditions = preconditions;
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
    let cap = match find_authorized_capability(proposal, capabilities) {
        Ok(cap) => cap,
        Err(witness) => return *witness,
    };
    if let Some(denied) = check_binding(proposal, cap, state) {
        return denied;
    }
    if let Some(denied) = check_lifetime(proposal, cap, state) {
        return denied;
    }
    if let Some(denied) = check_scope(proposal, cap) {
        return denied;
    }
    if let Some(denied) = check_state_witnesses(proposal, cap, state) {
        return denied;
    }
    finalize_decision(proposal, cap)
}

/// Authority clause: a capability held by the proposing actor that grants the
/// effect. Privileged effects additionally require root, user-minted authority:
/// a delegated or non-session capability cannot carry them, so a self-modifying
/// agent can never launder `UpdateGraph`/`UpdatePolicy` through a child grant.
fn find_authorized_capability<'a>(
    proposal: &EffectProposal,
    capabilities: &'a [Capability],
) -> Result<&'a Capability, Box<AdmissibilityWitness>> {
    let cap = capabilities
        .iter()
        .find(|c| c.holder == proposal.actor && c.grants(proposal.effect))
        .ok_or_else(|| {
            Box::new(deny(
                proposal,
                None,
                DenyReason::NoCapability,
                RecoveryClass::NeedsCapability,
            ))
        })?;
    if proposal.effect.is_privileged()
        && (cap.role != CapabilityRole::Session || cap.parent_capability_id.is_some())
    {
        return Err(Box::new(deny(
            proposal,
            Some(cap),
            DenyReason::PrivilegeEscalation,
            RecoveryClass::Fatal,
        )));
    }
    Ok(cap)
}

/// Binding clause: the capability is bound to the live graph revision,
/// authority epoch, and node generation it was minted for.
fn check_binding(
    proposal: &EffectProposal,
    cap: &Capability,
    state: &KernelState,
) -> Option<AdmissibilityWitness> {
    if !cap.graph_revision.is_empty()
        && state.witnesses.get("__graph_revision") != Some(&cap.graph_revision)
    {
        return Some(deny(
            proposal,
            Some(cap),
            DenyReason::StateWitnessMismatch,
            RecoveryClass::NeedsCapability,
        ));
    }
    if !cap.session_id.is_empty()
        && state
            .witnesses
            .get("__authority_epoch")
            .and_then(|value| value.parse::<u64>().ok())
            != Some(cap.authority_epoch)
    {
        return Some(deny(
            proposal,
            Some(cap),
            DenyReason::StateWitnessMismatch,
            RecoveryClass::NeedsCapability,
        ));
    }
    if proposal.generation != cap.node_generation {
        return Some(deny(
            proposal,
            Some(cap),
            DenyReason::StateWitnessMismatch,
            RecoveryClass::Retryable,
        ));
    }
    None
}

/// Lifetime clause: expiry `τ_c` and call budget `q_c`. An expiry with no
/// `__now` witness fails closed — the kernel cannot certify "not expired"
/// without a clock it trusts, and defaulting to open would make `expires_at`
/// decorative on any path that forgets to supply time.
fn check_lifetime(
    proposal: &EffectProposal,
    cap: &Capability,
    state: &KernelState,
) -> Option<AdmissibilityWitness> {
    if let Some(expiry) = cap.expires_at {
        match state
            .witnesses
            .get("__now")
            .and_then(|s| s.parse::<i64>().ok())
        {
            Some(now) if now <= expiry => {}
            _ => {
                return Some(deny(
                    proposal,
                    Some(cap),
                    DenyReason::Expired,
                    RecoveryClass::NeedsCapability,
                ));
            }
        }
    }
    if cap.max_calls == Some(0) {
        return Some(deny(
            proposal,
            Some(cap),
            DenyReason::CallBudgetExhausted,
            RecoveryClass::NeedsCapability,
        ));
    }
    None
}

/// Scope clause: path, command, and network scope of the effective `E_c`.
fn check_scope(proposal: &EffectProposal, cap: &Capability) -> Option<AdmissibilityWitness> {
    for path in proposal.path.iter().chain(proposal.additional_paths.iter()) {
        if !cap.path_scope.is_empty() && !cap.path_scope.iter().any(|p| p.matches(path)) {
            return Some(deny(
                proposal,
                Some(cap),
                DenyReason::PathOutOfScope,
                RecoveryClass::NeedsApproval,
            ));
        }
    }
    if let Some(command) = &proposal.command {
        if command.requires_shell() && !cap.grants(EffectKind::RunShell) {
            return Some(deny(
                proposal,
                Some(cap),
                DenyReason::ShellNotPermitted,
                RecoveryClass::NeedsApproval,
            ));
        }
        let mutation_effect = matches!(
            proposal.effect,
            EffectKind::WriteArtifact
                | EffectKind::ApplyPatch
                | EffectKind::MoveFile
                | EffectKind::DeleteFile
                | EffectKind::MutateDependencies
        );
        if classify_tier(command) == CommandTier::Mutation
            && !mutation_effect
            && proposal.effect.is_read_only()
        {
            return Some(deny(
                proposal,
                Some(cap),
                DenyReason::MutationNotPermitted,
                RecoveryClass::NeedsApproval,
            ));
        }
        if !cap.command_scope.is_empty()
            && !cap
                .command_scope
                .iter()
                .any(|p| p.matches(command.program_name()))
        {
            return Some(deny(
                proposal,
                Some(cap),
                DenyReason::CommandOutOfScope,
                RecoveryClass::NeedsApproval,
            ));
        }
    }
    if let Some(target) = &proposal.network_target {
        if !cap.network_scope.iter().any(|p| p.matches(target)) {
            return Some(deny(
                proposal,
                Some(cap),
                DenyReason::NetworkOutOfScope,
                RecoveryClass::NeedsApproval,
            ));
        }
    }
    None
}

/// State-witness clause: every recorded precondition hash still holds.
fn check_state_witnesses(
    proposal: &EffectProposal,
    cap: &Capability,
    state: &KernelState,
) -> Option<AdmissibilityWitness> {
    for w in &proposal.preconditions {
        match state.witnesses.get(&w.resource) {
            Some(current) if current == &w.content_hash => {}
            _ => {
                return Some(deny(
                    proposal,
                    Some(cap),
                    DenyReason::StateWitnessMismatch,
                    RecoveryClass::Retryable,
                ));
            }
        }
    }
    None
}

/// Risk-budget and approval clauses, then the final witness.
fn finalize_decision(proposal: &EffectProposal, cap: &Capability) -> AdmissibilityWitness {
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
        child.parent_capability_id = Some(parent.capability_id.clone());
        assert!(child.attenuates(&parent));
        assert!(parent.delegate(child).is_some());

        // Invalid child: tries to add an effect the parent lacks.
        let mut bad = Capability::new(ActorId::new("sub"), vec![EffectKind::UpdatePolicy]);
        bad.parent_capability_id = Some(parent.capability_id.clone());
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

    #[test]
    fn persistent_grant_signature_detects_policy_tampering() {
        let policy = GrantPolicy {
            policy_id: "p1".into(),
            workspace_root: "/workspace".into(),
            effect_ceiling: vec![EffectKind::ReadFile],
            path_ceiling: vec![PathPattern("src/*".into())],
            command_ceiling: vec![],
            network_ceiling: vec![],
            approval_ceiling: ApprovalPolicy::Ask,
            authority_epoch: 3,
            persistent: true,
            integrity_binding: "ledger:abc".into(),
        };
        let mut signed = SignedGrantPolicy::sign(policy, &[7u8; 32]).unwrap();
        let trusted: [u8; 32] = hex_decode(&signed.public_key).unwrap().try_into().unwrap();
        signed.verify_against(&trusted).unwrap();
        signed.policy.authority_epoch += 1;
        assert!(signed.verify_against(&trusted).is_err());
    }

    #[test]
    fn grant_signature_rejects_an_untrusted_signer() {
        // Re-signing a rewritten policy with a different key must not verify:
        // the embedded public key is not a trust anchor.
        let policy = GrantPolicy {
            policy_id: "p1".into(),
            workspace_root: "/workspace".into(),
            effect_ceiling: vec![EffectKind::ReadFile],
            path_ceiling: vec![],
            command_ceiling: vec![],
            network_ceiling: vec![],
            approval_ceiling: ApprovalPolicy::Ask,
            authority_epoch: 0,
            persistent: true,
            integrity_binding: "ledger:abc".into(),
        };
        let trusted = SignedGrantPolicy::sign(policy.clone(), &[7u8; 32]).unwrap();
        let trusted_key: [u8; 32] = hex_decode(&trusted.public_key).unwrap().try_into().unwrap();
        let mut widened = policy;
        widened.effect_ceiling.push(EffectKind::RunShell);
        let attacker = SignedGrantPolicy::sign(widened, &[9u8; 32]).unwrap();
        assert!(attacker.verify_against(&trusted_key).is_err());
    }

    #[test]
    fn no_network_parent_cannot_delegate_network_authority() {
        let parent = Capability::new(actor(), vec![EffectKind::NetworkFetch]).delegable();
        let mut child = Capability::new(ActorId::new("child"), vec![EffectKind::NetworkFetch]);
        child.parent_capability_id = Some(parent.capability_id.clone());
        child.role = CapabilityRole::Worker;
        child.network_scope = vec![NetworkPattern("*".into())];
        assert!(parent.delegate(child).is_none());
    }

    #[test]
    fn privileged_effect_requires_root_session_authority() {
        // A delegated (child) capability carrying a privileged effect is
        // rejected even though it nominally grants it.
        let mut cap = Capability::new(actor(), vec![EffectKind::UpdatePolicy]);
        cap.role = CapabilityRole::Worker;
        cap.parent_capability_id = Some("parent".into());
        let proposal = EffectProposal::new(actor(), "n1", EffectKind::UpdatePolicy);
        let w = check_admissibility(&proposal, &[cap], &KernelState::new());
        assert!(matches!(
            w.decision,
            AdmissibilityDecision::Deny {
                reason: DenyReason::PrivilegeEscalation
            }
        ));
    }

    #[test]
    fn expiry_without_a_now_witness_fails_closed() {
        let mut cap = Capability::new(actor(), vec![EffectKind::ReadFile]);
        cap.expires_at = Some(1_700_000_000);
        let proposal = EffectProposal::new(actor(), "n1", EffectKind::ReadFile);
        let w = check_admissibility(&proposal, std::slice::from_ref(&cap), &KernelState::new());
        assert!(matches!(
            w.decision,
            AdmissibilityDecision::Deny {
                reason: DenyReason::Expired
            }
        ));
        let mut state = KernelState::new();
        state.set_witness("__now", "1699999999");
        let w = check_admissibility(&proposal, &[cap], &state);
        assert_eq!(w.decision, AdmissibilityDecision::Allow);
    }
}
