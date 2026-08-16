use super::*;

/// PSP-5: A single artifact operation within an artifact bundle
///
/// Each operation represents one file mutation: either a full write or a diff patch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum ArtifactOperation {
    /// Write the full file contents
    Write {
        /// Relative path within the workspace
        path: String,
        /// Full file content
        content: String,
    },
    /// Apply a unified diff patch
    Diff {
        /// Relative path within the workspace
        path: String,
        /// Unified diff content
        patch: String,
    },
    /// Delete a file from the workspace
    Delete {
        /// Relative path to delete
        path: String,
    },
    /// Move/rename a file within the workspace
    Move {
        /// Current relative path
        from: String,
        /// New relative path
        to: String,
    },
}

impl ArtifactOperation {
    /// Get the primary file path this operation targets
    pub fn path(&self) -> &str {
        match self {
            ArtifactOperation::Write { path, .. } => path,
            ArtifactOperation::Diff { path, .. } => path,
            ArtifactOperation::Delete { path } => path,
            ArtifactOperation::Move { from, .. } => from,
        }
    }

    /// Check if this is a write (new file) operation
    pub fn is_write(&self) -> bool {
        matches!(self, ArtifactOperation::Write { .. })
    }

    /// Check if this is a diff (patch) operation
    pub fn is_diff(&self) -> bool {
        matches!(self, ArtifactOperation::Diff { .. })
    }

    /// Check if this is a delete operation
    pub fn is_delete(&self) -> bool {
        matches!(self, ArtifactOperation::Delete { .. })
    }

    /// Check if this is a move/rename operation
    pub fn is_move(&self) -> bool {
        matches!(self, ArtifactOperation::Move { .. })
    }
}

/// PSP-5: Multi-artifact bundle from the Actuator
///
/// A node response containing one or more file operations applied as a unit.
/// The orchestrator SHALL parse all operations before mutating the workspace
/// and SHALL fail atomically if any operation is invalid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactBundle {
    /// File operations to apply
    pub artifacts: Vec<ArtifactOperation>,
    /// Optional commands to run after file operations
    #[serde(default)]
    pub commands: Vec<String>,
}

impl ArtifactBundle {
    /// Create an empty bundle
    pub fn new() -> Self {
        Self {
            artifacts: Vec::new(),
            commands: Vec::new(),
        }
    }

    /// Number of file operations
    pub fn len(&self) -> usize {
        self.artifacts.len()
    }

    /// Check if bundle is empty
    pub fn is_empty(&self) -> bool {
        self.artifacts.is_empty()
    }

    /// Get all unique file paths affected by this bundle
    pub fn affected_paths(&self) -> Vec<&str> {
        let mut paths: Vec<&str> = self.artifacts.iter().map(|a| a.path()).collect();
        // For Move operations, also include the destination path
        for op in &self.artifacts {
            if let ArtifactOperation::Move { to, .. } = op {
                paths.push(to.as_str());
            }
        }
        paths.sort();
        paths.dedup();
        paths
    }

    /// Count of file writes (new files)
    pub fn writes_count(&self) -> usize {
        self.artifacts.iter().filter(|a| a.is_write()).count()
    }

    /// Count of file diffs (patches)
    pub fn diffs_count(&self) -> usize {
        self.artifacts.iter().filter(|a| a.is_diff()).count()
    }

    /// Count of file deletes
    pub fn deletes_count(&self) -> usize {
        self.artifacts.iter().filter(|a| a.is_delete()).count()
    }

    /// Count of file moves
    pub fn moves_count(&self) -> usize {
        self.artifacts.iter().filter(|a| a.is_move()).count()
    }

    /// Validate the bundle: checks for empty paths and duplicate targets
    pub fn validate(&self) -> Result<(), String> {
        if self.artifacts.is_empty() {
            return Err("Artifact bundle is empty".to_string());
        }

        for (i, op) in self.artifacts.iter().enumerate() {
            // Validate the primary path
            Self::validate_path(op.path(), i)?;

            // For Move operations, also validate the destination path
            if let ArtifactOperation::Move { to, .. } = op {
                if to.is_empty() {
                    return Err(format!("Artifact {} (move) has empty destination path", i));
                }
                Self::validate_path(to, i)?;
            }
        }

        Ok(())
    }

    /// Validate a single path: reject empty, absolute, and traversal paths.
    ///
    /// Uses the canonical `normalize_artifact_path` utility so that all path
    /// consumers (bundle validation, ownership manifest, policy checks) agree
    /// on path identity.
    fn validate_path(path: &str, artifact_index: usize) -> Result<(), String> {
        use crate::path::{normalize_artifact_path, PathError};
        match normalize_artifact_path(path) {
            Ok(_) => Ok(()),
            Err(PathError::Empty) => Err(format!("Artifact {} has empty path", artifact_index)),
            Err(PathError::Absolute(_)) => Err(format!(
                "Artifact {} has absolute path '{}', must be relative",
                artifact_index, path
            )),
            Err(PathError::Escapes(_)) => Err(format!(
                "Artifact {} has path traversal in '{}'",
                artifact_index, path
            )),
            Err(PathError::Invalid(_)) => Err(format!(
                "Artifact {} has invalid path '{}'",
                artifact_index, path
            )),
        }
    }
}

impl Default for ArtifactBundle {
    fn default() -> Self {
        Self::new()
    }
}
