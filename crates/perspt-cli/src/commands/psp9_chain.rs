//! Shared PSP-9 ledger chain reconstruction and verification.
//!
//! `perspt replay` and `perspt resume` verify the same durable chain; one
//! implementation keeps their verdicts from diverging.

use anyhow::{Context, Result};
use perspt_sdk::ledger::{Ledger, LedgerEvent};
use perspt_store::SessionStore;

/// Rebuild a session's ledger from the durable rows, verifying sequence,
/// predecessor, and stored hash against the recomputed chain. Returns the
/// rebuilt ledger and every sequence whose stored row diverges.
pub fn rebuild_and_verify(store: &SessionStore, session_id: &str) -> Result<(Ledger, Vec<i64>)> {
    let rows = store
        .get_psp9_events(session_id)
        .context("loading the PSP-9 event stream")?;
    let mut ledger = Ledger::new();
    let mut tampered = Vec::new();
    for (expected_sequence, row) in rows.iter().enumerate() {
        let mut diverged =
            row.sequence != expected_sequence as i64 || row.prev_hash != ledger.head();
        let event: LedgerEvent = serde_json::from_str(&row.event_json)
            .with_context(|| format!("decoding event at sequence {}", row.sequence))?;
        let head = ledger.append(event)?;
        diverged |= head != row.hash;
        if diverged {
            tampered.push(row.sequence);
        }
    }
    Ok((ledger, tampered))
}

/// Open a session store, preferring an explicit database path.
pub fn open_store(db_path: Option<&std::path::Path>, read_only: bool) -> Result<SessionStore> {
    match (db_path, read_only) {
        (Some(path), true) => SessionStore::open_read_only(path),
        (Some(path), false) => SessionStore::open(path),
        (None, true) => SessionStore::open_read_only(&SessionStore::default_db_path()?),
        (None, false) => SessionStore::new(),
    }
    .context("opening the session store")
}
