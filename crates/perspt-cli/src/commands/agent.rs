//! Agent command backed exclusively by the PSP-9 runtime.

use anyhow::{Context, Result};
use clap::Args;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// CLI arguments for `perspt agent`.
#[derive(Args, Debug, Clone)]
pub struct AgentArgs {
    /// Task description or path to task file
    pub task: String,

    /// Working directory
    #[arg(short, long)]
    pub workdir: Option<PathBuf>,

    /// Auto-approve changes without prompting
    #[arg(short, long)]
    pub yes: bool,

    /// Primary actuator model (alias for --actuator-model)
    #[arg(long)]
    pub model: Option<String>,

    /// Model that proposes governed coding tool calls
    #[arg(long)]
    pub actuator_model: Option<String>,

    /// Optional cheaper read-only repository exploration model
    #[arg(long)]
    pub explorer_model: Option<String>,

    /// Optional no-tool conjunctive diff adjudicator model
    #[arg(long)]
    pub adjudicator_model: Option<String>,

    /// Required measured energy descent per accepted checkpoint
    #[arg(long, default_value = "0.5")]
    pub rho_gate: f64,

    /// Maximum model turns per node
    #[arg(long, default_value = "12")]
    pub max_turns: u32,

    /// Maximum model-issued and nested tool calls per turn
    #[arg(long, default_value = "8")]
    pub max_calls_per_turn: u32,

    /// Shared non-descending and recovery budget
    #[arg(long, default_value = "4")]
    pub rejection_budget: u32,

    /// Maximum compiler/test/lint sensors run concurrently
    #[arg(long, default_value = "4")]
    pub max_parallel: usize,

    /// Persist signed grant intent for resume; fresh capabilities are still re-minted
    #[arg(long)]
    pub persistent_grants: bool,

    /// Domain package to run (e.g. coding, research); default: detect
    #[arg(long)]
    pub domain: Option<String>,

    /// Grant governed dependency mutation (cargo add, uv add, npm install)
    #[arg(long)]
    pub allow_dependency_mutation: bool,

    /// Allow the agent to spawn capability-attenuated child subagents
    #[arg(long)]
    pub allow_subagents: bool,

    /// Run coding processes without an OS sandbox (reduced isolation)
    #[arg(long)]
    pub allow_unisolated: bool,

    /// Concurrent work-graph nodes (default 1; above 1 requires --yes)
    #[arg(long, default_value = "1")]
    pub max_parallel_nodes: usize,

    /// Run only the read-only exploration phase: deterministic map plus
    /// an interactive explorer tool loop; nothing is mutated or promoted
    #[arg(long)]
    pub exploration_only: bool,

    /// Substitute validated [prompts] bundle sections live (Gate AE:
    /// experimental until a change record passes paired evaluation)
    #[arg(long)]
    pub allow_experimental_prompts: bool,

    /// Ordered sticky actuator fallback route; repeat to add routes
    #[arg(long = "fallback-model")]
    pub fallback_models: Vec<String>,

    /// Write the terminal session summary as JSON
    #[arg(long)]
    pub output_summary: Option<PathBuf>,

    /// Path to the PSP-9 ledger database (defaults to platform data dir)
    #[arg(long)]
    pub db_path: Option<PathBuf>,

    /// Start the web monitoring dashboard alongside the agent
    #[arg(long)]
    pub dashboard: bool,

    /// Port for the embedded dashboard server (default: 3000)
    #[arg(long, default_value = "3000")]
    pub dashboard_port: u16,
}

/// Run the governed agent across coding or domain packages.
pub async fn run(args: AgentArgs, config_override: Option<PathBuf>) -> Result<()> {
    if perspt_core::local_command::parse_local_command(&args.task).is_some() {
        println!();
        println!("{}", perspt_core::local_command::dedication_text());
        println!();
        return Ok(());
    }

    let working_dir = args.workdir.clone().unwrap_or(std::env::current_dir()?);
    let config = load_config(config_override)?;
    let prompt_overrides = load_prompt_overrides(&config, args.allow_experimental_prompts)?;
    let interactive = std::io::stdout().is_terminal();
    let run_config = build_run_config(&args, &config, interactive)?;

    let mut runtime = assemble_runtime(&working_dir, &config, &args, run_config, prompt_overrides)?;
    if args.dashboard {
        runtime = attach_dashboard_store(
            runtime,
            &working_dir,
            args.dashboard_port,
            args.db_path.as_ref(),
        )
        .await?;
    }

    println!("PSP-9 agent starting");
    println!("Task: {}", args.task);
    println!("Workspace: {}", working_dir.display());

    if args.exploration_only {
        let summary = runtime.run_exploration(args.task).await?;
        println!();
        println!("Exploration session: {}", summary.session_id);
        println!("Ledger head: {}", summary.ledger_head);
        println!("Nothing was mutated or promoted (read-only authority).");
        return Ok(());
    }

    let summary = execute_agent_run(runtime, args.task, interactive, args.yes).await?;
    report_summary(&summary, args.output_summary.as_ref())
}

