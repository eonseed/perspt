//! # Simple CLI Module (cli.rs)
//!
//! This module implements a simple command-line interface mode for Perspt that allows
//! direct question-and-answer interaction in the terminal without the TUI overlay.
//! This mode is designed for users who prefer a Unix-like command prompt experience,
//! scripting integration, or accessibility needs.
//!
//! ## Features
//!
//! - **Simple Prompt**: Unix-like `> ` prompt for user input
//! - **Direct Output**: Raw AI responses printed directly to stdout
//! - **Streaming Support**: Real-time response streaming for better UX
//! - **File Logging**: Optional logging of entire sessions to files
//! - **Clean Exit**: Proper handling of Ctrl+D and 'exit' commands
//!
//! ## Usage
//!
//! ```bash
//! # Start simple CLI mode
//! perspt --simple-cli
//!
//! # With logging
//! perspt --simple-cli --log-file session.txt
//! ```

use crate::llm_provider::GenAIProvider;
use crate::EOT_SIGNAL;
use anyhow::{Context, Result};
use std::io::Write;
use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

/// Runs the simple CLI mode for direct Q&A interaction.
///
/// This function implements a simple, interactive command-line loop that:
/// 1. Displays a Unix-like prompt (`> `)
/// 2. Reads user input asynchronously
/// 3. Sends input to the LLM provider with streaming
/// 4. Displays the response directly to stdout
/// 5. Optionally logs the entire session to a file
///
/// # Arguments
///
/// * `provider` - Arc reference to the configured LLM provider
/// * `model_name` - Name of the model to use for generating responses
/// * `log_file` - Optional file path for logging the session
///
/// # Returns
///
/// * `Result<()>` - Success or error if the CLI loop fails
///
/// # Exit Conditions
///
/// The CLI loop exits when:
/// - User presses Ctrl+D (EOF)
/// - User types 'exit' (case-insensitive)
/// - An unrecoverable error occurs
///
/// # Error Handling
///
/// Errors during individual LLM requests are displayed to the user
/// but don't terminate the session. Only critical errors (like file
/// I/O failures) will cause the session to end.
///
/// # Logging Format
///
/// When logging is enabled, the format is:
/// ```text
/// > [user input]
/// [ai response]
///
/// > [next user input]
/// [next ai response]
///
/// ```
pub async fn run_simple_cli(
    provider: Arc<GenAIProvider>,
    model_name: String,
    log_file: Option<String>,
) -> Result<()> {
    // Open log file if specified
    let mut log_handle = if let Some(ref path) = log_file {
        Some(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .with_context(|| format!("Failed to open log file: {}", path))?,
        )
    } else {
        None
    };

    let stdin = io::stdin();
    let mut stdin_reader = BufReader::new(stdin);
    let mut user_input = String::new();

    // Print welcome message
    println!("Perspt Simple CLI Mode");
    println!("Model: {}", model_name);
    if let Some(ref log_path) = log_file {
        println!("Logging to: {}", log_path);
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
            // User pressed Ctrl+D (EOF)
            println!(); // Add newline for clean exit
            break;
        }

        let trimmed_input = user_input.trim();

        // Skip empty input
        if trimmed_input.is_empty() {
            continue;
        }

        // Check for exit command
        if trimmed_input.eq_ignore_ascii_case("exit") {
            break;
        }

        // Log user input if logging is enabled
        if let Some(ref mut file) = log_handle {
            writeln!(file, "> {}", trimmed_input).context("Failed to write to log file")?;
        }

        // Create channel for streaming response
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Clone necessary data for the async task
        let provider_clone = Arc::clone(&provider);
        let model_name_clone = model_name.clone();
        let input_clone = trimmed_input.to_string();

        // Spawn async task for LLM request
        let request_handle = tokio::spawn(async move {
            provider_clone
                .generate_response_stream_to_channel(&model_name_clone, &input_clone, tx)
                .await
        });

        // Process streaming response
        let mut full_response = String::new();
        let mut response_started = false;

        while let Some(chunk) = rx.recv().await {
            if chunk == EOT_SIGNAL {
                break;
            }

            // Print the chunk immediately for real-time streaming
            print!("{}", chunk);
            std::io::stdout().flush()?;
            full_response.push_str(&chunk);
            response_started = true;
        }

        // Wait for the request task to complete and handle any errors
        match request_handle.await {
            Ok(Ok(())) => {
                // Success - response completed normally
                if response_started {
                    println!(); // Add newline after response
                }
            }
            Ok(Err(e)) => {
                // LLM request failed
                if !response_started {
                    println!("Error: {}", e);
                } else {
                    println!("\nError during response: {}", e);
                }
            }
            Err(e) => {
                // Task panicked or was cancelled
                println!("Request failed: {}", e);
            }
        }

        // Log the full response if logging is enabled
        if let Some(ref mut file) = log_handle {
            if !full_response.is_empty() {
                writeln!(file, "{}", full_response).context("Failed to write to log file")?;
            }
            writeln!(file) // Add blank line between exchanges
                .context("Failed to write to log file")?;
        }
    }

    println!("Goodbye!");
    Ok(())
}
