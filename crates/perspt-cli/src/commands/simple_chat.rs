//! Simple Chat Command - Unix-style CLI chat mode
//!
//! Provides a simple command-line interface for direct Q&A interaction
//! without the TUI overlay. Designed for scripting, piping, and accessibility.

use anyhow::{Context, Result};
use perspt_core::{GenAIProvider, EOT_SIGNAL};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

/// Arguments for the simple-chat command
pub struct SimpleChatArgs {
    pub model: Option<String>,
    pub log_file: Option<PathBuf>,
}

/// Run the simple CLI chat mode
pub async fn run(args: SimpleChatArgs) -> Result<()> {
    // Auto-detect provider from environment
    let (provider_type, default_model) = detect_provider_from_env();

    let model_name = args.model.unwrap_or_else(|| default_model.to_string());

    // Create provider
    let provider = Arc::new(
        GenAIProvider::new_with_config(Some(&provider_type), None)
            .context("Failed to create LLM provider. Ensure an API key is set.")?,
    );

    // Open log file if specified
    let mut log_handle = if let Some(ref path) = args.log_file {
        Some(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .with_context(|| format!("Failed to open log file: {}", path.display()))?,
        )
    } else {
        None
    };

    let stdin = io::stdin();
    let mut stdin_reader = BufReader::new(stdin);
    let mut user_input = String::new();

    // Easter egg state
    let mut easter_egg_triggered = false;

    // Welcome message
    println!("Perspt Simple Chat Mode");
    println!("Provider: {} | Model: {}", provider_type, model_name);
    if let Some(ref log_path) = args.log_file {
        println!("Logging to: {}", log_path.display());
    }
    println!("Type 'exit' or press Ctrl+D to quit.");
    println!();

    loop {
        // Display prompt
        print!("> ");
        std::io::stdout().flush()?;
        user_input.clear();

        // Read user input
        let bytes_read = stdin_reader
            .read_line(&mut user_input)
            .await
            .context("Failed to read from stdin")?;

        if bytes_read == 0 {
            // EOF (Ctrl+D)
            println!();
            break;
        }

        let trimmed_input = user_input.trim();

        // Skip empty input
        if trimmed_input.is_empty() {
            continue;
        }

        // Echo input if not running interactively (piped mode)
        if !atty::is(atty::Stream::Stdin) {
            println!("{}", trimmed_input);
        }

        // Check for exit command
        if trimmed_input.eq_ignore_ascii_case("exit") {
            break;
        }

        // Check for Easter egg
        if check_easter_egg(trimmed_input, &mut easter_egg_triggered) {
            display_easter_egg();
            continue;
        }

        // Log user input
        if let Some(ref mut file) = log_handle {
            writeln!(file, "> {}", trimmed_input).context("Failed to write to log file")?;
        }

        // Create channel for streaming response
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Clone for async task
        let provider_clone = Arc::clone(&provider);
        let model_clone = model_name.clone();
        let input_clone = trimmed_input.to_string();

        // Spawn async task for LLM request
        let request_handle = tokio::spawn(async move {
            provider_clone
                .generate_response_stream_to_channel(&model_clone, &input_clone, tx)
                .await
        });

        // Process streaming response
        let mut full_response = String::new();
        let mut response_started = false;

        while let Some(chunk) = rx.recv().await {
            if chunk == EOT_SIGNAL {
                break;
            }

            // Print chunk immediately for real-time streaming
            print!("{}", chunk);
            std::io::stdout().flush()?;
            full_response.push_str(&chunk);
            response_started = true;
        }

        // Handle request completion
        match request_handle.await {
            Ok(Ok(())) => {
                if response_started {
                    println!(); // Newline after response
                }
            }
            Ok(Err(e)) => {
                if !response_started {
                    println!("Error: {}", e);
                } else {
                    println!("\nError during response: {}", e);
                }
            }
            Err(e) => {
                println!("Request failed: {}", e);
            }
        }

        // Log response
        if let Some(ref mut file) = log_handle {
            if !full_response.is_empty() {
                writeln!(file, "{}", full_response).context("Failed to write to log file")?;
            }
            writeln!(file).context("Failed to write to log file")?;
        }
    }

    println!("Goodbye!");
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

/// Check for the Easter egg sequence
fn check_easter_egg(input: &str, triggered: &mut bool) -> bool {
    if *triggered {
        return false;
    }

    if input.eq_ignore_ascii_case("l-o-v-e") {
        *triggered = true;
        return true;
    }

    false
}

/// Display the Easter egg dedication message
fn display_easter_egg() {
    println!();
    println!("\x1b[35;1mSpecial Dedication\x1b[0m");
    println!();
    println!("\x1b[36mThis application is lovingly dedicated to\x1b[0m");
    println!("   \x1b[36;3mmy wonderful mother and grandma\x1b[0m");
    println!();
    println!("\x1b[32mThank you for your endless love, wisdom, and support\x1b[0m");
    println!();
    println!("\x1b[35;3mWith all my love and gratitude\x1b[0m");
    println!();
}
