use askama::Template;
use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;
use crate::views::overview::{GlobalStats, OverviewViewModel, SessionSummary};
use crate::views::psp9::LedgerProjection;

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
    let total_pages = total_sessions.div_ceil(PAGE_SIZE);

    let sessions = state.store.list_sessions_paginated(PAGE_SIZE, offset)?;

    // One ledger projection per listed session; a session with no PSP-9
    // events simply projects to an empty graph.
    let mut projections = Vec::new();
    for s in &sessions {
        let rows = state
            .store
            .get_psp9_events(&s.session_id)
            .unwrap_or_default();
        projections.push((s.session_id.clone(), LedgerProjection::from_rows(&rows)));
    }

    let vm = OverviewViewModel::from_store(sessions, &projections);

    let tmpl = OverviewTemplate {
        title: "Dashboard".to_string(),
        sessions: vm.sessions,
        stats: vm.global_stats,
        current_page,
        total_pages,
    };
    Ok(Html(tmpl.render()?))
}
