//! Chat command - Interactive TUI chat mode
//!
//! Provides a rich terminal interface for direct LLM conversation
//! with streaming responses, message history, and syntax highlighting.

use anyhow::{Context, Result};
use perspt_core::{Config, GenAIProvider};
use std::path::PathBuf;

/// Run the chat TUI.
///
/// `model` is the optional `--model` CLI override; `config_override` is the
/// optional `--config <PATH>` value.
pub async fn run(model: Option<String>, config_override: Option<PathBuf>) -> Result<()> {
    log::info!("Starting chat mode...");

    let config_path = config_override
        .or_else(perspt_core::paths::resolve_config_file)
        .or_else(perspt_core::paths::config_file);
    let config = match config_path {
        Some(ref path) => Config::load_from_path(path)?,
        None => Config::default(),
    };

    // Build a bound provider from config + env, with CLI --model taking precedence.
    let (provider, resolved) = GenAIProvider::from_config(&config, model.as_deref())
        .context("Failed to create LLM provider. Ensure an API key or config is set.")?;

    log::info!(
        "Using provider: {}, model: {}",
        resolved.provider,
        resolved.model
    );

    // Run the streaming chat TUI
    perspt_tui::run_chat_tui(provider, resolved.model).await?;

    Ok(())
}
