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

        /// Primary actuator model (alias for --actuator-model)
        #[arg(long)]
        model: Option<String>,

        /// Model that proposes governed coding tool calls
        #[arg(long)]
        actuator_model: Option<String>,

        /// Optional cheaper read-only repository exploration model
        #[arg(long)]
        explorer_model: Option<String>,

        /// Optional no-tool conjunctive diff adjudicator model
        #[arg(long)]
        adjudicator_model: Option<String>,

        /// Required measured energy descent per accepted checkpoint
        #[arg(long, default_value = "0.5")]
        rho_gate: f64,

        /// Maximum model turns per node
        #[arg(long, default_value = "12")]
        max_turns: u32,

        /// Maximum model-issued and nested tool calls per turn
        #[arg(long, default_value = "8")]
        max_calls_per_turn: u32,

        /// Shared non-descending and recovery budget
        #[arg(long, default_value = "4")]
        rejection_budget: u32,

        /// Maximum compiler/test/lint sensors run concurrently
        #[arg(long, default_value = "4")]
        max_parallel: usize,

        /// Persist signed grant intent for resume; fresh capabilities are still re-minted
        #[arg(long)]
        persistent_grants: bool,

        /// Domain package to run (e.g. coding, research); default: detect
        #[arg(long)]
        domain: Option<String>,

        /// Grant governed dependency mutation (cargo add, uv add, npm install)
        #[arg(long)]
        allow_dependency_mutation: bool,

        /// Concurrent work-graph nodes (default 1; above 1 requires --yes)
        #[arg(long, default_value = "1")]
        max_parallel_nodes: usize,

        /// Run only the read-only exploration phase: deterministic map plus
        /// an interactive explorer tool loop; nothing is mutated or promoted
        #[arg(long)]
        exploration_only: bool,

        /// Ordered sticky actuator fallback route; repeat to add routes
        #[arg(long = "fallback-model")]
        fallback_models: Vec<String>,

        /// Write the terminal session summary as JSON
        #[arg(long)]
        output_summary: Option<PathBuf>,

        /// Path to the PSP-9 ledger database (defaults to platform data dir)
        #[arg(long)]
        db_path: Option<PathBuf>,

        /// Start the web monitoring dashboard alongside the agent
        #[arg(long)]
        dashboard: bool,

        /// Port for the embedded dashboard server (default: 3000)
        #[arg(long, default_value = "3000")]
        dashboard_port: u16,
    },

    /// Initialize project memory and policy rules
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

        /// Roll back a session's newest completed promotion (session id prefix)
        #[arg(long)]
        rollback: Option<String>,

        /// Show ledger statistics
        #[arg(long)]
        stats: bool,
    },

    /// Show current agent status
    Status,

    /// Delayed audit labels and conformal activation (PSP-9)
    Audit {
        /// Sample id (or unique prefix) to label; omit to list pending samples
        sample: Option<String>,

        /// Label the sample as safe
        #[arg(long)]
        safe: bool,

        /// Label the sample as unsafe
        #[arg(long = "unsafe")]
        mark_unsafe: bool,
    },

    /// Print the provider capability matrix (PSP-9)
    Providers {
        /// Run live behavioral probes against every configured model route
        #[arg(long)]
        probe: bool,
    },

    /// Deterministic, credential-free audit replay of a session (PSP-9)
    Replay {
        /// The session id to replay
        session_id: String,

        /// Database file to inspect (defaults to the standard store)
        #[arg(long)]
        db_path: Option<std::path::PathBuf>,
    },

    /// Abort a PSP-9 session by revoking its authority epoch
    Abort {
        /// Force abort without confirmation
        #[arg(short, long)]
        force: bool,

        /// Session to abort (defaults to the newest running PSP-9 session)
        session_id: Option<String>,
    },

    /// Resume a paused or crashed session
    Resume {
        /// Session ID to resume
        session_id: Option<String>,

        /// Resume the most recent session
        #[arg(long)]
        last: bool,

        /// Database file to inspect (defaults to the standard store)
        #[arg(long)]
        db_path: Option<std::path::PathBuf>,
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

    /// Inspect and repair the local DuckDB store
    Db {
        #[command(subcommand)]
        command: DbCommands,
    },
}

#[derive(Subcommand)]
enum DbCommands {
    /// Quarantine a poisoned WAL after making durable backups
    Repair {
        /// Database file to repair
        #[arg(long)]
        db_path: PathBuf,

        /// Explicitly authorize WAL quarantine; the WAL is never deleted
        #[arg(long)]
        discard_wal: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Install a process-level rustls CryptoProvider before any TLS use. Both
    // `ring` and `aws-lc-rs` are present in the dependency tree (gcp_auth pulls
    // ring for Vertex ADC), so rustls 0.23 cannot auto-select one and would
    // panic on first HTTPS request. Pin `ring` explicitly.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let cli = Cli::parse();
    let config_override = cli.config.clone();

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
        None => commands::chat::run(None, config_override).await,
        Some(Commands::Chat { model }) => commands::chat::run(model, config_override).await,
        Some(Commands::Agent {
            task,
            workdir,
            yes,
            model,
            actuator_model,
            explorer_model,
            adjudicator_model,
            rho_gate,
            max_turns,
            max_calls_per_turn,
            rejection_budget,
            max_parallel,
            persistent_grants,
            domain,
            allow_dependency_mutation,
            max_parallel_nodes,
            exploration_only,
            fallback_models,
            output_summary,
            db_path,
            dashboard,
            dashboard_port,
        }) => {
            commands::agent::run(
                task,
                workdir,
                yes,
                model,
                actuator_model,
                explorer_model,
                adjudicator_model,
                fallback_models,
                output_summary,
                db_path,
                rho_gate,
                max_turns,
                max_calls_per_turn,
                rejection_budget,
                max_parallel,
                persistent_grants,
                domain,
                allow_dependency_mutation,
                max_parallel_nodes,
                exploration_only,
                dashboard,
                dashboard_port,
                config_override,
            )
            .await
        }
        Some(Commands::Init { memory, rules }) => commands::init::run(memory, rules).await,
        Some(Commands::Config { show, set, edit }) => {
            commands::config::run(show, set, edit, config_override).await
        }
        Some(Commands::Ledger {
            recent,
            rollback,
            stats,
        }) => commands::ledger::run(recent, rollback, stats).await,
        Some(Commands::Status) => commands::status::run().await,
        Some(Commands::Audit {
            sample,
            safe,
            mark_unsafe,
        }) => commands::audit::run(sample, safe, mark_unsafe).await,
        Some(Commands::Providers { probe }) => {
            commands::providers::run(config_override, probe).await
        }
        Some(Commands::Replay {
            session_id,
            db_path,
        }) => commands::replay::run(session_id, db_path).await,
        Some(Commands::Abort { force, session_id }) => {
            commands::abort::run(force, session_id).await
        }
        Some(Commands::Resume {
            session_id,
            last,
            db_path,
        }) => commands::resume::run(session_id, last, db_path).await,
        Some(Commands::SimpleChat { model, log_file }) => {
            commands::simple_chat::run(commands::simple_chat::SimpleChatArgs {
                model,
                log_file,
                config_override,
            })
            .await
        }
        Some(Commands::Dashboard { port, db_path }) => {
            commands::dashboard::run(port, db_path).await
        }
        Some(Commands::Db { command }) => match command {
            DbCommands::Repair {
                db_path,
                discard_wal,
            } => commands::db::repair(db_path, discard_wal).await,
        },
    }
}
