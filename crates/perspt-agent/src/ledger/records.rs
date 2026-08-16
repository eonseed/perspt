use super::*;

impl MerkleLedger {
    // =========================================================================
    // PSP-5 Phase 6: Provisional Branch, Interface Seal, Branch Flush Facades
    // =========================================================================

    /// Get the current session ID (helper for Phase 6 methods)
    pub(crate) fn session_id(&self) -> Result<String> {
        self.current_session
            .as_ref()
            .map(|s| s.session_id.clone())
            .context("No active session")
    }

    /// Record a new provisional branch for speculative child work
    pub fn record_provisional_branch(
        &self,
        branch: &perspt_core::types::ProvisionalBranch,
    ) -> Result<()> {
        let row = perspt_store::ProvisionalBranchRow {
            branch_id: branch.branch_id.clone(),
            session_id: branch.session_id.clone(),
            node_id: branch.node_id.clone(),
            parent_node_id: branch.parent_node_id.clone(),
            state: branch.state.to_string(),
            parent_seal_hash: branch.parent_seal_hash.map(|h| h.to_vec()),
            sandbox_dir: branch.sandbox_dir.clone(),
        };

        self.store.record_provisional_branch(&row)?;
        log::debug!(
            "Recorded provisional branch '{}' for node '{}' (parent: '{}')",
            branch.branch_id,
            branch.node_id,
            branch.parent_node_id
        );
        Ok(())
    }

    /// Update a provisional branch state
    pub fn update_branch_state(&self, branch_id: &str, new_state: &str) -> Result<()> {
        self.store.update_branch_state(branch_id, new_state)?;
        log::debug!("Updated branch '{}' state to '{}'", branch_id, new_state);
        Ok(())
    }

    /// Get all provisional branches for the current session
    pub fn get_provisional_branches(&self) -> Result<Vec<perspt_store::ProvisionalBranchRow>> {
        let session_id = self.session_id()?;
        self.store.get_provisional_branches(&session_id)
    }

    /// Get live (active/sealed) branches depending on a parent node
    pub fn get_live_branches_for_parent(
        &self,
        parent_node_id: &str,
    ) -> Result<Vec<perspt_store::ProvisionalBranchRow>> {
        let session_id = self.session_id()?;
        self.store
            .get_live_branches_for_parent(&session_id, parent_node_id)
    }

    /// Flush all live branches for a parent node and return flushed branch IDs
    pub fn flush_branches_for_parent(&self, parent_node_id: &str) -> Result<Vec<String>> {
        let session_id = self.session_id()?;
        self.store
            .flush_branches_for_parent(&session_id, parent_node_id)
    }

    /// Record a branch lineage edge (parent branch → child branch)
    pub fn record_branch_lineage(&self, lineage: &perspt_core::types::BranchLineage) -> Result<()> {
        let row = perspt_store::BranchLineageRow {
            lineage_id: lineage.lineage_id.clone(),
            parent_branch_id: lineage.parent_branch_id.clone(),
            child_branch_id: lineage.child_branch_id.clone(),
            depends_on_seal: lineage.depends_on_seal,
        };

        self.store.record_branch_lineage(&row)?;
        log::debug!(
            "Recorded branch lineage: {} → {}",
            lineage.parent_branch_id,
            lineage.child_branch_id
        );
        Ok(())
    }

    /// Record an interface seal for a node
    pub fn record_interface_seal(
        &self,
        seal: &perspt_core::types::InterfaceSealRecord,
    ) -> Result<()> {
        let row = perspt_store::InterfaceSealRow {
            seal_id: seal.seal_id.clone(),
            session_id: seal.session_id.clone(),
            node_id: seal.node_id.clone(),
            sealed_path: seal.sealed_path.clone(),
            artifact_kind: seal.artifact_kind.to_string(),
            seal_hash: seal.seal_hash.to_vec(),
            version: seal.version as i32,
        };

        self.store.record_interface_seal(&row)?;
        log::debug!(
            "Recorded interface seal '{}' for node '{}' at '{}'",
            seal.seal_id,
            seal.node_id,
            seal.sealed_path
        );
        Ok(())
    }

