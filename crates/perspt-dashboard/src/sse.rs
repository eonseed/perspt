use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use std::convert::Infallible;
use tokio_stream::wrappers::IntervalStream;
use tokio_stream::StreamExt;

use crate::state::AppState;
use crate::views::psp9::LedgerProjection;

/// SSE endpoint: pushes a `psp9-stats` summary for a session every 2 seconds
/// (ledger event count, measurement count, last measured energy).
///
/// Route: `GET /sse/{session_id}`
pub async fn sse_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let interval = tokio::time::interval(std::time::Duration::from_secs(2));
    let stream = IntervalStream::new(interval).map(move |_| {
        let payload = match state.store.get_psp9_events(&session_id) {
            Ok(rows) => stats_fragment(&LedgerProjection::from_rows(&rows)),
            Err(_) => String::from("<span>DB unavailable</span>"),
        };
        Ok(Event::default().event("psp9-stats").data(payload))
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// Render the live stats fragment. `concat!` keeps the payload a single
/// line: SSE `data:` frames must not contain raw newlines.
fn stats_fragment(projection: &LedgerProjection) -> String {
    let last_energy = projection
        .measurements
        .last()
        .map(|m| format!("{:.2}", m.energy))
        .unwrap_or_else(|| "—".into());
    format!(
        concat!(
            r#"<div class="stats shadow">"#,
            r#"<div class="stat"><div class="stat-title">Ledger Events</div>"#,
            r#"<div class="stat-value text-lg">{events}</div></div>"#,
            r#"<div class="stat"><div class="stat-title">Measurements</div>"#,
            r#"<div class="stat-value text-lg">{measurements}</div></div>"#,
            r#"<div class="stat"><div class="stat-title">Last Energy</div>"#,
            r#"<div class="stat-value text-lg">{last_energy}</div></div>"#,
            r#"</div>"#
        ),
        events = projection.event_count,
        measurements = projection.measurements.len(),
        last_energy = last_energy
    )
}
