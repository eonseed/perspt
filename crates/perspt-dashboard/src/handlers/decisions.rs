use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;
use crate::views::decisions::{
    DecisionsViewModel, EscalationRow, PlanRow, RepairRow, RewriteRow, SheafRow, VerificationRow,
};

#[derive(Template)]
#[template(path = "pages/decisions.html")]
struct DecisionsTemplate {
    session_id: String,
    active_tab: String,
    title: String,
    escalations: Vec<EscalationRow>,
    sheaf_validations: Vec<SheafRow>,
    rewrites: Vec<RewriteRow>,
    plan_revisions: Vec<PlanRow>,
    repair_footprints: Vec<RepairRow>,
    verifications: Vec<VerificationRow>,
    total_decisions: usize,
}

pub async fn decisions_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, DashboardError> {
    let escalations = state.store.get_escalation_reports(&session_id)?;
    let sheaf_validations = state.store.get_all_sheaf_validations(&session_id)?;
    let rewrites = state.store.get_rewrite_records(&session_id)?;
    let plan_revisions = state.store.get_plan_revisions(&session_id)?;
    let repair_footprints = state.store.get_all_repair_footprints(&session_id)?;
    let verifications = state.store.get_all_verification_results(&session_id)?;

    let vm = DecisionsViewModel::from_store(
        session_id.clone(),
        escalations,
        sheaf_validations,
        rewrites,
        plan_revisions,
        repair_footprints,
        verifications,
    );

    let total_decisions = vm.escalations.len()
        + vm.sheaf_validations.len()
        + vm.rewrites.len()
        + vm.plan_revisions.len()
        + vm.repair_footprints.len()
        + vm.verifications.len();

    let tmpl = DecisionsTemplate {
        session_id: vm.session_id,
        active_tab: "decisions".to_string(),
        title: "Decision Trace".to_string(),
        escalations: vm.escalations,
        sheaf_validations: vm.sheaf_validations,
        rewrites: vm.rewrites,
        plan_revisions: vm.plan_revisions,
        repair_footprints: vm.repair_footprints,
        verifications: vm.verifications,
        total_decisions,
    };
    Ok(Html(tmpl.render()?))
}
