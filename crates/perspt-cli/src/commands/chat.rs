//! Chat command - legacy interactive chat mode

use anyhow::Result;

/// Run the chat TUI (legacy mode)
pub async fn run() -> Result<()> {
    log::info!("Starting chat mode...");

    // Use the legacy chat TUI
    perspt_tui::run_tui()?;

    Ok(())
}
