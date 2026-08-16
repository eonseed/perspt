//! Ledger command — inspect the durable PSP-9 event chain.

use anyhow::{Context, Result};

/// Query the durable ledger.
pub async fn run(recent: bool, rollback: Option<String>, stats: bool) -> Result<()> {
    if let Some(hash) = rollback {
        anyhow::bail!(
            "`perspt ledger --rollback {hash}` is not implemented; \
             use `perspt abort` to revoke a session's authority instead"
        );
    }
    let store = perspt_store::SessionStore::new().context("opening the session store")?;
    if recent {
        show_recent(&store)
    } else if stats {
        show_stats(&store)
    } else {
        println!("Durable PSP-9 ledger");
        println!();
        println!("Usage:");
        println!("  perspt ledger --recent   Show the newest session's latest events");
        println!("  perspt ledger --stats    Show ledger statistics");
        Ok(())
    }
}

fn show_recent(store: &perspt_store::SessionStore) -> Result<()> {
    let sessions = store.list_recent_sessions(5)?;
    if sessions.is_empty() {
        println!("No sessions recorded yet. Run `perspt agent <task>` to start one.");
        return Ok(());
    }
    for session in &sessions {
        let events = store
            .get_psp9_events(&session.session_id)
            .unwrap_or_default();
        println!(
            "{}  {:<16} {:>5} event(s)  {}",
            &session.session_id[..8.min(session.session_id.len())],
            session.status,
            events.len(),
            session.task.chars().take(48).collect::<String>()
        );
        for row in events.iter().rev().take(5).rev() {
            let kind = serde_json::from_str::<serde_json::Value>(&row.event_json)
                .ok()
                .and_then(|value| {
                    value
                        .get("kind")
                        .or_else(|| value.get("event"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| "event".into());
            println!(
                "    #{:<5} {}  {}",
                row.sequence,
                &row.hash[..12.min(row.hash.len())],
                kind
            );
        }
    }
    Ok(())
}

fn show_stats(store: &perspt_store::SessionStore) -> Result<()> {
    let sessions = store.list_recent_sessions(1000)?;
    let mut total_events = 0usize;
    let mut psp9_sessions = 0usize;
    for session in &sessions {
        let events = store
            .get_psp9_events(&session.session_id)
            .unwrap_or_default();
        if !events.is_empty() {
            psp9_sessions += 1;
            total_events += events.len();
        }
    }
    println!("Ledger statistics:");
    println!("  Sessions:              {}", sessions.len());
    println!("  PSP-9 ledger sessions: {psp9_sessions}");
    println!("  PSP-9 ledger events:   {total_events}");
    if let Ok(path) = perspt_store::SessionStore::default_db_path() {
        if let Ok(metadata) = std::fs::metadata(&path) {
            println!("  Database size:         {} KiB", metadata.len() / 1024);
        }
    }
    Ok(())
}
