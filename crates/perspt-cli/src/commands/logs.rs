//! Logs command - view LLM request/response history

use anyhow::{Context, Result};

/// View LLM request/response logs
pub async fn run(session_id: Option<String>, last: bool, stats: bool, tui: bool) -> Result<()> {
    // If TUI mode is requested, launch the interactive viewer
    if tui {
        return perspt_tui::run_logs_viewer().await;
    }

    let store = perspt_store::SessionStore::new().context("Failed to open session store")?;

    // Handle --last flag
    let actual_id = if last {
        let sessions = store.list_recent_sessions(1)?;
        if sessions.is_empty() {
            println!("No sessions found.");
            return Ok(());
        }
        sessions[0].session_id.clone()
    } else if let Some(id) = session_id {
        id
    } else {
        // List recent sessions with detailed info
        let sessions = store.list_recent_sessions(10)?;
        if sessions.is_empty() {
            println!("No sessions found.");
            println!();
            println!("Start a new session with: perspt agent \"<task>\"");
            println!();
            println!("💡 Tip: Use --tui flag to launch interactive logs viewer");
            return Ok(());
        }

        println!("📋 Recent Sessions");
        println!("{}", "═".repeat(100));
        println!(
            "{:<38} {:<10} {:<8} {:<40}",
            "SESSION ID", "STATUS", "REQUESTS", "TASK"
        );
        println!("{}", "─".repeat(100));

        for session in &sessions {
            // Show full session ID (UUID format) so user can copy it
            let id_display = if session.session_id.len() > 36 {
                session.session_id[..36].to_string()
            } else {
                session.session_id.clone()
            };

            // Status with emoji
            let status_display = match session.status.as_str() {
                "COMPLETED" => "✅ Done",
                "RUNNING" => "🔄 Active",
                "PAUSED" => "⏸️ Paused",
                "FAILED" => "❌ Failed",
                _ => &session.status,
            };

            // Get request count for this session
            let request_count = store
                .get_llm_requests(&session.session_id)
                .map(|r| r.len())
                .unwrap_or(0);

            // Truncate task
            let task_display = if session.task.len() > 38 {
                format!("{}...", &session.task[..35])
            } else {
                session.task.clone()
            };

            println!(
                "{:<38} {:<10} {:<8} {:<40}",
                id_display, status_display, request_count, task_display
            );
        }

        println!("{}", "═".repeat(100));
        println!();
        println!("💡 Commands:");
        println!("   perspt logs --tui             Launch interactive logs viewer");
        println!("   perspt logs <session_id>      View LLM calls for that session");
        println!("   perspt logs --last --stats    Token usage for last session");
        println!("   perspt resume <session_id>    Resume that session");
        return Ok(());
    };

    // Get LLM requests
    let requests = store.get_llm_requests(&actual_id)?;

    if requests.is_empty() {
        println!("No LLM requests found for session: {}", actual_id);
        println!();
        println!("💡 Tips:");
        println!("   - Make sure you ran the agent with --log-llm flag");
        println!("   - Use 'perspt logs --tui' for interactive viewer");
        return Ok(());
    }

    if stats {
        // Show statistics
        let total_requests = requests.len();
        let total_tokens_in: i32 = requests.iter().map(|r| r.tokens_in).sum();
        let total_tokens_out: i32 = requests.iter().map(|r| r.tokens_out).sum();
        let total_latency: i32 = requests.iter().map(|r| r.latency_ms).sum();
        let avg_latency = if total_requests > 0 {
            total_latency / total_requests as i32
        } else {
            0
        };
        let total_prompt_chars: usize = requests.iter().map(|r| r.prompt.len()).sum();
        let total_response_chars: usize = requests.iter().map(|r| r.response.len()).sum();

        println!();
        println!(
            "📊 LLM Usage Statistics for session {}",
            &actual_id[..8.min(actual_id.len())]
        );
        println!("{}", "═".repeat(50));
        println!();
        println!(
            "  Total requests:      {}",
            colorize_number(total_requests as i32)
        );
        println!("  Total tokens in:     {}", total_tokens_in);
        println!("  Total tokens out:    {}", total_tokens_out);
        println!(
            "  Total prompt chars:  {}",
            format_number(total_prompt_chars)
        );
        println!(
            "  Total response chars:{}",
            format_number(total_response_chars)
        );
        println!();
        println!(
            "  Avg latency:         {}",
            colorize_latency(avg_latency as i64)
        );
        println!(
            "  Total latency:       {}",
            format_duration(total_latency as i64)
        );

        // Model breakdown
        let mut model_counts: std::collections::HashMap<String, (usize, i32)> =
            std::collections::HashMap::new();
        for r in &requests {
            let entry = model_counts.entry(r.model.clone()).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += r.latency_ms;
        }
        println!();
        println!("  By model:");
        for (model, (count, latency)) in model_counts {
            let avg = if count > 0 { latency / count as i32 } else { 0 };
            println!("    {} × {} (avg {}ms)", count, model, avg);
        }
        println!();
    } else {
        // Show individual requests
        println!();
        println!(
            "📜 LLM Requests for session {}",
            &actual_id[..8.min(actual_id.len())]
        );
        println!("{}", "═".repeat(80));

        for (i, req) in requests.iter().enumerate() {
            println!();
            println!(
                "┌──[ Request {} ]{}",
                i + 1,
                "─".repeat(60 - format!("{}", i + 1).len())
            );
            println!("│");
            println!("│  Model:    {}", colorize_model(&req.model));
            println!("│  Latency:  {}", colorize_latency(req.latency_ms as i64));
            println!(
                "│  Node:     {}",
                req.node_id.as_deref().unwrap_or("(none)")
            );
            println!(
                "│  Size:     {} → {} chars",
                req.prompt.len(),
                req.response.len()
            );
            println!("│");

            // Truncate prompt for display
            println!("│  📝 Prompt:");
            let prompt_lines: Vec<&str> = req.prompt.lines().take(5).collect();
            for line in &prompt_lines {
                let truncated = if line.len() > 70 {
                    format!("{}...", &line[..67])
                } else {
                    line.to_string()
                };
                println!("│     {}", truncated);
            }
            if req.prompt.lines().count() > 5 {
                println!("│     ... ({} more lines)", req.prompt.lines().count() - 5);
            }
            println!("│");

            // Truncate response for display
            println!("│  💬 Response:");
            let response_lines: Vec<&str> = req.response.lines().take(8).collect();
            for line in &response_lines {
                let truncated = if line.len() > 70 {
                    format!("{}...", &line[..67])
                } else {
                    line.to_string()
                };
                println!("│     {}", truncated);
            }
            if req.response.lines().count() > 8 {
                println!(
                    "│     ... ({} more lines)",
                    req.response.lines().count() - 8
                );
            }

            println!("│");
            println!("└{}", "─".repeat(72));
        }
        println!();
        println!("💡 Tip: Use 'perspt logs --tui' for interactive viewing with full content");
    }

    Ok(())
}

fn colorize_number(n: i32) -> String {
    format!("{}", n)
}

fn colorize_latency(ms: i64) -> String {
    if ms < 1000 {
        format!("{}ms ✓", ms)
    } else if ms < 3000 {
        format!("{}ms", ms)
    } else {
        format!("{}ms ⚠", ms)
    }
}

fn colorize_model(model: &str) -> String {
    model.to_string()
}

fn format_number(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}

fn format_duration(ms: i64) -> String {
    if ms >= 60_000 {
        format!("{}m {}s", ms / 60_000, (ms % 60_000) / 1000)
    } else if ms >= 1000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        format!("{}ms", ms)
    }
}
