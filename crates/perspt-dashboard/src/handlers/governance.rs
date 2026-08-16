use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;
use crate::views::friendly_name;
use crate::views::governance::{EpochRow, GovernanceViewModel, PendingAuditRow, VerdictRow};

#[derive(Template)]
#[template(path = "pages/governance.html")]
struct GovernanceTemplate {
    session_id: String,
    display_name: String,
    active_tab: String,
    title: String,
    authority_epoch: u64,
    grant_signed: bool,
    epochs: Vec<EpochRow>,
    verdicts: Vec<VerdictRow>,
    pending_audits: Vec<PendingAuditRow>,
}

pub async fn governance_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, DashboardError> {
    let authority_epoch = state.store.authority_epoch(&session_id).unwrap_or(0);
    let grant_policy = state.store.get_grant_policy(&session_id).unwrap_or(None);
    let epochs = state
        .store
        .all_psp9_calibration_epochs(50)
        .unwrap_or_default();
    let verdicts = state
        .store
        .get_psp9_verdicts(&session_id)
        .unwrap_or_default();
    let pending = state
        .store
        .pending_psp9_audit_samples(50)
        .unwrap_or_default();

    let vm = GovernanceViewModel::from_store(
        session_id,
        authority_epoch,
        grant_policy,
        epochs,
        verdicts,
        pending,
    );
    let tmpl = GovernanceTemplate {
        display_name: friendly_name(&vm.session_id),
        session_id: vm.session_id,
        active_tab: "governance".to_string(),
        title: "Governance".to_string(),
        authority_epoch: vm.authority_epoch,
        grant_signed: vm.grant_signed,
        epochs: vm.epochs,
        verdicts: vm.verdicts,
        pending_audits: vm.pending_audits,
    };
    Ok(Html(tmpl.render()?))
}
