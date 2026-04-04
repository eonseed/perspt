use askama::Template;
use axum::extract::State;
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;

#[derive(askama::Template)]
#[template(path = "pages/overview.html")]
struct OverviewTemplate {
    title: String,
}

pub async fn overview_handler(
    State(_state): State<AppState>,
) -> Result<impl IntoResponse, DashboardError> {
    let tmpl = OverviewTemplate {
        title: "Overview".to_string(),
    };
    Ok(Html(tmpl.render()?))
}
