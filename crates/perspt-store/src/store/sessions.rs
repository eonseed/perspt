use super::*;

impl SessionStore {
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
                                     node_class, owner_plugin, goal, parent_id, children, last_error_type,
                                         committed_at)
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

    /// Get session by ID
    pub fn get_session(&self, session_id: &str) -> Result<Option<SessionRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, task, working_dir, merkle_root, detected_toolchain, status FROM sessions \
             WHERE session_id = ?"
        )?;

        let mut rows = stmt.query([session_id])?;
        if let Some(row) = rows.next()? {
            // merkle_root is stored as BLOB; read directly as Option<Vec<u8>>
            // to match list_recent_sessions and avoid type mismatch on Blob columns.
            let merkle_root: Option<Vec<u8>> = row.get(3).ok();

            Ok(Some(SessionRecord {
                session_id: row.get(0)?,
                task: row.get(1)?,
                working_dir: row.get(2)?,
                merkle_root,
                detected_toolchain: row.get(4)?,
                status: row.get(5)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get the directory for session artifacts (`~/.local/share/perspt/sessions/<id>`)
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
            "SELECT node_id, session_id, v_syn, v_str, v_log, v_boot, v_sheaf, v_total FROM \
             energy_history WHERE session_id = ? AND node_id = ? ORDER BY timestamp",
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

    /// Get all energy history for a session (all nodes)
    pub fn get_session_energy_history(&self, session_id: &str) -> Result<Vec<EnergyRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT node_id, session_id, v_syn, v_str, v_log, v_boot, v_sheaf, v_total FROM \
             energy_history WHERE session_id = ? ORDER BY timestamp",
        )?;

        let mut rows = stmt.query([session_id])?;
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
        self.list_sessions_paginated(limit, 0)
    }

    /// List sessions with pagination (most recent first).
    pub fn list_sessions_paginated(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<SessionRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, task, working_dir, merkle_root, detected_toolchain, status
             FROM sessions ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )?;

        let mut rows = stmt.query([limit.to_string(), offset.to_string()])?;
        let mut records = Vec::new();

        while let Some(row) = rows.next()? {
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

    /// Count total number of sessions.
    pub fn count_sessions(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM sessions")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let count: i64 = row.get(0)?;
            Ok(count as usize)
        } else {
            Ok(0)
        }
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
}
