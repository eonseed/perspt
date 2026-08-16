//! Replay command (PSP-9 system 14, Paper III Theorem 7).
//!
//! `perspt replay <session>` performs **audit replay**: provider-free, no
//! tool re-execution. It rebuilds the SDK ledger from the durable event
//! stream, folds it deterministically, and reports chain validity, the
//! recomputed head, and the accepted trajectory. No credential is read —
//! that is the point.

use anyhow::Result;
use std::path::PathBuf;

/// Deterministic, credential-free audit replay of one session.
pub async fn run(session_id: String, db_path: Option<PathBuf>) -> Result<()> {
    let store = super::psp9_chain::open_store(db_path.as_deref(), true)?;
    let (ledger, tampered) = super::psp9_chain::rebuild_and_verify(&store, &session_id)?;
    if ledger.is_empty() {
        println!("No PSP-9 ledger events recorded for session {session_id}.");
        println!("Sessions before the governed tool loop are viewable with `perspt logs`.");
        return Ok(());
    }

    let report = perspt_sdk::audit_replay(&ledger);
    println!("Audit replay of session {session_id}");
    println!("  records:      {}", report.records);
    // Chain validity is the stored-row comparison; the rebuilt chain is
    // internally consistent by construction, so it carries no signal alone.
    println!("  chain valid:  {}", tampered.is_empty());
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
