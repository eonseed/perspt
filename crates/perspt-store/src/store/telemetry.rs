use super::*;

impl SessionStore {
    /// Record an LLM request/response
    pub fn record_llm_request(&self, record: &LlmRequestRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO llm_requests (session_id, node_id, model, prompt, response, tokens_in, tokens_out,
                latency_ms)
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

    /// Aggregate LLM statistics across all sessions: (count, sum_tokens_in, sum_tokens_out, sum_latency_ms)
    pub fn get_global_llm_summary(&self) -> Result<(i64, i64, i64, i64)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT COUNT(*), \
             COALESCE(SUM(CASE WHEN tokens_in > 0 THEN tokens_in ELSE (LENGTH(prompt) + 3) / 4 END), 0), \
             COALESCE(SUM(CASE WHEN tokens_out > 0 THEN tokens_out ELSE (LENGTH(response) + 3) / 4 END),
                 0), \
             COALESCE(MEDIAN(latency_ms), 0) \
             FROM llm_requests",
        )?;
        let result = stmt.query_row([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
        Ok(result)
    }

    // =========================================================================
    // PSP-5 Phase 3: Structural Digest & Context Provenance Persistence
    // =========================================================================
}
