//! Dashboard command — launches the web monitoring interface

use anyhow::{Context, Result};
use std::path::PathBuf;

/// Run the dashboard web server
pub async fn run(port: u16, db_path: Option<PathBuf>) -> Result<()> {
    // Resolve database path
    let db = match db_path {
        Some(p) => p,
        None => perspt_store::SessionStore::default_db_path()?,
    };

    if !db.exists() {
        anyhow::bail!(
            "Database not found at {}. Run `perspt agent` first to create it.",
            db.display()
        );
    }

    let store =
        perspt_store::SessionStore::open_read_only(&db).context("Failed to open database")?;

    let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let state = perspt_dashboard::state::AppState {
        store: std::sync::Arc::new(store),
        password: None, // TODO: load from config in Phase 5
        session_token: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        working_dir,
    };

    let app = perspt_dashboard::build_router(state);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    println!("Perspt dashboard listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
