use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use std::convert::Infallible;
use tokio_stream::wrappers::IntervalStream;
use tokio_stream::StreamExt;

use crate::state::AppState;
use crate::views::normalize_state;

/// SSE endpoint: pushes named events for a session every 2 seconds.
///
/// Route: `GET /sse/{session_id}`
pub async fn sse_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let interval = tokio::time::interval(std::time::Duration::from_secs(2));
    let stream = IntervalStream::new(interval).map(move |_| {
        let store = &state.store;
        let sid = &session_id;

        // Node stats summary
        let node_stats = match store.get_latest_node_states(sid) {
            Ok(nodes) => {
                let total = nodes.len();
                let committed = nodes
                    .iter()
                    .filter(|n| normalize_state(&n.state) == "completed")
                    .count();
                let failed = nodes.iter().filter(|n| normalize_state(&n.state) == "failed").count();
                let running = nodes.iter().filter(|n| normalize_state(&n.state) == "running").count();
                format!(
                    r#"<div class="stats shadow"><div class="stat"><div class="stat-title">Total</div><div class="stat-value text-lg">{total}</div></div><div class="stat"><div class="stat-title">Done</div><div class="stat-value text-lg">{committed}</div></div><div class="stat"><div class="stat-title">Running</div><div class="stat-value text-lg">{running}</div></div><div class="stat"><div class="stat-title">Failed</div><div class="stat-value text-lg text-error">{failed}</div></div></div>"#
                )
            }
            Err(_) => String::from("<span>DB unavailable</span>"),
        };

        Ok(Event::default().event("node-stats").data(node_stats))
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
