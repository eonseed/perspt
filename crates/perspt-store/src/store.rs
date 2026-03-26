//! Session Store Implementation
//!
//! Provides CRUD operations for SRBN sessions, node states, and energy history.

use anyhow::{Context, Result};
use duckdb::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

use crate::schema::init_schema;

/// Record for a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: String,
    pub task: String,
    pub working_dir: String,
    pub merkle_root: Option<Vec<u8>>,
    pub detected_toolchain: Option<String>,
    pub status: String,
}

/// Record for node state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStateRecord {
    pub node_id: String,
    pub session_id: String,
    pub state: String,
    pub v_total: f32,
    pub merkle_hash: Option<Vec<u8>>,
    pub attempt_count: i32,
    // PSP-5 Phase 8: Richer node snapshot for resume reconstruction
    pub node_class: Option<String>,
    pub owner_plugin: Option<String>,
    pub goal: Option<String>,
    pub parent_id: Option<String>,
    /// JSON-serialized Vec<String>
    pub children: Option<String>,
    pub last_error_type: Option<String>,
    pub committed_at: Option<String>,
}

/// Record for energy history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyRecord {
    pub node_id: String,
    pub session_id: String,
    pub v_syn: f32,
    pub v_str: f32,
    pub v_log: f32,
    pub v_boot: f32,
    pub v_sheaf: f32,
    pub v_total: f32,
}

/// Record for LLM request/response logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequestRecord {
    pub session_id: String,
    pub node_id: Option<String>,
    pub model: String,
    pub prompt: String,
    pub response: String,
    pub tokens_in: i32,
    pub tokens_out: i32,
    pub latency_ms: i32,
}

/// PSP-5 Phase 3: Record for structural digest persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralDigestRecord {
    pub digest_id: String,
    pub session_id: String,
    pub node_id: String,
    pub source_path: String,
    pub artifact_kind: String,
    pub hash: Vec<u8>,
    pub version: i32,
}

/// PSP-5 Phase 3: Record for context provenance persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextProvenanceRecord {
    pub session_id: String,
    pub node_id: String,
    pub context_package_id: String,
    /// JSON-serialized structural digest hashes
    pub structural_hashes: String,
    /// JSON-serialized summary hashes
    pub summary_hashes: String,
    /// JSON-serialized dependency commit hashes
    pub dependency_hashes: String,
    pub included_file_count: i32,
    pub total_bytes: i32,
}

/// PSP-5 Phase 5: Record for escalation report persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationReportRecord {
    pub session_id: String,
    pub node_id: String,
    /// Serialized EscalationCategory
    pub category: String,
    /// JSON-serialized RewriteAction
    pub action: String,
    /// JSON-serialized EnergyComponents
    pub energy_snapshot: String,
    /// JSON-serialized Vec<StageOutcome>
    pub stage_outcomes: String,
    /// Human-readable evidence
    pub evidence: String,
    /// JSON-serialized Vec<String>
    pub affected_node_ids: String,
}

/// PSP-5 Phase 5: Record for local graph rewrite persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewriteRecordRow {
    pub session_id: String,
    pub node_id: String,
    /// JSON-serialized RewriteAction
    pub action: String,
    /// Serialized EscalationCategory
    pub category: String,
    /// JSON-serialized Vec<String>
    pub requeued_nodes: String,
    /// JSON-serialized Vec<String>
    pub inserted_nodes: String,
}

/// PSP-5 Phase 5: Record for sheaf validation result persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheafValidationRow {
    pub session_id: String,
    pub node_id: String,
    pub validator_class: String,
    pub plugin_source: Option<String>,
    pub passed: bool,
    pub evidence_summary: String,
    /// JSON-serialized Vec<String>
    pub affected_files: String,
    pub v_sheaf_contribution: f32,
    /// JSON-serialized Vec<String>
    pub requeue_targets: String,
}

// =============================================================================
// PSP-5 Phase 6: Provisional Branch, Interface Seal, Branch Flush Records
// =============================================================================

/// PSP-5 Phase 6: Record for provisional branch persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionalBranchRow {
    pub branch_id: String,
    pub session_id: String,
    pub node_id: String,
    pub parent_node_id: String,
    pub state: String,
    pub parent_seal_hash: Option<Vec<u8>>,
    pub sandbox_dir: Option<String>,
}

/// PSP-5 Phase 6: Record for branch lineage persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchLineageRow {
    pub lineage_id: String,
    pub parent_branch_id: String,
    pub child_branch_id: String,
    pub depends_on_seal: bool,
}

/// PSP-5 Phase 6: Record for interface seal persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceSealRow {
    pub seal_id: String,
    pub session_id: String,
    pub node_id: String,
    pub sealed_path: String,
    pub artifact_kind: String,
    pub seal_hash: Vec<u8>,
    pub version: i32,
}

/// PSP-5 Phase 6: Record for branch flush decision persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchFlushRow {
    pub flush_id: String,
    pub session_id: String,
    pub parent_node_id: String,
    /// JSON-serialized Vec<String>
    pub flushed_branch_ids: String,
    /// JSON-serialized Vec<String>
    pub requeue_node_ids: String,
    pub reason: String,
}

// =============================================================================
// PSP-5 Phase 8: Task Graph and Review Outcome Records
// =============================================================================

/// PSP-5 Phase 8: Record for task graph edges (DAG reconstruction on resume)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraphEdgeRow {
    pub session_id: String,
    pub parent_node_id: String,
    pub child_node_id: String,
    pub edge_type: String,
}

/// PSP-5 Phase 8: Record for review outcome persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewOutcomeRow {
    pub session_id: String,
    pub node_id: String,
    /// One of: "approved", "rejected", "edit_requested", "correction_requested", "skipped"
    pub outcome: String,
    pub reviewer_note: Option<String>,
    /// Energy at time of review decision
    pub energy_at_review: Option<f64>,
    /// Whether verification was degraded when decision was made
    pub degraded: Option<bool>,
    /// Escalation category if the node had been classified
    pub escalation_category: Option<String>,
}

/// PSP-5 Phase 8: Record for verification result snapshot persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResultRow {
    pub session_id: String,
    pub node_id: String,
    /// JSON-serialized VerificationResult (full data for resume reconstruction)
    pub result_json: String,
    // Query-friendly summary fields
    pub syntax_ok: bool,
    pub build_ok: bool,
    pub tests_ok: bool,
    pub lint_ok: bool,
    pub diagnostics_count: i32,
    pub tests_passed: i32,
    pub tests_failed: i32,
    pub degraded: bool,
    pub degraded_reason: Option<String>,
}

/// PSP-5 Phase 8: Record for artifact bundle snapshot persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactBundleRow {
    pub session_id: String,
    pub node_id: String,
    /// JSON-serialized ArtifactBundle (full data for resume reconstruction)
    pub bundle_json: String,
    pub artifact_count: i32,
    pub command_count: i32,
    /// JSON-serialized Vec<String> of touched file paths
    pub touched_files: String,
}

