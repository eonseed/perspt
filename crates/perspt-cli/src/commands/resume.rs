//! Resume command - resume a paused or crashed session

use anyhow::{Context, Result};
use std::path::PathBuf;

/// Resume a paused or crashed session
pub async fn run(session_id: Option<String>) -> Result<()> {
    let store = perspt_store::SessionStore::new().context("Failed to open session store")?;

    match session_id {
        Some(id) => resume_session(&store, &id).await,
        None => list_sessions(&store).await,
    }
}

/// List recent sessions for the user to choose from
async fn list_sessions(store: &perspt_store::SessionStore) -> Result<()> {
    let sessions = store.list_recent_sessions(10)?;

    if sessions.is_empty() {
        println!("No sessions found.");
        println!();
        println!("Start a new session with: perspt agent \"<task>\"");
        return Ok(());
    }

    println!("Recent Sessions:");
    println!("{}", "─".repeat(80));
    println!("{:<12} {:<12} {:<50}", "SESSION ID", "STATUS", "TASK");
    println!("{}", "─".repeat(80));

    for session in &sessions {
        // Truncate task to 48 chars for display
        let task_display = if session.task.len() > 48 {
            format!("{}...", &session.task[..45])
        } else {
            session.task.clone()
        };

        // Shorten session ID for display
        let id_short = if session.session_id.len() > 10 {
            format!("{}...", &session.session_id[..8])
        } else {
            session.session_id.clone()
        };

        let status_emoji = match session.status.as_str() {
            "COMPLETED" => "✅",
            "RUNNING" => "🔄",
            "PAUSED" => "⏸️",
            "FAILED" => "❌",
            _ => "❓",
        };

        println!(
            "{:<12} {} {:<10} {:<50}",
            id_short, status_emoji, session.status, task_display
        );
    }

    println!("{}", "─".repeat(80));
    println!();
    println!("Resume with: perspt resume <session_id>");
    println!("Resume last: perspt resume --last");

    Ok(())
}

