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
#[allow(clippy::too_many_arguments)]
pub async fn run(
    task: String,
    workdir: Option<PathBuf>,
    auto_approve: bool,
    complexity_k: usize,
    mode: String,
    model: Option<String>,
    architect_model: Option<String>,
    actuator_model: Option<String>,
    verifier_model: Option<String>,
    speculator_model: Option<String>,
) -> Result<()> {
    let working_dir = workdir.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let exec_mode = ExecutionMode::from_str(&mode);

    // Resolve models: --model overrides all, otherwise use per-tier or defaults
    let architect = model.clone().or(architect_model);
    let actuator = model.clone().or(actuator_model);
    let verifier = model.clone().or(verifier_model);
    let speculator = model.or(speculator_model);

    // Get default model name for logging
    let default_model = perspt_agent::ModelTier::default_model_name();

    log::info!("Starting SRBN agent");
    log::info!("  Task: {}", task);
    log::info!("  Working directory: {:?}", working_dir);
    log::info!("  Auto-approve: {}", auto_approve);
    log::info!("  Complexity K: {}", complexity_k);
    log::info!("  Mode: {:?}", exec_mode);
    log::info!(
        "  Architect model: {}",
        architect.as_deref().unwrap_or_else(|| {
            log::debug!("Using default");
            default_model
        })
    );
    log::info!(
        "  Actuator model: {}",
        actuator.as_deref().unwrap_or(default_model)
    );
    log::info!(
        "  Verifier model: {}",
        verifier.as_deref().unwrap_or(default_model)
    );
    log::info!(
        "  Speculator model: {}",
        speculator.as_deref().unwrap_or(default_model)
    );

    // Create the orchestrator with model configuration
    let mut orchestrator = perspt_agent::SRBNOrchestrator::new_with_models(
        working_dir.clone(),
        auto_approve,
        architect,
        actuator,
        verifier,
        speculator,
    );

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

        // Start Python LSP (ty) for type checking
        println!("   🔍 Starting ty language server for Python...");
        if let Err(e) = orchestrator.start_python_lsp().await {
            log::warn!("Failed to start ty: {}", e);
            println!("   ⚠️ Continuing without LSP (ty not available)");
        } else {
            println!("   ✅ ty language server ready");
        }
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
