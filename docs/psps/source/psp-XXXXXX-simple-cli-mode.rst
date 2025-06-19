PSP: XXXXXX
Title: Simple CLI Mode for Direct Q&A
Author: Vikrant Rathore (@vikrantrathore)
Status: Draft
Type: Feature
Created: 2025-06-19
Discussion-To: <Link to GitHub Issue for this PSP>

========
Abstract
========

This PSP proposes the addition of a new command-line interface (CLI) mode for Perspt that allows for direct question-and-answer interaction within the terminal. This mode will provide a simple, Unix-like prompt for user input, display raw AI responses directly to the console, and optionally save the entire interaction to a file.

==========
Motivation
==========

Currently, Perspt's interaction model is centered around its terminal user interface (TUI), which is powerful for interactive sessions but can be cumbersome for users who want quick, scriptable, or logged interactions. A simpler, direct CLI mode would benefit:

*   **Power Users & Developers:** Who want to integrate Perspt into scripts or command-line workflows.
*   **Users with Accessibility Needs:** Who may find a simple, scrolling console output easier to work with than a TUI.
*   **Logging and Auditing:** Users who need to keep a record of their interactions with the AI for documentation or review.

================
Proposed Changes
================

.. rubric:: Functional Specification

A new command-line flag, `--simple-cli` (or a similar name), will be introduced to launch Perspt in this new mode.

**Behavioral Changes:**

*   **No TUI:** When launched with `--simple-cli`, Perspt will not render its TUI.
*   **Simple Prompt:** A simple, Unix-like prompt (e.g., `> `) will be displayed for user input.
*   **Direct Output:** The AI's response will be printed directly to standard output as raw text. The terminal will handle scrolling.
*   **Continuous Interaction:** After the AI response is complete, the prompt will reappear for the next question.
*   **Exit:** The application will exit cleanly when the user presses `Ctrl+C`.
*   **File Logging:** An optional argument, such as `--log-file <filename>`, can be used with `--simple-cli` to save the entire interaction (both user questions and AI responses) to the specified file. The output will still be printed to the console.

.. rubric:: UI/UX Design

The UI/UX is intentionally minimal:

*   **Prompt:** A simple, non-intrusive prompt (e.g., `> `).
*   **Output:** Raw text output from the AI, with no special rendering.
*   **Interaction:** A standard, synchronous request-response loop.

.. rubric:: Technical Specification

This section outlines the proposed implementation details for the simple CLI mode. The changes will primarily be in `src/main.rs` and will introduce a new `cli` module (in a new file `src/cli.rs`) to encapsulate the new logic.

*   **Argument Parsing (`src/main.rs`):**

    The existing `clap` command builder will be updated to include the new mode and its related options.

    .. code-block:: rust

       // In src/main.rs, inside the Command::new() block
       .arg(
           Arg::new("simple-cli")
               .long("simple-cli")
               .help("Run in simple CLI mode for direct Q&A")
               .action(clap::ArgAction::SetTrue),
       )
       .arg(
           Arg::new("log-file")
               .long("log-file")
               .value_name("FILE")
               .help("Optional file to log the CLI session")
               .requires("simple-cli"),
       )

*   **Application Flow (`src/main.rs`):**

    The `async main` function will parse these arguments and, if `--simple-cli` is present, delegate to a new `async` function for the CLI mode, passing the already-initialized configuration and provider.

    .. code-block:: rust

       // In src/main.rs

       mod cli; // New module

       #[tokio::main]
       async fn main() -> Result<()> {
           // ... existing argument parsing and config setup ...

           let list_models = matches.get_flag("list-models");
           let simple_cli_mode = matches.get_flag("simple-cli");
           let log_file = matches.get_one::<String>("log-file").cloned();

           // ... existing config and provider setup ...
           let provider = Arc::new(GenAIProvider::new_with_config(...)?);
           let validated_model = provider.validate_model(...).await?;

           if list_models {
               list_available_models(&provider, &config).await?;
               return Ok(());
           }

           if simple_cli_mode {
               // Run the new simple CLI mode
               cli::run_simple_cli(
                   provider,
                   validated_model,
                   log_file,
               ).await?;
           } else {
               // Run the existing TUI application
               let mut terminal = initialize_terminal().context("Failed to initialize terminal")?;
               run_ui(
                   &mut terminal,
                   config,
                   validated_model,
                   api_key_string,
                   provider,
               )
               .await
               .context("UI execution failed")?;
               cleanup_terminal()?;
           }

           Ok(())
       }

