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
    // PSP-5 Phase 8: Use latest-per-node dedup when available
    let latest_states = store
        .get_latest_node_states(&session.session_id)
        .unwrap_or_default();
    let display_states = if latest_states.is_empty() {
        &node_states
    } else {
        &latest_states
    };

    if !display_states.is_empty() {
        use perspt_core::types::NodeState;

        let completed = display_states
            .iter()
            .filter(|n| NodeState::from_display_str(&n.state).is_success())
            .count();
        let running = display_states
            .iter()
            .filter(|n| NodeState::from_display_str(&n.state).is_active())
            .count();
        let failed = display_states
            .iter()
            .filter(|n| NodeState::from_display_str(&n.state) == NodeState::Failed)
            .count();
        let queued = display_states
            .iter()
            .filter(|n| NodeState::from_display_str(&n.state) == NodeState::TaskQueued)
            .count();
        let retrying = display_states
            .iter()
            .filter(|n| NodeState::from_display_str(&n.state) == NodeState::Retry)
            .count();
        let escalated = display_states
            .iter()
            .filter(|n| NodeState::from_display_str(&n.state) == NodeState::Escalated)
            .count();

        println!();
        println!("📊 Node Lifecycle:");
        println!("   Total:       {}", display_states.len());
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

        // PSP-5 Phase 8: Node class breakdown
        let interface_count = display_states
            .iter()
            .filter(|n| n.node_class.as_deref() == Some("Interface"))
            .count();
        let impl_count = display_states
            .iter()
            .filter(|n| n.node_class.as_deref() == Some("Implementation"))
            .count();
        let integ_count = display_states
            .iter()
            .filter(|n| n.node_class.as_deref() == Some("Integration"))
            .count();
        if interface_count > 0 || impl_count > 0 || integ_count > 0 {
            println!();
            println!("🏗️  Node Classes:");
            if interface_count > 0 {
                println!("   🔌 Interface:      {}", interface_count);
            }
            if impl_count > 0 {
                println!("   ⚙️  Implementation: {}", impl_count);
            }
            if integ_count > 0 {
                println!("   🔗 Integration:    {}", integ_count);
            }
        }

        // PSP-5 Phase 7: Energy component breakdown from latest node
        if let Some(latest) = display_states.last() {
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
            let total_retries: i32 = display_states.iter().map(|n| n.attempt_count.max(0)).sum();
            if total_retries > 0 {
                println!("   ↻ Retries:   {} total across all nodes", total_retries);
            }
        }

        // PSP-5 Phase 8: Show verification stage summary for latest completed nodes
        let mut ver_ok = 0;
        let mut ver_degraded = 0;
        for ns in display_states {
            if let Ok(Some(vr)) = store.get_verification_result(&session.session_id, &ns.node_id) {
                if vr.degraded {
                    ver_degraded += 1;
                } else {
                    ver_ok += 1;
                }
            }
        }
        if ver_ok > 0 || ver_degraded > 0 {
            println!();
            println!("🔍 Verification:");
            println!("   ✅ Passed:    {}", ver_ok);
            if ver_degraded > 0 {
                println!("   ⚠️  Degraded:  {}", ver_degraded);
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

    // PSP-5 Phase 7: Show review audit summary
    let reviews = store.get_all_review_outcomes(&session.session_id)?;
    if !reviews.is_empty() {
        let approved = reviews
            .iter()
            .filter(|r| r.outcome.starts_with("approved") || r.outcome == "auto_approved")
            .count();
        let rejected = reviews
            .iter()
            .filter(|r| r.outcome == "rejected" || r.outcome == "aborted")
            .count();
        let corrected = reviews
            .iter()
            .filter(|r| r.outcome == "correction_requested")
            .count();
        let degraded_reviews = reviews.iter().filter(|r| r.degraded == Some(true)).count();

        println!();
        println!("📋 Review Audit:");
        println!("   Total:          {}", reviews.len());
        println!("   ✅ Approved:    {}", approved);
        if rejected > 0 {
            println!("   ❌ Rejected:    {}", rejected);
        }
        if corrected > 0 {
            println!("   🔄 Corrected:   {}", corrected);
        }
        if degraded_reviews > 0 {
            println!("   ⚠️  Degraded:    {}", degraded_reviews);
        }
    }

    // PSP-7: Step timeline and correction attempt summaries
    if let Ok(steps) = store.get_session_steps(&session.session_id) {
        if !steps.is_empty() {
            println!();
            println!("📊 Step Timeline ({} records):", steps.len());
            // Show per-step-type summary
            let mut step_counts = std::collections::HashMap::new();
            let mut total_duration_ms: i64 = 0;
            for s in &steps {
                *step_counts.entry(s.step.as_str()).or_insert(0u32) += 1;
                total_duration_ms += s.duration_ms as i64;
            }
            for (step, count) in &step_counts {
                println!("   {:<18} {}", step, count);
            }
            if total_duration_ms > 0 {
                println!(
                    "   Total step time:  {:.1}s",
                    total_duration_ms as f64 / 1000.0
                );
            }

            // Show correction attempts for nodes that had them
            let nodes_with_retries: Vec<_> = steps
                .iter()
                .filter(|s| s.step == "converge" && s.attempt_count > 0)
                .collect();
            if !nodes_with_retries.is_empty() {
                println!();
                println!("🔄 Correction Attempts:");
                let mut shown = std::collections::HashSet::new();
                for s in &nodes_with_retries {
                    if shown.insert(&s.node_id) {
                        if let Ok(attempts) =
                            store.get_correction_attempts(&session.session_id, &s.node_id)
                        {
                            let accepted = attempts.iter().filter(|a| a.accepted).count();
                            let rejected = attempts.len() - accepted;
                            println!(
                                "   {}: {} attempt(s) ({} accepted, {} rejected)",
                                s.node_id,
                                attempts.len(),
                                accepted,
                                rejected
                            );
                        }
                    }
                }
            }
        }
    }

    // PSP-5 Phase 8: Budget envelope status
    if let Ok(Some(budget)) = store.get_budget_envelope(&session.session_id) {
        println!();
        println!("💰 Budget:");
        let steps_str = budget
            .max_steps
            .map(|m| format!("{}/{}", budget.steps_used, m))
            .unwrap_or_else(|| format!("{}", budget.steps_used));
        println!("   Steps:       {}", steps_str);
        let cost_str = budget
            .max_cost_usd
            .map(|m| format!("${:.2}/${:.2}", budget.cost_used_usd, m))
            .unwrap_or_else(|| format!("${:.2}", budget.cost_used_usd));
        println!("   Cost:        {}", cost_str);
        println!("   Revisions:   {}", budget.revisions_used);
    }

    println!();
    println!("{}", "─".repeat(70));
    println!("Commands:");
    println!("  perspt resume --last    Resume this session");
    println!("  perspt logs --last      View LLM request history");
    println!("  perspt logs             List all sessions");

    Ok(())
}
