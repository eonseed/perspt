//! TUI module - Primary entry point for Perspt TUI
//!
//! Provides a unified interface for both Chat and Agent modes.

use crate::agent_app::AgentApp;
use crate::chat_app::ChatApp;
use anyhow::Result;
use perspt_core::GenAIProvider;

/// Application mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    /// Interactive chat with LLM
    Chat,
    /// SRBN Agent orchestration
    Agent,
}

/// Run the TUI in chat mode
///
/// # Arguments
/// * `provider` - The GenAI provider for LLM communication
/// * `model` - The model identifier to use
///
/// # Example
/// ```no_run
/// use perspt_tui::run_chat_tui;
/// use perspt_core::GenAIProvider;
///
/// #[tokio::main]
/// async fn main() {
///     let provider = GenAIProvider::new().unwrap();
///     run_chat_tui(provider, "gemini-2.0-flash".to_string()).await.unwrap();
/// }
/// ```
pub async fn run_chat_tui(provider: GenAIProvider, model: String) -> Result<()> {
    use crossterm::event::{
        DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    };
    use ratatui::crossterm::execute;
    use std::io::stdout;

    // Enable mouse capture for scroll wheel support
    execute!(stdout(), EnableMouseCapture)?;

    // Enable bracketed paste for multi-line paste handling
    execute!(stdout(), EnableBracketedPaste)?;

    // Enable keyboard enhancement for better modifier detection
    // This allows reliable Ctrl+Enter, Shift+Tab detection
    let _ = execute!(
        stdout(),
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
        )
    );

    let mut terminal = ratatui::init();
    let mut app = ChatApp::new(provider, model);

    let result = app.run(&mut terminal).await;

    // Restore terminal
    let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
    ratatui::restore();
    execute!(stdout(), DisableMouseCapture)?;

    result
}

/// Run the TUI in agent mode (legacy wrapper)
///
/// This function provides backward compatibility for the agent TUI.
/// It uses demo data as the orchestrator integration is pending.
pub fn run_agent_tui() -> std::io::Result<()> {
    let mut terminal = ratatui::init();
    let mut app = AgentApp::new();

    // Demo data for now
    use crate::task_tree::TaskStatus;
    app.task_tree.add_task(
        "root".to_string(),
        "Implement authentication".to_string(),
        0,
    );
    app.task_tree
        .add_task("auth-1".to_string(), "Create JWT module".to_string(), 1);
    app.task_tree
        .add_task("auth-2".to_string(), "Add password hashing".to_string(), 1);
    app.task_tree.update_status("root", TaskStatus::Running);
    app.task_tree.update_status("auth-1", TaskStatus::Completed);

    app.dashboard.total_nodes = 3;
    app.dashboard.completed_nodes = 1;
    app.dashboard.current_node = Some("auth-2".to_string());
    app.dashboard.update_energy(0.5);
    app.dashboard
        .log("Started task: Implement authentication".to_string());
    app.dashboard.log("OK: JWT module completed".to_string());

    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}

/// Legacy placeholder function - redirects to agent TUI
#[deprecated(note = "Use run_chat_tui or run_agent_tui instead")]
pub fn run_tui() -> Result<()> {
    run_agent_tui().map_err(|e| anyhow::anyhow!("TUI error: {}", e))
}
