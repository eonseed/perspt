//! Ledger command — inspect the durable PSP-9 event chain.

use anyhow::{Context, Result};

/// Query the durable ledger, or roll an accepted promotion back.
pub async fn run(recent: bool, rollback: Option<String>, stats: bool) -> Result<()> {
    let store = perspt_store::SessionStore::new().context("opening the session store")?;
    if let Some(session) = rollback {
        return roll_back(&store, &session);
    }
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

/// Undo the session's newest completed promotion: restore every file's
/// recorded pre-image through the hardened descriptor-relative path,
/// record a rollback event, and label the rolled-back candidate UNSAFE —
/// the rollback boundary is the delayed unsafe-label source feeding the
/// conformal stream and the independence estimator (spec lines 2670–2673).
fn roll_back(store: &perspt_store::SessionStore, session_prefix: &str) -> Result<()> {
    let sessions = store.list_recent_sessions(200)?;
    let session = sessions
        .iter()
        .find(|s| s.session_id.starts_with(session_prefix))
        .with_context(|| format!("no session matches {session_prefix:?}"))?;
    let promotion = store
        .completed_external_effects(&session.session_id)?
        .into_iter()
        .find(|effect| effect.idempotency_key.starts_with("promote:"))
        .context("session has no completed promotion to roll back")?;
    let intent: serde_json::Value = serde_json::from_str(&promotion.intent_json)?;
    let workspace = std::path::PathBuf::from(
        intent
            .get("workspace_root")
            .and_then(serde_json::Value::as_str)
            .context("promotion intent has no workspace binding")?,
    );
    let candidate_root = intent
        .get("candidate_root")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let files = intent
        .get("files")
        .and_then(serde_json::Value::as_array)
        .context("promotion intent has no file manifest")?;

    let root = perspt_agent::promote::WorkspaceRoot::open(&workspace)?;
    let mut restored = Vec::new();
    for file in files {
        let relative = file
            .get("path")
            .and_then(serde_json::Value::as_str)
            .context("promotion file has no path")?;
        let target = root.target_dir(relative, true)?;
        verify_current_matches(&target, file, relative)?;
        let before = file
            .get("before_hash")
            .and_then(serde_json::Value::as_str)
            .map(|hash| fetch_artifact(store, hash))
            .transpose()?;
        target.apply(before.as_deref())?;
        restored.push(relative.to_string());
    }

    append_chained_event(
        store,
        &session.session_id,
        serde_json::json!({
            "promotion": promotion.idempotency_key,
            "restored": restored,
            "candidate_root": candidate_root,
        }),
    )?;
    // The rollback boundary IS the delayed unsafe label: the promoted
    // candidate was found wrong in practice.
    let labeled_samples = store.label_psp9_calibration_sample(&candidate_root, true)?;
    let labeled_verdicts = store.label_psp9_verdicts(&candidate_root, true)?;
    println!(
        "Rolled back {} file(s) from {}; labeled UNSAFE \
         ({labeled_samples} calibration sample(s), {labeled_verdicts} verdict(s)).",
        restored.len(),
        promotion.idempotency_key
    );
    Ok(())
}

/// Refuse rollback when the workspace file no longer matches the promoted
/// content — user edits after promotion are never silently destroyed.
fn verify_current_matches(
    target: &perspt_agent::promote::TargetDir,
    file: &serde_json::Value,
    relative: &str,
) -> Result<()> {
    let current = target.read_optional()?;
    let current_hash = current.as_deref().map(perspt_sdk::content_hash);
    let after_hash = file
        .get("after_hash")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    anyhow::ensure!(
        current_hash == after_hash,
        "{relative} changed after promotion; refusing rollback (resolve manually)"
    );
    Ok(())
}

fn fetch_artifact(store: &perspt_store::SessionStore, hash: &str) -> Result<Vec<u8>> {
    let bytes = store
        .get_psp9_artifact(hash)?
        .with_context(|| format!("missing promotion artifact {hash}"))?;
    anyhow::ensure!(
        perspt_sdk::content_hash(&bytes) == hash,
        "promotion artifact hash mismatch"
    );
    Ok(bytes)
}

/// Extend the session's verified hash chain with the rollback event.
fn append_chained_event(
    store: &perspt_store::SessionStore,
    session_id: &str,
    payload: serde_json::Value,
) -> Result<()> {
    let (ledger, tampered) = super::psp9_chain::rebuild_and_verify(store, session_id)?;
    anyhow::ensure!(
        tampered.is_empty(),
        "ledger diverges at sequences {tampered:?}; refusing to extend it"
    );
    let record = ledger
        .stage(perspt_sdk::LedgerEvent::Custom {
            kind: "promotion_rolled_back".into(),
            payload,
        })
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    store.record_psp9_event(&perspt_store::Psp9LedgerRow {
        session_id: session_id.to_string(),
        sequence: record.sequence as i64,
        event_json: serde_json::to_string(&record.event)?,
        prev_hash: record.prev_hash.clone(),
        hash: record.hash.clone(),
    })?;
    Ok(())
}
