//! perspt-cli: CLI entry point for Perspt
//!
//! Provides subcommands for chat mode, agent mode, configuration, and ledger management.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod commands;

/// Perspt - AI-powered coding assistant with stability guarantees
#[derive(Parser)]
#[command(name = "perspt")]
#[command(author = "Vikrant Rathore, Ronak Rathore")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "AI-powered coding assistant with SRBN stability guarantees", long_about = None)]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Configuration file path
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Commands {
    /// Start an interactive chat session (default)
    Chat {
        /// Model to use for chat
        #[arg(short, long)]
        model: Option<String>,
    },

    /// Run the SRBN agent on a task
    Agent {
        /// Task description or path to task file
        task: String,

        /// Working directory
        #[arg(short, long)]
        workdir: Option<PathBuf>,

        /// Auto-approve changes without prompting
        #[arg(short, long)]
        yes: bool,

        /// Auto-approve only safe (read-only) operations
        #[arg(long)]
        auto_approve_safe: bool,

        /// Maximum complexity K for sub-graph approval
        #[arg(short = 'k', long, default_value = "5")]
        complexity: usize,

        /// Execution mode: cautious, balanced, or yolo
        #[arg(short, long, default_value = "balanced")]
        mode: String,

        /// Model to use for ALL agent tiers (overrides per-tier settings)
        #[arg(long)]
        model: Option<String>,

        /// Model for Architect tier (deep reasoning/planning)
        #[arg(long)]
        architect_model: Option<String>,

        /// Model for Actuator tier (code generation)
        #[arg(long)]
        actuator_model: Option<String>,

        /// Model for Verifier tier (stability checking)
        #[arg(long)]
        verifier_model: Option<String>,

        /// Model for Speculator tier (fast lookahead/exploration)
        #[arg(long)]
        speculator_model: Option<String>,

        /// Energy weights α,β,γ (comma-separated, e.g., "1.0,0.5,2.0")
        #[arg(long, default_value = "1.0,0.5,2.0")]
        energy_weights: String,

        /// Stability threshold ε (default: 0.1)
        #[arg(long, default_value = "0.1")]
        stability_threshold: f32,

        /// Maximum cost in USD (0 = unlimited)
        #[arg(long, default_value = "0")]
        max_cost: f32,

        /// Maximum steps/iterations (0 = unlimited)
        #[arg(long, default_value = "0")]
        max_steps: usize,

        /// Defer tests until sheaf validation (faster iteration, tests run at end)
        #[arg(long)]
        defer_tests: bool,

        /// Log all LLM requests/responses to database for debugging
        #[arg(long)]
        log_llm: bool,

        /// Force single-file execution (Solo Mode) instead of project-first planning
        #[arg(long)]
        single_file: bool,

        /// Verifier strictness: default, strict, or minimal
        #[arg(long, default_value = "default")]
        verifier_strictness: String,

        /// Fallback model for Architect tier (used when primary fails structured-output)
        #[arg(long)]
        architect_fallback_model: Option<String>,

        /// Fallback model for Actuator tier (used when primary fails structured-output)
        #[arg(long)]
        actuator_fallback_model: Option<String>,

        /// Fallback model for Verifier tier (used when primary fails structured-output)
        #[arg(long)]
        verifier_fallback_model: Option<String>,

        /// Fallback model for Speculator tier (used when primary fails structured-output)
        #[arg(long)]
        speculator_fallback_model: Option<String>,

        /// Export the task graph as JSON to a file after planning (before execution)
        #[arg(long)]
        output_plan: Option<PathBuf>,
    },

    /// Initialize project configuration
    Init {
        /// Create PERSPT.md project memory file
        #[arg(long)]
        memory: bool,

        /// Create default Starlark policy rules
        #[arg(long)]
        rules: bool,
    },

    /// Manage configuration
    Config {
        /// Show current configuration
        #[arg(long)]
        show: bool,

        /// Set a configuration value (key=value)
        #[arg(long)]
        set: Option<String>,

        /// Edit configuration in $EDITOR
        #[arg(long)]
        edit: bool,
    },

    /// Query and manage the Merkle ledger
    Ledger {
        /// Show recent commits
        #[arg(long)]
        recent: bool,

        /// Rollback to a specific commit hash
        #[arg(long)]
        rollback: Option<String>,

        /// Show ledger statistics
        #[arg(long)]
        stats: bool,
    },

    /// Show current agent status
    Status,

    /// Abort the current agent session
    Abort {
        /// Force abort without confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Resume a paused or crashed session
    Resume {
        /// Session ID to resume (or --last for most recent)
        session_id: Option<String>,
    },

    /// View LLM request/response logs
    Logs {
        /// Session ID to view logs for
        session_id: Option<String>,

        /// Show logs from the most recent session
        #[arg(long)]
        last: bool,

        /// Show usage statistics instead of individual requests
        #[arg(long)]
        stats: bool,

        /// Launch interactive TUI logs viewer
        #[arg(long)]
        tui: bool,
    },

    /// Simple CLI chat mode (no TUI)
    SimpleChat {
        /// Model to use for chat
        #[arg(short, long)]
        model: Option<String>,

        /// Log session to file
        #[arg(long)]
        log_file: Option<std::path::PathBuf>,
    },

    /// Launch the web monitoring dashboard
    Dashboard {
        /// Port to listen on
        #[arg(short, long, default_value = "3000")]
        port: u16,

        /// Path to the database file (defaults to platform data dir)
        #[arg(long)]
        db_path: Option<std::path::PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    // Suppress logs for TUI modes (Chat, Agent) to prevent bleeding into terminal
    let log_level = if cli.verbose {
        "debug"
    } else if matches!(
        cli.command,
        None | Some(Commands::Chat { .. }) | Some(Commands::Agent { .. })
    ) {
        // Chat and Agent modes use TUI - suppress all logs
        "off"
    } else if matches!(cli.command, Some(Commands::SimpleChat { .. })) {
        // Simple chat only shows errors
        "error"
    } else {
        "info"
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    match cli.command {
        None | Some(Commands::Chat { model: _ }) => commands::chat::run().await,
        Some(Commands::Agent {
            task,
            workdir,
            yes,
            auto_approve_safe: _,
            complexity,
            mode,
            model,
            architect_model,
            actuator_model,
            verifier_model,
            speculator_model,
            energy_weights,
            stability_threshold,
            max_cost,
            max_steps,
            defer_tests,
            log_llm,
            single_file,
            verifier_strictness,
            architect_fallback_model,
            actuator_fallback_model,
            verifier_fallback_model,
            speculator_fallback_model,
            output_plan,
        }) => {
            commands::agent::run(
                task,
                workdir,
                yes,
                complexity,
                mode,
                model,
                architect_model,
                actuator_model,
                verifier_model,
                speculator_model,
                defer_tests,
                log_llm,
                single_file,
                verifier_strictness,
                architect_fallback_model,
                actuator_fallback_model,
                verifier_fallback_model,
                speculator_fallback_model,
                output_plan,
                energy_weights,
                stability_threshold,
                max_cost,
                max_steps,
            )
            .await
        }
        Some(Commands::Init { memory, rules }) => commands::init::run(memory, rules).await,
        Some(Commands::Config { show, set, edit }) => commands::config::run(show, set, edit).await,
        Some(Commands::Ledger {
            recent,
            rollback,
            stats,
        }) => commands::ledger::run(recent, rollback, stats).await,
        Some(Commands::Status) => commands::status::run().await,
        Some(Commands::Abort { force }) => commands::abort::run(force).await,
        Some(Commands::Resume { session_id }) => commands::resume::run(session_id).await,
        Some(Commands::Logs {
            session_id,
            last,
            stats,
            tui,
        }) => commands::logs::run(session_id, last, stats, tui).await,
        Some(Commands::SimpleChat { model, log_file }) => {
            commands::simple_chat::run(commands::simple_chat::SimpleChatArgs { model, log_file })
                .await
        }
        Some(Commands::Dashboard { port, db_path }) => {
            commands::dashboard::run(port, db_path).await
        }
    }
}
