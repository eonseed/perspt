use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;
use crate::views::dag::{RevisionRow, TopoEdgeRow, TopoNodeRow, TopologyViewModel};
use crate::views::friendly_name;
use crate::views::psp9::LedgerProjection;

#[derive(Template)]
#[template(path = "pages/dag.html")]
struct TopologyTemplate {
    session_id: String,
    display_name: String,
    active_tab: String,
    title: String,
    revisions: Vec<RevisionRow>,
    latest_revision_short: String,
    nodes: Vec<TopoNodeRow>,
    edges: Vec<TopoEdgeRow>,
    total_nodes: usize,
    stable_nodes: usize,
    running_nodes: usize,
    stopped_nodes: usize,
}

/// Routes: `GET /sessions/{id}/dag` and `GET /sessions/{id}/topology`.
pub async fn topology_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, DashboardError> {
    let rows = state.store.get_psp9_events(&session_id)?;
    let projection = LedgerProjection::from_rows(&rows);
    let vm = TopologyViewModel::from_projection(session_id, &projection);

    let tmpl = TopologyTemplate {
        display_name: friendly_name(&vm.session_id),
        session_id: vm.session_id,
        active_tab: "dag".to_string(),
        title: "Topology".to_string(),
        revisions: vm.revisions,
        latest_revision_short: vm.latest_revision_short,
        total_nodes: vm.nodes.len(),
        nodes: vm.nodes,
        edges: vm.edges,
        stable_nodes: vm.stable_nodes,
        running_nodes: vm.running_nodes,
        stopped_nodes: vm.stopped_nodes,
    };
    Ok(Html(tmpl.render()?))
}
