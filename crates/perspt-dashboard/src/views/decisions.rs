use perspt_store::{
    EscalationReportRecord, PlanRevisionRow, RepairFootprintRow, RewriteRecordRow,
    SheafValidationRow, VerificationResultRow,
};

/// View model for the decisions trace page
pub struct DecisionsViewModel {
    pub session_id: String,
    pub escalations: Vec<EscalationRow>,
    pub sheaf_validations: Vec<SheafRow>,
    pub rewrites: Vec<RewriteRow>,
    pub plan_revisions: Vec<PlanRow>,
    pub repair_footprints: Vec<RepairRow>,
    pub verifications: Vec<VerificationRow>,
}

pub struct EscalationRow {
    pub node_id: String,
    pub category: String,
    pub action: String,
    pub evidence: String,
}

pub struct SheafRow {
    pub node_id: String,
    pub validator_class: String,
    pub passed: bool,
    pub evidence_summary: String,
    pub v_sheaf_contribution: f32,
}

pub struct RewriteRow {
    pub node_id: String,
    pub action: String,
    pub category: String,
    pub requeued_nodes: String,
    pub inserted_nodes: String,
}

pub struct PlanRow {
    pub revision_id: String,
    pub sequence: i32,
    pub reason: String,
    pub status: String,
}

pub struct RepairRow {
    pub node_id: String,
    pub attempt: i32,
    pub diagnosis: String,
    pub resolved: bool,
}

pub struct VerificationRow {
    pub node_id: String,
    pub syntax_ok: bool,
    pub build_ok: bool,
    pub tests_ok: bool,
    pub lint_ok: bool,
    pub tests_passed: i32,
    pub tests_failed: i32,
    pub degraded: bool,
    pub degraded_reason: String,
}

impl DecisionsViewModel {
    pub fn from_store(
        session_id: String,
        escalations: Vec<EscalationReportRecord>,
        sheaf_validations: Vec<SheafValidationRow>,
        rewrites: Vec<RewriteRecordRow>,
        plan_revisions: Vec<PlanRevisionRow>,
        repair_footprints: Vec<RepairFootprintRow>,
        verifications: Vec<VerificationResultRow>,
    ) -> Self {
        Self {
            session_id,
            escalations: escalations
                .into_iter()
                .map(|e| EscalationRow {
                    node_id: e.node_id,
                    category: e.category,
                    action: e.action,
                    evidence: e.evidence,
                })
                .collect(),
            sheaf_validations: sheaf_validations
                .into_iter()
                .map(|s| SheafRow {
                    node_id: s.node_id,
                    validator_class: s.validator_class,
                    passed: s.passed,
                    evidence_summary: s.evidence_summary,
                    v_sheaf_contribution: s.v_sheaf_contribution,
                })
                .collect(),
            rewrites: rewrites
                .into_iter()
                .map(|r| RewriteRow {
                    node_id: r.node_id,
                    action: r.action,
                    category: r.category,
                    requeued_nodes: r.requeued_nodes,
                    inserted_nodes: r.inserted_nodes,
                })
                .collect(),
            plan_revisions: plan_revisions
                .into_iter()
                .map(|p| PlanRow {
                    revision_id: p.revision_id,
                    sequence: p.sequence,
                    reason: p.reason,
                    status: p.status,
                })
                .collect(),
            repair_footprints: repair_footprints
                .into_iter()
                .map(|r| RepairRow {
                    node_id: r.node_id,
                    attempt: r.attempt,
                    diagnosis: r.diagnosis,
                    resolved: r.resolved,
                })
                .collect(),
            verifications: verifications
                .into_iter()
                .map(|v| VerificationRow {
                    node_id: v.node_id,
                    syntax_ok: v.syntax_ok,
                    build_ok: v.build_ok,
                    tests_ok: v.tests_ok,
                    lint_ok: v.lint_ok,
                    tests_passed: v.tests_passed,
                    tests_failed: v.tests_failed,
                    degraded: v.degraded,
                    degraded_reason: v.degraded_reason.unwrap_or_default(),
                })
                .collect(),
        }
    }
}
