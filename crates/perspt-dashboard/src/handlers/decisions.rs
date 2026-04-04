use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;

#[derive(askama::Template)]
#[template(path = "pages/decisions.html")]
struct DecisionsTemplate {
    title: String,
    session_id: String,
}

pub async fn decisions_handler(
    State(_state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, DashboardError> {
    let tmpl = DecisionsTemplate {
        title: "Decision Trace".to_string(),
        session_id,
    };
    Ok(Html(tmpl.render()?))
}
