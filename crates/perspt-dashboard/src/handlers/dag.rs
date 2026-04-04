use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;
use crate::views::dag::{DagEdge, DagNode, DagViewModel};

#[derive(Template)]
#[template(path = "pages/dag.html")]
struct DagTemplate {
    title: String,
    session_id: String,
    nodes: Vec<DagNode>,
    edges: Vec<DagEdge>,
}

pub async fn dag_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, DashboardError> {
    let nodes = state.store.get_latest_node_states(&session_id)?;
    let edges = state.store.get_task_graph_edges(&session_id)?;
    let vm = DagViewModel::from_store(session_id.clone(), nodes, edges);

    let tmpl = DagTemplate {
        title: "DAG Topology".to_string(),
        session_id: vm.session_id,
        nodes: vm.nodes,
        edges: vm.edges,
    };
    Ok(Html(tmpl.render()?))
}
