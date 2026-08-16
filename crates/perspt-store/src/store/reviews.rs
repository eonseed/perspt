use super::*;

impl SessionStore {
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
                    CAST(syntax_ok AS VARCHAR), CAST(build_ok AS VARCHAR), CAST(tests_ok AS VARCHAR),
                        CAST(lint_ok AS VARCHAR), \
                    diagnostics_count, tests_passed, tests_failed, CAST(degraded AS VARCHAR),
                        degraded_reason \
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
                    CAST(syntax_ok AS VARCHAR), CAST(build_ok AS VARCHAR), CAST(tests_ok AS VARCHAR),
                        CAST(lint_ok AS VARCHAR), \
                    diagnostics_count, tests_passed, tests_failed, CAST(degraded AS VARCHAR),
                        degraded_reason \
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
}
