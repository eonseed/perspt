use super::*;
/// Sensor availability status for a single verification stage.
///
/// Tells downstream consumers whether the preferred tool was available,
/// a fallback was used, or the stage had no usable sensor at all.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SensorStatus {
    /// The preferred tool ran successfully.
    Available,
    /// A fallback tool was used instead of the primary.
    Fallback {
        /// Name of the tool that actually ran.
        actual: String,
        /// Why the primary was not available.
        reason: String,
    },
    /// No tool was available for this stage.
    Unavailable {
        /// What went wrong.
        reason: String,
    },
}

impl std::fmt::Display for SensorStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SensorStatus::Available => write!(f, "available"),
            SensorStatus::Fallback { actual, .. } => write!(f, "fallback({})", actual),
            SensorStatus::Unavailable { reason } => write!(f, "unavailable({})", reason),
        }
    }
}

/// Outcome of a single verification stage (syntax, build, test, lint).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageOutcome {
    /// Which verification stage this covers.
    pub stage: String,
    /// Whether the stage passed.
    pub passed: bool,
    /// Sensor status for this stage.
    pub sensor_status: SensorStatus,
    /// Optional output captured from the tool.
    pub output: Option<String>,
}

// =============================================================================
// PSP-5 Phase 3: Context Provenance, Structural Digests, Restriction Maps
// =============================================================================

/// PSP-5 Phase 3: Kind of structural artifact being digested
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    /// Exported function/trait/class signature
    Signature,
    /// API schema (JSON schema, protobuf, etc.)
    Schema,
    /// Module-level symbol inventory
    SymbolInventory,
    /// Interface seal for dependency checking
    InterfaceSeal,
}

impl std::fmt::Display for ArtifactKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArtifactKind::Signature => write!(f, "signature"),
            ArtifactKind::Schema => write!(f, "schema"),
            ArtifactKind::SymbolInventory => write!(f, "symbol_inventory"),
            ArtifactKind::InterfaceSeal => write!(f, "interface_seal"),
        }
    }
}

/// PSP-5 Phase 3: Hash of a compile-critical structural artifact
///
/// Structural digests represent machine-verifiable content (exported signatures,
/// schemas, symbol inventories) that nodes depend on. When the digest changes,
/// dependent nodes must re-verify.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralDigest {
    /// Unique digest identifier
    pub digest_id: String,
    /// What kind of structural artifact this is
    pub artifact_kind: ArtifactKind,
    /// SHA-256 hash of the artifact content
    pub hash: [u8; 32],
    /// Node that produced this artifact
    pub source_node_id: String,
    /// Source file path (relative to workspace)
    pub source_path: String,
    /// Monotonically increasing version
    pub version: u32,
}

impl StructuralDigest {
    /// Create a new digest from raw content
    pub fn from_content(
        source_node_id: impl Into<String>,
        source_path: impl Into<String>,
        artifact_kind: ArtifactKind,
        content: &[u8],
    ) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut sha = [0u8; 32];
        // Use a simple hash for the digest (real impl would use SHA-256)
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        let h = hasher.finish().to_le_bytes();
        sha[..8].copy_from_slice(&h);

        let node_id = source_node_id.into();
        let path = source_path.into();
        let digest_id = format!("{}:{}:{}", node_id, path, artifact_kind);

        Self {
            digest_id,
            artifact_kind,
            hash: sha,
            source_node_id: node_id,
            source_path: path,
            version: 1,
        }
    }

    /// Check if this digest matches another (same content hash)
    pub fn matches(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

/// PSP-5 Phase 3: Kind of semantic summary being digested
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryKind {
    /// Intent summary from parent/architect
    IntentSummary,
    /// Verifier results summary
    VerifierResults,
    /// Design rationale
    DesignRationale,
}

/// PSP-5 Phase 3: Condensed summary with hash for provenance tracking
///
/// Summary digests represent advisory semantic content (intent summaries,
/// verifier results) whose hashes are recorded for reproducibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryDigest {
    /// Unique identifier
    pub digest_id: String,
    /// Node that produced this summary
    pub source_node_id: String,
    /// What kind of summary this is
    pub kind: SummaryKind,
    /// SHA-256 hash of the summary content
    pub hash: [u8; 32],
    /// Byte length of original content
    pub original_byte_length: usize,
    /// The condensed summary text
    pub summary_text: String,
}

/// PSP-5 Phase 3: Context budget controlling node context assembly
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextBudget {
    /// Maximum total bytes for the context package
    pub byte_limit: usize,
    /// Maximum number of files to include
    pub file_count_limit: usize,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self {
            byte_limit: 100 * 1024, // 100KB default
            file_count_limit: 20,
        }
    }
}