fn load_config(config_override: Option<PathBuf>) -> Result<perspt_core::Config> {
    let config_path = config_override
        .or_else(perspt_core::paths::resolve_config_file)
        .or_else(perspt_core::paths::config_file);
    match config_path {
        Some(path) => perspt_core::Config::load_from_path(&path),
        None => Ok(perspt_core::Config::default()),
    }
}

fn load_prompt_overrides(
    config: &perspt_core::Config,
    allow_experimental: bool,
) -> Result<perspt_core::prompts::SectionOverrides> {
    // PSP-10 system 25: configured prompt bundles are validated before any
    // session opens; an invalid bundle refuses startup, never a silent fallback.
    let Some(prompts) = &config.prompts else {
        return Ok(perspt_core::prompts::SectionOverrides::default());
    };
    if prompts.bundles.is_empty() {
        return Ok(perspt_core::prompts::SectionOverrides::default());
    }
    super::prompts::validate_configured_bundles(&prompts.bundles)?;
    if allow_experimental {
        return super::prompts::load_bundle_overrides(&prompts.bundles);
    }
    println!(
        "prompt bundles validated; live substitution requires \
         --allow-experimental-prompts (Gate AE)"
    );
    Ok(perspt_core::prompts::SectionOverrides::default())
}

fn build_run_config(
    args: &AgentArgs,
    config: &perspt_core::Config,
    interactive: bool,
) -> Result<perspt_agent::Psp9RunConfig> {
    let approval_policy = if args.yes {
        perspt_sdk::ApprovalPolicy::Auto
    } else {
        perspt_sdk::ApprovalPolicy::Ask
    };
    let run_config = perspt_agent::Psp9RunConfig {
        max_turns: args.max_turns,
        max_calls_per_turn: args.max_calls_per_turn,
        rejection_budget: args.rejection_budget,
        rho_gate: args.rho_gate.max(0.000_001),
        approval_policy,
        max_parallel_verifiers: args.max_parallel.max(1),
        persistent_grants: args.persistent_grants,
        allow_unisolated_verifiers: args.allow_unisolated,
        allow_dependency_mutation: args.allow_dependency_mutation,
        allow_subagents: args.allow_subagents,
        max_parallel_nodes: args.max_parallel_nodes.max(1),
        ..perspt_agent::Psp9RunConfig::default()
    };
    // Multi-node dispatch is non-interactive: promotion approval cannot be
    // prompted per node while other nodes run.
    anyhow::ensure!(
        args.max_parallel_nodes <= 1 || args.yes,
        "--max-parallel-nodes above 1 requires --yes"
    );
    // Fail fast: with Ask approval and no terminal, promotion could never be
    // approved and a fully verified run would silently escalate. Exploration
    // never prompts, so it is exempt.
    anyhow::ensure!(
        interactive || args.yes || args.exploration_only,
        "non-interactive run requires --yes: promotion approval cannot be \
         prompted without a terminal"
    );

    let reduced_isolation = args.allow_unisolated
        || config
            .verification
            .as_ref()
            .and_then(|verification| verification.allow_unisolated)
            .unwrap_or(false);
    if reduced_isolation && !args.exploration_only {
        eprintln!(
            "WARNING: reduced isolation is enabled. Agent-started programs can use your \
             host-user authority."
        );
    }
    Ok(run_config)
}

fn assemble_runtime(
    working_dir: &Path,
    config: &perspt_core::Config,
    args: &AgentArgs,
    run_config: perspt_agent::Psp9RunConfig,
    prompt_overrides: perspt_core::prompts::SectionOverrides,
) -> Result<perspt_agent::Psp9AgentRuntime> {
    let mut runtime = perspt_agent::Psp9AgentRuntime::from_config(
        working_dir.to_path_buf(),
        config,
        perspt_agent::Psp9ModelRoutes {
            primary: args.model.clone(),
            actuator: args.actuator_model.clone(),
            explorer: args.explorer_model.clone(),
            adjudicator: args.adjudicator_model.clone(),
            fallbacks: args.fallback_models.clone(),
        },
        run_config,
    )?;

    let selected = select_domain(working_dir, args.domain.as_deref())?;
    println!("Domain: {}", selected.domain_id());
    runtime = runtime.with_domain(selected);
    if !prompt_overrides.is_empty() {
        runtime = runtime.with_prompt_overrides(prompt_overrides);
    }

    // Composition root: the shipped read-only tool families (system
    // explorer, local DB explorer) register through the same public path a
    // third-party family would use.
    let mut tool_handlers = perspt_agent::CandidateHandlerRegistry::with_builtins();
    perspt_agent::tools::families::register_standard_families(&mut tool_handlers)?;
    runtime = runtime
        .with_tool_family(perspt_agent::tools::families::standard_family_entries())
        .with_tool_handlers(tool_handlers);

    if let Some(path) = args.db_path.as_ref() {
        runtime = runtime.with_database_path(path.clone());
    }
    Ok(runtime)
}

