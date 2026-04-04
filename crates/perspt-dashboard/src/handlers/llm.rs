use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;

#[derive(askama::Template)]
#[template(path = "pages/llm.html")]
struct LlmTemplate {
    title: String,
    session_id: String,
}

pub async fn llm_handler(
    State(_state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, DashboardError> {
    let tmpl = LlmTemplate {
        title: "LLM Telemetry".to_string(),
        session_id,
    };
    Ok(Html(tmpl.render()?))
}
