//! Simple Chat Command - Unix-style CLI chat mode
//!
//! Provides a simple command-line interface for direct Q&A interaction
//! without the TUI overlay. Designed for scripting, piping, and accessibility.

use anyhow::{Context, Result};
use perspt_core::{Config, GenAIProvider, EOT_SIGNAL};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

/// Arguments for the simple-chat command
pub struct SimpleChatArgs {
    pub model: Option<String>,
    pub log_file: Option<PathBuf>,
    pub config_override: Option<PathBuf>,
}

/// Run the simple CLI chat mode
#[derive(Debug, Clone)]
struct SimpleChatMessage {
    role: String,
    content: String,
}

/// Prune conversation history to maintain context budget limit (32,000 chars)
fn prune_messages(messages: &mut Vec<SimpleChatMessage>) {
    loop {
        let total_chars: usize = messages.iter().map(|m| m.content.len()).sum();
        if total_chars <= 32000 {
            break;
        }

        // Retain System prompt if it is the first element
        let remove_idx = if messages.first().map(|m| m.role == "System").unwrap_or(false) {
            if messages.len() > 1 {
                1
            } else {
                break;
            }
        } else {
            0
        };

        if messages.len() > remove_idx {
            messages.remove(remove_idx);
        } else {
            break;
        }
    }
}

/// Run the simple CLI chat mode
pub async fn run(args: SimpleChatArgs) -> Result<()> {
    let config_path = args
        .config_override
        .or_else(perspt_core::paths::resolve_config_file)
        .or_else(perspt_core::paths::config_file);
    let config = match config_path {
        Some(ref path) => Config::load_from_path(path)?,
        None => Config::default(),
    };

    // Build a bound provider from config + env, with CLI --model taking precedence.
    let (provider, resolved) = GenAIProvider::from_config(&config, args.model.as_deref())
        .context("Failed to create LLM provider. Ensure an API key or config is set.")?;
    let provider = Arc::new(provider);
    let provider_type = resolved.provider;
    let mut model_name = resolved.model;

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

    // Setup rustyline for terminal editing
    let is_terminal = std::io::stdin().is_terminal();
    let mut rl = DefaultEditor::new()?;
    if is_terminal {
        if let Some(history_path) = perspt_core::paths::history_file() {
            if let Some(parent) = history_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = rl.load_history(&history_path);
        }
    }

    // Keep conversation history
    let mut messages: Vec<SimpleChatMessage> = Vec::new();

    // Easter egg state
    let mut easter_egg_triggered = false;

    // Welcome message
    println!("Perspt Simple Chat Mode");
    println!("Provider: {} | Model: {}", provider_type, model_name);
    if let Some(ref log_path) = args.log_file {
        println!("Logging to: {}", log_path.display());
    }
    println!("Type /help to see available commands, or Ctrl+D to quit.");
    println!();

    loop {
        let trimmed_input = if is_terminal {
            match rl.readline("> ") {
                Ok(line) => {
                    let trimmed = line.trim().to_string();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let _ = rl.add_history_entry(&trimmed);
                    trimmed
                }
                Err(ReadlineError::Interrupted) => {
                    println!("Ctrl-C");
                    continue;
                }
                Err(ReadlineError::Eof) => {
                    println!();
                    break;
                }
                Err(err) => {
                    println!("Error reading line: {:?}", err);
                    break;
                }
            }
        } else {
            user_input.clear();
            let bytes_read = stdin_reader
                .read_line(&mut user_input)
                .await
                .context("Failed to read from stdin")?;
            if bytes_read == 0 {
                break;
            }
            let trimmed = user_input.trim().to_string();
            if trimmed.is_empty() {
                continue;
            }
            println!("{}", trimmed);
            trimmed
        };

        // Check for slash commands
        if trimmed_input.starts_with('/') {
            let cmd = trimmed_input.to_lowercase();
            if cmd == "/exit" || cmd == "/quit" {
                break;
            } else if cmd == "/clear" {
                messages.clear();
                println!("Conversation history cleared.");
                continue;
            } else if cmd.starts_with("/model") {
                let parts: Vec<&str> = trimmed_input.split_whitespace().collect();
                if parts.len() > 1 {
                    let new_model = parts[1..].join(" ");
                    model_name = new_model;
                    println!("Switched model to: {}", model_name);
                } else {
                    println!("Usage: /model <name>");
                }
                continue;
            } else if cmd == "/help" {
                println!("Available Slash Commands:");
                println!("  /exit, /quit      - Exit the simple-chat session");
                println!("  /clear            - Reset the active conversation history");
                println!("  /model <name>     - Switch the active model on the fly");
                println!("  /help             - Show this help menu");
                continue;
            } else {
                println!("Unknown command: {}. Type /help for available commands.", trimmed_input);
                continue;
            }
        }

        // Check for exit command (fallback)
        if trimmed_input.eq_ignore_ascii_case("exit") {
            break;
        }

        // Check for Easter egg
        if check_easter_egg(&trimmed_input, &mut easter_egg_triggered) {
            display_easter_egg();
            continue;
        }

        // Log user input
        if let Some(ref mut file) = log_handle {
            writeln!(file, "> {}", trimmed_input).context("Failed to write to log file")?;
        }

        // Add user message to conversation history
        messages.push(SimpleChatMessage {
            role: "User".into(),
            content: trimmed_input.clone(),
        });
        prune_messages(&mut messages);

        // Build context from history
        let context: Vec<String> = messages
            .iter()
            .filter(|m| m.role != "System")
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect();
        let prompt_input = context.join("\n");

        // Create channel for streaming response
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Clone for async task
        let provider_clone = Arc::clone(&provider);
        let model_clone = model_name.clone();
        let input_clone = prompt_input;

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

        if !full_response.is_empty() {
            messages.push(SimpleChatMessage {
                role: "Assistant".into(),
                content: full_response.clone(),
            });
        }

        // Log response
        if let Some(ref mut file) = log_handle {
            if !full_response.is_empty() {
                writeln!(file, "{}", full_response).context("Failed to write to log file")?;
            }
            writeln!(file).context("Failed to write to log file")?;
        }
    }

    if is_terminal {
        if let Some(history_path) = perspt_core::paths::history_file() {
            let _ = rl.save_history(&history_path);
        }
    }

    println!("Goodbye!");
    Ok(())
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
