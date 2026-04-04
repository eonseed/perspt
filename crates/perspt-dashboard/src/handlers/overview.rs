use askama::Template;
use axum::extract::State;
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;
use crate::views::overview::{GlobalStats, OverviewViewModel, SessionSummary};

#[derive(Template)]
#[template(path = "pages/overview.html")]
struct OverviewTemplate {
    title: String,
    sessions: Vec<SessionSummary>,
    stats: GlobalStats,
}

pub async fn overview_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, DashboardError> {
    let sessions = state.store.list_recent_sessions(50)?;

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
    };
    Ok(Html(tmpl.render()?))
}
