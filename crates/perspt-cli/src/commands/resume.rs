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

    // Get completed nodes
    let node_states = store.get_node_states(&actual_id)?;
    let completed_count = node_states
        .iter()
        .filter(|n| n.state == "COMPLETED" || n.state == "STABLE")
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
            store.update_session_status(&actual_id, "COMPLETED")?;
            println!("✅ Session completed successfully!");
        }
        Err(e) => {
            store.update_session_status(&actual_id, "FAILED")?;
            println!("❌ Session failed: {}", e);
            return Err(e);
        }
    }

    Ok(())
}
