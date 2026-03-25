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
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO node_states (node_id, session_id, state, v_total, merkle_hash, attempt_count)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            [
                &record.node_id,
                &record.session_id,
                &record.state,
                &record.v_total.to_string(),
                &record.merkle_hash.as_ref().map(hex::encode).unwrap_or_default(),
                &record.attempt_count.to_string(),
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
            "SELECT node_id, session_id, state, v_total, merkle_hash, attempt_count
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
                &record.parent_seal_hash.as_ref().map(|h| hex::encode(h)).unwrap_or_default(),
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
                parent_seal_hash: row.get::<_, Option<String>>(5)?.and_then(|h| hex::decode(h).ok()),
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
                parent_seal_hash: row.get::<_, Option<String>>(5)?.and_then(|h| hex::decode(h).ok()),
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
        let mut stmt = conn.prepare(
            "SELECT child_branch_id FROM branch_lineage WHERE parent_branch_id = ?",
        )?;
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
    pub fn get_interface_seals(&self, session_id: &str, node_id: &str) -> Result<Vec<InterfaceSealRow>> {
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
                seal_hash: row.get::<_, String>(5).ok().and_then(|h| hex::decode(h).ok()).unwrap_or_default(),
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
}
