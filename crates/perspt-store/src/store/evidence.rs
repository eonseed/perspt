use super::*;

impl SessionStore {
    // =========================================================================
    // PSP-5 Phase 3: Structural Digest & Context Provenance Persistence
    // =========================================================================

    /// Record a structural digest
    pub fn record_structural_digest(&self, record: &StructuralDigestRecord) -> Result<()> {
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO structural_digests (digest_id, session_id, node_id, source_path, artifact_kind,
                hash, version)
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
            INSERT INTO context_provenance (session_id, node_id, context_package_id, structural_hashes,
                summary_hashes, dependency_hashes, included_file_count, total_bytes)
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
            "SELECT session_id, node_id, context_package_id, structural_hashes, summary_hashes, \
             dependency_hashes, included_file_count, total_bytes
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
            INSERT INTO escalation_reports (session_id, node_id, category, action, energy_snapshot,
                stage_outcomes, evidence, affected_node_ids)
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
            "SELECT session_id, node_id, category, action, energy_snapshot, stage_outcomes, evidence, \
             affected_node_ids
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
            INSERT INTO rewrite_records (session_id, node_id, action, category, requeued_nodes,
                inserted_nodes)
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
            INSERT INTO sheaf_validations (session_id, node_id, validator_class, plugin_source, passed,
                evidence_summary, affected_files, v_sheaf_contribution, requeue_targets)
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
            "SELECT session_id, node_id, validator_class, plugin_source, passed, evidence_summary, \
             affected_files, v_sheaf_contribution, requeue_targets
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

    /// Get all sheaf validations for a session (all nodes).
    pub fn get_all_sheaf_validations(&self, session_id: &str) -> Result<Vec<SheafValidationRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, node_id, validator_class, plugin_source, passed, evidence_summary, \
             affected_files, v_sheaf_contribution, requeue_targets
             FROM sheaf_validations WHERE session_id = ? ORDER BY created_at",
        )?;
        let mut rows = stmt.query([session_id])?;
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
}
