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

    /// Update session status
    pub fn update_session_status(&self, session_id: &str, status: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE sessions SET status = ?, updated_at = CURRENT_TIMESTAMP WHERE session_id = ?",
            [status, session_id],
        )?;
        Ok(())
    }
}
