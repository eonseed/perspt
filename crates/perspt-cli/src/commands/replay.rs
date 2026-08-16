//! Replay command (PSP-9 system 14, Paper III Theorem 7).
//!
//! `perspt replay <session>` performs **audit replay**: provider-free, no
//! tool re-execution. It rebuilds the SDK ledger from the durable event
//! stream, folds it deterministically, and reports chain validity, the
//! recomputed head, and the accepted trajectory. No credential is read —
//! that is the point.

use anyhow::{Context, Result};
use perspt_sdk::ledger::{Ledger, LedgerEvent};
use perspt_store::SessionStore;

/// Deterministic, credential-free audit replay of one session.
pub async fn run(session_id: String) -> Result<()> {
    let store = SessionStore::open_read_only(&SessionStore::default_db_path()?)
        .context("opening the session store read-only")?;
    let rows = store
        .get_psp9_events(&session_id)
        .context("loading the PSP-9 event stream")?;
    if rows.is_empty() {
        println!("No PSP-9 ledger events recorded for session {session_id}.");
        println!("Sessions before the governed tool loop are viewable with `perspt logs`.");
        return Ok(());
    }

    // Rebuild the chain by re-appending events; the fold recomputes every
    // hash, so tampering with any stored record changes the head.
    let mut ledger = Ledger::new();
    let mut tampered = Vec::new();
    for (expected_sequence, row) in rows.iter().enumerate() {
        if row.sequence != expected_sequence as i64 || row.prev_hash != ledger.head() {
            tampered.push(row.sequence);
        }
        let event: LedgerEvent = serde_json::from_str(&row.event_json)
            .with_context(|| format!("decoding event at sequence {}", row.sequence))?;
        let head = ledger.append(event)?;
        if head != row.hash {
            tampered.push(row.sequence);
        }
    }

    let report = perspt_sdk::audit_replay(&ledger);
    println!("Audit replay of session {session_id}");
    println!("  records:      {}", report.records);
    println!("  chain valid:  {}", report.chain_ok && tampered.is_empty());
    if !tampered.is_empty() {
        println!("  TAMPERED at sequences: {tampered:?}");
        println!("  The recomputed chain diverges from the stored hashes.");
    }
    println!("  head:         {}", report.head);
    println!(
        "  accepted trajectory ({} checkpoint(s)):",
        report.accepted.len()
    );
    for (node, generation, energy) in &report.accepted {
        println!("    {node} gen {generation}: V = {energy:.3}");
    }
    println!();
    println!("Audit replay re-ran no tools and read no provider credentials.");
    Ok(())
}
