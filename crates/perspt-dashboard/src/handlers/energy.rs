use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};

use crate::error::DashboardError;
use crate::state::AppState;
use crate::views::energy::{EnergyPoint, EnergyViewModel};

#[derive(Template)]
#[template(path = "pages/energy.html")]
struct EnergyTemplate {
    title: String,
    session_id: String,
    records: Vec<EnergyPoint>,
}

pub async fn energy_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, DashboardError> {
    let records = state.store.get_session_energy_history(&session_id)?;
    let vm = EnergyViewModel::from_records(session_id.clone(), records);

    let tmpl = EnergyTemplate {
        title: "Energy Convergence".to_string(),
        session_id: vm.session_id,
        records: vm.records,
    };
    Ok(Html(tmpl.render()?))
}