*   **Simple CLI Implementation (new file `src/cli.rs`):**

    A new module will contain the core logic for the simple, interactive, and asynchronous command-line loop. It will use streaming for a responsive feel.

    .. code-block:: rust

       // In a new file: src/cli.rs

       use anyhow::{Context, Result};
       use std::io::Write;
       use std::sync::Arc;
       use tokio::io::{self, AsyncBufReadExt, BufReader};
       use tokio::sync::mpsc;
       use crate::llm_provider::GenAIProvider;
       use crate::EOT_SIGNAL;

       pub async fn run_simple_cli(
           provider: Arc<GenAIProvider>,
           model_name: String,
           log_file: Option<String>,
       ) -> Result<()> {
           let mut log_handle = if let Some(path) = log_file {
               Some(
                   std::fs::OpenOptions::new()
                       .create(true)
                       .append(true)
                       .open(path)
                       .context("Failed to open log file")?,
               )
           } else {
               None
           };

           let mut stdin_reader = BufReader::new(io::stdin());
           let mut user_input = String::new();

           println!("Entering Simple CLI Mode. Press Ctrl+D or type 'exit' to quit.");

           loop {
               print!("> ");
               std::io::stdout().flush()?;
               user_input.clear();

               if stdin_reader.read_line(&mut user_input).await? == 0 {
                   // User pressed Ctrl+D (EOF)
                   println!(); // Newline for clean exit
                   break;
               }

               let trimmed_input = user_input.trim();
               if trimmed_input.is_empty() {
                   continue;
               }
               if trimmed_input.eq_ignore_ascii_case("exit") {
                   break;
               }

               if let Some(ref mut file) = log_handle {
                   writeln!(file, "> {}", trimmed_input)?;
               }

               let (tx, mut rx) = mpsc::unbounded_channel();

               let provider_clone = Arc::clone(&provider);
               let model_name_clone = model_name.clone();
               let input_clone = trimmed_input.to_string();

               tokio::spawn(async move {
                   let _ = provider_clone
                       .generate_response_stream_to_channel(
                           &model_name_clone,
                           &input_clone,
                           tx,
                       )
                       .await;
               });

               let mut full_response = String::new();
               while let Some(chunk) = rx.recv().await {
                   if chunk == EOT_SIGNAL {
                       break;
                   }
                   print!("{}", chunk);
                   std::io::stdout().flush()?;
                   full_response.push_str(&chunk);
               }
               println!(); // Add a newline after the full response

               if let Some(ref mut file) = log_handle {
                   writeln!(file, "{}\n", full_response)?;
               }
           }
           Ok(())
       }

*   **Dependencies:**

    This feature can be built using the existing project crates. The `tokio` dependency with the `io-util` and `macros` features is already in use and will support the asynchronous CLI loop.

*   **Configuration:**

    The simple CLI mode will respect the existing configuration mechanisms (`config.json`, environment variables, CLI arguments) for all settings related to the LLM provider (e.g., API keys, model choice). The configuration will be loaded and processed at startup in `main.rs` before the provider is initialized.

=========
Rationale
=========

This approach was chosen for its simplicity and broad utility. It aligns with the Unix philosophy of creating simple, composable tools.

**Alternatives Considered:**

*   **Enhancing the existing TUI:** This would add complexity to the existing TUI and not fully address the need for a simple, scriptable interface.
*   **Creating a separate application:** This would create unnecessary fragmentation of the project. A new mode within the existing application is a more cohesive solution.

=======================
Backwards Compatibility
=======================

This change is fully backwards-compatible. The existing TUI remains the default mode of operation. The new mode is only activated when the `--simple-cli` flag is used.

=========
Copyright
=========

This document is placed in the public domain or under the CC0-1.0-Universal license, whichever is more permissive.
