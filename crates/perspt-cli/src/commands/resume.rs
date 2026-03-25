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

    // Create orchestrator with the same settings
    let mut orchestrator = perspt_agent::SRBNOrchestrator::new(
        working_dir.clone(),
        false, // Don't auto-approve on resume
    );

    // Run the task (orchestrator will skip completed nodes based on ledger)
    match orchestrator.run(session.task.clone()).await {
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
