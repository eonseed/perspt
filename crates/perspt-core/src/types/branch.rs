use super::*;

// =============================================================================
// PSP-5 Phase 6: Provisional Branch Ledger and Interface-Sealed Speculation
// =============================================================================

/// PSP-5 Phase 6: State of a provisional branch.
///
/// Provisional branches store speculative child work separately from committed
/// ledger state.  A branch transitions through Active → Sealed → Merged or
/// Flushed, and never enters committed node state without explicit merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionalBranchState {
    /// Branch is executing speculatively; verification has not yet completed.
    Active,
    /// Interface for the branch's parent node is sealed; child work may proceed.
    Sealed,
    /// Branch was merged into committed state after parent met stability threshold.
    Merged,
    /// Branch was discarded because parent verification failed.
    Flushed,
}

impl std::fmt::Display for ProvisionalBranchState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProvisionalBranchState::Active => write!(f, "active"),
            ProvisionalBranchState::Sealed => write!(f, "sealed"),
            ProvisionalBranchState::Merged => write!(f, "merged"),
            ProvisionalBranchState::Flushed => write!(f, "flushed"),
        }
    }
}

/// PSP-5 Phase 6: Provisional branch tracking speculative child work.
///
/// Created before speculative generation begins so the runtime can track
/// branch lifecycle, enforce seal prerequisites, and flush on parent failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionalBranch {
    /// Unique branch identifier.
    pub branch_id: String,
    /// Session this branch belongs to.
    pub session_id: String,
    /// The node executing speculatively in this branch.
    pub node_id: String,
    /// Parent node whose interface this branch depends on.
    pub parent_node_id: String,
    /// Current branch state.
    pub state: ProvisionalBranchState,
    /// SHA-256 hash of the parent interface seal this branch depends on.
    /// `None` if the parent has not yet produced a seal.
    pub parent_seal_hash: Option<[u8; 32]>,
    /// Sandbox workspace directory (if verification ran in sandbox).
    pub sandbox_dir: Option<String>,
    /// Timestamp of branch creation (epoch seconds).
    pub created_at: i64,
    /// Timestamp of last state transition (epoch seconds).
    pub updated_at: i64,
}

impl ProvisionalBranch {
    /// Create a new active provisional branch.
    pub fn new(
        branch_id: impl Into<String>,
        session_id: impl Into<String>,
        node_id: impl Into<String>,
        parent_node_id: impl Into<String>,
    ) -> Self {
        let now = epoch_secs();
        Self {
            branch_id: branch_id.into(),
            session_id: session_id.into(),
            node_id: node_id.into(),
            parent_node_id: parent_node_id.into(),
            state: ProvisionalBranchState::Active,
            parent_seal_hash: None,
            sandbox_dir: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Whether this branch is still eligible for merge (active or sealed).
    pub fn is_live(&self) -> bool {
        matches!(
            self.state,
            ProvisionalBranchState::Active | ProvisionalBranchState::Sealed
        )
    }

    /// Whether this branch has been discarded.
    pub fn is_flushed(&self) -> bool {
        self.state == ProvisionalBranchState::Flushed
    }
}

/// PSP-5 Phase 6: Parent → child branch lineage record.
///
/// Records the dependency edge between a parent branch (or committed node)
/// and a child provisional branch.  Used by flush propagation to find all
/// descendants that must be discarded when a parent fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchLineage {
    /// Unique lineage record ID.
    pub lineage_id: String,
    /// Parent branch ID (or committed node ID if the parent is committed).
    pub parent_branch_id: String,
    /// Child branch ID.
    pub child_branch_id: String,
    /// Whether the dependency is on the parent's sealed interface (vs. full output).
    pub depends_on_seal: bool,
}

/// PSP-5 Phase 6: Record of a sealed interface produced by a node.
///
/// An interface seal is a hash over the exported signatures, schemas, or symbol
/// inventories that downstream nodes depend on.  Once sealed, the interface is
/// immutable within the current SRBN iteration — dependent context is assembled
/// from the seal rather than from mutable parent implementation files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceSealRecord {
    /// Unique seal identifier.
    pub seal_id: String,
    /// Session this seal belongs to.
    pub session_id: String,
    /// Node that produced (and owns) this seal.
    pub node_id: String,
    /// Path of the sealed artifact (relative to workspace).
    pub sealed_path: String,
    /// The kind of structural artifact that was sealed.
    pub artifact_kind: ArtifactKind,
    /// SHA-256 hash of the sealed content.
    pub seal_hash: [u8; 32],
    /// Monotonically increasing version (incremented on re-seal after parent retry).
    pub version: u32,
    /// Timestamp of seal creation (epoch seconds).
    pub created_at: i64,
}

