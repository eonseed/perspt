//! DuckDB Schema Initialization
//!
//! Creates the required tables for SRBN session persistence.

use anyhow::Result;
use duckdb::Connection;

/// Add a column to a table, ignoring errors if it already exists (backward-compatible migration).
fn add_column_if_not_exists(conn: &Connection, table: &str, column: &str, col_type: &str) {
    let sql = format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, col_type);
    let _ = conn.execute(&sql, []);
}

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

    // =========================================================================
    // PSP-5 Phase 6: Provisional branch ledger and interface-sealed speculation
    // =========================================================================

    conn.execute(
        "CREATE SEQUENCE IF NOT EXISTS seq_provisional_branches_id START 1",
        [],
    )?;
    conn.execute(
        "CREATE SEQUENCE IF NOT EXISTS seq_branch_lineage_id START 1",
        [],
    )?;
    conn.execute(
        "CREATE SEQUENCE IF NOT EXISTS seq_interface_seals_id START 1",
        [],
    )?;
    conn.execute(
        "CREATE SEQUENCE IF NOT EXISTS seq_branch_flushes_id START 1",
        [],
    )?;

    // Provisional branches — speculative child work stored separately from committed state
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS provisional_branches (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_provisional_branches_id'),
            branch_id VARCHAR NOT NULL,
            session_id VARCHAR NOT NULL,
            node_id VARCHAR NOT NULL,
            parent_node_id VARCHAR NOT NULL,
            state VARCHAR NOT NULL DEFAULT 'active',
            parent_seal_hash BLOB,
            sandbox_dir VARCHAR,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        )
        "#,
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_provisional_branches_session ON provisional_branches(session_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_provisional_branches_parent ON provisional_branches(parent_node_id)",
        [],
    )?;

    // Branch lineage — parent→child dependency edges for flush propagation
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS branch_lineage (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_branch_lineage_id'),
            lineage_id VARCHAR NOT NULL,
            parent_branch_id VARCHAR NOT NULL,
            child_branch_id VARCHAR NOT NULL,
            depends_on_seal BOOLEAN NOT NULL DEFAULT true,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
        "#,
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_branch_lineage_parent ON branch_lineage(parent_branch_id)",
        [],
    )?;

    // Interface seals — immutable seal records for dependency gating
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS interface_seals (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_interface_seals_id'),
            seal_id VARCHAR NOT NULL,
            session_id VARCHAR NOT NULL,
            node_id VARCHAR NOT NULL,
            sealed_path VARCHAR NOT NULL,
            artifact_kind VARCHAR NOT NULL,
            seal_hash BLOB NOT NULL,
            version INTEGER DEFAULT 1,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        )
        "#,
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_interface_seals_session ON interface_seals(session_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_interface_seals_node ON interface_seals(node_id)",
        [],
    )?;

    // Branch flushes — records of flush decisions for audit and resume
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS branch_flushes (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_branch_flushes_id'),
            flush_id VARCHAR NOT NULL,
            session_id VARCHAR NOT NULL,
            parent_node_id VARCHAR NOT NULL,
            flushed_branch_ids TEXT NOT NULL,
            requeue_node_ids TEXT NOT NULL,
            reason TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        )
        "#,
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_branch_flushes_session ON branch_flushes(session_id)",
        [],
    )?;

    // =========================================================================
    // PSP-5 Phase 8: Ledger-backed node commits and resume correctness
    // =========================================================================

    // Extend node_states with Phase 8 columns for richer node snapshots.
    // Uses ADD COLUMN to migrate existing databases; errors are silently
    // ignored when the column already exists.
    add_column_if_not_exists(conn, "node_states", "node_class", "VARCHAR");
    add_column_if_not_exists(conn, "node_states", "owner_plugin", "VARCHAR");
    add_column_if_not_exists(conn, "node_states", "goal", "TEXT");
    add_column_if_not_exists(conn, "node_states", "parent_id", "VARCHAR");
    add_column_if_not_exists(conn, "node_states", "children", "TEXT");
    add_column_if_not_exists(conn, "node_states", "last_error_type", "VARCHAR");
    add_column_if_not_exists(conn, "node_states", "committed_at", "VARCHAR");

    // Task graph edges for deterministic DAG reconstruction on resume
    conn.execute(
        "CREATE SEQUENCE IF NOT EXISTS seq_task_graph_edges_id START 1",
        [],
    )?;

    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS task_graph_edges (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_task_graph_edges_id'),
            session_id VARCHAR NOT NULL,
            parent_node_id VARCHAR NOT NULL,
            child_node_id VARCHAR NOT NULL,
            edge_type VARCHAR NOT NULL DEFAULT 'dependency',
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        )
        "#,
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_task_graph_edges_session ON task_graph_edges(session_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_task_graph_edges_parent ON task_graph_edges(parent_node_id)",
        [],
    )?;

    // Review outcomes for explicit approval/rejection tracking
    conn.execute(
        "CREATE SEQUENCE IF NOT EXISTS seq_review_outcomes_id START 1",
        [],
    )?;

    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS review_outcomes (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_review_outcomes_id'),
            session_id VARCHAR NOT NULL,
            node_id VARCHAR NOT NULL,
            outcome VARCHAR NOT NULL,
            reviewer_note TEXT,
            energy_at_review DOUBLE,
            degraded BOOLEAN,
            escalation_category VARCHAR,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        )
        "#,
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_review_outcomes_session ON review_outcomes(session_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_review_outcomes_node ON review_outcomes(node_id)",
        [],
    )?;

    // Verification result snapshots for resume and status display
    conn.execute(
        "CREATE SEQUENCE IF NOT EXISTS seq_verification_results_id START 1",
        [],
    )?;

    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS verification_results (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_verification_results_id'),
            session_id VARCHAR NOT NULL,
            node_id VARCHAR NOT NULL,
            result_json TEXT NOT NULL,
            syntax_ok BOOLEAN NOT NULL DEFAULT false,
            build_ok BOOLEAN NOT NULL DEFAULT false,
            tests_ok BOOLEAN NOT NULL DEFAULT false,
            lint_ok BOOLEAN NOT NULL DEFAULT false,
            diagnostics_count INTEGER DEFAULT 0,
            tests_passed INTEGER DEFAULT 0,
            tests_failed INTEGER DEFAULT 0,
            degraded BOOLEAN NOT NULL DEFAULT false,
            degraded_reason VARCHAR,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        )
        "#,
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_verification_results_session ON verification_results(session_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_verification_results_node ON verification_results(node_id)",
        [],
    )?;

    // Artifact bundle snapshots for resume and diff review
    conn.execute(
        "CREATE SEQUENCE IF NOT EXISTS seq_artifact_bundles_id START 1",
        [],
    )?;

    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS artifact_bundles (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_artifact_bundles_id'),
            session_id VARCHAR NOT NULL,
            node_id VARCHAR NOT NULL,
            bundle_json TEXT NOT NULL,
            artifact_count INTEGER DEFAULT 0,
            command_count INTEGER DEFAULT 0,
            touched_files TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        )
        "#,
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_artifact_bundles_session ON artifact_bundles(session_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_artifact_bundles_node ON artifact_bundles(node_id)",
        [],
    )?;

    // =========================================================================
    // Plan Revision, Feature Charter, and Repair Footprint Tables
    // =========================================================================

    // Feature charters - scope constraints for sessions
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS feature_charters (
            charter_id VARCHAR PRIMARY KEY,
            session_id VARCHAR NOT NULL,
            scope_description TEXT NOT NULL,
            max_modules INTEGER,
            max_files INTEGER,
            max_revisions INTEGER,
            language_constraint VARCHAR,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        )
        "#,
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_feature_charters_session ON feature_charters(session_id)",
        [],
    )?;

    // Plan revisions - track plan evolution within a session
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS plan_revisions (
            revision_id VARCHAR PRIMARY KEY,
            session_id VARCHAR NOT NULL,
            sequence INTEGER NOT NULL,
            plan_json TEXT NOT NULL,
            reason VARCHAR NOT NULL,
            supersedes VARCHAR,
            status VARCHAR NOT NULL DEFAULT 'active',
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        )
        "#,
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_plan_revisions_session ON plan_revisions(session_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_plan_revisions_status ON plan_revisions(status)",
        [],
    )?;

    // Repair footprints - bounded repair units during correction
    conn.execute(
        "CREATE SEQUENCE IF NOT EXISTS seq_repair_footprints_id START 1",
        [],
    )?;
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS repair_footprints (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_repair_footprints_id'),
            footprint_id VARCHAR NOT NULL UNIQUE,
            session_id VARCHAR NOT NULL,
            node_id VARCHAR NOT NULL,
            revision_id VARCHAR NOT NULL,
            attempt INTEGER NOT NULL,
            affected_files TEXT NOT NULL,
            bundle_json TEXT NOT NULL,
            diagnosis TEXT NOT NULL,
            resolved BOOLEAN NOT NULL DEFAULT false,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        )
        "#,
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_repair_footprints_session ON repair_footprints(session_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_repair_footprints_node ON repair_footprints(node_id)",
        [],
    )?;

    // Budget envelopes - session-level budget tracking
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS budget_envelopes (
            session_id VARCHAR PRIMARY KEY,
            max_steps INTEGER,
            steps_used INTEGER NOT NULL DEFAULT 0,
            max_revisions INTEGER,
            revisions_used INTEGER NOT NULL DEFAULT 0,
            max_cost_usd DOUBLE,
            cost_used_usd DOUBLE NOT NULL DEFAULT 0.0,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        )
        "#,
        [],
    )?;

    // =========================================================================
    // PSP-7: SRBN step records and correction attempt telemetry
    // =========================================================================

    conn.execute(
        "CREATE SEQUENCE IF NOT EXISTS seq_srbn_step_records_id START 1",
        [],
    )?;
    conn.execute(
        "CREATE SEQUENCE IF NOT EXISTS seq_correction_attempts_id START 1",
        [],
    )?;

    // SRBN step records — one row per orchestration step transition per node.
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS srbn_step_records (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_srbn_step_records_id'),
            session_id VARCHAR NOT NULL,
            node_id VARCHAR NOT NULL,
            step VARCHAR NOT NULL,
            outcome VARCHAR NOT NULL,
            energy_json TEXT,
            parse_state VARCHAR,
            retry_classification VARCHAR,
            attempt_count INTEGER DEFAULT 0,
            duration_ms INTEGER DEFAULT 0,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        )
        "#,
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_srbn_step_records_session ON srbn_step_records(session_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_srbn_step_records_node ON srbn_step_records(session_id, node_id)",
        [],
    )?;

    // Correction attempts — one row per correction round-trip within convergence.
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS correction_attempts (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_correction_attempts_id'),
            session_id VARCHAR NOT NULL,
            node_id VARCHAR NOT NULL,
            attempt INTEGER NOT NULL,
            parse_state VARCHAR NOT NULL,
            retry_classification VARCHAR,
            response_fingerprint VARCHAR NOT NULL,
            response_length INTEGER NOT NULL,
            energy_json TEXT,
            accepted BOOLEAN NOT NULL,
            rejection_reason TEXT,
            created_at BIGINT NOT NULL,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
        )
        "#,
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_correction_attempts_session ON correction_attempts(session_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_correction_attempts_node ON correction_attempts(session_id, node_id)",
        [],
    )?;

    log::info!("DuckDB schema initialized successfully");
    Ok(())
}
