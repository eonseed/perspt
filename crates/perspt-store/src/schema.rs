//! DuckDB Schema Initialization
//!
//! Creates the required tables for SRBN session persistence.

use anyhow::Result;
use duckdb::Connection;

/// Initialize the DuckDB schema for SRBN persistence
pub fn init_schema(conn: &Connection) -> Result<()> {
    // Sessions table - top-level session tracking
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS sessions (
            session_id VARCHAR PRIMARY KEY,
            task TEXT NOT NULL,
            working_dir TEXT NOT NULL,
            merkle_root BLOB,
            detected_toolchain VARCHAR,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            status VARCHAR DEFAULT 'active'
        )
        "#,
        [],
    )?;

    // Node states table - per-node state tracking
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS node_states (
            id INTEGER PRIMARY KEY,
            node_id VARCHAR NOT NULL,
            session_id VARCHAR NOT NULL,
            state VARCHAR NOT NULL,
            v_total REAL DEFAULT 0.0,
            merkle_hash BLOB,
            attempt_count INTEGER DEFAULT 0,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        )
        "#,
        [],
    )?;

    // Energy history table - tracks V(x) over time for convergence analysis
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS energy_history (
            id INTEGER PRIMARY KEY,
            node_id VARCHAR NOT NULL,
            session_id VARCHAR NOT NULL,
            timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            v_syn REAL DEFAULT 0.0,
            v_str REAL DEFAULT 0.0,
            v_log REAL DEFAULT 0.0,
            v_boot REAL DEFAULT 0.0,
            v_sheaf REAL DEFAULT 0.0,
            v_total REAL DEFAULT 0.0,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        )
        "#,
        [],
    )?;

    // Index for fast session lookup
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_node_states_session ON node_states(session_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_energy_history_session ON energy_history(session_id)",
        [],
    )?;

    log::info!("DuckDB schema initialized successfully");
    Ok(())
}
