use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;
use crate::views::energy::{EnergyPoint, EnergySummary, EnergyViewModel};
use crate::views::friendly_name;
use crate::views::psp9::LedgerProjection;

#[derive(Template)]
#[template(path = "pages/energy.html")]
struct EnergyTemplate {
    session_id: String,
    display_name: String,
    active_tab: String,
    title: String,
    records: Vec<EnergyPoint>,
    summary: EnergySummary,
}

pub async fn energy_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, DashboardError> {
    let rows = state.store.get_psp9_events(&session_id)?;
    let projection = LedgerProjection::from_rows(&rows);
    let vm = EnergyViewModel::from_projection(session_id, &projection);

    let tmpl = EnergyTemplate {
        display_name: friendly_name(&vm.session_id),
        session_id: vm.session_id,
        active_tab: "energy".to_string(),
        title: "Energy Trajectory".to_string(),
        records: vm.records,
        summary: vm.summary,
    };
    Ok(Html(tmpl.render()?))
}
