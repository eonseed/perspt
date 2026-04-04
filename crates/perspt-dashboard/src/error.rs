use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

/// Dashboard-specific error type
#[derive(Debug)]
pub enum DashboardError {
    /// Database query failed
    Store(anyhow::Error),
    /// Template rendering failed
    Template(askama::Error),
    /// Generic internal error
    Internal(String),
}

impl std::fmt::Display for DashboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(e) => write!(f, "Store error: {e}"),
            Self::Template(e) => write!(f, "Template error: {e}"),
            Self::Internal(msg) => write!(f, "Internal error: {msg}"),
        }
    }
}

impl std::error::Error for DashboardError {}

impl From<anyhow::Error> for DashboardError {
    fn from(e: anyhow::Error) -> Self {
        Self::Store(e)
    }
}

impl From<askama::Error> for DashboardError {
    fn from(e: askama::Error) -> Self {
        Self::Template(e)
    }
}

impl IntoResponse for DashboardError {
    fn into_response(self) -> Response {
        let body = format!(
            "<html><body><h1>Error</h1><pre>{}</pre></body></html>",
            html_escape(&self.to_string())
        );
        (StatusCode::INTERNAL_SERVER_ERROR, Html(body)).into_response()
    }
}

/// Minimal HTML escaping for error messages
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
