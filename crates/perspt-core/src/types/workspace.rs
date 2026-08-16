use super::*;

// =============================================================================
// PSP-000005 Types — Project-First Execution Model
// =============================================================================

/// PSP-5: Execution mode for the runtime
///
/// Project mode is the default. Solo mode only activates on explicit single-file
/// intent keywords or via `--single-file` CLI flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Default: treat task as a multi-file project
    #[default]
    Project,
    /// Explicit single-file execution
    Solo,
}

impl std::fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionMode::Project => write!(f, "project"),
            ExecutionMode::Solo => write!(f, "solo"),
        }
    }
}

/// PSP-5: Workspace state classification
///
/// Determined at session start by inspecting the working directory for project
/// metadata and cross-referencing with the task description. Drives the
/// init/bootstrap/context strategy for the rest of the session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceState {
    /// Directory contains recognized project metadata (Cargo.toml, pyproject.toml, etc.)
    ExistingProject {
        /// Plugin names detected in the workspace
        plugins: Vec<String>,
    },
    /// Empty or non-project directory; language inferred from the task description
    Greenfield {
        /// Language inferred from task keywords (e.g. "rust", "python")
        inferred_lang: Option<String>,
    },
    /// Directory has files but no recognized project metadata and no language inferred
    #[default]
    Ambiguous,
}

impl std::fmt::Display for WorkspaceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkspaceState::ExistingProject { plugins } => {
                write!(f, "existing-project({})", plugins.join(", "))
            }
            WorkspaceState::Greenfield { inferred_lang } => {
                write!(
                    f,
                    "greenfield({})",
                    inferred_lang.as_deref().unwrap_or("unknown")
                )
            }
            WorkspaceState::Ambiguous => write!(f, "ambiguous"),
        }
    }
}

/// PSP-5: Node class distinguishing interface, implementation, and integration nodes
///
/// - **Interface** nodes define exported signatures, schemas, and verifier scope.
/// - **Implementation** nodes operate on node-owned files plus sealed interfaces.
/// - **Integration** nodes reconcile cross-owner or cross-plugin boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NodeClass {
    /// Defines exported signatures, schemas, ownership manifests
    Interface,
    /// Operates on node-owned files plus adjacent sealed interfaces
    #[default]
    Implementation,
    /// Reconciles cross-owner or cross-plugin boundaries
    Integration,
}

impl std::fmt::Display for NodeClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeClass::Interface => write!(f, "interface"),
            NodeClass::Implementation => write!(f, "implementation"),
            NodeClass::Integration => write!(f, "integration"),
        }
    }
}

/// PSP-5: Verifier strictness presets
///
/// Controls which verification stages are required for stability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VerifierStrictness {
    /// Default: compilation + tests required, warnings allowed
    #[default]
    Default,
    /// Strict: compilation + tests + linting (e.g. clippy -D warnings)
    Strict,
    /// Minimal: syntax/parse check only, no tests required
    Minimal,
}

// =============================================================================
// PSP-5 Phase 2: Ownership Manifests
// =============================================================================

/// PSP-5 Phase 2: A single ownership entry mapping a file to its owning node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipEntry {
    /// The node ID that owns this file
    pub owner_node_id: String,
    /// The language plugin responsible for this file
    pub owner_plugin: String,
    /// The node class of the owning node
    pub node_class: NodeClass,
}

/// PSP-5 Phase 2: Ownership manifest tracking file-to-node bindings
///
/// Enforces ownership closure: a node may only modify files it owns,
/// unless it is an Integration node (which may cross ownership boundaries).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipManifest {
    /// File path → ownership entry
    entries: std::collections::HashMap<String, OwnershipEntry>,
    /// Maximum files a single node may touch (bounded fanout)
    #[serde(default = "OwnershipManifest::default_fanout")]
    fanout_limit: usize,
}

impl Default for OwnershipManifest {
    fn default() -> Self {
        Self::new()
    }
}

