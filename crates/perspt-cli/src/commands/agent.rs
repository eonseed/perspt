//! Agent command - SRBN agent execution mode

use anyhow::Result;
use std::io::IsTerminal;
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
    defer_tests: bool,
    log_llm: bool,
    single_file: bool,
    verifier_strictness: String,
    architect_fallback_model: Option<String>,
    actuator_fallback_model: Option<String>,
    verifier_fallback_model: Option<String>,
    speculator_fallback_model: Option<String>,
    output_plan: Option<PathBuf>,
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
    log::info!("  Defer tests: {}", defer_tests);
    log::info!("  Log LLM: {}", log_llm);
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
        architect_fallback_model,
        actuator_fallback_model,
        verifier_fallback_model,
        speculator_fallback_model,
    );

    // Set complexity threshold, defer_tests, and log_llm
    orchestrator.context.complexity_k = complexity_k;
    orchestrator.context.defer_tests = defer_tests;
    orchestrator.context.log_llm = log_llm;

    // PSP-5: Set explicit single-file mode if --single-file flag was provided
    if single_file {
        orchestrator.context.execution_mode = perspt_core::types::ExecutionMode::Solo;
    }

    // PSP-5: Set verifier strictness from CLI flag
    orchestrator.context.verifier_strictness = match verifier_strictness.to_lowercase().as_str() {
        "strict" => perspt_core::types::VerifierStrictness::Strict,
        "minimal" => perspt_core::types::VerifierStrictness::Minimal,
        _ => perspt_core::types::VerifierStrictness::Default,
    };

    println!("🚀 SRBN Agent starting...");
    println!("   Session: {}", orchestrator.session_id());
    println!("   Task: {}", task);

    // PSP-5: Provisional plugin scan for user feedback only.
    // Authoritative detection happens inside orchestrator.run() after
    // workspace classification and potential greenfield init.
    {
        let registry = perspt_core::plugin::PluginRegistry::new();
        let detected = registry.detect_all(&working_dir);
        if detected.is_empty() {
            println!("   Plugins: none detected yet (will re-detect after init)");
        } else {
            let names: Vec<&str> = detected.iter().map(|p| p.name()).collect();
            println!("   Plugins (provisional): {}", names.join(", "));
        }
    }

    println!();

    // Check if we should run in TUI mode or headless mode
    let is_tty = std::io::stdout().is_terminal();

    if is_tty && !auto_approve {
        // Interactive mode with TUI - run orchestrator with TUI integration
        println!("Running in interactive TUI mode...");
        println!("(Use --yes flag to run headlessly)");
        println!();

        // Run with TUI integration (orchestrator starts LSP internally after classification)
        perspt_tui::run_agent_tui_with_orchestrator(orchestrator, task).await?;
    } else {
        // Headless mode - run orchestrator directly
        println!(
            "Running in headless mode (auto-approve={})...",
            auto_approve
        );
        println!();

        // Run the SRBN control loop (orchestrator starts LSP internally after classification)
        match orchestrator.run(task.clone()).await {
            Ok(()) => {
                println!();
                println!("✅ Task completed successfully!");
                println!("   Nodes processed: {}", orchestrator.node_count());

                // PSP-5: Export structured plan as JSON if --output-plan was given
                if let Some(ref plan_path) = output_plan {
                    let nodes: Vec<_> = orchestrator
                        .graph
                        .node_indices()
                        .map(|idx| &orchestrator.graph[idx])
                        .collect();
                    match serde_json::to_string_pretty(&nodes) {
                        Ok(json) => {
                            if let Err(e) = std::fs::write(plan_path, &json) {
                                eprintln!(
                                    "⚠️  Failed to write plan to {}: {}",
                                    plan_path.display(),
                                    e
                                );
                            } else {
                                println!("   Plan exported to {}", plan_path.display());
                            }
                        }
                        Err(e) => {
                            eprintln!("⚠️  Failed to serialize plan: {}", e);
                        }
                    }
                }

                // PSP-5 Phase 7: Structured headless summary
                let sid = orchestrator.session_id().to_string();
                if let Ok(store) = perspt_store::SessionStore::new() {
                    // VERIFY summary
                    if let Ok(nodes) = store.get_node_states(&sid) {
                        let completed = nodes
                            .iter()
                            .filter(|n| n.state == "COMPLETED" || n.state == "STABLE")
                            .count();
                        let failed = nodes.iter().filter(|n| n.state == "FAILED").count();
                        let retries: i32 = nodes.iter().map(|n| n.attempt_count.max(0)).sum();
                        println!();
                        println!(
                            "[VERIFY] {}/{} nodes completed, {} failed, {} retries",
                            completed,
                            nodes.len(),
                            failed,
                            retries
                        );

                        // ENERGY summary from latest node
                        if let Some(latest) = nodes.last() {
                            if let Ok(history) = store.get_energy_history(&sid, &latest.node_id) {
                                if let Some(e) = history.last() {
                                    println!("[ENERGY] V(x)={:.3} syn={:.2} str={:.2} log={:.2} boot={:.2} sheaf={:.2}",
                                        e.v_total, e.v_syn, e.v_str, e.v_log, e.v_boot, e.v_sheaf);
                                }
                            }
                        }
                    }
                    // Escalation summary
                    if let Ok(escalations) = store.get_escalation_reports(&sid) {
                        if !escalations.is_empty() {
                            println!("[ESCALATE] {} escalation(s) recorded", escalations.len());
                        }
                    }
                    // Branch summary
                    if let Ok(branches) = store.get_provisional_branches(&sid) {
                        if !branches.is_empty() {
                            let merged = branches.iter().filter(|b| b.state == "merged").count();
                            let flushed = branches.iter().filter(|b| b.state == "flushed").count();
                            println!(
                                "[BRANCH] {} total, {} merged, {} flushed",
                                branches.len(),
                                merged,
                                flushed
                            );
                        }
                    }
                    println!("[COMMIT] Session {} complete", &sid[..sid.len().min(16)]);
                }
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