    /// Get all interface seals for a node in the current session
    pub fn get_interface_seals(
        &self,
        node_id: &str,
    ) -> Result<Vec<perspt_store::InterfaceSealRow>> {
        let session_id = self.session_id()?;
        self.store.get_interface_seals(&session_id, node_id)
    }

    /// Record a branch flush decision
    pub fn record_branch_flush(&self, flush: &perspt_core::types::BranchFlushRecord) -> Result<()> {
        let row = perspt_store::BranchFlushRow {
            flush_id: flush.flush_id.clone(),
            session_id: flush.session_id.clone(),
            parent_node_id: flush.parent_node_id.clone(),
            flushed_branch_ids: serde_json::to_string(&flush.flushed_branch_ids)
                .unwrap_or_default(),
            requeue_node_ids: serde_json::to_string(&flush.requeue_node_ids).unwrap_or_default(),
            reason: flush.reason.clone(),
        };

        self.store.record_branch_flush(&row)?;
        log::debug!(
            "Recorded branch flush for parent '{}': {} branches flushed",
            flush.parent_node_id,
            flush.flushed_branch_ids.len()
        );
        Ok(())
    }

    /// Get all branch flush records for the current session
    pub fn get_branch_flushes(&self) -> Result<Vec<perspt_store::BranchFlushRow>> {
        let session_id = self.session_id()?;
        self.store.get_branch_flushes(&session_id)
    }

    // =========================================================================
    // PSP-5 Phase 7: Review Outcome Persistence
    // =========================================================================

    /// Persist a review decision as an audit record.
    pub fn record_review_outcome(
        &self,
        node_id: &str,
        outcome: &str,
        reviewer_note: Option<&str>,
        energy_at_review: Option<f64>,
        degraded: Option<bool>,
        escalation_category: Option<&str>,
    ) -> Result<()> {
        let session_id = self.session_id()?;
        let row = perspt_store::ReviewOutcomeRow {
            session_id,
            node_id: node_id.to_string(),
            outcome: outcome.to_string(),
            reviewer_note: reviewer_note.map(|s| s.to_string()),
            energy_at_review,
            degraded,
            escalation_category: escalation_category.map(|s| s.to_string()),
        };
        self.store.record_review_outcome(&row)
    }

    /// Get all review outcomes for a node.
    pub fn get_review_outcomes(
        &self,
        node_id: &str,
    ) -> Result<Vec<perspt_store::ReviewOutcomeRow>> {
        let session_id = self.session_id()?;
        self.store.get_review_outcomes(&session_id, node_id)
    }

    /// Get all review outcomes across the session.
    pub fn get_all_review_outcomes(&self) -> Result<Vec<perspt_store::ReviewOutcomeRow>> {
        let session_id = self.session_id()?;
        self.store.get_all_review_outcomes(&session_id)
    }

    // =========================================================================
    // PSP-5 Phase 7: Shared Review & Provenance Aggregation Helpers
    // =========================================================================

