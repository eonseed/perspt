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

/// Session store for SRBN persistence
pub struct SessionStore {
    conn: Connection,
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

        Ok(Self { conn })
    }

    /// Get the default database path (~/.local/share/perspt/perspt.db)
    pub fn default_db_path() -> Result<PathBuf> {
        let data_dir = dirs::data_local_dir()
            .context("Could not find local data directory")?
            .join("perspt");
        Ok(data_dir.join("perspt.db"))
    }

    /// Create a new session
    pub fn create_session(&self, session: &SessionRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO sessions (session_id, task, working_dir, merkle_root, detected_toolchain, status)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            [
                &session.session_id,
                &session.task,
                &session.working_dir,
                &session.merkle_root.as_ref().map(|v| hex::encode(v)).unwrap_or_default(),
                &session.detected_toolchain.clone().unwrap_or_default(),
                &session.status,
            ],
        )?;
        Ok(())
    }

    /// Update session merkle root
    pub fn update_merkle_root(&self, session_id: &str, merkle_root: &[u8]) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET merkle_root = ?, updated_at = CURRENT_TIMESTAMP WHERE session_id = ?",
            [hex::encode(merkle_root), session_id.to_string()],
        )?;
        Ok(())
    }

    /// Record node state
    pub fn record_node_state(&self, record: &NodeStateRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO node_states (node_id, session_id, state, v_total, merkle_hash, attempt_count)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            [
                &record.node_id,
                &record.session_id,
                &record.state,
                &record.v_total.to_string(),
                &record.merkle_hash.as_ref().map(|v| hex::encode(v)).unwrap_or_default(),
                &record.attempt_count.to_string(),
            ],
        )?;
        Ok(())
    }

    /// Record energy measurement
    pub fn record_energy(&self, record: &EnergyRecord) -> Result<()> {
        self.conn.execute(
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
        let mut stmt = self.conn.prepare(
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

    /// Get energy history for a node
    pub fn get_energy_history(&self, session_id: &str, node_id: &str) -> Result<Vec<EnergyRecord>> {
        let mut stmt = self.conn.prepare(
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
}
