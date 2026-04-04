use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use std::convert::Infallible;
use tokio_stream::wrappers::IntervalStream;
use tokio_stream::StreamExt;

use crate::state::AppState;

/// SSE endpoint: pushes named events for a session every 2 seconds.
///
/// Route: `GET /sse/{session_id}`
pub async fn sse_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let interval = tokio::time::interval(std::time::Duration::from_secs(2));
    let stream = IntervalStream::new(interval).map(move |_| {
        // Build a heartbeat event; real data events will be added in Phase 3
        let _store = &state.store;
        let _sid = &session_id;
        Ok(Event::default().event("heartbeat").data("ok"))
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
