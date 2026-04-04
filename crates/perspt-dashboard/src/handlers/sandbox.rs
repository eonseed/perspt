use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;
use crate::views::sandbox::{SandboxBranch, SandboxViewModel};

#[derive(Template)]
#[template(path = "pages/sandbox.html")]
struct SandboxTemplate {
    title: String,
    session_id: String,
    branches: Vec<SandboxBranch>,
}

pub async fn sandbox_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, DashboardError> {
    let rows = state.store.get_provisional_branches(&session_id)?;
    let vm = SandboxViewModel::from_store(session_id.clone(), rows);

    let tmpl = SandboxTemplate {
        title: "Sandbox Monitoring".to_string(),
        session_id: vm.session_id,
        branches: vm.branches,
    };
    Ok(Html(tmpl.render()?))
}
