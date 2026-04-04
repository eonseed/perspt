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
            "/sessions/{session_id}",
            get(handlers::session_detail::session_detail_handler),
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use perspt_store::SessionStore;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tower::ServiceExt;

    fn test_db_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("perspt_dash_test_{}.db", rand::random::<u64>()))
    }

    /// Create a test AppState with a temp store (no password).
    fn test_state_open() -> AppState {
        let db = test_db_path();
        let store = SessionStore::open(&db).expect("temp store");
        AppState {
            store: Arc::new(store),
            password: None,
            session_token: Arc::new(Mutex::new(None)),
            working_dir: std::path::PathBuf::from("/tmp"),
            is_localhost: true,
        }
    }

    /// Create a test AppState with a password set.
    fn test_state_auth(password: &str) -> AppState {
        let db = test_db_path();
        let store = SessionStore::open(&db).expect("temp store");
        AppState {
            store: Arc::new(store),
            password: Some(password.to_string()),
            session_token: Arc::new(Mutex::new(None)),
            working_dir: std::path::PathBuf::from("/tmp"),
            is_localhost: true,
        }
    }

    // ── Route smoke tests (open access) ──

    #[tokio::test]
    async fn overview_returns_200() {
        let app = build_router(test_state_open());
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn session_detail_returns_200() {
        let app = build_router(test_state_open());
        let req = Request::builder()
            .uri("/sessions/test-session")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn login_page_returns_200() {
        let app = build_router(test_state_open());
        let req = Request::builder()
            .uri("/login")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn dag_page_returns_200() {
        let app = build_router(test_state_open());
        let req = Request::builder()
            .uri("/sessions/test-session/dag")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn energy_page_returns_200() {
        let app = build_router(test_state_open());
        let req = Request::builder()
            .uri("/sessions/test-session/energy")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn llm_page_returns_200() {
        let app = build_router(test_state_open());
        let req = Request::builder()
            .uri("/sessions/test-session/llm")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn sandbox_page_returns_200() {
        let app = build_router(test_state_open());
        let req = Request::builder()
            .uri("/sessions/test-session/sandbox")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn decisions_page_returns_200() {
        let app = build_router(test_state_open());
        let req = Request::builder()
            .uri("/sessions/test-session/decisions")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    // ── SSE test ──

    #[tokio::test]
    async fn sse_returns_event_stream() {
        let app = build_router(test_state_open());
        let req = Request::builder()
            .uri("/sse/test-session")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let ct = res.headers().get("content-type").unwrap().to_str().unwrap();
        assert!(ct.contains("text/event-stream"));
    }

    // ── Auth tests ──

    #[tokio::test]
    async fn unauth_request_redirects_to_login() {
        let app = build_router(test_state_auth("secret123"));
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        let location = res.headers().get("location").unwrap().to_str().unwrap();
        assert_eq!(location, "/login");
    }

    #[tokio::test]
    async fn invalid_cookie_redirects_to_login() {
        let app = build_router(test_state_auth("secret123"));
        let req = Request::builder()
            .uri("/")
            .header("cookie", "perspt_session=wrong-token")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
    }

    #[tokio::test]
    async fn valid_cookie_passes_auth() {
        let state = test_state_auth("secret123");
        // Pre-set a known token
        *state.session_token.lock().await = Some("valid-token-123".to_string());

        let app = build_router(state);
        let req = Request::builder()
            .uri("/")
            .header("cookie", "perspt_session=valid-token-123")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn sse_behind_auth() {
        let app = build_router(test_state_auth("secret123"));
        let req = Request::builder()
            .uri("/sse/test-session")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        // Should redirect, not 200
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
    }
}
