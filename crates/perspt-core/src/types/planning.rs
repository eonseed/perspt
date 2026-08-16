use super::*;

// =========================================================================
// Plan Revision and Repair Domain Types
// =========================================================================

/// Status of a plan revision within a session.
///
/// Each session may produce multiple plan revisions as the architect responds
/// to verification failures, scope changes, or governance policies.  Only one
/// revision is active at any time; previous revisions are superseded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanRevisionStatus {
    /// The revision is the current active plan driving execution.
    #[default]
    Active,
    /// A newer revision has replaced this one.
    Superseded,
    /// The revision was explicitly abandoned (e.g., user abort).
    Cancelled,
}

impl std::fmt::Display for PlanRevisionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Superseded => write!(f, "superseded"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// A single plan revision within a session.
///
/// Tracks the evolution of the architect's plan over time.  When the verifier
/// or governance policy triggers a replan, a new `PlanRevision` is created,
/// the previous one is marked `Superseded`, and the new revision becomes
/// the active plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanRevision {
    /// Unique revision identifier.
    pub revision_id: String,
    /// Session this revision belongs to.
    pub session_id: String,
    /// Monotonically-increasing sequence number within the session (1-based).
    pub sequence: u32,
    /// The plan content.
    pub plan: TaskPlan,
    /// Why this revision was created (`"initial"`, `"verification_failure"`,
    /// `"scope_change"`, `"governance_budget_exceeded"`, …).
    pub reason: String,
    /// If this revision supersedes an earlier one, its ID.
    pub supersedes: Option<String>,
    /// Current status of this revision.
    pub status: PlanRevisionStatus,
    /// Epoch seconds when this revision was created.
    pub created_at: i64,
}

impl PlanRevision {
    /// Create the initial plan revision for a session.
    pub fn initial(session_id: impl Into<String>, plan: TaskPlan) -> Self {
        Self {
            revision_id: uuid_v4(),
            session_id: session_id.into(),
            sequence: 1,
            plan,
            reason: "initial".to_string(),
            supersedes: None,
            status: PlanRevisionStatus::Active,
            created_at: epoch_secs(),
        }
    }

    /// Create a successor revision that supersedes `previous`.
    pub fn successor(previous: &PlanRevision, plan: TaskPlan, reason: impl Into<String>) -> Self {
        Self {
            revision_id: uuid_v4(),
            session_id: previous.session_id.clone(),
            sequence: previous.sequence + 1,
            plan,
            reason: reason.into(),
            supersedes: Some(previous.revision_id.clone()),
            status: PlanRevisionStatus::Active,
            created_at: epoch_secs(),
        }
    }

    /// Whether this is the current active revision.
    pub fn is_active(&self) -> bool {
        self.status == PlanRevisionStatus::Active
    }
}

/// Adaptive planning policy that selects the agent phase stack
/// based on task scale and workspace type.
///
/// Each variant maps to a different level of orchestration complexity:
/// - `LocalEdit` — Actuator + Verifier only; no architect needed
/// - `FeatureIncrement` — Architect + Actuator + Verifier
/// - `LargeFeature` — Full 4-agent stack with Speculator
/// - `GreenfieldBuild` — Full stack with workspace-setup node first
/// - `ArchitecturalRevision` — Architect + Speculator first, then execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PlanningPolicy {
    /// Small, localized change: skip architect planning.
    LocalEdit,
    /// Mid-size feature: architect decomposes, actuator implements.
    #[default]
    FeatureIncrement,
    /// Large feature: full SRBN loop with speculative execution.
    LargeFeature,
    /// New project: full stack with bootstrap ordering.
    GreenfieldBuild,
    /// Cross-cutting redesign: plan-first, execute later.
    ArchitecturalRevision,
}

impl PlanningPolicy {
    /// Whether this policy requires architect planning.
    pub fn needs_architect(&self) -> bool {
        !matches!(self, Self::LocalEdit)
    }

    /// Whether this policy activates the speculator.
    pub fn needs_speculator(&self) -> bool {
        matches!(
            self,
            Self::LargeFeature | Self::GreenfieldBuild | Self::ArchitecturalRevision
        )
    }
}

/// A scoping document that constrains what the architect may plan.
///
/// The `FeatureCharter` sits above individual task plans and provides
/// boundaries: maximum module count, maximum files, language policy,
/// and a human-readable description of the intended outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureCharter {
    /// Unique charter identifier (typically per session).
    pub charter_id: String,
    /// Session ID.
    pub session_id: String,
    /// Human-readable scope description (the user's original request).
    pub scope_description: String,
    /// Maximum number of modules/nodes the architect may produce.
    pub max_modules: Option<u32>,
    /// Maximum total files the plan may create.
    pub max_files: Option<u32>,
    /// Maximum plan revisions before hard escalation.
    pub max_revisions: Option<u32>,
    /// Language or plugin constraint (e.g. `"rust"`, `"python"`).
    pub language_constraint: Option<String>,
    /// Epoch seconds when the charter was created.
    pub created_at: i64,
}

