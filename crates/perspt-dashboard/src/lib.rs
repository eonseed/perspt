//! perspt-dashboard: Real-time web dashboard for Perspt agent monitoring
//!
//! Provides a browser-based interface for observing agent execution, including
//! DAG topology, energy convergence, LLM telemetry, and decision traces.

pub mod auth;
pub mod error;
pub mod handlers;
pub mod sse;
pub mod state;
pub mod views;

use axum::middleware;
use axum::routing::{get, post};
use axum::Router;
use tower_http::services::ServeDir;

use state::AppState;

/// Build the dashboard router with all routes and middleware.
pub fn build_router(state: AppState) -> Router {
    // Public routes (no auth)
    let public = Router::new()
        .route("/login", get(auth::login_page))
        .route("/login", post(auth::login_handler));

    // Protected routes (behind auth middleware)
    let protected = Router::new()
        .route("/", get(handlers::overview::overview_handler))
        .route(
            "/sessions/{session_id}/dag",
            get(handlers::dag::dag_handler),
        )
        .route(
            "/sessions/{session_id}/energy",
            get(handlers::energy::energy_handler),
        )
        .route(
            "/sessions/{session_id}/llm",
            get(handlers::llm::llm_handler),
        )
        .route(
            "/sessions/{session_id}/sandbox",
            get(handlers::sandbox::sandbox_handler),
        )
        .route(
            "/sessions/{session_id}/decisions",
            get(handlers::decisions::decisions_handler),
        )
        .route("/sse/{session_id}", get(sse::sse_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ));

    let static_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static");

    Router::new()
        .merge(public)
        .merge(protected)
        .nest_service("/static", ServeDir::new(static_dir))
        .with_state(state)
}
