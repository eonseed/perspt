//! Chat command - Interactive TUI chat mode
//!
//! Provides a rich terminal interface for direct LLM conversation
//! with streaming responses, message history, and syntax highlighting.

use anyhow::{Context, Result};
use perspt_core::GenAIProvider;

/// Run the chat TUI
pub async fn run() -> Result<()> {
    log::info!("Starting chat mode...");

    // Auto-detect provider from environment
    let (provider_type, default_model) = detect_provider_from_env();

    log::info!(
        "Using provider: {}, model: {}",
        provider_type,
        default_model
    );

    // Create provider
    let provider = GenAIProvider::new_with_config(Some(provider_type), None)
        .context("Failed to create LLM provider. Ensure an API key is set.")?;

    // Run the new streaming chat TUI
    perspt_tui::run_chat_tui(provider, default_model.to_string()).await?;

    Ok(())
}

/// Detect provider and default model from environment variables
fn detect_provider_from_env() -> (&'static str, &'static str) {
    if std::env::var("GEMINI_API_KEY").is_ok() {
        ("gemini", "gemini-flash-lite-latest")
    } else if std::env::var("OPENAI_API_KEY").is_ok() {
        ("openai", "gpt-4o-mini")
    } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        ("anthropic", "claude-3-5-sonnet-20241022")
    } else if std::env::var("GROQ_API_KEY").is_ok() {
        ("groq", "llama-3.1-8b-instant")
    } else if std::env::var("COHERE_API_KEY").is_ok() {
        ("cohere", "command-r-plus")
    } else if std::env::var("XAI_API_KEY").is_ok() {
        ("xai", "grok-beta")
    } else if std::env::var("DEEPSEEK_API_KEY").is_ok() {
        ("deepseek", "deepseek-chat")
    } else {
        // Default to Ollama for local usage
        ("ollama", "llama3.2")
    }
}
