use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;
use crate::views::decisions::{DecisionsViewModel, Psp9EventRow};
use crate::views::friendly_name;

#[derive(Template)]
#[template(path = "pages/decisions.html")]
struct DecisionsTemplate {
    session_id: String,
    display_name: String,
    active_tab: String,
    title: String,
    psp9_events: Vec<Psp9EventRow>,
}

pub async fn decisions_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, DashboardError> {
    let psp9_events = state.store.get_psp9_events(&session_id)?;
    let vm = DecisionsViewModel::from_store(session_id, psp9_events);

    let tmpl = DecisionsTemplate {
        display_name: friendly_name(&vm.session_id),
        session_id: vm.session_id,
        active_tab: "decisions".to_string(),
        title: "Decision Trace".to_string(),
        psp9_events: vm.psp9_events,
    };
    Ok(Html(tmpl.render()?))
}
