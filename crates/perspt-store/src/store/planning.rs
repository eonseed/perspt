use super::*;

impl SessionStore {
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

// =========================================================================
// Plan Revision, Feature Charter, and Repair Footprint Methods
// =========================================================================

impl SessionStore {
    /// Record a feature charter for a session.
    pub fn record_feature_charter(&self, row: &FeatureCharterRow) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO feature_charters (charter_id, session_id, scope_description, max_modules, \
             max_files, max_revisions, language_constraint) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            duckdb::params![
                row.charter_id,
                row.session_id,
                row.scope_description,
                row.max_modules,
                row.max_files,
                row.max_revisions,
                row.language_constraint,
            ],
        )?;
        Ok(())
    }

    /// Get the feature charter for a session.
    pub fn get_feature_charter(&self, session_id: &str) -> Result<Option<FeatureCharterRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT charter_id, session_id, scope_description, max_modules, max_files, max_revisions, \
             language_constraint \
             FROM feature_charters WHERE session_id = ? LIMIT 1",
        )?;
        let mut rows = stmt.query([session_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(FeatureCharterRow {
                charter_id: row.get(0)?,
                session_id: row.get(1)?,
                scope_description: row.get(2)?,
                max_modules: row.get(3)?,
                max_files: row.get(4)?,
                max_revisions: row.get(5)?,
                language_constraint: row.get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Record a plan revision.
    pub fn record_plan_revision(&self, row: &PlanRevisionRow) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO plan_revisions (revision_id, session_id, sequence, plan_json, reason, \
             supersedes, status) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            duckdb::params![
                row.revision_id,
                row.session_id,
                row.sequence,
                row.plan_json,
                row.reason,
                row.supersedes,
                row.status,
            ],
        )?;
        Ok(())
    }

    /// Get the active plan revision for a session.
    pub fn get_active_plan_revision(&self, session_id: &str) -> Result<Option<PlanRevisionRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT revision_id, session_id, sequence, plan_json, reason, supersedes, status \
             FROM plan_revisions WHERE session_id = ? AND status = 'active' \
             ORDER BY sequence DESC LIMIT 1",
        )?;
        let mut rows = stmt.query([session_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(PlanRevisionRow {
                revision_id: row.get(0)?,
                session_id: row.get(1)?,
                sequence: row.get(2)?,
                plan_json: row.get(3)?,
                reason: row.get(4)?,
                supersedes: row.get(5)?,
                status: row.get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get all plan revisions for a session, ordered by sequence.
    pub fn get_plan_revisions(&self, session_id: &str) -> Result<Vec<PlanRevisionRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT revision_id, session_id, sequence, plan_json, reason, supersedes, status \
             FROM plan_revisions WHERE session_id = ? ORDER BY sequence ASC",
        )?;
        let mut rows = stmt.query([session_id])?;
        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
            results.push(PlanRevisionRow {
                revision_id: row.get(0)?,
                session_id: row.get(1)?,
                sequence: row.get(2)?,
                plan_json: row.get(3)?,
                reason: row.get(4)?,
                supersedes: row.get(5)?,
                status: row.get(6)?,
            });
        }
        Ok(results)
    }

    /// Supersede a plan revision (set status to 'superseded').
    pub fn supersede_plan_revision(&self, revision_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE plan_revisions SET status = 'superseded' WHERE revision_id = ?",
            [revision_id],
        )?;
        Ok(())
    }

    /// Record a repair footprint.
    pub fn record_repair_footprint(&self, row: &RepairFootprintRow) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO repair_footprints (footprint_id, session_id, node_id, revision_id, attempt, \
             affected_files, bundle_json, diagnosis, resolved) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            duckdb::params![
                row.footprint_id,
                row.session_id,
                row.node_id,
                row.revision_id,
                row.attempt,
                row.affected_files,
                row.bundle_json,
                row.diagnosis,
                row.resolved,
            ],
        )?;
        Ok(())
    }

    /// Get repair footprints for a node, ordered by attempt.
    pub fn get_repair_footprints(
        &self,
        session_id: &str,
        node_id: &str,
    ) -> Result<Vec<RepairFootprintRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT footprint_id, session_id, node_id, revision_id, attempt, affected_files, bundle_json, \
             diagnosis, resolved \
             FROM repair_footprints WHERE session_id = ? AND node_id = ? ORDER BY attempt ASC",
        )?;
        let mut rows = stmt.query([session_id, node_id])?;
        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
            results.push(RepairFootprintRow {
                footprint_id: row.get(0)?,
                session_id: row.get(1)?,
                node_id: row.get(2)?,
                revision_id: row.get(3)?,
                attempt: row.get(4)?,
                affected_files: row.get(5)?,
                bundle_json: row.get(6)?,
                diagnosis: row.get(7)?,
                resolved: row.get(8)?,
            });
        }
        Ok(results)
    }

    /// Get all repair footprints for a session (all nodes).
    pub fn get_all_repair_footprints(&self, session_id: &str) -> Result<Vec<RepairFootprintRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT footprint_id, session_id, node_id, revision_id, attempt, affected_files, bundle_json, \
             diagnosis, resolved \
             FROM repair_footprints WHERE session_id = ? ORDER BY attempt ASC",
        )?;
        let mut rows = stmt.query([session_id])?;
        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
            results.push(RepairFootprintRow {
                footprint_id: row.get(0)?,
                session_id: row.get(1)?,
                node_id: row.get(2)?,
                revision_id: row.get(3)?,
                attempt: row.get(4)?,
                affected_files: row.get(5)?,
                bundle_json: row.get(6)?,
                diagnosis: row.get(7)?,
                resolved: row.get(8)?,
            });
        }
        Ok(results)
    }

    /// Mark a repair footprint as resolved.
    pub fn resolve_repair_footprint(&self, footprint_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE repair_footprints SET resolved = true WHERE footprint_id = ?",
            [footprint_id],
        )?;
        Ok(())
    }

    /// Record or update a budget envelope for a session.
    pub fn upsert_budget_envelope(&self, row: &BudgetEnvelopeRow) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        // Try insert first, update on conflict
        conn.execute(
            "INSERT INTO budget_envelopes (session_id, max_steps, steps_used, max_revisions, \
             revisions_used, max_cost_usd, cost_used_usd) \
             VALUES (?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT (session_id) DO UPDATE SET \
             max_steps = EXCLUDED.max_steps, steps_used = EXCLUDED.steps_used, \
             max_revisions = EXCLUDED.max_revisions, revisions_used = EXCLUDED.revisions_used, \
             max_cost_usd = EXCLUDED.max_cost_usd, cost_used_usd = EXCLUDED.cost_used_usd",
            duckdb::params![
                row.session_id,
                row.max_steps,
                row.steps_used,
                row.max_revisions,
                row.revisions_used,
                row.max_cost_usd,
                row.cost_used_usd,
            ],
        )?;
        Ok(())
    }

    /// Get the budget envelope for a session.
    pub fn get_budget_envelope(&self, session_id: &str) -> Result<Option<BudgetEnvelopeRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, max_steps, steps_used, max_revisions, revisions_used, max_cost_usd, \
             cost_used_usd \
             FROM budget_envelopes WHERE session_id = ?",
        )?;
        let mut rows = stmt.query([session_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(BudgetEnvelopeRow {
                session_id: row.get(0)?,
                max_steps: row.get(1)?,
                steps_used: row.get(2)?,
                max_revisions: row.get(3)?,
                revisions_used: row.get(4)?,
                max_cost_usd: row.get(5)?,
                cost_used_usd: row.get(6)?,
            }))
        } else {
            Ok(None)
        }
    }
}

// =========================================================================
// PSP-7: SRBN Step Records and Correction Attempts
// =========================================================================

impl SessionStore {}
