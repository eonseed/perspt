use super::*;

use std::sync::Mutex;

/// Session store for SRBN persistence
pub struct SessionStore {
    pub(crate) conn: Mutex<Connection>,
}

impl SessionStore {
    /// Create a new session store with default path
    pub fn new() -> Result<Self> {
        let db_path = Self::default_db_path()?;
        Self::open(&db_path)
    }

    /// Open a session store at the given path
    pub fn open(path: &PathBuf) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path).context("Failed to open DuckDB")?;
        init_schema(&conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Open a session store in read-only mode for concurrent dashboard reads.
    ///
    /// Uses `AccessMode::ReadOnly` so the dashboard can read alongside the
    /// agent's write lock. Does **not** call `init_schema()` (a write op).
    /// The database file must already exist.
    pub fn open_read_only(path: &std::path::Path) -> Result<Self> {
        let config = duckdb::Config::default()
            .access_mode(duckdb::AccessMode::ReadOnly)
            .context("Failed to configure DuckDB read-only mode")?;
        let conn = Connection::open_with_flags(path, config)
            .context("Failed to open DuckDB in read-only mode")?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Get the default database path (~/.local/share/perspt/perspt.db or similar)
    pub fn default_db_path() -> Result<PathBuf> {
        perspt_core::paths::database_path().context("Could not determine platform data directory")
    }
}