use std::sync::Mutex;

/// Session store for SRBN persistence
pub struct SessionStore {
    conn: Mutex<Connection>,
}

impl SessionStore {
    /// Create a new session store with default path
    pub fn new() -> Result<Self> {
        let db_path = Self::default_db_path()?;
        Self::open(&db_path)
    }

    /// Open a session store at the given path
    pub fn open(path: &PathBuf) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path).context("Failed to open DuckDB")?;
        init_schema(&conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Get the default database path (~/.local/share/perspt/perspt.db or similar)
    pub fn default_db_path() -> Result<PathBuf> {
        let data_dir = dirs::data_local_dir()
            .context("Could not find local data directory")?
            .join("perspt");
        Ok(data_dir.join("perspt.db"))
    }

    /// Create a new session
    pub fn create_session(&self, session: &SessionRecord) -> Result<()> {
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO sessions (session_id, task, working_dir, merkle_root, detected_toolchain, status)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            [
                &session.session_id,
                &session.task,
                &session.working_dir,
                &session.merkle_root.as_ref().map(hex::encode).unwrap_or_default(),
                &session.detected_toolchain.clone().unwrap_or_default(),
                &session.status,
            ],
        )?;
        Ok(())
    }

    /// Update session merkle root
    pub fn update_merkle_root(&self, session_id: &str, merkle_root: &[u8]) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE sessions SET merkle_root = ?, updated_at = CURRENT_TIMESTAMP WHERE session_id = ?",
            [hex::encode(merkle_root), session_id.to_string()],
        )?;
        Ok(())
    }

    /// Record node state
    pub fn record_node_state(&self, record: &NodeStateRecord) -> Result<()> {
        let v_total = record.v_total.to_string();
        let merkle_hash = record
            .merkle_hash
            .as_ref()
            .map(hex::encode)
            .unwrap_or_default();
        let attempt_count = record.attempt_count.to_string();
        let node_class = record.node_class.clone().unwrap_or_default();
        let owner_plugin = record.owner_plugin.clone().unwrap_or_default();
        let goal = record.goal.clone().unwrap_or_default();
        let parent_id = record.parent_id.clone().unwrap_or_default();
        let children = record.children.clone().unwrap_or_default();
        let last_error_type = record.last_error_type.clone().unwrap_or_default();
        let committed_at = record.committed_at.clone().unwrap_or_default();

        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO node_states (node_id, session_id, state, v_total, merkle_hash, attempt_count,
                                     node_class, owner_plugin, goal, parent_id, children, last_error_type, committed_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            [
                &record.node_id,
                &record.session_id,
                &record.state,
                &v_total,
                &merkle_hash,
                &attempt_count,
                &node_class,
                &owner_plugin,
                &goal,
                &parent_id,
                &children,
                &last_error_type,
                &committed_at,
            ],
        )?;
        Ok(())
    }

    /// Record energy measurement
    pub fn record_energy(&self, record: &EnergyRecord) -> Result<()> {
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO energy_history (node_id, session_id, v_syn, v_str, v_log, v_boot, v_sheaf, v_total)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            [
                &record.node_id,
                &record.session_id,
                &record.v_syn.to_string(),
                &record.v_str.to_string(),
                &record.v_log.to_string(),
                &record.v_boot.to_string(),
                &record.v_sheaf.to_string(),
                &record.v_total.to_string(),
            ],
        )?;
        Ok(())
    }

    /// Calculate Merkle hash for content
    pub fn calculate_hash(content: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(content);
        hasher.finalize().to_vec()
    }

    /// Get session by ID
    pub fn get_session(&self, session_id: &str) -> Result<Option<SessionRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, task, working_dir, merkle_root, detected_toolchain, status FROM sessions WHERE session_id = ?"
        )?;

        let mut rows = stmt.query([session_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(SessionRecord {
                session_id: row.get(0)?,
                task: row.get(1)?,
                working_dir: row.get(2)?,
                merkle_root: row
                    .get::<_, Option<String>>(3)?
                    .and_then(|s| hex::decode(s).ok()),
                detected_toolchain: row.get(4)?,
                status: row.get(5)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get the directory for session artifacts (~/.local/share/perspt/sessions/<id>)
    pub fn get_session_dir(&self, session_id: &str) -> Result<PathBuf> {
        let data_dir = dirs::data_local_dir()
            .context("Could not find local data directory")?
            .join("perspt")
            .join("sessions")
            .join(session_id);
        Ok(data_dir)
    }

    /// Ensure a session directory exists and return the path
    pub fn create_session_dir(&self, session_id: &str) -> Result<PathBuf> {
        let dir = self.get_session_dir(session_id)?;
        if !dir.exists() {
            std::fs::create_dir_all(&dir).context("Failed to create session directory")?;
        }
        Ok(dir)
    }

    /// Get energy history for a node (query)
    pub fn get_energy_history(&self, session_id: &str, node_id: &str) -> Result<Vec<EnergyRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT node_id, session_id, v_syn, v_str, v_log, v_boot, v_sheaf, v_total FROM energy_history WHERE session_id = ? AND node_id = ? ORDER BY timestamp"
        )?;

        let mut rows = stmt.query([session_id, node_id])?;
        let mut records = Vec::new();

        while let Some(row) = rows.next()? {
            records.push(EnergyRecord {
                node_id: row.get(0)?,
                session_id: row.get(1)?,
                v_syn: row.get::<_, f64>(2)? as f32,
                v_str: row.get::<_, f64>(3)? as f32,
                v_log: row.get::<_, f64>(4)? as f32,
                v_boot: row.get::<_, f64>(5)? as f32,
                v_sheaf: row.get::<_, f64>(6)? as f32,
                v_total: row.get::<_, f64>(7)? as f32,
            });
        }

        Ok(records)
    }

    /// List recent sessions (newest first)
    pub fn list_recent_sessions(&self, limit: usize) -> Result<Vec<SessionRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, task, working_dir, merkle_root, detected_toolchain, status
             FROM sessions ORDER BY created_at DESC LIMIT ?",
        )?;

        let mut rows = stmt.query([limit.to_string()])?;
        let mut records = Vec::new();

        while let Some(row) = rows.next()? {
            // merkle_root is stored as BLOB, read it directly as Option<Vec<u8>>
            let merkle_root: Option<Vec<u8>> = row.get(3).ok();

            records.push(SessionRecord {
                session_id: row.get(0)?,
                task: row.get(1)?,
                working_dir: row.get(2)?,
                merkle_root,
                detected_toolchain: row.get(4)?,
                status: row.get(5)?,
            });
        }

        Ok(records)
    }

    /// Get all node states for a session
    pub fn get_node_states(&self, session_id: &str) -> Result<Vec<NodeStateRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT node_id, session_id, state, v_total, CAST(merkle_hash AS VARCHAR), attempt_count, \
                    node_class, owner_plugin, goal, parent_id, children, last_error_type, committed_at \
             FROM node_states WHERE session_id = ? ORDER BY created_at",
        )?;

        let mut rows = stmt.query([session_id])?;
        let mut records = Vec::new();

        while let Some(row) = rows.next()? {
            records.push(NodeStateRecord {
                node_id: row.get(0)?,
                session_id: row.get(1)?,
                state: row.get(2)?,
                v_total: row.get::<_, f64>(3)? as f32,
                merkle_hash: row
                    .get::<_, Option<String>>(4)?
                    .and_then(|s| hex::decode(s).ok()),
                attempt_count: row.get(5)?,
                node_class: row.get::<_, Option<String>>(6)?.filter(|s| !s.is_empty()),
                owner_plugin: row.get::<_, Option<String>>(7)?.filter(|s| !s.is_empty()),
                goal: row.get::<_, Option<String>>(8)?.filter(|s| !s.is_empty()),
                parent_id: row.get::<_, Option<String>>(9)?.filter(|s| !s.is_empty()),
                children: row.get::<_, Option<String>>(10)?.filter(|s| !s.is_empty()),
                last_error_type: row.get::<_, Option<String>>(11)?.filter(|s| !s.is_empty()),
                committed_at: row.get::<_, Option<String>>(12)?.filter(|s| !s.is_empty()),
            });
        }

        Ok(records)
    }

    /// Update session status
    pub fn update_session_status(&self, session_id: &str, status: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE sessions SET status = ?, updated_at = CURRENT_TIMESTAMP WHERE session_id = ?",
            [status, session_id],
        )?;
        Ok(())
    }

    /// Record an LLM request/response
    pub fn record_llm_request(&self, record: &LlmRequestRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO llm_requests (session_id, node_id, model, prompt, response, tokens_in, tokens_out, latency_ms)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            [
                &record.session_id,
                &record.node_id.clone().unwrap_or_default(),
                &record.model,
                &record.prompt,
                &record.response,
                &record.tokens_in.to_string(),
                &record.tokens_out.to_string(),
                &record.latency_ms.to_string(),
            ],
        )?;
        Ok(())
    }

    /// Get LLM requests for a session
    pub fn get_llm_requests(&self, session_id: &str) -> Result<Vec<LlmRequestRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, node_id, model, prompt, response, tokens_in, tokens_out, latency_ms
             FROM llm_requests WHERE session_id = ? ORDER BY timestamp",
        )?;

        let mut rows = stmt.query([session_id])?;
        let mut records = Vec::new();

        while let Some(row) = rows.next()? {
            let node_id: Option<String> = row.get(1)?;
            records.push(LlmRequestRecord {
                session_id: row.get(0)?,
                node_id: if node_id.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                    None
                } else {
                    node_id
                },
                model: row.get(2)?,
                prompt: row.get(3)?,
                response: row.get(4)?,
                tokens_in: row.get(5)?,
                tokens_out: row.get(6)?,
                latency_ms: row.get(7)?,
            });
        }

        Ok(records)
    }

    /// Count all LLM requests in the database (for debugging)
    pub fn count_all_llm_requests(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM llm_requests")?;
        let count: i64 = stmt.query_row([], |row| row.get(0))?;
        Ok(count)
    }

    /// Get all LLM requests (for debugging)
    pub fn get_all_llm_requests(&self, limit: usize) -> Result<Vec<LlmRequestRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, node_id, model, prompt, response, tokens_in, tokens_out, latency_ms
             FROM llm_requests ORDER BY timestamp DESC LIMIT ?",
        )?;

        let mut rows = stmt.query([limit as i64])?;
        let mut records = Vec::new();

        while let Some(row) = rows.next()? {
            let node_id: Option<String> = row.get(1)?;
            records.push(LlmRequestRecord {
                session_id: row.get(0)?,
                node_id: if node_id.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                    None
                } else {
                    node_id
                },
                model: row.get(2)?,
                prompt: row.get(3)?,
                response: row.get(4)?,
                tokens_in: row.get(5)?,
                tokens_out: row.get(6)?,
                latency_ms: row.get(7)?,
            });
        }

        Ok(records)
    }

    // =========================================================================
    // PSP-5 Phase 3: Structural Digest & Context Provenance Persistence
    // =========================================================================

    /// Record a structural digest
    pub fn record_structural_digest(&self, record: &StructuralDigestRecord) -> Result<()> {
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO structural_digests (digest_id, session_id, node_id, source_path, artifact_kind, hash, version)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
            [
                &record.digest_id,
                &record.session_id,
                &record.node_id,
                &record.source_path,
                &record.artifact_kind,
                &hex::encode(&record.hash),
                &record.version.to_string(),
            ],
        )?;
        Ok(())
    }

    /// Get structural digests for a session and node
    pub fn get_structural_digests(
        &self,
        session_id: &str,
        node_id: &str,
    ) -> Result<Vec<StructuralDigestRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT digest_id, session_id, node_id, source_path, artifact_kind, hash, version
             FROM structural_digests WHERE session_id = ? AND node_id = ? ORDER BY created_at",
        )?;

        let mut rows = stmt.query([session_id, node_id])?;
        let mut records = Vec::new();

        while let Some(row) = rows.next()? {
            records.push(StructuralDigestRecord {
                digest_id: row.get(0)?,
                session_id: row.get(1)?,
                node_id: row.get(2)?,
                source_path: row.get(3)?,
                artifact_kind: row.get(4)?,
                hash: row
                    .get::<_, String>(5)
                    .ok()
                    .and_then(|s| hex::decode(s).ok())
                    .unwrap_or_default(),
                version: row.get(5)?,
            });
        }

        Ok(records)
    }

    /// Record context provenance for a node
    pub fn record_context_provenance(&self, record: &ContextProvenanceRecord) -> Result<()> {
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO context_provenance (session_id, node_id, context_package_id, structural_hashes, summary_hashes, dependency_hashes, included_file_count, total_bytes)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            [
                &record.session_id,
                &record.node_id,
                &record.context_package_id,
                &record.structural_hashes,
                &record.summary_hashes,
                &record.dependency_hashes,
                &record.included_file_count.to_string(),
                &record.total_bytes.to_string(),
            ],
        )?;
        Ok(())
    }

    /// Get context provenance for a session and node
    pub fn get_context_provenance(
        &self,
        session_id: &str,
        node_id: &str,
    ) -> Result<Option<ContextProvenanceRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, node_id, context_package_id, structural_hashes, summary_hashes, dependency_hashes, included_file_count, total_bytes
             FROM context_provenance WHERE session_id = ? AND node_id = ? ORDER BY created_at DESC LIMIT 1",
        )?;

        let mut rows = stmt.query([session_id, node_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(ContextProvenanceRecord {
                session_id: row.get(0)?,
                node_id: row.get(1)?,
                context_package_id: row.get(2)?,
                structural_hashes: row.get(3)?,
                summary_hashes: row.get(4)?,
                dependency_hashes: row.get(5)?,
                included_file_count: row.get(6)?,
                total_bytes: row.get(7)?,
            }))
        } else {
            Ok(None)
        }
    }

    // =========================================================================
    // PSP-5 Phase 5: Escalation, Rewrite, and Sheaf Validation Persistence
    // =========================================================================

    /// Record an escalation report
    pub fn record_escalation_report(&self, record: &EscalationReportRecord) -> Result<()> {
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO escalation_reports (session_id, node_id, category, action, energy_snapshot, stage_outcomes, evidence, affected_node_ids)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            [
                &record.session_id,
                &record.node_id,
                &record.category,
                &record.action,
                &record.energy_snapshot,
                &record.stage_outcomes,
                &record.evidence,
                &record.affected_node_ids,
            ],
        )?;
        Ok(())
    }

    /// Get escalation reports for a session
    pub fn get_escalation_reports(&self, session_id: &str) -> Result<Vec<EscalationReportRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, node_id, category, action, energy_snapshot, stage_outcomes, evidence, affected_node_ids
             FROM escalation_reports WHERE session_id = ? ORDER BY created_at",
        )?;
        let mut rows = stmt.query([session_id])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            records.push(EscalationReportRecord {
                session_id: row.get(0)?,
                node_id: row.get(1)?,
                category: row.get(2)?,
                action: row.get(3)?,
                energy_snapshot: row.get(4)?,
                stage_outcomes: row.get(5)?,
                evidence: row.get(6)?,
                affected_node_ids: row.get(7)?,
            });
        }
        Ok(records)
    }

    /// Record a local graph rewrite
    pub fn record_rewrite(&self, record: &RewriteRecordRow) -> Result<()> {
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO rewrite_records (session_id, node_id, action, category, requeued_nodes, inserted_nodes)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            [
                &record.session_id,
                &record.node_id,
                &record.action,
                &record.category,
                &record.requeued_nodes,
                &record.inserted_nodes,
            ],
        )?;
        Ok(())
    }

    /// Get rewrite records for a session
    pub fn get_rewrite_records(&self, session_id: &str) -> Result<Vec<RewriteRecordRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, node_id, action, category, requeued_nodes, inserted_nodes
             FROM rewrite_records WHERE session_id = ? ORDER BY created_at",
        )?;
        let mut rows = stmt.query([session_id])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            records.push(RewriteRecordRow {
                session_id: row.get(0)?,
                node_id: row.get(1)?,
                action: row.get(2)?,
                category: row.get(3)?,
                requeued_nodes: row.get(4)?,
                inserted_nodes: row.get(5)?,
            });
        }
        Ok(records)
    }

    /// Record a sheaf validation result
    pub fn record_sheaf_validation(&self, record: &SheafValidationRow) -> Result<()> {
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO sheaf_validations (session_id, node_id, validator_class, plugin_source, passed, evidence_summary, affected_files, v_sheaf_contribution, requeue_targets)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            [
                &record.session_id,
                &record.node_id,
                &record.validator_class,
                &record.plugin_source.clone().unwrap_or_default(),
                &record.passed.to_string(),
                &record.evidence_summary,
                &record.affected_files,
                &record.v_sheaf_contribution.to_string(),
                &record.requeue_targets,
            ],
        )?;
        Ok(())
    }

    /// Get sheaf validation results for a session and node
    pub fn get_sheaf_validations(
        &self,
        session_id: &str,
        node_id: &str,
    ) -> Result<Vec<SheafValidationRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, node_id, validator_class, plugin_source, passed, evidence_summary, affected_files, v_sheaf_contribution, requeue_targets
             FROM sheaf_validations WHERE session_id = ? AND node_id = ? ORDER BY created_at",
        )?;
        let mut rows = stmt.query([session_id, node_id])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            records.push(SheafValidationRow {
                session_id: row.get(0)?,
                node_id: row.get(1)?,
                validator_class: row.get(2)?,
                plugin_source: row.get::<_, Option<String>>(3)?,
                passed: row.get::<_, String>(4)?.parse().unwrap_or(false),
                evidence_summary: row.get(5)?,
                affected_files: row.get(6)?,
                v_sheaf_contribution: row.get::<_, f64>(7)? as f32,
                requeue_targets: row.get(8)?,
            });
        }
        Ok(records)
    }

    // =========================================================================
    // PSP-5 Phase 6: Provisional Branch CRUD
    // =========================================================================

    /// Record a new provisional branch
    pub fn record_provisional_branch(&self, record: &ProvisionalBranchRow) -> Result<()> {
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO provisional_branches (branch_id, session_id, node_id, parent_node_id, state, parent_seal_hash, sandbox_dir)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
            [
                &record.branch_id,
                &record.session_id,
                &record.node_id,
                &record.parent_node_id,
                &record.state,
                &record.parent_seal_hash.as_ref().map(hex::encode).unwrap_or_default(),
                &record.sandbox_dir.clone().unwrap_or_default(),
            ],
        )?;
        Ok(())
    }

    /// Update a provisional branch state
    pub fn update_branch_state(&self, branch_id: &str, new_state: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE provisional_branches SET state = ?, updated_at = CURRENT_TIMESTAMP WHERE branch_id = ?",
            [new_state, branch_id],
        )?;
        Ok(())
    }

    /// Get all provisional branches for a session
    pub fn get_provisional_branches(&self, session_id: &str) -> Result<Vec<ProvisionalBranchRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT branch_id, session_id, node_id, parent_node_id, state, parent_seal_hash, sandbox_dir
             FROM provisional_branches WHERE session_id = ? ORDER BY created_at",
        )?;
        let mut rows = stmt.query([session_id])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            records.push(ProvisionalBranchRow {
                branch_id: row.get(0)?,
                session_id: row.get(1)?,
                node_id: row.get(2)?,
                parent_node_id: row.get(3)?,
                state: row.get(4)?,
                parent_seal_hash: row
                    .get::<_, Option<String>>(5)?
                    .and_then(|h| hex::decode(h).ok()),
                sandbox_dir: row.get::<_, Option<String>>(6)?,
            });
        }
        Ok(records)
    }

    /// Get live (active/sealed) provisional branches depending on a parent node
    pub fn get_live_branches_for_parent(
        &self,
        session_id: &str,
        parent_node_id: &str,
    ) -> Result<Vec<ProvisionalBranchRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT branch_id, session_id, node_id, parent_node_id, state, parent_seal_hash, sandbox_dir
             FROM provisional_branches
             WHERE session_id = ? AND parent_node_id = ? AND state IN ('active', 'sealed')
             ORDER BY created_at",
        )?;
        let mut rows = stmt.query([session_id, parent_node_id])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            records.push(ProvisionalBranchRow {
                branch_id: row.get(0)?,
                session_id: row.get(1)?,
                node_id: row.get(2)?,
                parent_node_id: row.get(3)?,
                state: row.get(4)?,
                parent_seal_hash: row
                    .get::<_, Option<String>>(5)?
                    .and_then(|h| hex::decode(h).ok()),
                sandbox_dir: row.get::<_, Option<String>>(6)?,
            });
        }
        Ok(records)
    }

    /// Mark all live branches for a parent as flushed
    pub fn flush_branches_for_parent(
        &self,
        session_id: &str,
        parent_node_id: &str,
    ) -> Result<Vec<String>> {
        let live = self.get_live_branches_for_parent(session_id, parent_node_id)?;
        let branch_ids: Vec<String> = live.iter().map(|b| b.branch_id.clone()).collect();
        for bid in &branch_ids {
            self.update_branch_state(bid, "flushed")?;
        }
        Ok(branch_ids)
    }

    // =========================================================================
    // PSP-5 Phase 6: Branch Lineage CRUD
    // =========================================================================

    /// Record a branch lineage edge
    pub fn record_branch_lineage(&self, record: &BranchLineageRow) -> Result<()> {
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO branch_lineage (lineage_id, parent_branch_id, child_branch_id, depends_on_seal)
            VALUES (?, ?, ?, ?)
            "#,
            [
                &record.lineage_id,
                &record.parent_branch_id,
                &record.child_branch_id,
                &record.depends_on_seal.to_string(),
            ],
        )?;
        Ok(())
    }

    /// Get child branch IDs for a parent branch
    pub fn get_child_branches(&self, parent_branch_id: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT child_branch_id FROM branch_lineage WHERE parent_branch_id = ?")?;
        let mut rows = stmt.query([parent_branch_id])?;
        let mut ids = Vec::new();
        while let Some(row) = rows.next()? {
            ids.push(row.get(0)?);
        }
        Ok(ids)
    }

    // =========================================================================
    // PSP-5 Phase 6: Interface Seal CRUD
    // =========================================================================

    /// Record an interface seal
    pub fn record_interface_seal(&self, record: &InterfaceSealRow) -> Result<()> {
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO interface_seals (seal_id, session_id, node_id, sealed_path, artifact_kind, seal_hash, version)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
            [
                &record.seal_id,
                &record.session_id,
                &record.node_id,
                &record.sealed_path,
                &record.artifact_kind,
                &hex::encode(&record.seal_hash),
                &record.version.to_string(),
            ],
        )?;
        Ok(())
    }

    /// Get all interface seals for a node
    pub fn get_interface_seals(
        &self,
        session_id: &str,
        node_id: &str,
    ) -> Result<Vec<InterfaceSealRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT seal_id, session_id, node_id, sealed_path, artifact_kind, seal_hash, version
             FROM interface_seals WHERE session_id = ? AND node_id = ? ORDER BY created_at",
        )?;
        let mut rows = stmt.query([session_id, node_id])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            records.push(InterfaceSealRow {
                seal_id: row.get(0)?,
                session_id: row.get(1)?,
                node_id: row.get(2)?,
                sealed_path: row.get(3)?,
                artifact_kind: row.get(4)?,
                seal_hash: row
                    .get::<_, String>(5)
                    .ok()
                    .and_then(|h| hex::decode(h).ok())
                    .unwrap_or_default(),
                version: row.get::<_, i32>(6)?,
            });
        }
        Ok(records)
    }

    /// Check whether a node has any interface seals
    pub fn has_interface_seals(&self, session_id: &str, node_id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM interface_seals WHERE session_id = ? AND node_id = ?",
            [session_id, node_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    // =========================================================================
    // PSP-5 Phase 6: Branch Flush CRUD
    // =========================================================================

    /// Record a branch flush decision
    pub fn record_branch_flush(&self, record: &BranchFlushRow) -> Result<()> {
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO branch_flushes (flush_id, session_id, parent_node_id, flushed_branch_ids, requeue_node_ids, reason)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            [
                &record.flush_id,
                &record.session_id,
                &record.parent_node_id,
                &record.flushed_branch_ids,
                &record.requeue_node_ids,
                &record.reason,
            ],
        )?;
        Ok(())
    }

    /// Get all branch flush records for a session
    pub fn get_branch_flushes(&self, session_id: &str) -> Result<Vec<BranchFlushRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT flush_id, session_id, parent_node_id, flushed_branch_ids, requeue_node_ids, reason
             FROM branch_flushes WHERE session_id = ? ORDER BY created_at",
        )?;
        let mut rows = stmt.query([session_id])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            records.push(BranchFlushRow {
                flush_id: row.get(0)?,
                session_id: row.get(1)?,
                parent_node_id: row.get(2)?,
                flushed_branch_ids: row.get(3)?,
                requeue_node_ids: row.get(4)?,
                reason: row.get(5)?,
            });
        }
        Ok(records)
    }

    // =========================================================================
    // PSP-5 Phase 8: Node Snapshot, Task Graph, and Review Outcome Persistence
    // =========================================================================

    /// Get the latest node state snapshot per node for a session (for resume reconstruction).
    ///
    /// Returns at most one record per node_id, picking the most recently created row.
    pub fn get_latest_node_states(&self, session_id: &str) -> Result<Vec<NodeStateRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "WITH ranked AS ( \
                 SELECT *, ROW_NUMBER() OVER (PARTITION BY node_id ORDER BY created_at DESC) AS rn \
                 FROM node_states WHERE session_id = ? \
             ) \
             SELECT node_id, session_id, state, v_total, CAST(merkle_hash AS VARCHAR), attempt_count, \
                    node_class, owner_plugin, goal, parent_id, children, last_error_type, committed_at \
             FROM ranked WHERE rn = 1 ORDER BY created_at",
        )?;

        let mut rows = stmt.query([session_id])?;
        let mut records = Vec::new();

        while let Some(row) = rows.next()? {
            records.push(NodeStateRecord {
                node_id: row.get(0)?,
                session_id: row.get(1)?,
                state: row.get(2)?,
                v_total: row.get::<_, f64>(3)? as f32,
                merkle_hash: row
                    .get::<_, Option<String>>(4)?
                    .and_then(|s| hex::decode(s).ok()),
                attempt_count: row.get(5)?,
                node_class: row.get::<_, Option<String>>(6)?.filter(|s| !s.is_empty()),
                owner_plugin: row.get::<_, Option<String>>(7)?.filter(|s| !s.is_empty()),
                goal: row.get::<_, Option<String>>(8)?.filter(|s| !s.is_empty()),
                parent_id: row.get::<_, Option<String>>(9)?.filter(|s| !s.is_empty()),
                children: row.get::<_, Option<String>>(10)?.filter(|s| !s.is_empty()),
                last_error_type: row.get::<_, Option<String>>(11)?.filter(|s| !s.is_empty()),
                committed_at: row.get::<_, Option<String>>(12)?.filter(|s| !s.is_empty()),
            });
        }

        Ok(records)
    }

    /// Record a task graph edge (parent→child dependency)
    pub fn record_task_graph_edge(&self, record: &TaskGraphEdgeRow) -> Result<()> {
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO task_graph_edges (session_id, parent_node_id, child_node_id, edge_type)
            VALUES (?, ?, ?, ?)
            "#,
            [
                &record.session_id,
                &record.parent_node_id,
                &record.child_node_id,
                &record.edge_type,
            ],
        )?;
        Ok(())
    }

    /// Get all task graph edges for a session
    pub fn get_task_graph_edges(&self, session_id: &str) -> Result<Vec<TaskGraphEdgeRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, parent_node_id, child_node_id, edge_type \
             FROM task_graph_edges WHERE session_id = ? ORDER BY created_at",
        )?;
        let mut rows = stmt.query([session_id])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            records.push(TaskGraphEdgeRow {
                session_id: row.get(0)?,
                parent_node_id: row.get(1)?,
                child_node_id: row.get(2)?,
                edge_type: row.get(3)?,
            });
        }
        Ok(records)
    }

    /// Get child node IDs for a parent in a session's task graph
    pub fn get_children_of_node(
        &self,
        session_id: &str,
        parent_node_id: &str,
    ) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT child_node_id FROM task_graph_edges \
             WHERE session_id = ? AND parent_node_id = ? ORDER BY created_at",
        )?;
        let mut rows = stmt.query([session_id, parent_node_id])?;
        let mut ids = Vec::new();
        while let Some(row) = rows.next()? {
            ids.push(row.get(0)?);
        }
        Ok(ids)
    }

    /// Record a review outcome (approval, rejection, edit request)
    pub fn record_review_outcome(&self, record: &ReviewOutcomeRow) -> Result<()> {
        let reviewer_note = record.reviewer_note.clone().unwrap_or_default();
        let escalation_category = record.escalation_category.clone().unwrap_or_default();
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO review_outcomes (session_id, node_id, outcome, reviewer_note,
                                         energy_at_review, degraded, escalation_category)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
            duckdb::params![
                record.session_id,
                record.node_id,
                record.outcome,
                reviewer_note,
                record.energy_at_review.unwrap_or(0.0),
                record.degraded.unwrap_or(false),
                escalation_category,
            ],
        )?;
        Ok(())
    }

    /// Get all review outcomes for a node
    pub fn get_review_outcomes(
        &self,
        session_id: &str,
        node_id: &str,
    ) -> Result<Vec<ReviewOutcomeRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, node_id, outcome, reviewer_note, \
             energy_at_review, degraded, escalation_category \
             FROM review_outcomes WHERE session_id = ? AND node_id = ? ORDER BY created_at",
        )?;
        let mut rows = stmt.query([session_id, node_id])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            records.push(ReviewOutcomeRow {
                session_id: row.get(0)?,
                node_id: row.get(1)?,
                outcome: row.get(2)?,
                reviewer_note: row.get::<_, Option<String>>(3)?.filter(|s| !s.is_empty()),
                energy_at_review: row.get::<_, Option<f64>>(4)?,
                degraded: row.get::<_, Option<bool>>(5)?,
                escalation_category: row.get::<_, Option<String>>(6)?.filter(|s| !s.is_empty()),
            });
        }
        Ok(records)
    }

    /// Get the most recent review outcome for a node
    pub fn get_latest_review_outcome(
        &self,
        session_id: &str,
        node_id: &str,
    ) -> Result<Option<ReviewOutcomeRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, node_id, outcome, reviewer_note, \
             energy_at_review, degraded, escalation_category \
             FROM review_outcomes WHERE session_id = ? AND node_id = ? \
             ORDER BY created_at DESC LIMIT 1",
        )?;
        let mut rows = stmt.query([session_id, node_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(ReviewOutcomeRow {
                session_id: row.get(0)?,
                node_id: row.get(1)?,
                outcome: row.get(2)?,
                reviewer_note: row.get::<_, Option<String>>(3)?.filter(|s| !s.is_empty()),
                energy_at_review: row.get::<_, Option<f64>>(4)?,
                degraded: row.get::<_, Option<bool>>(5)?,
                escalation_category: row.get::<_, Option<String>>(6)?.filter(|s| !s.is_empty()),
            }))
        } else {
            Ok(None)
        }
    }

    /// Get all review outcomes for a session (across all nodes).
    pub fn get_all_review_outcomes(&self, session_id: &str) -> Result<Vec<ReviewOutcomeRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, node_id, outcome, reviewer_note, \
             energy_at_review, degraded, escalation_category \
             FROM review_outcomes WHERE session_id = ? ORDER BY created_at",
        )?;
        let mut rows = stmt.query([session_id])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            records.push(ReviewOutcomeRow {
                session_id: row.get(0)?,
                node_id: row.get(1)?,
                outcome: row.get(2)?,
                reviewer_note: row.get::<_, Option<String>>(3)?.filter(|s| !s.is_empty()),
                energy_at_review: row.get::<_, Option<f64>>(4)?,
                degraded: row.get::<_, Option<bool>>(5)?,
                escalation_category: row.get::<_, Option<String>>(6)?.filter(|s| !s.is_empty()),
            });
        }
        Ok(records)
    }

    // =========================================================================
    // PSP-5 Phase 8: Verification Result and Artifact Bundle Persistence
    // =========================================================================

    /// Record a verification result snapshot for a node
    pub fn record_verification_result(&self, record: &VerificationResultRow) -> Result<()> {
        let syntax_ok = record.syntax_ok.to_string();
        let build_ok = record.build_ok.to_string();
        let tests_ok = record.tests_ok.to_string();
        let lint_ok = record.lint_ok.to_string();
        let diagnostics_count = record.diagnostics_count.to_string();
        let tests_passed = record.tests_passed.to_string();
        let tests_failed = record.tests_failed.to_string();
        let degraded = record.degraded.to_string();
        let degraded_reason = record.degraded_reason.clone().unwrap_or_default();

        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO verification_results (session_id, node_id, result_json,
                syntax_ok, build_ok, tests_ok, lint_ok,
                diagnostics_count, tests_passed, tests_failed, degraded, degraded_reason)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            [
                &record.session_id,
                &record.node_id,
                &record.result_json,
                &syntax_ok,
                &build_ok,
                &tests_ok,
                &lint_ok,
                &diagnostics_count,
                &tests_passed,
                &tests_failed,
                &degraded,
                &degraded_reason,
            ],
        )?;
        Ok(())
    }

    /// Get the latest verification result for a node
    pub fn get_verification_result(
        &self,
        session_id: &str,
        node_id: &str,
    ) -> Result<Option<VerificationResultRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, node_id, result_json, \
                    CAST(syntax_ok AS VARCHAR), CAST(build_ok AS VARCHAR), CAST(tests_ok AS VARCHAR), CAST(lint_ok AS VARCHAR), \
                    diagnostics_count, tests_passed, tests_failed, CAST(degraded AS VARCHAR), degraded_reason \
             FROM verification_results \
             WHERE session_id = ? AND node_id = ? \
             ORDER BY created_at DESC LIMIT 1",
        )?;
        let mut rows = stmt.query([session_id, node_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(VerificationResultRow {
                session_id: row.get(0)?,
                node_id: row.get(1)?,
                result_json: row.get(2)?,
                syntax_ok: row.get::<_, String>(3)?.parse().unwrap_or(false),
                build_ok: row.get::<_, String>(4)?.parse().unwrap_or(false),
                tests_ok: row.get::<_, String>(5)?.parse().unwrap_or(false),
                lint_ok: row.get::<_, String>(6)?.parse().unwrap_or(false),
                diagnostics_count: row.get(7)?,
                tests_passed: row.get(8)?,
                tests_failed: row.get(9)?,
                degraded: row.get::<_, String>(10)?.parse().unwrap_or(false),
                degraded_reason: row.get::<_, Option<String>>(11)?.filter(|s| !s.is_empty()),
            }))
        } else {
            Ok(None)
        }
    }

    /// Get all verification results for a session (for status display)
    pub fn get_all_verification_results(
        &self,
        session_id: &str,
    ) -> Result<Vec<VerificationResultRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "WITH ranked AS ( \
                 SELECT *, ROW_NUMBER() OVER (PARTITION BY node_id ORDER BY created_at DESC) AS rn \
                 FROM verification_results WHERE session_id = ? \
             ) \
             SELECT session_id, node_id, result_json, \
                    CAST(syntax_ok AS VARCHAR), CAST(build_ok AS VARCHAR), CAST(tests_ok AS VARCHAR), CAST(lint_ok AS VARCHAR), \
                    diagnostics_count, tests_passed, tests_failed, CAST(degraded AS VARCHAR), degraded_reason \
             FROM ranked WHERE rn = 1 ORDER BY created_at",
        )?;
        let mut rows = stmt.query([session_id])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            records.push(VerificationResultRow {
                session_id: row.get(0)?,
                node_id: row.get(1)?,
                result_json: row.get(2)?,
                syntax_ok: row.get::<_, String>(3)?.parse().unwrap_or(false),
                build_ok: row.get::<_, String>(4)?.parse().unwrap_or(false),
                tests_ok: row.get::<_, String>(5)?.parse().unwrap_or(false),
                lint_ok: row.get::<_, String>(6)?.parse().unwrap_or(false),
                diagnostics_count: row.get(7)?,
                tests_passed: row.get(8)?,
                tests_failed: row.get(9)?,
                degraded: row.get::<_, String>(10)?.parse().unwrap_or(false),
                degraded_reason: row.get::<_, Option<String>>(11)?.filter(|s| !s.is_empty()),
            });
        }
        Ok(records)
    }

    /// Record an artifact bundle snapshot for a node
    pub fn record_artifact_bundle(&self, record: &ArtifactBundleRow) -> Result<()> {
        let artifact_count = record.artifact_count.to_string();
        let command_count = record.command_count.to_string();

        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO artifact_bundles (session_id, node_id, bundle_json,
                artifact_count, command_count, touched_files)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            [
                &record.session_id,
                &record.node_id,
                &record.bundle_json,
                &artifact_count,
                &command_count,
                &record.touched_files,
            ],
        )?;
        Ok(())
    }

    /// Get the latest artifact bundle for a node
    pub fn get_artifact_bundle(
        &self,
        session_id: &str,
        node_id: &str,
    ) -> Result<Option<ArtifactBundleRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, node_id, bundle_json, artifact_count, command_count, touched_files \
             FROM artifact_bundles \
             WHERE session_id = ? AND node_id = ? \
             ORDER BY created_at DESC LIMIT 1",
        )?;
        let mut rows = stmt.query([session_id, node_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(ArtifactBundleRow {
                session_id: row.get(0)?,
                node_id: row.get(1)?,
                bundle_json: row.get(2)?,
                artifact_count: row.get(3)?,
                command_count: row.get(4)?,
                touched_files: row.get(5)?,
            }))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create an in-memory store for testing
    fn test_store() -> SessionStore {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join(format!("perspt_test_{}.db", uuid::Uuid::new_v4()));
        SessionStore::open(&db_path).expect("Failed to create test store")
    }

    fn seed_session(store: &SessionStore, session_id: &str) {
        let record = SessionRecord {
            session_id: session_id.to_string(),
            task: "test task".to_string(),
            working_dir: "/tmp/test".to_string(),
            merkle_root: None,
            detected_toolchain: None,
            status: "RUNNING".to_string(),
        };
        store.create_session(&record).unwrap();
    }

    #[test]
    fn test_node_state_phase8_roundtrip() {
        let store = test_store();
        let sid = "test-sess-1";
        seed_session(&store, sid);

        let record = NodeStateRecord {
            node_id: "node-1".to_string(),
            session_id: sid.to_string(),
            state: "Completed".to_string(),
            v_total: 0.42,
            merkle_hash: Some(vec![0xab; 32]),
            attempt_count: 3,
            node_class: Some("Interface".to_string()),
            owner_plugin: Some("rust".to_string()),
            goal: Some("Implement API".to_string()),
            parent_id: Some("root".to_string()),
            children: Some(r#"["child-a","child-b"]"#.to_string()),
            last_error_type: Some("CompilationError".to_string()),
            committed_at: Some("2025-01-01T00:00:00Z".to_string()),
        };

        store.record_node_state(&record).unwrap();

        let states = store.get_latest_node_states(sid).unwrap();
        assert_eq!(states.len(), 1);
        let r = &states[0];
        assert_eq!(r.node_id, "node-1");
        assert_eq!(r.state, "Completed");
        assert_eq!(r.attempt_count, 3);
        assert_eq!(r.node_class.as_deref(), Some("Interface"));
        assert_eq!(r.owner_plugin.as_deref(), Some("rust"));
        assert_eq!(r.goal.as_deref(), Some("Implement API"));
        assert_eq!(r.parent_id.as_deref(), Some("root"));
        assert!(r.children.is_some());
        assert_eq!(r.last_error_type.as_deref(), Some("CompilationError"));
        assert_eq!(r.committed_at.as_deref(), Some("2025-01-01T00:00:00Z"));
    }

    #[test]
    fn test_task_graph_edge_roundtrip() {
        let store = test_store();
        let sid = "test-graph-1";
        seed_session(&store, sid);

        let edge = TaskGraphEdgeRow {
            session_id: sid.to_string(),
            parent_node_id: "parent-1".to_string(),
            child_node_id: "child-1".to_string(),
            edge_type: "depends_on".to_string(),
        };
        store.record_task_graph_edge(&edge).unwrap();

        let edges = store.get_task_graph_edges(sid).unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].parent_node_id, "parent-1");
        assert_eq!(edges[0].child_node_id, "child-1");
        assert_eq!(edges[0].edge_type, "depends_on");
    }

    #[test]
    fn test_verification_result_roundtrip() {
        let store = test_store();
        let sid = "test-vr-1";
        seed_session(&store, sid);

        let row = VerificationResultRow {
            session_id: sid.to_string(),
            node_id: "node-v".to_string(),
            result_json: r#"{"syntax_ok":true}"#.to_string(),
            syntax_ok: true,
            build_ok: true,
            tests_ok: false,
            lint_ok: true,
            diagnostics_count: 2,
            tests_passed: 5,
            tests_failed: 1,
            degraded: false,
            degraded_reason: None,
        };
        store.record_verification_result(&row).unwrap();

        let got = store.get_verification_result(sid, "node-v").unwrap();
        assert!(got.is_some());
        let got = got.unwrap();
        assert!(got.syntax_ok);
        assert!(got.build_ok);
        assert!(!got.tests_ok);
        assert_eq!(got.tests_passed, 5);
        assert_eq!(got.tests_failed, 1);
        assert!(!got.degraded);
    }

    #[test]
    fn test_verification_result_degraded() {
        let store = test_store();
        let sid = "test-vr-deg";
        seed_session(&store, sid);

        let row = VerificationResultRow {
            session_id: sid.to_string(),
            node_id: "node-d".to_string(),
            result_json: "{}".to_string(),
            syntax_ok: true,
            build_ok: false,
            tests_ok: false,
            lint_ok: false,
            diagnostics_count: 0,
            tests_passed: 0,
            tests_failed: 0,
            degraded: true,
            degraded_reason: Some("LSP unavailable".to_string()),
        };
        store.record_verification_result(&row).unwrap();

        let got = store
            .get_verification_result(sid, "node-d")
            .unwrap()
            .unwrap();
        assert!(got.degraded);
        assert_eq!(got.degraded_reason.as_deref(), Some("LSP unavailable"));
    }

    #[test]
    fn test_artifact_bundle_roundtrip() {
        let store = test_store();
        let sid = "test-ab-1";
        seed_session(&store, sid);

        let row = ArtifactBundleRow {
            session_id: sid.to_string(),
            node_id: "node-a".to_string(),
            bundle_json: r#"{"artifacts":[],"commands":[]}"#.to_string(),
            artifact_count: 3,
            command_count: 1,
            touched_files: r#"["src/main.rs","src/lib.rs","tests/test.rs"]"#.to_string(),
        };
        store.record_artifact_bundle(&row).unwrap();

        let got = store.get_artifact_bundle(sid, "node-a").unwrap();
        assert!(got.is_some());
        let got = got.unwrap();
        assert_eq!(got.artifact_count, 3);
        assert_eq!(got.command_count, 1);
        assert!(got.touched_files.contains("main.rs"));
    }

    #[test]
    fn test_latest_node_states_dedup() {
        let store = test_store();
        let sid = "test-dedup";
        seed_session(&store, sid);

        // Insert two states for the same node
        let r1 = NodeStateRecord {
            node_id: "node-x".to_string(),
            session_id: sid.to_string(),
            state: "Coding".to_string(),
            v_total: 0.5,
            merkle_hash: None,
            attempt_count: 1,
            node_class: None,
            owner_plugin: None,
            goal: None,
            parent_id: None,
            children: None,
            last_error_type: None,
            committed_at: None,
        };
        store.record_node_state(&r1).unwrap();

        let r2 = NodeStateRecord {
            node_id: "node-x".to_string(),
            session_id: sid.to_string(),
            state: "Completed".to_string(),
            v_total: 0.3,
            merkle_hash: None,
            attempt_count: 2,
            node_class: Some("Implementation".to_string()),
            owner_plugin: None,
            goal: Some("Updated goal".to_string()),
            parent_id: None,
            children: None,
            last_error_type: None,
            committed_at: Some("2025-01-02T00:00:00Z".to_string()),
        };
        store.record_node_state(&r2).unwrap();

        // get_latest should return only the last entry
        let latest = store.get_latest_node_states(sid).unwrap();
        assert_eq!(latest.len(), 1);
        assert_eq!(latest[0].state, "Completed");
        assert_eq!(latest[0].attempt_count, 2);
        assert_eq!(latest[0].goal.as_deref(), Some("Updated goal"));
    }

    #[test]
    fn test_backward_compat_empty_phase8_fields() {
        let store = test_store();
        let sid = "test-compat";
        seed_session(&store, sid);

        // Insert a node with all Phase 8 fields as None (pre-Phase-8 session)
        let r = NodeStateRecord {
            node_id: "old-node".to_string(),
            session_id: sid.to_string(),
            state: "COMPLETED".to_string(),
            v_total: 1.0,
            merkle_hash: None,
            attempt_count: 1,
            node_class: None,
            owner_plugin: None,
            goal: None,
            parent_id: None,
            children: None,
            last_error_type: None,
            committed_at: None,
        };
        store.record_node_state(&r).unwrap();

        let latest = store.get_latest_node_states(sid).unwrap();
        assert_eq!(latest.len(), 1);
        assert!(latest[0].node_class.is_none());
        assert!(latest[0].goal.is_none());
        assert!(latest[0].committed_at.is_none());

        // Verification and artifact lookups should return None
        let vr = store.get_verification_result(sid, "old-node").unwrap();
        assert!(vr.is_none());
        let ab = store.get_artifact_bundle(sid, "old-node").unwrap();
        assert!(ab.is_none());
    }

    #[test]
    fn test_review_outcome_roundtrip() {
        let store = test_store();
        let sid = "test-review";
        seed_session(&store, sid);

        let row = ReviewOutcomeRow {
            session_id: sid.to_string(),
            node_id: "node-r".to_string(),
            outcome: "approved".to_string(),
            reviewer_note: Some("LGTM".to_string()),
            energy_at_review: None,
            degraded: None,
            escalation_category: None,
        };
        store.record_review_outcome(&row).unwrap();

        let outcomes = store.get_review_outcomes(sid, "node-r").unwrap();
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].outcome, "approved");
        assert_eq!(outcomes[0].reviewer_note.as_deref(), Some("LGTM"));
    }

    #[test]
    fn test_review_outcome_with_audit_fields() {
        let store = test_store();
        let sid = "test-review-audit";
        seed_session(&store, sid);

        let row = ReviewOutcomeRow {
            session_id: sid.to_string(),
            node_id: "node-a".to_string(),
            outcome: "rejected".to_string(),
            reviewer_note: Some("Needs rework".to_string()),
            energy_at_review: Some(0.42),
            degraded: Some(true),
            escalation_category: Some("complexity".to_string()),
        };
        store.record_review_outcome(&row).unwrap();

        let outcomes = store.get_review_outcomes(sid, "node-a").unwrap();
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].outcome, "rejected");
        assert_eq!(outcomes[0].energy_at_review, Some(0.42));
        assert_eq!(outcomes[0].degraded, Some(true));
        assert_eq!(
            outcomes[0].escalation_category.as_deref(),
            Some("complexity")
        );
    }

    #[test]
    fn test_get_all_review_outcomes() {
        let store = test_store();
        let sid = "test-review-all";
        seed_session(&store, sid);

        for (node, outcome) in &[("n1", "approved"), ("n2", "rejected"), ("n1", "approved")] {
            let row = ReviewOutcomeRow {
                session_id: sid.to_string(),
                node_id: node.to_string(),
                outcome: outcome.to_string(),
                reviewer_note: None,
                energy_at_review: None,
                degraded: None,
                escalation_category: None,
            };
            store.record_review_outcome(&row).unwrap();
        }

        let all = store.get_all_review_outcomes(sid).unwrap();
        assert_eq!(all.len(), 3);
    }
}
