use super::*;

impl SessionStore {
    /// Record an orchestration step transition.
    pub fn record_step(&self, record: &SrbnStepRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"INSERT INTO srbn_step_records
               (session_id, node_id, step, outcome, energy_json,
                parse_state, retry_classification, attempt_count, duration_ms)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            duckdb::params![
                record.session_id,
                record.node_id,
                record.step,
                record.outcome,
                record.energy_json,
                record.parse_state,
                record.retry_classification,
                record.attempt_count,
                record.duration_ms,
            ],
        )?;
        Ok(())
    }

    /// Retrieve the step timeline for a given node in chronological order.
    pub fn get_step_timeline(
        &self,
        session_id: &str,
        node_id: &str,
    ) -> Result<Vec<SrbnStepRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"SELECT session_id, node_id, step, outcome, energy_json,
                      parse_state, retry_classification, attempt_count, duration_ms
               FROM srbn_step_records
               WHERE session_id = ? AND node_id = ?
               ORDER BY id ASC"#,
        )?;
        let rows = stmt.query_map(duckdb::params![session_id, node_id], |row| {
            Ok(SrbnStepRecord {
                session_id: row.get(0)?,
                node_id: row.get(1)?,
                step: row.get(2)?,
                outcome: row.get(3)?,
                energy_json: row.get(4)?,
                parse_state: row.get(5)?,
                retry_classification: row.get(6)?,
                attempt_count: row.get(7)?,
                duration_ms: row.get(8)?,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Retrieve all step records for a session, ordered by id.
    pub fn get_session_steps(&self, session_id: &str) -> Result<Vec<SrbnStepRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"SELECT session_id, node_id, step, outcome, energy_json,
                      parse_state, retry_classification, attempt_count, duration_ms
               FROM srbn_step_records
               WHERE session_id = ?
               ORDER BY id ASC"#,
        )?;
        let rows = stmt.query_map(duckdb::params![session_id], |row| {
            Ok(SrbnStepRecord {
                session_id: row.get(0)?,
                node_id: row.get(1)?,
                step: row.get(2)?,
                outcome: row.get(3)?,
                energy_json: row.get(4)?,
                parse_state: row.get(5)?,
                retry_classification: row.get(6)?,
                attempt_count: row.get(7)?,
                duration_ms: row.get(8)?,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Record a correction attempt within a convergence loop.
    pub fn record_correction_attempt(&self, record: &CorrectionAttemptRow) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"INSERT INTO correction_attempts
               (session_id, node_id, attempt, parse_state, retry_classification,
                response_fingerprint, response_length, energy_json,
                accepted, rejection_reason, created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            duckdb::params![
                record.session_id,
                record.node_id,
                record.attempt,
                record.parse_state,
                record.retry_classification,
                record.response_fingerprint,
                record.response_length,
                record.energy_json,
                record.accepted,
                record.rejection_reason,
                record.created_at,
            ],
        )?;
        Ok(())
    }

    /// Retrieve all correction attempts for a node, ordered by attempt number.
    pub fn get_correction_attempts(
        &self,
        session_id: &str,
        node_id: &str,
    ) -> Result<Vec<CorrectionAttemptRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"SELECT session_id, node_id, attempt, parse_state, retry_classification,
                      response_fingerprint, response_length, energy_json,
                      accepted, rejection_reason, created_at
               FROM correction_attempts
               WHERE session_id = ? AND node_id = ?
               ORDER BY attempt ASC"#,
        )?;
        let rows = stmt.query_map(duckdb::params![session_id, node_id], |row| {
            Ok(CorrectionAttemptRow {
                session_id: row.get(0)?,
                node_id: row.get(1)?,
                attempt: row.get(2)?,
                parse_state: row.get(3)?,
                retry_classification: row.get(4)?,
                response_fingerprint: row.get(5)?,
                response_length: row.get(6)?,
                energy_json: row.get(7)?,
                accepted: row.get(8)?,
                rejection_reason: row.get(9)?,
                created_at: row.get(10)?,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Retrieve all correction attempts for a session, ordered by node then attempt.
    pub fn get_session_correction_attempts(
        &self,
        session_id: &str,
    ) -> Result<Vec<CorrectionAttemptRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"SELECT session_id, node_id, attempt, parse_state, retry_classification,
                      response_fingerprint, response_length, energy_json,
                      accepted, rejection_reason, created_at
               FROM correction_attempts
               WHERE session_id = ?
               ORDER BY node_id ASC, attempt ASC"#,
        )?;
        let rows = stmt.query_map(duckdb::params![session_id], |row| {
            Ok(CorrectionAttemptRow {
                session_id: row.get(0)?,
                node_id: row.get(1)?,
                attempt: row.get(2)?,
                parse_state: row.get(3)?,
                retry_classification: row.get(4)?,
                response_fingerprint: row.get(5)?,
                response_length: row.get(6)?,
                energy_json: row.get(7)?,
                accepted: row.get(8)?,
                rejection_reason: row.get(9)?,
                created_at: row.get(10)?,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }
}
