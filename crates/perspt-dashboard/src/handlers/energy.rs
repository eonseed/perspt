use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;
use crate::views::energy::{EnergyPoint, EnergySummary, EnergyViewModel};
use crate::views::friendly_name;

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
    let records = state.store.get_session_energy_history(&session_id)?;
    let vm = EnergyViewModel::from_records(session_id.clone(), records);

    let tmpl = EnergyTemplate {
        display_name: friendly_name(&vm.session_id),
        session_id: vm.session_id,
        active_tab: "energy".to_string(),
        title: "Energy Convergence".to_string(),
        records: vm.records,
        summary: vm.summary,
    };
    Ok(Html(tmpl.render()?))
}
