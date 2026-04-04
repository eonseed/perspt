use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;
use crate::views::overview::BudgetSummary;
use crate::views::session_detail::{NodeSummaryRow, SessionDetailViewModel, VerifSummaryRow};

#[derive(Template)]
#[template(path = "pages/session_detail.html")]
struct SessionDetailTemplate {
    session_id: String,
    active_tab: String,
    task: String,
    working_dir: String,
    status: String,
    toolchain: String,
    total_nodes: usize,
    completed_nodes: usize,
    failed_nodes: usize,
    running_nodes: usize,
    llm_request_count: usize,
    llm_tokens_in: i64,
    llm_tokens_out: i64,
    avg_energy: f32,
    budget: Option<BudgetSummary>,
    nodes: Vec<NodeSummaryRow>,
    verifications: Vec<VerifSummaryRow>,
}

pub async fn session_detail_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, DashboardError> {
    let session = state.store.get_session(&session_id)?;
    let (task, working_dir, status, toolchain) = match session {
        Some(s) => (s.task, s.working_dir, s.status, s.detected_toolchain),
        None => (
            "Unknown".to_string(),
            String::new(),
            "unknown".to_string(),
            None,
        ),
    };

    let nodes = state
        .store
        .get_latest_node_states(&session_id)
        .unwrap_or_default();
    let llm_records = state
        .store
        .get_llm_requests(&session_id)
        .unwrap_or_default();
    let energy_records = state
        .store
        .get_session_energy_history(&session_id)
        .unwrap_or_default();
    let budget = state.store.get_budget_envelope(&session_id).ok().flatten();
    let verifications = state
        .store
        .get_all_verification_results(&session_id)
        .unwrap_or_default();

    let vm = SessionDetailViewModel::from_store(
        session_id,
        task,
        working_dir,
        status,
        toolchain,
        nodes,
        &llm_records,
        &energy_records,
        budget,
        verifications,
    );

    let tmpl = SessionDetailTemplate {
        session_id: vm.session_id,
        active_tab: "summary".to_string(),
        task: vm.task,
        working_dir: vm.working_dir,
        status: vm.status,
        toolchain: vm.toolchain,
        total_nodes: vm.total_nodes,
        completed_nodes: vm.completed_nodes,
        failed_nodes: vm.failed_nodes,
        running_nodes: vm.running_nodes,
        llm_request_count: vm.llm_request_count,
        llm_tokens_in: vm.llm_tokens_in,
        llm_tokens_out: vm.llm_tokens_out,
        avg_energy: vm.avg_energy,
        budget: vm.budget,
        nodes: vm.nodes,
        verifications: vm.verifications,
    };
    Ok(Html(tmpl.render()?))
}
