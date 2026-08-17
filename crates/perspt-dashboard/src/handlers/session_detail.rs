use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;
use crate::views::friendly_name;
use crate::views::psp9::LedgerProjection;
use crate::views::session_detail::{NodeSummaryRow, SessionDetailViewModel};

#[derive(Template)]
#[template(path = "pages/session_detail.html")]
struct SessionDetailTemplate {
    session_id: String,
    display_name: String,
    active_tab: String,
    task: String,
    working_dir: String,
    status: String,
    toolchain: String,
    total_nodes: usize,
    stable_nodes: usize,
    running_nodes: usize,
    event_count: usize,
    measurement_count: usize,
    last_energy: String,
    avg_energy: String,
    nodes: Vec<NodeSummaryRow>,
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

    let rows = state.store.get_psp9_events(&session_id).unwrap_or_default();
    let projection = LedgerProjection::from_rows(&rows);
    let vm = SessionDetailViewModel::from_store(
        session_id,
        task,
        working_dir,
        status,
        toolchain,
        &projection,
    );

    let tmpl = SessionDetailTemplate {
        display_name: friendly_name(&vm.session_id),
        session_id: vm.session_id,
        active_tab: "summary".to_string(),
        task: vm.task,
        working_dir: vm.working_dir,
        status: vm.status,
        toolchain: vm.toolchain,
        total_nodes: vm.total_nodes,
        stable_nodes: vm.stable_nodes,
        running_nodes: vm.running_nodes,
        event_count: vm.event_count,
        measurement_count: vm.measurement_count,
        last_energy: vm.last_energy,
        avg_energy: vm.avg_energy,
        nodes: vm.nodes,
    };
    Ok(Html(tmpl.render()?))
}
