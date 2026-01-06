//! DuckDB Merkle Ledger
//!
//! Persistent storage for session history, commits, and Merkle proofs.

use anyhow::{Context, Result};
pub use perspt_store::{LlmRequestRecord, NodeStateRecord, SessionRecord, SessionStore};
use std::path::{Path, PathBuf};

/// Merkle commit record (Legacy wrapper for compatibility)
#[derive(Debug, Clone)]
pub struct MerkleCommit {
    pub commit_id: String,
    pub session_id: String,
    pub node_id: String,
    pub merkle_root: [u8; 32],
    pub parent_hash: Option<[u8; 32]>,
    pub timestamp: i64,
    pub energy: f32,
    pub stable: bool,
}

/// Session record (Legacy wrapper for compatibility)
#[derive(Debug, Clone)]
pub struct SessionRecordLegacy {
    pub session_id: String,
    pub task: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub status: String,
    pub total_nodes: usize,
    pub completed_nodes: usize,
}

/// Merkle Ledger using DuckDB for persistence
pub struct MerkleLedger {
    /// Session store from perspt-store
    store: SessionStore,
    /// Current session metadata (legacy cache)
    current_session: Option<SessionRecordLegacy>,
    /// Session artifact directory
    session_dir: Option<PathBuf>,
}

impl MerkleLedger {
    /// Create a new ledger (opens or creates database)
    pub fn new() -> Result<Self> {
        let store = SessionStore::new().context("Failed to initialize session store")?;
        Ok(Self {
            store,
            current_session: None,
            session_dir: None,
        })
    }

    /// Create an in-memory ledger (for testing)
    pub fn in_memory() -> Result<Self> {
        // Use a unique temp db for testing to avoid collisions
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join(format!("perspt_test_{}.db", uuid::Uuid::new_v4()));
        let store = SessionStore::open(&db_path)?;
        Ok(Self {
            store,
            current_session: None,
            session_dir: None,
        })
    }

    /// Start a new session
    pub fn start_session(&mut self, session_id: &str, task: &str, working_dir: &str) -> Result<()> {
        let record = SessionRecord {
            session_id: session_id.to_string(),
            task: task.to_string(),
            working_dir: working_dir.to_string(),
            merkle_root: None,
            detected_toolchain: None,
            status: "RUNNING".to_string(),
        };

        self.store.create_session(&record)?;

        // Create physical artifact directory
        let dir = self.store.create_session_dir(session_id)?;
        self.session_dir = Some(dir);

        let legacy_record = SessionRecordLegacy {
            session_id: session_id.to_string(),
            task: task.to_string(),
            started_at: chrono_timestamp(),
            ended_at: None,
            status: "RUNNING".to_string(),
            total_nodes: 0,
            completed_nodes: 0,
        };
        self.current_session = Some(legacy_record);

        log::info!("Started persistent session: {}", session_id);
        Ok(())
    }

    /// Record energy measurement
    pub fn record_energy(
        &self,
        node_id: &str,
        energy: &crate::types::EnergyComponents,
        total_energy: f32,
    ) -> Result<()> {
        let session_id = self
            .current_session
            .as_ref()
            .map(|s| s.session_id.clone())
            .context("No active session to record energy")?;

        let record = perspt_store::EnergyRecord {
            node_id: node_id.to_string(),
            session_id,
            v_syn: energy.v_syn,
            v_str: energy.v_str,
            v_log: energy.v_log,
            v_boot: energy.v_boot,
            v_sheaf: energy.v_sheaf,
            v_total: total_energy,
        };

        self.store.record_energy(&record)?;
        Ok(())
    }

    /// Commit a stable node state
    pub fn commit_node(
        &mut self,
        node_id: &str,
        merkle_root: [u8; 32],
        _parent_hash: Option<[u8; 32]>,
        energy: f32,
        state_json: String,
    ) -> Result<String> {
        let session_id = self
            .current_session
            .as_ref()
            .map(|s| s.session_id.clone())
            .context("No active session to commit")?;

        let commit_id = generate_commit_id();

        let record = NodeStateRecord {
            node_id: node_id.to_string(),
            session_id: session_id.clone(),
            state: state_json,
            v_total: energy,
            merkle_hash: Some(merkle_root.to_vec()),
            attempt_count: 1, // Placeholder
        };

        self.store.record_node_state(&record)?;
        self.store.update_merkle_root(&session_id, &merkle_root)?;

        log::info!("Committed node {} to store", node_id);

        // Update session progress
        if let Some(ref mut session) = self.current_session {
            session.completed_nodes += 1;
        }

        Ok(commit_id)
    }

    /// End the current session
    pub fn end_session(&mut self, status: &str) -> Result<()> {
        if let Some(ref mut session) = self.current_session {
            session.ended_at = Some(chrono_timestamp());
            session.status = status.to_string();
            log::info!(
                "Ended session {} with status: {}",
                session.session_id,
                status
            );
        }
        Ok(())
    }

    /// Get artifacts directory
    pub fn artifacts_dir(&self) -> Option<&Path> {
        self.session_dir.as_deref()
    }

    /// Get session statistics (legacy facade)
    pub fn get_stats(&self) -> LedgerStats {
        LedgerStats {
            total_sessions: 0, // Would query store.count_sessions()
            total_commits: 0,
            db_size_bytes: 0,
        }
    }

    /// Get the current merkle root (legacy facade)
    pub fn current_merkle_root(&self) -> [u8; 32] {
        [0u8; 32] // Placeholder
    }

    /// Record an LLM request/response for debugging and cost tracking
    pub fn record_llm_request(
        &self,
        model: &str,
        prompt: &str,
        response: &str,
        node_id: Option<&str>,
        latency_ms: i32,
    ) -> Result<()> {
        let session_id = self
            .current_session
            .as_ref()
            .map(|s| s.session_id.clone())
            .context("No active session to record LLM request")?;

        let record = LlmRequestRecord {
            session_id,
            node_id: node_id.map(|s| s.to_string()),
            model: model.to_string(),
            prompt: prompt.to_string(),
            response: response.to_string(),
            tokens_in: 0, // TODO: Extract from provider response if available
            tokens_out: 0,
            latency_ms,
        };

        self.store.record_llm_request(&record)?;
        log::debug!(
            "Recorded LLM request: model={}, prompt_len={}, response_len={}",
            model,
            prompt.len(),
            response.len()
        );
        Ok(())
    }

    /// Get access to the underlying store (for direct queries)
    pub fn store(&self) -> &SessionStore {
        &self.store
    }
}

/// Ledger statistics (Legacy)
#[derive(Debug, Clone)]
pub struct LedgerStats {
    pub total_sessions: usize,
    pub total_commits: usize,
    pub db_size_bytes: u64,
}

/// Generate a unique commit ID
fn generate_commit_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{:x}", now)
}

/// Get current timestamp
fn chrono_timestamp() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
