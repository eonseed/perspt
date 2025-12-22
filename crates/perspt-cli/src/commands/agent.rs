//! Agent command - SRBN agent execution mode

use anyhow::Result;
use std::path::PathBuf;

/// Execution mode
#[derive(Debug, Clone, Copy)]
pub enum ExecutionMode {
    /// Maximum user involvement, approve every change
    Cautious,
    /// Balanced - approve sub-graphs based on complexity K
    Balanced,
    /// Minimal prompts, auto-approve most changes
    Yolo,
}

impl ExecutionMode {
    fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "cautious" => ExecutionMode::Cautious,
            "yolo" => ExecutionMode::Yolo,
            _ => ExecutionMode::Balanced,
        }
    }
}

/// Run the SRBN agent on a task
pub async fn run(
    task: String,
    workdir: Option<PathBuf>,
    auto_approve: bool,
    complexity_k: usize,
    mode: String,
) -> Result<()> {
    let working_dir = workdir.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let exec_mode = ExecutionMode::from_str(&mode);

    log::info!("Starting SRBN agent");
    log::info!("  Task: {}", task);
    log::info!("  Working directory: {:?}", working_dir);
    log::info!("  Auto-approve: {}", auto_approve);
    log::info!("  Complexity K: {}", complexity_k);
    log::info!("  Mode: {:?}", exec_mode);

    // Create the orchestrator
    let mut orchestrator = perspt_agent::SRBNOrchestrator::new(working_dir.clone(), auto_approve);

    // Set complexity threshold
    orchestrator.context.complexity_k = complexity_k;

    println!("🚀 SRBN Agent starting...");
    println!("   Session: {}", orchestrator.session_id());
    println!("   Task: {}", task);
    println!();

    // Check if we should run in TUI mode or headless mode
    let is_tty = atty::is(atty::Stream::Stdout);

    if is_tty && !auto_approve {
        // Interactive mode with TUI
        println!("Running in interactive TUI mode...");
        println!("(Use --yes flag to run headlessly)");
        perspt_tui::run_agent_tui()?;
    } else {
        // Headless mode - run orchestrator directly
        println!(
            "Running in headless mode (auto-approve={})...",
            auto_approve
        );
        println!();

        // Run the SRBN control loop
        match orchestrator.run(task.clone()).await {
            Ok(()) => {
                println!();
                println!("✅ Task completed successfully!");
                println!("   Nodes processed: {}", orchestrator.node_count());
            }
            Err(e) => {
                println!();
                println!("❌ Task failed: {}", e);
                return Err(e);
            }
        }
    }

    println!();
    println!("✓ Agent session completed");
    Ok(())
}
