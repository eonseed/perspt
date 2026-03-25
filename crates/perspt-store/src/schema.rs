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

    // Create sequences for auto-incrementing IDs (DuckDB doesn't auto-increment INTEGER PRIMARY KEY)
    conn.execute(
        "CREATE SEQUENCE IF NOT EXISTS seq_node_states_id START 1",
        [],
    )?;
    conn.execute(
        "CREATE SEQUENCE IF NOT EXISTS seq_energy_history_id START 1",
        [],
    )?;
    conn.execute(
        "CREATE SEQUENCE IF NOT EXISTS seq_llm_requests_id START 1",
        [],
    )?;

    // Node states table - per-node state tracking
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS node_states (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_node_states_id'),
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
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_energy_history_id'),
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

    // LLM requests table - stores all LLM interactions for debugging and analysis
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS llm_requests (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_llm_requests_id'),
            session_id VARCHAR NOT NULL,
            node_id VARCHAR,
            model VARCHAR NOT NULL,
            prompt TEXT NOT NULL,
            response TEXT NOT NULL,
            tokens_in INTEGER DEFAULT 0,
            tokens_out INTEGER DEFAULT 0,
            latency_ms INTEGER DEFAULT 0,
            timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        )
        "#,
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_llm_requests_session ON llm_requests(session_id)",
        [],
    )?;

    // PSP-5 Phase 3: Sequences for context provenance tables
    conn.execute(
        "CREATE SEQUENCE IF NOT EXISTS seq_structural_digests_id START 1",
        [],
    )?;
    conn.execute(
        "CREATE SEQUENCE IF NOT EXISTS seq_context_provenance_id START 1",
        [],
    )?;

    // PSP-5 Phase 3: Structural digests - hashes of compile-critical artifacts
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS structural_digests (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_structural_digests_id'),
            digest_id VARCHAR NOT NULL,
            session_id VARCHAR NOT NULL,
            node_id VARCHAR NOT NULL,
            source_path VARCHAR NOT NULL,
            artifact_kind VARCHAR NOT NULL,
            hash BLOB NOT NULL,
            version INTEGER DEFAULT 1,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        )
        "#,
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_structural_digests_session ON structural_digests(session_id)",
        [],
    )?;

    // PSP-5 Phase 3: Context provenance - audit trail of what context was used per node
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS context_provenance (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_context_provenance_id'),
            session_id VARCHAR NOT NULL,
            node_id VARCHAR NOT NULL,
            context_package_id VARCHAR NOT NULL,
            structural_hashes TEXT,
            summary_hashes TEXT,
            dependency_hashes TEXT,
            included_file_count INTEGER DEFAULT 0,
            total_bytes INTEGER DEFAULT 0,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        )
        "#,
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_context_provenance_session ON context_provenance(session_id)",
        [],
    )?;

    // =========================================================================
    // PSP-5 Phase 5: Escalation evidence and rewrite lineage
    // =========================================================================

    conn.execute(
        "CREATE SEQUENCE IF NOT EXISTS seq_escalation_reports_id START 1",
        [],
    )?;
    conn.execute(
        "CREATE SEQUENCE IF NOT EXISTS seq_rewrite_records_id START 1",
        [],
    )?;
    conn.execute(
        "CREATE SEQUENCE IF NOT EXISTS seq_sheaf_validations_id START 1",
        [],
    )?;

    // Escalation reports — one row per classified non-convergence event
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS escalation_reports (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_escalation_reports_id'),
            session_id VARCHAR NOT NULL,
            node_id VARCHAR NOT NULL,
            category VARCHAR NOT NULL,
            action VARCHAR NOT NULL,
            energy_snapshot TEXT,
            stage_outcomes TEXT,
            evidence TEXT,
            affected_node_ids TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        )
        "#,
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_escalation_reports_session ON escalation_reports(session_id)",
        [],
    )?;

    // Rewrite records — one row per local graph rewrite applied
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS rewrite_records (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_rewrite_records_id'),
            session_id VARCHAR NOT NULL,
            node_id VARCHAR NOT NULL,
            action VARCHAR NOT NULL,
            category VARCHAR NOT NULL,
            requeued_nodes TEXT,
            inserted_nodes TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        )
        "#,
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_rewrite_records_session ON rewrite_records(session_id)",
        [],
    )?;

    // Sheaf validation results — one row per validator pass per node
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS sheaf_validations (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_sheaf_validations_id'),
            session_id VARCHAR NOT NULL,
            node_id VARCHAR NOT NULL,
            validator_class VARCHAR NOT NULL,
            plugin_source VARCHAR,
            passed BOOLEAN NOT NULL,
            evidence_summary TEXT,
            affected_files TEXT,
            v_sheaf_contribution REAL DEFAULT 0.0,
            requeue_targets TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        )
        "#,
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sheaf_validations_session ON sheaf_validations(session_id)",
        [],
    )?;

    log::info!("DuckDB schema initialized successfully");
    Ok(())
}
