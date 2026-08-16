//! The workspace realizer (PSP-9 system 10, Paper I Definition 12.2).
//!
//! The gate is evaluated on the candidate workspace that would actually be
//! checkpointed, never on the model's account of it. `WorkspaceState` is a
//! cheap content-addressed handle — hashes and identifiers, never file
//! contents — because the kernel clones every attempt into its trace.
//!
//! The kernel's `srbn::Realizer` is **synchronous**, while Perspt's
//! realization re-reads an overlay on a tokio runtime. The split is
//! deliberate: [`snapshot_workspace`] does the I/O *first*, and
//! [`SnapshotRealizer`] is a pure sync adapter over already-materialized
//! snapshots, used by the `stabilize_realized` conformance oracle. The live
//! tool loop calls [`snapshot_workspace`] directly at its measure boundary.
//!
//! Coding claims no numeric `‖r_k‖`: the fraction of denied or rewritten
//! edits is [`ProjectionMismatch`] telemetry, recorded but never summed into
//! `V` and never labeled a realizability residual.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use perspt_sdk::srbn;
use sha2::{Digest, Sha256};

/// Content-addressed view of the files a node generation touched.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkspaceState {
    /// Workspace-relative path → SHA-256 of the file's bytes, or `None` for
    /// a path that does not exist (deleted or never created).
    pub files: BTreeMap<String, Option<String>>,
}

impl WorkspaceState {
    /// A stable digest of the whole state, for ledger heads and witnesses.
    pub fn root_hash(&self) -> String {
        let mut hasher = Sha256::new();
        for (path, hash) in &self.files {
            hasher.update(path.as_bytes());
            hasher.update([0u8]);
            hasher.update(hash.as_deref().unwrap_or("absent").as_bytes());
            hasher.update([0u8]);
        }
        hex_digest(hasher)
    }
}

/// Re-read the named paths from the overlay root and hash what is actually
/// on disk. This is the realization boundary: whatever the model claims,
/// measurement runs against this snapshot.
pub fn snapshot_workspace(root: &Path, paths: &[String]) -> Result<WorkspaceState> {
    let mut files = BTreeMap::new();
    for rel in paths {
        let absolute = root.join(rel);
        let hash = match std::fs::read(&absolute) {
            Ok(bytes) => {
                let mut hasher = Sha256::new();
                hasher.update(&bytes);
                Some(hex_digest(hasher))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e).with_context(|| format!("re-reading {rel:?}")),
        };
        files.insert(rel.clone(), hash);
    }
    Ok(WorkspaceState { files })
}

/// Projection telemetry (PSP-9 system 10): denied, stale, ambiguous, or
/// normalized edits. Operational telemetry about the projection — **not**
/// Paper I's `‖r_k‖`, which requires a proxy geometry coding does not have.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectionMismatch {
    pub denied_proposals: u32,
    pub stale_witnesses: u32,
    pub ambiguous_edits: u32,
    pub formatter_rewrites: u32,
    pub partial_applications: u32,
}

impl ProjectionMismatch {
    /// Total mismatch events this turn.
    pub fn total(&self) -> u32 {
        self.denied_proposals
            + self.stale_witnesses
            + self.ambiguous_edits
            + self.formatter_rewrites
            + self.partial_applications
    }
}

/// Sync adapter over pre-materialized snapshots for the kernel's
/// `stabilize_realized` conformance oracle. Coding realization is
/// unmeasured: the structural Definition 12.2 property holds (the barrier
/// sees the realized state), but no numeric residual is claimed.
pub struct SnapshotRealizer;

impl srbn::Realizer<WorkspaceState> for SnapshotRealizer {
    fn realize(
        &mut self,
        proposed: WorkspaceState,
    ) -> srbn::SrbnResult<srbn::Realization<WorkspaceState>> {
        // The proposed value is already a re-read snapshot; realization is
        // the identity here and deliberately unmeasured.
        Ok(srbn::Realization::unmeasured(proposed))
    }
}

fn hex_digest(hasher: Sha256) -> String {
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_reads_what_is_actually_on_disk() {
        let dir = std::env::temp_dir().join(format!("perspt_realize_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.rs"), "fn a() {}").unwrap();

        let state =
            snapshot_workspace(&dir, &["a.rs".to_string(), "missing.rs".to_string()]).unwrap();
        assert!(state.files["a.rs"].is_some());
        assert!(state.files["missing.rs"].is_none());

        // An adversarial model claiming an edit changes nothing: the re-read
        // snapshot is identical until the file actually changes (Gate V).
        let again =
            snapshot_workspace(&dir, &["a.rs".to_string(), "missing.rs".to_string()]).unwrap();
        assert_eq!(state.root_hash(), again.root_hash());

        std::fs::write(dir.join("a.rs"), "fn a() { changed() }").unwrap();
        let changed = snapshot_workspace(&dir, &["a.rs".to_string()]).unwrap();
        assert_ne!(state.files["a.rs"], changed.files["a.rs"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn root_hash_is_order_independent_by_construction() {
        let mut a = WorkspaceState::default();
        a.files.insert("x".into(), Some("h1".into()));
        a.files.insert("y".into(), None);
        let mut b = WorkspaceState::default();
        b.files.insert("y".into(), None);
        b.files.insert("x".into(), Some("h1".into()));
        assert_eq!(a.root_hash(), b.root_hash());
    }

    #[test]
    fn kernel_conformance_loop_runs_over_snapshots() {
        // The restore-best oracle: a barrier over snapshot handles, driven
        // through srbn::stabilize_realized with the sync adapter.
        let params = perspt_sdk::StabilityParameters::measured(0.5, 0.0);
        let initial = WorkspaceState::default();
        let barrier = |state: &WorkspaceState| {
            let v = state.files.len() as f64;
            perspt_sdk::AgentBarrierResult::new(state.files.is_empty(), v * v)
        };
        let updater = |mut state: WorkspaceState,
                       _b: &srbn::BarrierResult<
            perspt_sdk::CorrectionDirectionSet,
            perspt_sdk::Evidence,
        >| {
            state.files.pop_last();
            Ok(state)
        };
        let mut start = initial;
        for i in 0..3 {
            start.files.insert(format!("f{i}"), None);
        }
        let result =
            perspt_sdk::stabilize_realized(start, barrier, updater, SnapshotRealizer, &params, 8)
                .unwrap();
        assert_eq!(result.status, srbn::Status::Stable);
        assert!(result.max_realizability_residual().is_none(), "unmeasured");
    }
}
