use super::*;

// =========================================================================
// Plan Revision, Feature Charter, Repair Footprint, Budget Envelope Facades
// =========================================================================

impl MerkleLedger {
    /// Record a feature charter for the current session.
    pub fn record_feature_charter(&self, charter: &perspt_core::FeatureCharter) -> Result<()> {
        let session_id = self.session_id()?;
        let row = perspt_store::FeatureCharterRow {
            charter_id: charter.charter_id.clone(),
            session_id,
            scope_description: charter.scope_description.clone(),
            max_modules: charter.max_modules.map(|v| v as i32),
            max_files: charter.max_files.map(|v| v as i32),
            max_revisions: charter.max_revisions.map(|v| v as i32),
            language_constraint: charter.language_constraint.clone(),
        };
        self.store.record_feature_charter(&row)?;
        log::debug!("Recorded feature charter '{}'", charter.charter_id);
        Ok(())
    }

    /// Get the feature charter for the current session.
    pub fn get_feature_charter(&self) -> Result<Option<perspt_store::FeatureCharterRow>> {
        let session_id = self.session_id()?;
        self.store.get_feature_charter(&session_id)
    }

    /// Record a plan revision for the current session.
    pub fn record_plan_revision(&self, revision: &perspt_core::PlanRevision) -> Result<()> {
        let session_id = self.session_id()?;
        let plan_json = serde_json::to_string(&revision.plan).unwrap_or_default();
        let row = perspt_store::PlanRevisionRow {
            revision_id: revision.revision_id.clone(),
            session_id,
            sequence: revision.sequence as i32,
            plan_json,
            reason: revision.reason.clone(),
            supersedes: revision.supersedes.clone(),
            status: revision.status.to_string(),
        };
        self.store.record_plan_revision(&row)?;
        log::debug!(
            "Recorded plan revision '{}' (seq={}, status={})",
            revision.revision_id,
            revision.sequence,
            revision.status
        );
        Ok(())
    }

    /// Get the active plan revision for the current session.
    pub fn get_active_plan_revision(&self) -> Result<Option<perspt_store::PlanRevisionRow>> {
        let session_id = self.session_id()?;
        self.store.get_active_plan_revision(&session_id)
    }

    /// Get all plan revisions for the current session.
    pub fn get_plan_revisions(&self) -> Result<Vec<perspt_store::PlanRevisionRow>> {
        let session_id = self.session_id()?;
        self.store.get_plan_revisions(&session_id)
    }

    /// Supersede a plan revision by ID.
    pub fn supersede_plan_revision(&self, revision_id: &str) -> Result<()> {
        self.store.supersede_plan_revision(revision_id)?;
        log::debug!("Superseded plan revision '{}'", revision_id);
        Ok(())
    }

    /// Record a repair footprint for a node.
    pub fn record_repair_footprint(&self, footprint: &perspt_core::RepairFootprint) -> Result<()> {
        let session_id = self.session_id()?;
        let row = perspt_store::RepairFootprintRow {
            footprint_id: footprint.footprint_id.clone(),
            session_id,
            node_id: footprint.node_id.clone(),
            revision_id: footprint.revision_id.clone(),
            attempt: footprint.attempt as i32,
            affected_files: serde_json::to_string(&footprint.affected_files).unwrap_or_default(),
            bundle_json: serde_json::to_string(&footprint.applied_bundle).unwrap_or_default(),
            diagnosis: footprint.diagnosis.clone(),
            resolved: footprint.resolved,
        };
        self.store.record_repair_footprint(&row)?;
        log::debug!(
            "Recorded repair footprint '{}' for node '{}' (attempt {})",
            footprint.footprint_id,
            footprint.node_id,
            footprint.attempt
        );
        Ok(())
    }

    /// Get repair footprints for a node in the current session.
    pub fn get_repair_footprints(
        &self,
        node_id: &str,
    ) -> Result<Vec<perspt_store::RepairFootprintRow>> {
        let session_id = self.session_id()?;
        self.store.get_repair_footprints(&session_id, node_id)
    }

    /// Record or update the budget envelope for the current session.
    pub fn upsert_budget_envelope(&self, budget: &perspt_core::BudgetEnvelope) -> Result<()> {
        let session_id = self.session_id()?;
        let row = perspt_store::BudgetEnvelopeRow {
            session_id,
            max_steps: budget.max_steps.map(|v| v as i32),
            steps_used: budget.steps_used as i32,
            max_revisions: budget.max_revisions.map(|v| v as i32),
            revisions_used: budget.revisions_used as i32,
            max_cost_usd: budget.max_cost_usd,
            cost_used_usd: budget.cost_used_usd,
        };
        self.store.upsert_budget_envelope(&row)?;
        log::debug!("Upserted budget envelope for session");
        Ok(())
    }

    /// Get the budget envelope for the current session.
    pub fn get_budget_envelope(&self) -> Result<Option<perspt_store::BudgetEnvelopeRow>> {
        let session_id = self.session_id()?;
        self.store.get_budget_envelope(&session_id)
    }

    // =====================================================================
    // PSP-7: SRBN step records and correction attempt wrappers
    // =====================================================================

    /// Record an orchestration step transition for the current session.
    pub fn record_step(&self, record: &perspt_store::SrbnStepRecord) -> Result<()> {
        self.store.record_step(record)
    }

    /// Retrieve the step timeline for a node in the current session.
    pub fn get_step_timeline(&self, node_id: &str) -> Result<Vec<perspt_store::SrbnStepRecord>> {
        let session_id = self.session_id()?;
        self.store.get_step_timeline(&session_id, node_id)
    }

    /// Retrieve all step records for the current session.
    pub fn get_session_steps(&self) -> Result<Vec<perspt_store::SrbnStepRecord>> {
        let session_id = self.session_id()?;
        self.store.get_session_steps(&session_id)
    }

    /// Record a correction attempt for the current session.
    pub fn record_correction_attempt(
        &self,
        record: &perspt_store::CorrectionAttemptRow,
    ) -> Result<()> {
        self.store.record_correction_attempt(record)
    }

    /// Retrieve all correction attempts for a node in the current session.
    pub fn get_correction_attempts(
        &self,
        node_id: &str,
    ) -> Result<Vec<perspt_store::CorrectionAttemptRow>> {
        let session_id = self.session_id()?;
        self.store.get_correction_attempts(&session_id, node_id)
    }
}