    /// Build a review-ready summary for a single node.
    ///
    /// Aggregates energy history, escalation reports, sheaf validations,
    /// context provenance, interface seals, and branch state from the store
    /// into a single struct consumable by both TUI and CLI surfaces.
    pub fn node_review_summary(&self, node_id: &str) -> Result<NodeReviewSummary> {
        let session_id = self.session_id()?;

        let energy_history = self
            .store
            .get_energy_history(&session_id, node_id)
            .unwrap_or_default();

        let latest_energy = energy_history.last().cloned();

        let escalation_reports = self
            .store
            .get_escalation_reports(&session_id)
            .unwrap_or_default()
            .into_iter()
            .filter(|r| r.node_id == node_id)
            .collect::<Vec<_>>();

        let sheaf_validations = self
            .store
            .get_sheaf_validations(&session_id, node_id)
            .unwrap_or_default();

        let interface_seals = self
            .store
            .get_interface_seals(&session_id, node_id)
            .unwrap_or_default();

        let context_provenance = self
            .store
            .get_context_provenance(&session_id, node_id)
            .ok()
            .flatten()
            .into_iter()
            .collect::<Vec<_>>();

        let branches: Vec<_> = self
            .store
            .get_provisional_branches(&session_id)
            .unwrap_or_default()
            .into_iter()
            .filter(|b| b.node_id == node_id)
            .collect();

        let attempt_count = energy_history.len().max(1) as u32;

        Ok(NodeReviewSummary {
            node_id: node_id.to_string(),
            latest_energy,
            energy_history,
            attempt_count,
            escalation_reports,
            sheaf_validations,
            interface_seals,
            context_provenance,
            branches,
        })
    }

    /// Build a session-level summary aggregating lifecycle counts, energy
    /// stats, escalation activity, and branch provenance.
    pub fn session_summary(&self) -> Result<SessionReviewSummary> {
        let session_id = self.session_id()?;

        let node_states = self.store.get_node_states(&session_id).unwrap_or_default();
        let total_nodes = node_states.len();
        let completed = node_states
            .iter()
            .filter(|n| n.state == "COMPLETED" || n.state == "STABLE")
            .count();
        let failed = node_states.iter().filter(|n| n.state == "FAILED").count();
        let escalated = node_states
            .iter()
            .filter(|n| n.state == "Escalated")
            .count();

        // Collect latest energy per node
        let mut total_energy: f32 = 0.0;
        let mut node_energies: Vec<(String, perspt_store::EnergyRecord)> = Vec::new();
        for ns in &node_states {
            if let Ok(history) = self.store.get_energy_history(&session_id, &ns.node_id) {
                if let Some(latest) = history.last() {
                    total_energy += latest.v_total;
                    node_energies.push((ns.node_id.clone(), latest.clone()));
                }
            }
        }

        let escalation_reports = self
            .store
            .get_escalation_reports(&session_id)
            .unwrap_or_default();

        let branches = self
            .store
            .get_provisional_branches(&session_id)
            .unwrap_or_default();

        let active_branches = branches.iter().filter(|b| b.state == "active").count();
        let sealed_branches = branches.iter().filter(|b| b.state == "sealed").count();
        let merged_branches = branches.iter().filter(|b| b.state == "merged").count();
        let flushed_branches = branches.iter().filter(|b| b.state == "flushed").count();

        let flushes = self
            .store
            .get_branch_flushes(&session_id)
            .unwrap_or_default();

        let (review_total, reviews_approved, reviews_rejected, reviews_corrected) =
            self.review_tallies(&session_id);

        Ok(SessionReviewSummary {
            session_id,
            total_nodes,
            completed,
            failed,
            escalated,
            total_energy,
            node_energies,
            escalation_reports,
            branches_total: branches.len(),
            active_branches,
            sealed_branches,
            merged_branches,
            flushed_branches,
            flush_decisions: flushes,
            review_total,
            reviews_approved,
            reviews_rejected,
            reviews_corrected,
        })
    }

    /// Tally review outcomes: (total, approved, rejected, corrected).
    fn review_tallies(&self, session_id: &str) -> (usize, usize, usize, usize) {
        let review_outcomes = self
            .store
            .get_all_review_outcomes(session_id)
            .unwrap_or_default();
        let approved = review_outcomes
            .iter()
            .filter(|r| r.outcome.starts_with("approved") || r.outcome == "auto_approved")
            .count();
        let rejected = review_outcomes
            .iter()
            .filter(|r| r.outcome == "rejected" || r.outcome == "aborted")
            .count();
        let corrected = review_outcomes
            .iter()
            .filter(|r| r.outcome == "correction_requested")
            .count();
        (review_outcomes.len(), approved, rejected, corrected)
    }
}
