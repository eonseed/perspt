use askama::Template;
use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;
use crate::views::overview::{GlobalStats, OverviewViewModel, SessionSummary};

const PAGE_SIZE: usize = 20;

#[derive(serde::Deserialize)]
pub struct PaginationParams {
    pub page: Option<usize>,
}

#[derive(Template)]
#[template(path = "pages/overview.html")]
struct OverviewTemplate {
    title: String,
    sessions: Vec<SessionSummary>,
    stats: GlobalStats,
    current_page: usize,
    total_pages: usize,
}

pub async fn overview_handler(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<impl IntoResponse, DashboardError> {
    let current_page = params.page.unwrap_or(1).max(1);
    let offset = (current_page - 1) * PAGE_SIZE;

    let total_sessions = state.store.count_sessions().unwrap_or(0);
    let total_pages = (total_sessions + PAGE_SIZE - 1) / PAGE_SIZE.max(1);

    let sessions = state
        .store
        .list_sessions_paginated(PAGE_SIZE, offset)?;

    let mut nodes_by_session = Vec::new();
    let mut budgets = Vec::new();
    for s in &sessions {
        let nodes = state
            .store
            .get_latest_node_states(&s.session_id)
            .unwrap_or_default();
        nodes_by_session.push((s.session_id.clone(), nodes));

        let budget = state
            .store
            .get_budget_envelope(&s.session_id)
            .ok()
            .flatten();
        budgets.push((s.session_id.clone(), budget));
    }

    let llm_summary = state.store.get_global_llm_summary().unwrap_or((0, 0, 0, 0));

    let vm = OverviewViewModel::from_store(sessions, &nodes_by_session, &budgets, llm_summary);

    let tmpl = OverviewTemplate {
        title: "Dashboard".to_string(),
        sessions: vm.sessions,
        stats: vm.global_stats,
        current_page,
        total_pages,
    };
    Ok(Html(tmpl.render()?))
}
