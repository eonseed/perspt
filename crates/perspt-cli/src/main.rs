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
        /// Session ID to resume
        session_id: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
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
            energy_weights: _,
            stability_threshold: _,
            max_cost: _,
            max_steps: _,
        }) => commands::agent::run(task, workdir, yes, complexity, mode).await,
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
    }
}
