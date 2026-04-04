use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;
use crate::views::dag::{DagEdge, DagNode, DagViewModel};

#[derive(Template)]
#[template(path = "pages/dag.html")]
struct DagTemplate {
    session_id: String,
    active_tab: String,
    title: String,
    nodes: Vec<DagNode>,
    edges: Vec<DagEdge>,
    total_nodes: usize,
    committed_nodes: usize,
    failed_nodes: usize,
    running_nodes: usize,
}

pub async fn dag_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, DashboardError> {
    let nodes = state.store.get_latest_node_states(&session_id)?;
    let edges = state.store.get_task_graph_edges(&session_id)?;
    let vm = DagViewModel::from_store(session_id.clone(), nodes, edges);

    let total_nodes = vm.nodes.len();
    let committed_nodes = vm.nodes.iter().filter(|n| n.state == "completed").count();
    let failed_nodes = vm.nodes.iter().filter(|n| n.state == "failed").count();
    let running_nodes = vm.nodes.iter().filter(|n| n.state == "running").count();

    let tmpl = DagTemplate {
        session_id: vm.session_id,
        active_tab: "dag".to_string(),
        title: "DAG Topology".to_string(),
        nodes: vm.nodes,
        edges: vm.edges,
        total_nodes,
        committed_nodes,
        failed_nodes,
        running_nodes,
    };
    Ok(Html(tmpl.render()?))
}
