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
        let (status, user_message) = match &self {
            Self::Store(_) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Database is currently unavailable. The agent session store may be locked or unreachable.",
            ),
            Self::Template(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to render the page template.",
            ),
            Self::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.as_str()),
        };
        let escaped = html_escape(user_message);
        let body = format!(
            r#"<!DOCTYPE html><html data-theme="dark"><head><meta charset="utf-8"><title>Error — Perspt Dashboard</title>
<link rel="stylesheet" href="/static/dashboard.css"></head>
<body class="min-h-screen flex items-center justify-center bg-base-300">
<div class="card bg-base-100 shadow-xl max-w-lg"><div class="card-body">
<h2 class="card-title text-error">Error {}</h2>
<p>{}</p>
<div class="card-actions justify-end"><a href="/" class="btn btn-primary btn-sm">Back to sessions</a></div>
</div></div></body></html>"#,
            status.as_u16(),
            escaped
        );
        (status, Html(body)).into_response()
    }
}

/// Minimal HTML escaping for error messages
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
