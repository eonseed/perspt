use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;
use crate::views::llm::{LlmRow, LlmViewModel};

#[derive(Template)]
#[template(path = "pages/llm.html")]
struct LlmTemplate {
    title: String,
    session_id: String,
    requests: Vec<LlmRow>,
    total_tokens_in: i64,
    total_tokens_out: i64,
    total_latency_ms: i64,
    request_count: usize,
}

pub async fn llm_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, DashboardError> {
    let records = state.store.get_llm_requests(&session_id)?;
    let vm = LlmViewModel::from_records(session_id.clone(), records);

    let tmpl = LlmTemplate {
        title: "LLM Telemetry".to_string(),
        session_id: vm.session_id,
        requests: vm.requests,
        total_tokens_in: vm.total_tokens_in,
        total_tokens_out: vm.total_tokens_out,
        total_latency_ms: vm.total_latency_ms,
        request_count: vm.request_count,
    };
    Ok(Html(tmpl.render()?))
}
