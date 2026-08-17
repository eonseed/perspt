//! TUI module - Primary entry point for Perspt TUI
//!
//! Provides a unified interface for both Chat and Agent modes.

use crate::chat_app::ChatApp;
use anyhow::Result;
use perspt_core::GenAIProvider;

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
