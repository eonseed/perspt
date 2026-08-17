//! DuckDB Schema Initialization
//!
//! Creates the required tables for SRBN session persistence.

use anyhow::Result;
use duckdb::Connection;

const CURRENT_SCHEMA_VERSION: i64 = 1;
const EXPECTED_DUCKDB_VERSION: &str = "v1.5.5";

/// Initialize the store through an idempotent transactional migration.
pub fn init_schema(conn: &Connection) -> Result<()> {
    check_dynamic_abi(conn)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (\
         version BIGINT PRIMARY KEY, applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP)",
    )?;
    let applied: bool = conn.query_row(
        "SELECT count(*) > 0 FROM schema_migrations WHERE version = ?",
        [CURRENT_SCHEMA_VERSION],
        |row| row.get(0),
    )?;
    if applied {
        return Ok(());
    }
    conn.execute_batch("BEGIN TRANSACTION")?;
    let migration = apply_schema_current(conn).and_then(|()| {
        conn.execute(
            "INSERT INTO schema_migrations (version) VALUES (?)",
            [CURRENT_SCHEMA_VERSION],
        )?;
        Ok(())
    });
    match migration {
        Ok(()) => conn.execute_batch("COMMIT")?,
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(error.context("applying store schema migration v1"));
        }
    }
    conn.execute_batch("CHECKPOINT")?;
    Ok(())
}

fn check_dynamic_abi(conn: &Connection) -> Result<()> {
    if cfg!(feature = "bundled") {
        return Ok(());
    }
    let actual: String = conn.query_row("SELECT version()", [], |row| row.get(0))?;
    anyhow::ensure!(
        actual.starts_with(EXPECTED_DUCKDB_VERSION),
        "incompatible dynamically linked DuckDB ABI: expected {}, found {}; \
         install DuckDB 1.5.5 or build with the `bundled` feature",
        EXPECTED_DUCKDB_VERSION,
        actual
    );
    Ok(())
}

/// Current schema body. Every statement remains idempotent so an interrupted
/// first migration can be retried safely after its transaction rolls back.
fn apply_schema_current(conn: &Connection) -> Result<()> {
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

    // =========================================================================
    // PSP-5 Phase 5: Escalation evidence and rewrite lineage
    // =========================================================================

    // =========================================================================
    // PSP-5 Phase 6: Provisional branch ledger and interface-sealed speculation
    // =========================================================================

    // =========================================================================
    // PSP-5 Phase 8: Ledger-backed node commits and resume correctness
    // =========================================================================

    // =========================================================================
    // Plan Revision, Feature Charter, and Repair Footprint Tables
    // =========================================================================

    // =========================================================================
    // PSP-7: SRBN step records and correction attempt telemetry
    // =========================================================================

    // PSP-9 system 14: the durable canonical event stream. The SDK keeps the
    // chain semantics; this table keeps the bytes.
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS psp9_ledger_events (
            session_id VARCHAR NOT NULL,
            sequence BIGINT NOT NULL,
            event_json TEXT NOT NULL,
            prev_hash VARCHAR NOT NULL,
            hash VARCHAR NOT NULL,
            PRIMARY KEY (session_id, sequence)
        )
        "#,
        [],
    )?;
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS psp9_authority_epochs (
            session_id VARCHAR PRIMARY KEY,
            epoch BIGINT NOT NULL
        )
        "#,
        [],
    )?;
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS psp9_grant_policies (
            policy_id VARCHAR PRIMARY KEY,
            session_id VARCHAR NOT NULL,
            policy_json TEXT NOT NULL,
            revoked BOOLEAN NOT NULL DEFAULT FALSE,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
        "#,
        [],
    )?;
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS psp9_external_effects (
            session_id VARCHAR NOT NULL,
            idempotency_key VARCHAR NOT NULL,
            intent_hash VARCHAR NOT NULL,
            intent_json TEXT NOT NULL,
            result_json TEXT,
            status VARCHAR NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            completed_at TIMESTAMP,
            PRIMARY KEY (session_id, idempotency_key)
        )
        "#,
        [],
    )?;
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS psp9_artifacts (
            content_hash VARCHAR PRIMARY KEY,
            content BLOB NOT NULL,
            byte_len BIGINT NOT NULL,
            media_type VARCHAR NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
        "#,
        [],
    )?;
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS psp9_context_checkpoints (
            session_id VARCHAR NOT NULL,
            covered_event_root VARCHAR NOT NULL,
            checkpoint_json TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (session_id, covered_event_root)
        )
        "#,
        [],
    )?;
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS psp9_verdicts (
            session_id VARCHAR NOT NULL,
            candidate_id VARCHAR NOT NULL,
            validator_id VARCHAR NOT NULL,
            stratum VARCHAR NOT NULL,
            missed BOOLEAN NOT NULL,
            unsafe_label BOOLEAN,
            evidence_hash VARCHAR NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (session_id, candidate_id, validator_id)
        )
        "#,
        [],
    )?;
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS psp9_calibration_epochs (
            epoch_id VARCHAR PRIMARY KEY,
            stratum VARCHAR NOT NULL,
            target_rho DOUBLE NOT NULL,
            threshold DOUBLE,
            state VARCHAR NOT NULL,
            sample_count BIGINT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
        "#,
        [],
    )?;
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS psp9_calibration_samples (
            epoch_id VARCHAR NOT NULL,
            sample_id VARCHAR NOT NULL,
            score DOUBLE NOT NULL,
            -- NULL until the delayed audit label arrives; a sample only
            -- counts toward the conformal floor once labeled.
            unsafe_label BOOLEAN,
            audit_selected BOOLEAN NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (epoch_id, sample_id)
        )
        "#,
        [],
    )?;
    // Migration for databases created before delayed audit labels existed.
    // Inspect the catalog first rather than relying on ignored DDL errors.
    if column_is_nullable(conn, "psp9_calibration_samples", "unsafe_label")? == Some(false) {
        conn.execute(
            "ALTER TABLE psp9_calibration_samples ALTER COLUMN unsafe_label DROP NOT NULL",
            [],
        )?;
    }

    log::info!("DuckDB schema initialized successfully");
    Ok(())
}

fn column_is_nullable(conn: &Connection, table: &str, column: &str) -> Result<Option<bool>> {
    let mut statement = conn.prepare(
        "SELECT is_nullable FROM information_schema.columns \
         WHERE table_schema = 'main' AND table_name = ? AND column_name = ?",
    )?;
    let mut rows = statement.query([table, column])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    let nullable: String = row.get(0)?;
    Ok(Some(nullable.eq_ignore_ascii_case("YES")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_migration_is_transactional_and_idempotent() {
        let connection = Connection::open_in_memory().unwrap();
        init_schema(&connection).unwrap();
        init_schema(&connection).unwrap();
        let count: i64 = connection
            .query_row("SELECT count(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn delayed_label_column_is_nullable() {
        let connection = Connection::open_in_memory().unwrap();
        init_schema(&connection).unwrap();
        assert_eq!(
            column_is_nullable(&connection, "psp9_calibration_samples", "unsafe_label").unwrap(),
            Some(true)
        );
    }
}
