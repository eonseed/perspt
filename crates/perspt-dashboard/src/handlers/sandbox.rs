use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;
use crate::views::friendly_name;
use crate::views::sandbox::{SandboxBranch, SandboxViewModel};

#[derive(Template)]
#[template(path = "pages/sandbox.html")]
struct SandboxTemplate {
    session_id: String,
    display_name: String,
    active_tab: String,
    title: String,
    branches: Vec<SandboxBranch>,
    active_count: usize,
    merged_count: usize,
    flushed_count: usize,
}

pub async fn sandbox_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, DashboardError> {
    let rows = state.store.get_provisional_branches(&session_id)?;
    let vm = SandboxViewModel::from_store(session_id.clone(), rows);

    let active_count = vm.branches.iter().filter(|b| b.state == "active").count();
    let merged_count = vm.branches.iter().filter(|b| b.state == "merged").count();
    let flushed_count = vm.branches.iter().filter(|b| b.state == "flushed").count();

    let tmpl = SandboxTemplate {
        display_name: friendly_name(&vm.session_id),
        session_id: vm.session_id,
        active_tab: "sandbox".to_string(),
        title: "Sandbox Monitoring".to_string(),
        branches: vm.branches,
        active_count,
        merged_count,
        flushed_count,
    };
    Ok(Html(tmpl.render()?))
}
