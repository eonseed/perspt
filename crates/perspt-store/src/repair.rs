//! Recoverable DuckDB WAL quarantine.

use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairReport {
    pub database_path: PathBuf,
    pub wal_path: PathBuf,
    pub wal_size: u64,
    pub wal_sha256: String,
    pub database_backup: PathBuf,
    pub wal_backup: PathBuf,
    pub wal_quarantine: PathBuf,
    pub recovered_table_count: u64,
}

/// Quarantine a WAL only after durable backups exist, then verify the database
/// through a read-only connection. Historical bytes are never deleted.
pub fn repair_database(path: &Path, discard_wal: bool) -> Result<RepairReport> {
    anyhow::ensure!(discard_wal, "repair requires explicit --discard-wal");
    validate_regular(path, "database")?;
    let wal = wal_path(path);
    validate_regular(&wal, "WAL")?;
    let parent = path.parent().context("database path has no parent")?;
    let stamp = timestamp()?;
    let database_backup = sibling(path, &format!("backup.{stamp}"))?;
    let wal_backup = sibling(&wal, &format!("backup.{stamp}"))?;
    let wal_quarantine = sibling(&wal, &format!("quarantine.{stamp}"))?;
    ensure_targets_absent(&[&database_backup, &wal_backup, &wal_quarantine])?;

    let wal_size = std::fs::metadata(&wal)?.len();
    let wal_sha256 = sha256_file(&wal)?;
    durable_copy(path, &database_backup)?;
    durable_copy(&wal, &wal_backup)?;
    std::fs::rename(&wal, &wal_quarantine).context("quarantining DuckDB WAL")?;
    sync_directory(parent)?;

    let recovered = verify_recovery(path);
    let recovered_table_count = match recovered {
        Ok(count) => count,
        Err(error) => {
            restore_wal(&wal, &wal_quarantine, parent)?;
            return Err(error.context("database recovery failed; original WAL restored"));
        }
    };
    Ok(RepairReport {
        database_path: path.to_path_buf(),
        wal_path: wal,
        wal_size,
        wal_sha256,
        database_backup,
        wal_backup,
        wal_quarantine,
        recovered_table_count,
    })
}

fn validate_regular(path: &Path, label: &str) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("reading {label} metadata at {}", path.display()))?;
    anyhow::ensure!(
        metadata.file_type().is_file(),
        "{label} is not a regular file"
    );
    Ok(())
}

fn wal_path(database: &Path) -> PathBuf {
    let mut name = database.as_os_str().to_os_string();
    name.push(".wal");
    PathBuf::from(name)
}

fn sibling(path: &Path, suffix: &str) -> Result<PathBuf> {
    let parent = path.parent().context("repair target has no parent")?;
    let mut name: OsString = path
        .file_name()
        .context("repair target has no filename")?
        .into();
    name.push(".");
    name.push(suffix);
    Ok(parent.join(name))
}

fn timestamp() -> Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock precedes Unix epoch")?
        .as_millis())
}

fn ensure_targets_absent(paths: &[&Path]) -> Result<()> {
    for path in paths {
        anyhow::ensure!(
            !path.exists(),
            "repair output already exists: {}",
            path.display()
        );
    }
    Ok(())
}

fn durable_copy(source: &Path, destination: &Path) -> Result<()> {
    let mut input = File::open(source)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    std::io::copy(&mut input, &mut output)?;
    output.flush()?;
    output.sync_all()?;
    sync_directory(destination.parent().context("backup has no parent")?)
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn verify_recovery(path: &Path) -> Result<u64> {
    let config = duckdb::Config::default()
        .access_mode(duckdb::AccessMode::ReadOnly)
        .context("configuring read-only recovery verification")?;
    let connection = duckdb::Connection::open_with_flags(path, config)
        .context("opening recovered database read-only")?;
    connection
        .query_row(
            "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'main'",
            [],
            |row| row.get(0),
        )
        .context("querying recovered database catalog")
}

fn restore_wal(wal: &Path, quarantine: &Path, parent: &Path) -> Result<()> {
    if wal.exists() {
        let failed = sibling(wal, &format!("failed-recovery.{}", timestamp()?))?;
        std::fs::rename(wal, failed).context("preserving WAL created during failed recovery")?;
    }
    std::fs::rename(quarantine, wal).context("restoring original DuckDB WAL")?;
    sync_directory(parent)
}

fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wal_path_appends_instead_of_replacing_extension() {
        assert_eq!(
            wal_path(Path::new("state.db")),
            PathBuf::from("state.db.wal")
        );
    }

    #[test]
    fn repair_requires_explicit_discard_flag() {
        let temporary = tempfile::tempdir().unwrap();
        let database = temporary.path().join("state.db");
        std::fs::write(&database, b"not opened").unwrap();
        assert!(repair_database(&database, false).is_err());
    }
}
