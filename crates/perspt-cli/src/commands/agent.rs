//! Agent command backed exclusively by the PSP-9 runtime.

use anyhow::{Context, Result};
use std::io::IsTerminal;
use std::path::PathBuf;

/// Run the governed coding agent.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    task: String,
    workdir: Option<PathBuf>,
    auto_approve: bool,
    model: Option<String>,
    actuator_model: Option<String>,
    explorer_model: Option<String>,
    adjudicator_model: Option<String>,
    fallback_models: Vec<String>,
    output_summary: Option<PathBuf>,
    db_path: Option<PathBuf>,
    rho_gate: f64,
    max_turns: u32,
    max_calls_per_turn: u32,
    rejection_budget: u32,
    max_parallel: usize,
    persistent_grants: bool,
    exploration_only: bool,
    dashboard: bool,
    dashboard_port: u16,
    config_override: Option<PathBuf>,
) -> Result<()> {
    if perspt_core::local_command::parse_local_command(&task).is_some() {
        println!();
        println!("{}", perspt_core::local_command::dedication_text());
        println!();
        return Ok(());
    }

    let working_dir = workdir.unwrap_or(std::env::current_dir()?);
    let config_path = config_override
        .or_else(perspt_core::paths::resolve_config_file)
        .or_else(perspt_core::paths::config_file);
    let config = match config_path {
        Some(path) => perspt_core::Config::load_from_path(&path)?,
        None => perspt_core::Config::default(),
    };
    let approval_policy = if auto_approve {
        perspt_sdk::ApprovalPolicy::Auto
    } else {
        perspt_sdk::ApprovalPolicy::Ask
    };
    let run_config = perspt_agent::Psp9RunConfig {
        max_turns,
        max_calls_per_turn,
        rejection_budget,
        rho_gate: rho_gate.max(0.000_001),
        approval_policy,
        max_parallel_verifiers: max_parallel.max(1),
        persistent_grants,
        ..perspt_agent::Psp9RunConfig::default()
    };
    let interactive = std::io::stdout().is_terminal();
    // Fail fast: with Ask approval and no terminal, promotion could never be
    // approved and a fully verified run would silently escalate. Exploration
    // never prompts, so it is exempt.
    anyhow::ensure!(
        interactive || auto_approve || exploration_only,
        "non-interactive run requires --yes: promotion approval cannot be \
         prompted without a terminal"
    );

    let mut runtime = perspt_agent::Psp9AgentRuntime::from_config(
        working_dir.clone(),
        &config,
        perspt_agent::Psp9ModelRoutes {
            primary: model,
            actuator: actuator_model,
            explorer: explorer_model,
            adjudicator: adjudicator_model,
            fallbacks: fallback_models,
        },
        run_config,
    )?;

    if let Some(path) = db_path.as_ref() {
        runtime = runtime.with_database_path(path.clone());
    }

    if dashboard {
        // One live handle: the dashboard reads through the same connection
        // the runtime writes, so no second DuckDB handle contends on the
        // database file.
        let store = std::sync::Arc::new(match db_path.as_ref() {
            Some(path) => perspt_store::SessionStore::open(path)?,
            None => perspt_store::SessionStore::new()?,
        });
        start_dashboard(&working_dir, dashboard_port, store.clone()).await?;
        runtime = runtime.with_session_store(store);
    }

    println!("PSP-9 agent starting");
    println!("Task: {task}");
    println!("Workspace: {}", working_dir.display());

    if exploration_only {
        let summary = runtime.run_exploration(task).await?;
        println!();
        println!("Exploration session: {}", summary.session_id);
        println!("Ledger head: {}", summary.ledger_head);
        println!("Nothing was mutated or promoted (read-only authority).");
        return Ok(());
    }

    let summary = if interactive && !auto_approve {
        perspt_tui::run_agent_tui_with_runtime(runtime, task.clone()).await?
    } else {
        runtime.run(task.clone()).await?
    };

    println!();
    println!("Outcome: {:?}", summary.outcome);
    println!("Session: {}", summary.session_id);
    println!("Turns: {}", summary.turns_used);
    println!("Ledger head: {}", summary.ledger_head);
    if !summary.promoted_paths.is_empty() {
        println!("Promoted paths: {}", summary.promoted_paths.join(", "));
    }

    if let Some(path) = output_summary {
        let output = serde_json::to_string_pretty(&serde_json::json!({
            "session_id": summary.session_id,
            "node_id": summary.node_id,
            "outcome": summary.outcome,
            "turns_used": summary.turns_used,
            "ledger_head": summary.ledger_head,
            "promoted_paths": summary.promoted_paths,
        }))?;
        std::fs::write(&path, output)
            .with_context(|| format!("writing run summary to {}", path.display()))?;
    }
    Ok(())
}

async fn start_dashboard(
    working_dir: &std::path::Path,
    port: u16,
    store: std::sync::Arc<perspt_store::SessionStore>,
) -> Result<()> {
    let state = perspt_dashboard::state::AppState {
        store,
        password: None,
        session_token: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        working_dir: working_dir.to_path_buf(),
        is_localhost: true,
    };
    let app = perspt_dashboard::build_router(state);
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .with_context(|| format!("binding dashboard to {address}"))?;
    println!("Dashboard: http://{address}");
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            log::error!("dashboard server failed: {error}");
        }
    });
    Ok(())
}
