use perspt_store::SessionStore;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Shared application state for all dashboard handlers
#[derive(Clone)]
pub struct AppState {
    /// Read-only session store
    pub store: Arc<SessionStore>,
    /// Optional dashboard password (None = open access)
    pub password: Option<String>,
    /// Active session token (set on successful login)
    pub session_token: Arc<Mutex<Option<String>>>,
    /// Working directory the agent is operating in
    pub working_dir: PathBuf,
}