impl OwnershipManifest {
    /// Create a new empty manifest with the default fanout limit
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            fanout_limit: Self::default_fanout(),
        }
    }

    /// Create with a custom fanout limit
    pub fn with_fanout_limit(limit: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            fanout_limit: limit,
        }
    }

    fn default_fanout() -> usize {
        20
    }

    /// Assign a file to an owning node.
    ///
    /// The path is normalized before insertion so that `src/main.rs` and
    /// `./src/main.rs` resolve to the same key.
    pub fn assign(
        &mut self,
        path: impl Into<String>,
        owner_node_id: impl Into<String>,
        owner_plugin: impl Into<String>,
        node_class: NodeClass,
    ) {
        let key = crate::path::normalize_path_key(&path.into()).unwrap_or_default();
        if key.is_empty() {
            return; // silently skip invalid paths
        }
        self.entries.insert(
            key,
            OwnershipEntry {
                owner_node_id: owner_node_id.into(),
                owner_plugin: owner_plugin.into(),
                node_class,
            },
        );
    }

    /// Look up the owner of a file path.
    ///
    /// The path is normalized before lookup.
    pub fn owner_of(&self, path: &str) -> Option<&OwnershipEntry> {
        let key = crate::path::normalize_path_key(path)?;
        self.entries.get(&key)
    }

    /// List all files owned by a specific node
    pub fn files_owned_by(&self, node_id: &str) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|(_, entry)| entry.owner_node_id == node_id)
            .map(|(path, _)| path.as_str())
            .collect()
    }

    /// Get the total number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the manifest is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the fanout limit
    pub fn fanout_limit(&self) -> usize {
        self.fanout_limit
    }

    /// Validate that a bundle respects ownership boundaries
    ///
    /// Rules:
    /// - **Implementation** nodes: all paths must be owned by this node
    /// - **Interface** nodes: all paths must be owned by this node
    /// - **Integration** nodes: paths may cross ownership boundaries
    /// - Fanout limit: bundle must not exceed max files per node
    /// - Unregistered paths (new files) are allowed and will be auto-assigned
    pub fn validate_bundle(
        &self,
        bundle: &ArtifactBundle,
        node_id: &str,
        node_class: NodeClass,
    ) -> Result<(), String> {
        let artifact_count = bundle.len();

        // Check fanout limit
        if artifact_count > self.fanout_limit {
            return Err(format!(
                "Bundle has {} artifacts, exceeding fanout limit of {}",
                artifact_count, self.fanout_limit
            ));
        }

        // Integration nodes can cross ownership boundaries
        if node_class == NodeClass::Integration {
            return Ok(());
        }

        // For Interface and Implementation nodes, check ownership
        for op in &bundle.artifacts {
            let raw_path = op.path();
            let key =
                crate::path::normalize_path_key(raw_path).unwrap_or_else(|| raw_path.to_string());
            if let Some(entry) = self.entries.get(&key) {
                if entry.owner_node_id != node_id {
                    return Err(format!(
                        "Ownership violation: file '{}' is owned by node '{}', \
                         but node '{}' ({}) attempted to modify it. \
                         Only Integration nodes may cross ownership boundaries.",
                        raw_path, entry.owner_node_id, node_id, node_class
                    ));
                }
            }
            // Unregistered paths (new files) are allowed — they'll be assigned to this node
        }

        Ok(())
    }

    /// Auto-assign unregistered paths from a bundle to a node
    ///
    /// Called after validate_bundle succeeds, this registers any new paths
    /// in the manifest so future nodes can't claim them.
    pub fn assign_new_paths(
        &mut self,
        bundle: &ArtifactBundle,
        node_id: &str,
        owner_plugin: &str,
        node_class: NodeClass,
    ) {
        for op in &bundle.artifacts {
            let raw_path = op.path();
            let key =
                crate::path::normalize_path_key(raw_path).unwrap_or_else(|| raw_path.to_string());
            if !self.entries.contains_key(&key) {
                self.assign(raw_path, node_id, owner_plugin, node_class);
            }
        }
    }
}
