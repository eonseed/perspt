//! Abort command — revoke a session's authority epoch (PSP-9 level 4).
//!
//! Revocation is durable: the bumped epoch invalidates every capability
//! minted under the old epoch, so an in-flight promotion recheck fails and a
//! stale promotion intent is refused on resume. The workspace itself is not
//! touched — a PSP-9 run mutates only its disposable candidate overlay until
//! a certified promotion, so there is nothing to roll back.

use anyhow::{Context, Result};
use std::io::{self, Write};

/// Abort a PSP-9 agent session by revoking its authority epoch.
pub async fn run(force: bool, session_id: Option<String>) -> Result<()> {
    let store = perspt_store::SessionStore::new().context("opening the session store")?;
    let session = match session_id {
        Some(id) => store
            .get_session(&id)?
            .with_context(|| format!("session not found: {id}"))?,
        None => store
            .list_recent_sessions(10)?
            .into_iter()
            .find(|session| session.status == "RUNNING_PSP9")
            .context("no running PSP-9 session found; pass an explicit session id")?,
    };

    if !force {
        print!(
            "⚠ Revoke authority for session {} ({})? [y/N] ",
            &session.session_id[..8.min(session.session_id.len())],
            session.status
        );
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Abort cancelled");
            return Ok(());
        }
    }

    let new_epoch = store.revoke_authority(&session.session_id)?;
    store.update_session_status(&session.session_id, "ABORTED_PSP9")?;

    println!("✓ Authority revoked for session {}", session.session_id);
    println!("  New authority epoch: {new_epoch}");
    println!("  In-flight promotions and stale resume intents are now refused.");
    println!("  The source workspace was not modified; candidate overlays are disposable.");
    Ok(())
}