/// PSP-5 Phase 3: Restriction map defining a node's context boundary
///
/// The restriction map bounds what a node can see. It is derived from the
/// task graph, ownership manifest, and parent scope. A node SHALL NOT receive
/// the full repository by default.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RestrictionMap {
    /// The node this restriction applies to
    pub node_id: String,
    /// Context budget (byte and file-count limits)
    #[serde(default)]
    pub budget: ContextBudget,
    /// Files the node owns and can see in full
    #[serde(default)]
    pub owned_files: Vec<String>,
    /// Adjacent sealed interfaces the node can reference
    #[serde(default)]
    pub sealed_interfaces: Vec<String>,
    /// Structural digests for external dependencies (preferred over raw files)
    #[serde(default)]
    pub structural_digests: Vec<StructuralDigest>,
    /// Summary digests for advisory context
    #[serde(default)]
    pub summary_digests: Vec<SummaryDigest>,
    /// Dependency commit hashes this node relies on
    #[serde(default)]
    pub dependency_commits: std::collections::HashMap<String, Vec<u8>>,
}

impl RestrictionMap {
    /// Create a restriction map for a node with default budget
    pub fn for_node(node_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            ..Default::default()
        }
    }

    /// Total structural bytes (approximation)
    pub fn structural_bytes(&self) -> usize {
        self.structural_digests
            .iter()
            .map(|d| d.source_path.len() + 64)
            .sum::<usize>()
            + self.sealed_interfaces.len() * 128
    }
}

/// PSP-5 Phase 3: Reproducible context package for node execution
///
/// A context package is the complete, bounded input assembled for a node's
/// LLM prompt. It records exactly what was included so the same context can
/// be reconstructed from the ledger and repository state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextPackage {
    /// Unique package identifier
    pub package_id: String,
    /// The node this context was assembled for
    pub node_id: String,
    /// The restriction map used
    pub restriction_map: RestrictionMap,
    /// Raw file contents included (path → content)
    #[serde(default)]
    pub included_files: std::collections::HashMap<String, String>,
    /// Structural digests included in this package
    #[serde(default)]
    pub structural_digests: Vec<StructuralDigest>,
    /// Summary digests included in this package
    #[serde(default)]
    pub summary_digests: Vec<SummaryDigest>,
    /// Total byte size of the assembled context
    pub total_bytes: usize,
    /// Whether budget was exceeded and content was trimmed
    pub budget_exceeded: bool,
    /// Timestamp of assembly
    pub created_at: i64,
}

impl ContextPackage {
    /// Create a new empty context package for a node
    pub fn new(node_id: impl Into<String>) -> Self {
        let nid = node_id.into();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        Self {
            package_id: format!("ctx_{}_{}", nid, ts),
            node_id: nid,
            created_at: ts,
            ..Default::default()
        }
    }

    /// Add a file to the context package, respecting budget
    pub fn add_file(&mut self, path: &str, content: String) -> bool {
        let new_bytes = self.total_bytes + content.len();
        if new_bytes > self.restriction_map.budget.byte_limit {
            self.budget_exceeded = true;
            return false;
        }
        if self.included_files.len() >= self.restriction_map.budget.file_count_limit {
            self.budget_exceeded = true;
            return false;
        }
        self.total_bytes = new_bytes;
        self.included_files.insert(path.to_string(), content);
        true
    }

    /// Add a structural digest (always fits, they're small)
    pub fn add_structural_digest(&mut self, digest: StructuralDigest) {
        self.structural_digests.push(digest);
    }

    /// Add a summary digest
    pub fn add_summary_digest(&mut self, digest: SummaryDigest) {
        self.total_bytes += digest.summary_text.len();
        self.summary_digests.push(digest);
    }

    /// Get the provenance record for this package
    pub fn provenance(&self) -> ContextProvenance {
        ContextProvenance {
            node_id: self.node_id.clone(),
            context_package_id: self.package_id.clone(),
            structural_digest_hashes: self
                .structural_digests
                .iter()
                .map(|d| (d.digest_id.clone(), d.hash))
                .collect(),
            summary_digest_hashes: self
                .summary_digests
                .iter()
                .map(|d| (d.digest_id.clone(), d.hash))
                .collect(),
            dependency_commit_hashes: self
                .restriction_map
                .dependency_commits
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            included_file_count: self.included_files.len(),
            total_bytes: self.total_bytes,
            created_at: self.created_at,
        }
    }
}

/// PSP-5 Phase 3: Provenance record tracking what context was used
///
/// Records the hashes of all summaries, contracts, and dependency commits
/// used to derive a node's prompt context. This enables reproducibility:
/// the same context package can be reconstructed from persisted state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextProvenance {
    /// Node this provenance belongs to
    pub node_id: String,
    /// Context package ID
    pub context_package_id: String,
    /// Structural digest ID → hash pairs used
    #[serde(default)]
    pub structural_digest_hashes: Vec<(String, [u8; 32])>,
    /// Summary digest ID → hash pairs used
    #[serde(default)]
    pub summary_digest_hashes: Vec<(String, [u8; 32])>,
    /// Dependency node → commit hash pairs
    #[serde(default)]
    pub dependency_commit_hashes: Vec<(String, Vec<u8>)>,
    /// Number of raw files included
    pub included_file_count: usize,
    /// Total bytes in context package
    pub total_bytes: usize,
    /// When this provenance was recorded
    pub created_at: i64,
}
