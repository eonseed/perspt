//! Status command - show agent session status

use anyhow::{Context, Result};

/// Show current agent status
pub async fn run() -> Result<()> {
    let store = perspt_store::SessionStore::new().context("Failed to open session store")?;

    println!("📊 SRBN Agent Status");
    println!("{}", "═".repeat(70));

    // Get the most recent session
    let sessions = store.list_recent_sessions(1)?;

    if sessions.is_empty() {
        println!("No agent sessions found.");
        println!();
        println!("Start a new session with:");
        println!("  perspt agent \"<task description>\"");
        return Ok(());
    }

    let session = &sessions[0];

    // Session info
    println!(
        "📁 Session:    {}",
        &session.session_id[..16.min(session.session_id.len())]
    );

    let status_display = match session.status.as_str() {
        "COMPLETED" => "✅ Completed",
        "RUNNING" => "🔄 Running",
        "PAUSED" => "⏸️ Paused",
        "FAILED" => "❌ Failed",
        "active" => "🔄 Active",
        _ => &session.status,
    };
    println!("📌 Status:     {}", status_display);
    println!("📂 Directory:  {}", session.working_dir);
    println!("📝 Task:       {}", session.task);

    if let Some(toolchain) = &session.detected_toolchain {
        println!("🔧 Toolchain:  {}", toolchain);
    }

    // Get node states
    let node_states = store.get_node_states(&session.session_id)?;
    if !node_states.is_empty() {
        let completed = node_states
            .iter()
            .filter(|n| n.state == "COMPLETED" || n.state == "STABLE")
            .count();
        let running = node_states
            .iter()
            .filter(|n| n.state == "RUNNING" || n.state == "Coding" || n.state == "Verifying")
            .count();
        let failed = node_states.iter().filter(|n| n.state == "FAILED").count();
        let queued = node_states
            .iter()
            .filter(|n| n.state == "Queued" || n.state == "QUEUED")
            .count();
        let retrying = node_states
            .iter()
            .filter(|n| n.state == "Retrying" || n.state == "RETRYING")
            .count();
        let escalated = node_states
            .iter()
            .filter(|n| n.state == "Escalated" || n.state == "ESCALATED")
            .count();

        println!();
        println!("📊 Node Lifecycle:");
        println!("   Total:       {}", node_states.len());
        println!("   ✅ Completed: {}", completed);
        if queued > 0 {
            println!("   ◇  Queued:    {}", queued);
        }
        if running > 0 {
            println!("   🔄 Running:   {}", running);
        }
        if retrying > 0 {
            println!("   ↻  Retrying:  {}", retrying);
        }
        if failed > 0 {
            println!("   ❌ Failed:    {}", failed);
        }
        if escalated > 0 {
            println!("   ⚠️  Escalated: {}", escalated);
        }

        // PSP-5 Phase 7: Energy component breakdown from latest node
        if let Some(latest) = node_states.last() {
            println!("   ⚡ Energy:    V(x) = {:.3}", latest.v_total);
            // Try to get energy component detail
            if let Ok(energy_history) =
                store.get_energy_history(&session.session_id, &latest.node_id)
            {
                if let Some(last_energy) = energy_history.last() {
                    println!(
                        "   Components:  syn={:.2} str={:.2} log={:.2} boot={:.2} sheaf={:.2}",
                        last_energy.v_syn,
                        last_energy.v_str,
                        last_energy.v_log,
                        last_energy.v_boot,
                        last_energy.v_sheaf
                    );
                }
            }
            // Retry info
            let total_retries: i32 = node_states.iter().map(|n| n.attempt_count.max(0)).sum();
            if total_retries > 0 {
                println!("   ↻ Retries:   {} total across all nodes", total_retries);
            }
        }
    }

    // PSP-5 Phase 7: Escalation reports
    let escalations = store.get_escalation_reports(&session.session_id)?;
    if !escalations.is_empty() {
        println!();
        println!("⚠️  Escalations ({}):", escalations.len());
        for esc in escalations.iter().take(3) {
            println!("   {} → {} ({})", esc.node_id, esc.category, esc.action);
        }
        if escalations.len() > 3 {
            println!("   ... and {} more", escalations.len() - 3);
        }
    }

    // Get LLM request stats
    let llm_requests = store.get_llm_requests(&session.session_id)?;
    if !llm_requests.is_empty() {
        let total_tokens: i32 = llm_requests
            .iter()
            .map(|r| r.tokens_in + r.tokens_out)
            .sum();
        println!();
        println!("🤖 LLM Usage:");
        println!("   Requests:    {}", llm_requests.len());
        println!("   Tokens:      {}", total_tokens);
    }

    // PSP-5 Phase 6: Show provisional branch status
    let branches = store.get_provisional_branches(&session.session_id)?;
    if !branches.is_empty() {
        let active = branches.iter().filter(|b| b.state == "active").count();
        let sealed = branches.iter().filter(|b| b.state == "sealed").count();
        let merged = branches.iter().filter(|b| b.state == "merged").count();
        let flushed = branches.iter().filter(|b| b.state == "flushed").count();
        println!();
        println!("🌿 Provisional Branches:");
        println!("   Total:       {}", branches.len());
        if active > 0 {
            println!("   🔄 Active:   {}", active);
        }
        if sealed > 0 {
            println!("   🔒 Sealed:   {}", sealed);
        }
        if merged > 0 {
            println!("   ✅ Merged:   {}", merged);
        }
        if flushed > 0 {
            println!("   ❌ Flushed:  {}", flushed);
        }
    }

    // PSP-5 Phase 6: Show recent flush decisions
    let flushes = store.get_branch_flushes(&session.session_id)?;
    if !flushes.is_empty() {
        println!();
        println!("🗑️  Recent Flush Decisions:");
        for flush in flushes.iter().take(3) {
            println!(
                "   Parent: {}  Reason: {}",
                flush.parent_node_id, flush.reason
            );
        }
    }

    println!();
    println!("{}", "─".repeat(70));
    println!("Commands:");
    println!("  perspt resume --last    Resume this session");
    println!("  perspt logs --last      View LLM request history");
    println!("  perspt logs             List all sessions");

    Ok(())
}
