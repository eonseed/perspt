use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;

#[derive(askama::Template)]
#[template(path = "pages/sandbox.html")]
struct SandboxTemplate {
    title: String,
    session_id: String,
}

pub async fn sandbox_handler(
    State(_state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, DashboardError> {
    let tmpl = SandboxTemplate {
        title: "Sandbox Monitoring".to_string(),
        session_id,
    };
    Ok(Html(tmpl.render()?))
}
