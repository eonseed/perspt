use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;
use crate::views::backlog::{BacklogViewModel, NodeEnergyRow, StateCountRow};
use crate::views::friendly_name;
use crate::views::psp9::LedgerProjection;

#[derive(Template)]
#[template(path = "pages/backlog.html")]
struct BacklogTemplate {
    session_id: String,
    display_name: String,
    active_tab: String,
    title: String,
    state_counts: Vec<StateCountRow>,
    backlog_nodes: usize,
    unmeasured_backlog_nodes: usize,
    phi: String,
    drift: String,
    measurement_count: usize,
    node_rows: Vec<NodeEnergyRow>,
    has_revision: bool,
}

/// Route: `GET /sessions/{id}/backlog`.
pub async fn backlog_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, DashboardError> {
    let rows = state.store.get_psp9_events(&session_id)?;
    let projection = LedgerProjection::from_rows(&rows);
    let vm = BacklogViewModel::from_projection(session_id, &projection);

    let tmpl = BacklogTemplate {
        display_name: friendly_name(&vm.session_id),
        session_id: vm.session_id,
        active_tab: "backlog".to_string(),
        title: "Backlog Diagnostics".to_string(),
        state_counts: vm.state_counts,
        backlog_nodes: vm.backlog_nodes,
        unmeasured_backlog_nodes: vm.unmeasured_backlog_nodes,
        phi: vm.phi,
        drift: vm.drift,
        measurement_count: vm.measurement_count,
        node_rows: vm.node_rows,
        has_revision: vm.has_revision,
    };
    Ok(Html(tmpl.render()?))
}
