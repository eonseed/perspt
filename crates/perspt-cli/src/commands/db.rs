//! Store repair commands.

use std::path::PathBuf;

use anyhow::Result;

pub async fn repair(db_path: PathBuf, discard_wal: bool) -> Result<()> {
    let report = perspt_store::repair_database(&db_path, discard_wal)?;
    println!("DuckDB recovery verified.");
    println!("WAL size: {} bytes", report.wal_size);
    println!("WAL SHA-256: {}", report.wal_sha256);
    println!("Database backup: {}", report.database_backup.display());
    println!("WAL backup: {}", report.wal_backup.display());
    println!("WAL quarantine: {}", report.wal_quarantine.display());
    println!("Recovered tables: {}", report.recovered_table_count);
    println!("No WAL record count was inferred.");
    Ok(())
}
