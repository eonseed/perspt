use super::*;

impl SessionStore {
    // =========================================================================
    // PSP-5 Phase 6: Provisional Branch CRUD
    // =========================================================================

    /// Record a new provisional branch
    pub fn record_provisional_branch(&self, record: &ProvisionalBranchRow) -> Result<()> {
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO provisional_branches (branch_id, session_id, node_id, parent_node_id, state,
                parent_seal_hash, sandbox_dir)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
            [
                &record.branch_id,
                &record.session_id,
                &record.node_id,
                &record.parent_node_id,
                &record.state,
                &record
                    .parent_seal_hash
                    .as_ref()
                    .map(hex::encode)
                    .unwrap_or_default(),
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
            // parent_seal_hash is BLOB; read directly as Option<Vec<u8>>
            let parent_seal_hash: Option<Vec<u8>> = row.get(5).ok();
            records.push(ProvisionalBranchRow {
                branch_id: row.get(0)?,
                session_id: row.get(1)?,
                node_id: row.get(2)?,
                parent_node_id: row.get(3)?,
                state: row.get(4)?,
                parent_seal_hash,
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
            // parent_seal_hash is BLOB; read directly as Option<Vec<u8>>
            let parent_seal_hash: Option<Vec<u8>> = row.get(5).ok();
            records.push(ProvisionalBranchRow {
                branch_id: row.get(0)?,
                session_id: row.get(1)?,
                node_id: row.get(2)?,
                parent_node_id: row.get(3)?,
                state: row.get(4)?,
                parent_seal_hash,
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
            INSERT INTO interface_seals (seal_id, session_id, node_id, sealed_path, artifact_kind,
                seal_hash, version)
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
            INSERT INTO branch_flushes (flush_id, session_id, parent_node_id, flushed_branch_ids,
                requeue_node_ids, reason)
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
}