fn select_domain(
    working_dir: &Path,
    explicit_domain: Option<&str>,
) -> Result<Arc<dyn perspt_sdk::AgentDomainPackage>> {
    // Composition root: domains come from the open registry — an explicit
    // --domain id wins, otherwise the best detection. The coding domain is
    // the fallback when nothing activates (an empty scratch directory).
    let mut domains = perspt_sdk::DomainRegistry::new();
    domains.register(Arc::new(perspt_coding::CodingDomain::new()));
    domains.register(Arc::new(perspt_research::ResearchDomain::new()));
    let snapshot = workspace_snapshot(working_dir);
    let explicit = explicit_domain.map(perspt_sdk::DomainId::new);
    match (&explicit, domains.select(explicit.as_ref(), &snapshot)) {
        (_, Some(package)) => Ok(package),
        (Some(id), None) => anyhow::bail!(
            "unknown domain {id:?}; registered: {:?}",
            domains.domain_ids()
        ),
        (None, None) => domains
            .by_id(&perspt_sdk::DomainId::new("coding"))
            .context("coding domain must be registered"),
    }
}

async fn attach_dashboard_store(
    runtime: perspt_agent::Psp9AgentRuntime,
    working_dir: &Path,
    port: u16,
    db_path: Option<&PathBuf>,
) -> Result<perspt_agent::Psp9AgentRuntime> {
    // One live handle: the dashboard reads through the same connection
    // the runtime writes, so no second DuckDB handle contends on the
    // database file.
    let store = Arc::new(match db_path {
        Some(path) => perspt_store::SessionStore::open(path).with_context(|| {
            format!(
                "opening session database {}; if it is corrupted (for \
                 example a poisoned WAL), run `perspt db repair {}`",
                path.display(),
                path.display()
            )
        })?,
        None => perspt_store::SessionStore::new()?,
    });
    start_dashboard(working_dir, port, store.clone()).await?;
    Ok(runtime.with_session_store(store))
}

async fn execute_agent_run(
    runtime: perspt_agent::Psp9AgentRuntime,
    task: String,
    interactive: bool,
    auto_approve: bool,
) -> Result<perspt_agent::Psp9RunSummary> {
    if interactive && !auto_approve {
        return perspt_tui::run_agent_tui_with_runtime(runtime, task).await;
    }
    // Headless runs still narrate: every ledger narration line prints as
    // a heartbeat instead of minutes of silence between model turns.
    let (event_sender, mut event_receiver) =
        tokio::sync::mpsc::unbounded_channel::<perspt_core::AgentEvent>();
    let (_action_sender, action_receiver) =
        tokio::sync::mpsc::unbounded_channel::<perspt_core::AgentAction>();
    let printer = tokio::spawn(async move {
        while let Some(event) = event_receiver.recv().await {
            match event {
                perspt_core::AgentEvent::Log(line) => println!("  {line}"),
                perspt_core::AgentEvent::Error(line) => eprintln!("  error: {line}"),
                perspt_core::AgentEvent::TaskStatusChanged { node_id, status } => {
                    println!("  [{node_id}] {status:?}");
                }
                _ => {}
            }
        }
    });
    let summary = runtime
        .connect_tui(event_sender, action_receiver)
        .run(task)
        .await?;
    printer.abort();
    Ok(summary)
}

fn report_summary(
    summary: &perspt_agent::Psp9RunSummary,
    output_summary: Option<&PathBuf>,
) -> Result<()> {
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
        std::fs::write(path, output)
            .with_context(|| format!("writing run summary to {}", path.display()))?;
    }
    // Escalation and incomplete work are a nonzero process exit: CI and
    // scripts must not read a stopped node as success.
    if !matches!(summary.outcome, perspt_sdk::NodeTerminalOutcome::HardPass) {
        std::process::exit(2);
    }
    Ok(())
}

/// A bounded, read-only file listing for domain detection: top-level
/// entries plus one directory level is enough for every marker file the
/// registered domains look for.
fn workspace_snapshot(working_dir: &Path) -> perspt_sdk::WorkspaceSnapshot {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(working_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.path().is_dir() {
                if let Ok(nested) = std::fs::read_dir(entry.path()) {
                    for child in nested.flatten() {
                        files.push(format!("{name}/{}", child.file_name().to_string_lossy()));
                    }
                }
            } else {
                files.push(name);
            }
        }
    }
    files.sort();
    perspt_sdk::WorkspaceSnapshot::new(working_dir.display().to_string(), files)
}

async fn start_dashboard(
    working_dir: &Path,
    port: u16,
    store: Arc<perspt_store::SessionStore>,
) -> Result<()> {
    let state = perspt_dashboard::state::AppState {
        store,
        password: None,
        session_token: Arc::new(tokio::sync::Mutex::new(None)),
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