impl FeatureCharter {
    /// Create a new charter with just a scope description.
    pub fn new(session_id: impl Into<String>, scope_description: impl Into<String>) -> Self {
        Self {
            charter_id: uuid_v4(),
            session_id: session_id.into(),
            scope_description: scope_description.into(),
            max_modules: None,
            max_files: None,
            max_revisions: None,
            language_constraint: None,
            created_at: epoch_secs(),
        }
    }
}

/// A bounded repair unit that records what was changed during a correction.
///
/// Instead of raw `last_written_file` tracking, every correction pass creates
/// a `RepairFootprint` that records the affected files, applied bundle,
/// verification result before/after, and the node being repaired.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairFootprint {
    /// Unique footprint identifier.
    pub footprint_id: String,
    /// Session ID.
    pub session_id: String,
    /// Node ID being repaired.
    pub node_id: String,
    /// Which plan revision was active when the repair happened.
    pub revision_id: String,
    /// Correction attempt number within this node (1-based).
    pub attempt: u32,
    /// Files that were modified by the repair bundle.
    pub affected_files: Vec<String>,
    /// The artifact bundle applied during this repair.
    pub applied_bundle: ArtifactBundle,
    /// Brief summary of what was wrong (from verifier output).
    pub diagnosis: String,
    /// Whether the repair resolved the issue.
    pub resolved: bool,
    /// Epoch seconds.
    pub created_at: i64,
}

impl RepairFootprint {
    /// Create a new repair footprint.
    pub fn new(
        session_id: impl Into<String>,
        node_id: impl Into<String>,
        revision_id: impl Into<String>,
        attempt: u32,
        bundle: &ArtifactBundle,
        diagnosis: impl Into<String>,
    ) -> Self {
        let affected_files = bundle
            .affected_paths()
            .into_iter()
            .map(String::from)
            .collect();
        Self {
            footprint_id: uuid_v4(),
            session_id: session_id.into(),
            node_id: node_id.into(),
            revision_id: revision_id.into(),
            attempt,
            affected_files,
            applied_bundle: bundle.clone(),
            diagnosis: diagnosis.into(),
            resolved: false,
            created_at: epoch_secs(),
        }
    }

    /// Mark this footprint as having resolved the issue.
    pub fn mark_resolved(&mut self) {
        self.resolved = true;
    }
}

/// Declared dependency expectations for a planned task.
///
/// Used during verification to confirm that the environment matches what
/// the architect assumed when producing the plan (e.g. required packages,
/// expected setup commands, or required toolchain version).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DependencyExpectation {
    /// Packages or crates the task expects to be available.
    pub required_packages: Vec<String>,
    /// Setup commands that must have succeeded before this task runs.
    pub setup_commands: Vec<String>,
    /// Minimum toolchain version string (e.g. `"1.75"` for Rust).
    pub min_toolchain_version: Option<String>,
}

/// Budget envelope for plan execution.
///
/// Tracks cost, step, and revision budgets for a session.  The governance
/// layer checks these limits before allowing further execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetEnvelope {
    /// Session ID.
    pub session_id: String,
    /// Maximum number of node execution steps allowed.
    pub max_steps: Option<u32>,
    /// Steps consumed so far.
    pub steps_used: u32,
    /// Maximum number of plan revisions allowed.
    pub max_revisions: Option<u32>,
    /// Revisions consumed so far.
    pub revisions_used: u32,
    /// Maximum total cost in USD.
    pub max_cost_usd: Option<f64>,
    /// Cost consumed so far.
    pub cost_used_usd: f64,
}

impl BudgetEnvelope {
    /// Create a new budget envelope with no limits.
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            max_steps: None,
            steps_used: 0,
            max_revisions: None,
            revisions_used: 0,
            max_cost_usd: None,
            cost_used_usd: 0.0,
        }
    }

    /// Whether the step budget is exhausted.
    pub fn steps_exhausted(&self) -> bool {
        self.max_steps.is_some_and(|max| self.steps_used >= max)
    }

    /// Whether the revision budget is exhausted.
    pub fn revisions_exhausted(&self) -> bool {
        self.max_revisions
            .is_some_and(|max| self.revisions_used >= max)
    }

    /// Whether the cost budget is exhausted.
    pub fn cost_exhausted(&self) -> bool {
        self.max_cost_usd
            .is_some_and(|max| self.cost_used_usd >= max)
    }

    /// Whether any budget limit has been exceeded.
    pub fn any_exhausted(&self) -> bool {
        self.steps_exhausted() || self.revisions_exhausted() || self.cost_exhausted()
    }

    /// Record a step.
    pub fn record_step(&mut self) {
        self.steps_used += 1;
    }

    /// Record a plan revision.
    pub fn record_revision(&mut self) {
        self.revisions_used += 1;
    }

    /// Record cost.
    pub fn record_cost(&mut self, usd: f64) {
        self.cost_used_usd += usd;
    }
}

/// Helper: current epoch seconds.
pub(crate) fn epoch_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Helper: generate a UUID v4 string (simplified).
pub(crate) fn uuid_v4() -> String {
    // Use timestamp + random-ish counter for unique IDs without pulling uuid crate
    // The orchestrator and ledger layers use the `uuid` crate directly when available.
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    now.as_nanos().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