/// Resume a specific session
async fn resume_session(store: &perspt_store::SessionStore, session_id: &str) -> Result<()> {
    // Handle --last flag
    let actual_id = if session_id == "--last" {
        let sessions = store.list_recent_sessions(1)?;
        if sessions.is_empty() {
            anyhow::bail!("No sessions found to resume");
        }
        sessions[0].session_id.clone()
    } else {
        session_id.to_string()
    };

    // Get the session
    let session = store
        .get_session(&actual_id)?
        .context(format!("Session not found: {}", actual_id))?;

    println!("📂 Resuming session: {}", session.session_id);
    println!("📝 Task: {}", session.task);
    println!("📁 Working dir: {}", session.working_dir);
    println!("🔖 Status: {}", session.status);

    if session.status.ends_with("_PSP9") {
        return resume_psp9(store, &session).await;
    }

    // Get completed nodes
    let node_states = store.get_node_states(&actual_id)?;
    let completed_count = node_states
        .iter()
        .filter(|n| perspt_core::types::NodeState::from_display_str(&n.state).is_success())
        .count();

    println!(
        "✅ Completed nodes: {}/{}",
        completed_count,
        node_states.len()
    );

    // PSP-5 Phase 6: Show provisional branch state
    let branches = store.get_provisional_branches(&actual_id)?;
    if !branches.is_empty() {
        let active = branches.iter().filter(|b| b.state == "active").count();
        let flushed = branches.iter().filter(|b| b.state == "flushed").count();
        if active > 0 || flushed > 0 {
            println!(
                "🌿 Provisional: {} active, {} flushed (of {} total)",
                active,
                flushed,
                branches.len()
            );
        }
    }

    // PSP-5 Phase 7: Show trust context before resuming
    let escalations = store.get_escalation_reports(&actual_id)?;
    if !escalations.is_empty() {
        println!("⚠️  Escalations: {} recorded", escalations.len());
    }
    // Show last energy state
    if let Some(latest) = node_states.last() {
        if let Ok(energy_history) = store.get_energy_history(&actual_id, &latest.node_id) {
            if let Some(last_energy) = energy_history.last() {
                println!(
                    "⚡ Last energy: V(x)={:.3} (syn={:.2} str={:.2} log={:.2})",
                    last_energy.v_total, last_energy.v_syn, last_energy.v_str, last_energy.v_log
                );
            }
        }
    }
    let total_retries: i32 = node_states.iter().map(|n| n.attempt_count.max(0)).sum();
    if total_retries > 0 {
        println!("↻  Total retries: {}", total_retries);
    }

    // PSP-5 Phase 8: Show budget position before resuming
    if let Ok(Some(budget)) = store.get_budget_envelope(&actual_id) {
        let steps_str = budget
            .max_steps
            .map(|m| format!("{}/{}", budget.steps_used, m))
            .unwrap_or_else(|| format!("{}", budget.steps_used));
        let cost_str = budget
            .max_cost_usd
            .map(|m| format!("${:.2}/${:.2}", budget.cost_used_usd, m))
            .unwrap_or_else(|| format!("${:.2}", budget.cost_used_usd));
        println!("💰 Budget: steps={} cost={}", steps_str, cost_str);
    }

    // PSP-7: Show step timeline and correction summary before resuming
    if let Ok(steps) = store.get_session_steps(&actual_id) {
        if !steps.is_empty() {
            let corrections: Vec<_> = steps
                .iter()
                .filter(|s| s.step == "converge" && s.attempt_count > 0)
                .collect();
            if !corrections.is_empty() {
                let total_attempts: i32 = corrections.iter().map(|s| s.attempt_count).sum();
                println!(
                    "🔄 Corrections: {} node(s), {} attempts",
                    corrections.len(),
                    total_attempts
                );
            }
        }
    }

    // Check if session is already completed
    if session.status == "COMPLETED" {
        println!();
        println!("ℹ️  This session is already completed.");
        println!("   Start a new session with: perspt agent \"<task>\"");
        return Ok(());
    }

    // Update session status to RUNNING
    store.update_session_status(&actual_id, "RUNNING")?;

    // Create orchestrator and resume
    let working_dir = PathBuf::from(&session.working_dir);

    if !working_dir.exists() {
        anyhow::bail!(
            "Working directory no longer exists: {}",
            session.working_dir
        );
    }

    println!();
    println!("🚀 Resuming orchestration...");
    println!();

    // PSP-5 Phase 8: Rehydrate from persisted session state instead of
    // creating a fresh orchestrator that would re-plan from scratch.
    let mut orchestrator = perspt_agent::SRBNOrchestrator::new(
        working_dir.clone(),
        false, // Don't auto-approve on resume
    );

    // Attempt ledger-backed rehydration; fall back to fresh run if the
    // session has no persisted node data (pre-Phase-8 session or empty DAG).
    let rehydrated = match orchestrator.rehydrate_session(&actual_id) {
        Ok(snapshot) => {
            let total = snapshot.node_details.len();
            let terminal = snapshot
                .node_details
                .iter()
                .filter(|d| {
                    matches!(
                        d.record.state.as_str(),
                        "Completed"
                            | "COMPLETED"
                            | "STABLE"
                            | "Failed"
                            | "FAILED"
                            | "Aborted"
                            | "ABORTED"
                    )
                })
                .count();
            println!(
                "📦 Rehydrated {} nodes ({} terminal, {} to resume)",
                total,
                terminal,
                total - terminal
            );

            // Show degraded conditions
            let missing_goals = snapshot
                .node_details
                .iter()
                .filter(|d| d.record.goal.is_none())
                .count();
            if missing_goals > 0 {
                println!(
                    "⚠️  Degraded: {} nodes missing goal metadata (older session)",
                    missing_goals
                );
            }

            true
        }
        Err(e) => {
            println!(
                "⚠️  Cannot rehydrate session ({}), falling back to fresh run",
                e
            );
            false
        }
    };

    let result = if rehydrated {
        orchestrator.run_resumed().await
    } else {
        orchestrator.run(session.task.clone()).await
    };

    match result {
        Ok(()) => {
            println!("✅ Session completed successfully!");
        }
        Err(e) => {
            println!("❌ Session failed: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

/// PSP-9 resume is credential-free recovery, not a return to the legacy
/// orchestrator. It verifies the ledger and completes bracketed promotion
/// intents from content-addressed artifacts. A new model loop is started as a
/// new session because live capabilities are intentionally not serialized.
async fn resume_psp9(
    store: &perspt_store::SessionStore,
    session: &perspt_store::SessionRecord,
) -> Result<()> {
    verify_psp9_chain(store, &session.session_id)?;
    let pending = store.pending_external_effects(&session.session_id)?;
    if pending.is_empty() {
        println!("PSP-9 ledger is valid and has no incomplete external effects.");
        anyhow::ensure!(
            session.status != "RUNNING_PSP9",
            "the process stopped inside a model/tool loop without a durable candidate checkpoint; \
             exact continuation is unavailable, and Perspt will not silently restart the task"
        );
        println!("This session is terminal; start a new agent session for additional work.");
        return Ok(());
    }
    let current_epoch = store.authority_epoch(&session.session_id)?;
    for effect in pending {
        let intent: serde_json::Value = serde_json::from_str(&effect.intent_json)?;
        let epoch = intent
            .get("authority_epoch")
            .and_then(serde_json::Value::as_u64)
            .context("promotion intent has no authority epoch")?;
        anyhow::ensure!(
            epoch == current_epoch,
            "refusing stale promotion intent at epoch {epoch}; current epoch is {current_epoch}"
        );
        let workspace = PathBuf::from(
            intent
                .get("workspace_root")
                .and_then(serde_json::Value::as_str)
                .context("promotion intent has no workspace binding")?,
        )
        .canonicalize()?;
        anyhow::ensure!(
            workspace == PathBuf::from(&session.working_dir).canonicalize()?,
            "promotion workspace binding differs from the session workspace"
        );
        let files = intent
            .get("files")
            .and_then(serde_json::Value::as_array)
            .context("promotion intent has no file manifest")?;
        for file in files {
            recover_promoted_file(store, &workspace, file)?;
        }
        store.complete_external_effect(
            &session.session_id,
            &effect.idempotency_key,
            &serde_json::json!({"recovered": true, "files": files.len()}).to_string(),
        )?;
        println!(
            "Completed interrupted promotion {} ({} file(s)).",
            effect.idempotency_key,
            files.len()
        );
    }
    store.update_session_status(&session.session_id, "COMPLETED_PSP9")?;
    Ok(())
}

fn verify_psp9_chain(store: &perspt_store::SessionStore, session_id: &str) -> Result<()> {
    let rows = store.get_psp9_events(session_id)?;
    anyhow::ensure!(
        !rows.is_empty(),
        "PSP-9 session has no durable ledger events"
    );
    let mut ledger = perspt_sdk::Ledger::new();
    for (sequence, row) in rows.iter().enumerate() {
        anyhow::ensure!(row.sequence == sequence as i64, "ledger sequence mismatch");
        anyhow::ensure!(
            row.prev_hash == ledger.head(),
            "ledger predecessor mismatch"
        );
        let event: perspt_sdk::LedgerEvent = serde_json::from_str(&row.event_json)?;
        let head = ledger.append(event)?;
        anyhow::ensure!(
            head == row.hash,
            "ledger hash mismatch at sequence {sequence}"
        );
    }
    Ok(())
}

fn recover_promoted_file(
    store: &perspt_store::SessionStore,
    workspace: &std::path::Path,
    manifest: &serde_json::Value,
) -> Result<()> {
    let relative = manifest
        .get("path")
        .and_then(serde_json::Value::as_str)
        .context("promotion file has no path")?;
    let relative_path = std::path::Path::new(relative);
    anyhow::ensure!(
        !relative_path.is_absolute()
            && !relative_path
                .components()
                .any(|component| component == std::path::Component::ParentDir),
        "promotion path escapes workspace: {relative:?}"
    );
    let target = workspace.join(relative_path);
    let before = manifest
        .get("before_hash")
        .and_then(serde_json::Value::as_str);
    let after = manifest
        .get("after_hash")
        .and_then(serde_json::Value::as_str);
    let current_bytes = std::fs::read(&target).ok();
    let current = current_bytes.as_deref().map(perspt_sdk::content_hash);
    if current.as_deref() == after {
        return Ok(());
    }
    anyhow::ensure!(
        current.as_deref() == before,
        "workspace file changed after promotion intent: {relative}"
    );
    match after {
        Some(hash) => {
            let bytes = store
                .get_psp9_artifact(hash)?
                .with_context(|| format!("missing promotion artifact {hash}"))?;
            anyhow::ensure!(
                perspt_sdk::content_hash(&bytes) == hash,
                "promotion artifact hash mismatch"
            );
            let parent = target.parent().context("promotion target has no parent")?;
            std::fs::create_dir_all(parent)?;
            let staged = parent.join(format!(".perspt-resume-{}", &hash[..12.min(hash.len())]));
            std::fs::write(&staged, bytes)?;
            std::fs::rename(staged, target)?;
        }
        None if target.is_file() => std::fs::remove_file(target)?,
        None => {}
    }
    Ok(())
}