impl InterfaceSealRecord {
    /// Create a new seal from existing structural digest data.
    pub fn from_digest(
        session_id: impl Into<String>,
        node_id: impl Into<String>,
        digest: &StructuralDigest,
    ) -> Self {
        let nid = node_id.into();
        let sid = session_id.into();
        let seal_id = format!("seal_{}_{}", nid, digest.source_path);
        Self {
            seal_id,
            session_id: sid,
            node_id: nid,
            sealed_path: digest.source_path.clone(),
            artifact_kind: digest.artifact_kind,
            seal_hash: digest.hash,
            version: digest.version,
            created_at: epoch_secs(),
        }
    }

    /// Check whether this seal matches a given digest hash.
    pub fn matches_hash(&self, hash: &[u8; 32]) -> bool {
        self.seal_hash == *hash
    }
}

/// PSP-5 Phase 6: Record of a branch flush decision.
///
/// Persisted so that resume and status surfaces can show why speculative work
/// was discarded and which nodes need re-execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchFlushRecord {
    /// Unique flush record ID.
    pub flush_id: String,
    /// Session this flush belongs to.
    pub session_id: String,
    /// Parent node whose failure triggered the flush.
    pub parent_node_id: String,
    /// Branch IDs that were flushed.
    pub flushed_branch_ids: Vec<String>,
    /// Node IDs that should be requeued after the parent stabilizes.
    pub requeue_node_ids: Vec<String>,
    /// Human-readable reason for the flush.
    pub reason: String,
    /// Timestamp of the flush decision (epoch seconds).
    pub created_at: i64,
}

impl BranchFlushRecord {
    /// Create a new flush record.
    pub fn new(
        session_id: impl Into<String>,
        parent_node_id: impl Into<String>,
        flushed_branch_ids: Vec<String>,
        requeue_node_ids: Vec<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            flush_id: format!("flush_{}", uuid_v4()),
            session_id: session_id.into(),
            parent_node_id: parent_node_id.into(),
            flushed_branch_ids,
            requeue_node_ids,
            reason: reason.into(),
            created_at: epoch_secs(),
        }
    }
}

/// PSP-5 Phase 6: Dependency tracking for nodes blocked on a parent seal.
///
/// When a child node depends on a parent's sealed interface that has not yet
/// been produced, the child is registered as a blocked dependent.  Once the
/// parent seals its interface, blocked dependents are unblocked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedDependency {
    /// Child node that is blocked.
    pub child_node_id: String,
    /// Parent node whose seal the child requires.
    pub parent_node_id: String,
    /// Sealed interface paths the child depends on.
    pub required_seal_paths: Vec<String>,
    /// Timestamp when the block was registered (epoch seconds).
    pub blocked_at: i64,
}

impl BlockedDependency {
    /// Create a new blocked dependency record.
    pub fn new(
        child_node_id: impl Into<String>,
        parent_node_id: impl Into<String>,
        required_seal_paths: Vec<String>,
    ) -> Self {
        Self {
            child_node_id: child_node_id.into(),
            parent_node_id: parent_node_id.into(),
            required_seal_paths,
            blocked_at: epoch_secs(),
        }
    }
}
