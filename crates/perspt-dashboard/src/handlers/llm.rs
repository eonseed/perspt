use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;
use crate::views::friendly_name;
use crate::views::llm::{LlmRow, LlmViewModel};

#[derive(Template)]
#[template(path = "pages/llm.html")]
struct LlmTemplate {
    session_id: String,
    display_name: String,
    active_tab: String,
    title: String,
    requests: Vec<LlmRow>,
    total_tokens_in: i64,
    total_tokens_out: i64,
    total_latency_secs: f64,
    avg_latency_secs: f64,
    request_count: usize,
    models_used: Vec<String>,
    tokens_estimated: bool,
}

pub async fn llm_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, DashboardError> {
    let records = state.store.get_llm_requests(&session_id)?;
    let vm = LlmViewModel::from_records(session_id.clone(), records);

    let tmpl = LlmTemplate {
        display_name: friendly_name(&vm.session_id),
        session_id: vm.session_id,
        active_tab: "llm".to_string(),
        title: "LLM Telemetry".to_string(),
        requests: vm.requests,
        total_tokens_in: vm.total_tokens_in,
        total_tokens_out: vm.total_tokens_out,
        total_latency_secs: vm.total_latency_secs,
        avg_latency_secs: vm.avg_latency_secs,
        request_count: vm.request_count,
        models_used: vm.models_used,
        tokens_estimated: vm.tokens_estimated,
    };
    Ok(Html(tmpl.render()?))
}
