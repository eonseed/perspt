//! # Perspt - Performance LLM Chat CLI
//!
//! A high-performance terminal-based chat application for interacting with various Large Language Models (LLMs)
//! through a unified interface. Built with Rust for speed and reliability.
//!
//! ## Overview
//!
//! Perspt provides a beautiful terminal user interface for chatting with multiple LLM providers including:
//! - OpenAI (GPT models)
//! - Anthropic (Claude models)
//! - Google (Gemini models)
//! - Groq (Fast inference models)
//! - Cohere (Command models)
//! - XAI (Grok models)
//! - DeepSeek (Chat and reasoning models)
//! - Ollama (Local models)
//!
//! ## Features
//!
//! - **Unified API**: Single interface for multiple LLM providers
//! - **Real-time streaming**: Live response streaming for better user experience
//! - **Robust error handling**: Comprehensive panic recovery and error categorization
//! - **Configuration management**: Flexible JSON-based configuration
//! - **Terminal UI**: Beautiful, responsive terminal interface with markdown rendering
//! - **Conversation saving**: Export chat conversations to text files with `/save` command
//! - **Model discovery**: Automatic model listing and validation
//!
//! ## Architecture
//!
//! The application follows a modular architecture:
//! - [`main`](crate): Entry point, CLI argument parsing, and application initialization
//! - [`config`]: Configuration management and loading
//! - [`llm_provider`]: LLM provider abstraction and implementation
//! - [`ui`]: Terminal user interface and event handling
//!
//! ## Usage
//!
//! ```bash
//! # Basic usage with default OpenAI provider
//! perspt
//!
//! # Specify a different provider
//! perspt --provider-type anthropic --model-name claude-3-sonnet-20240229
//!
//! # Use custom configuration file
//! perspt --config /path/to/config.json
//!
//! # List available models for current provider
//! perspt --list-models
//! ```
//!
//! ## Configuration
//!
//! See [`AppConfig`] for detailed configuration options.
//! The application uses JSON configuration files to manage provider settings,
//! API keys, and UI preferences.
//!
//! ## Error Handling
//!
//! The application implements comprehensive error handling and panic recovery.
//! All critical operations are wrapped in appropriate error contexts for!
//! better debugging and user experience.

// src/main.rs
use anyhow::{Context, Result};
use clap::{Arg, Command};
use std::io;
use std::panic;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::mpsc;

/// End-of-transmission signal used to indicate completion of streaming responses.
/// This constant is used throughout the application to signal when an LLM has
/// finished sending its response.
pub const EOT_SIGNAL: &str = "<<EOT>>";

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::LevelFilter;

use crate::config::AppConfig;
use crate::llm_provider::GenAIProvider;
use crate::ui::{run_ui, AppEvent};

mod cli;
mod config;
mod llm_provider;
mod ui;

/// Global flag to track terminal raw mode state for proper cleanup during panics.
/// This mutex-protected boolean ensures that the terminal state can be properly
/// restored even when the application panics, preventing terminal corruption.
static TERMINAL_RAW_MODE: Mutex<bool> = Mutex::new(false);

/// Sets up a comprehensive panic hook that ensures proper terminal restoration.
///
/// This function configures a custom panic handler that:
/// - Immediately disables raw terminal mode
/// - Exits alternate screen mode
/// - Clears the terminal display
/// - Provides user-friendly error messages with context-specific help
/// - Exits the application cleanly
///
/// The panic hook is designed to handle common failure scenarios like:
/// - Missing environment variables (PROJECT_ID, AWS credentials)
/// - API authentication failures
/// - Network connectivity issues
///
/// # Safety
///
/// This function should be called early in main() before any terminal
/// operations to ensure proper cleanup in all failure scenarios.
fn setup_panic_hook() {
    let _original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // Force terminal restoration immediately
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);

        // Clear the screen and move cursor to start
        print!("\x1b[2J\x1b[H");

        // Print a clean error message
        eprintln!();
        eprintln!("🚨 Application Error: External Library Panic");
        eprintln!("═══════════════════════════════════════════");
        eprintln!();
        eprintln!("The application encountered a fatal error in an external library.");
        eprintln!(
            "This is typically caused by missing environment variables or configuration issues."
        );
        eprintln!();

        // Extract useful information from panic
        let panic_str = format!("{panic_info}");
        if panic_str.contains("PROJECT_ID") {
            eprintln!("❌ Missing Google Cloud Configuration:");
            eprintln!(
                "   Please set the PROJECT_ID environment variable to your Google Cloud project ID"
            );
            eprintln!("   Example: export PROJECT_ID=your-project-id");
        } else if panic_str.contains("AWS") || panic_str.contains("credentials") {
            eprintln!("❌ Missing AWS Configuration:");
            eprintln!("   Please configure your AWS credentials using one of these methods:");
            eprintln!("   - Set AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY environment variables");
            eprintln!("   - Configure AWS CLI: aws configure");
            eprintln!("   - Set AWS_PROFILE environment variable");
        } else if panic_str.contains("NotPresent") {
            eprintln!("❌ Missing Environment Variable:");
            eprintln!("   A required environment variable is not set.");
            eprintln!("   Please check the documentation for your chosen provider.");
        } else {
            eprintln!("❌ Configuration Error:");
            eprintln!("   {panic_str}");
        }

        eprintln!();
        eprintln!("💡 Troubleshooting Tips:");
        eprintln!("   - Check your provider configuration");
        eprintln!("   - Verify all required environment variables are set");
        eprintln!("   - Use --help for available options");
        eprintln!("   - Try a different provider (e.g., --provider-type openai)");
        eprintln!();

        // Don't call the original hook to avoid double panic output
        std::process::exit(1);
    }));
}

/// Updates the global terminal raw mode flag.
///
/// This function safely updates the global raw mode state flag using a mutex
/// to prevent race conditions. The flag is used by the panic handler to determine
/// whether terminal cleanup is necessary.
///
/// # Arguments
///
/// * `enabled` - Boolean indicating whether raw mode is currently enabled
///
/// # Thread Safety
///
/// This function is thread-safe and can be called from multiple threads
/// simultaneously without data races.
fn set_raw_mode_flag(enabled: bool) {
    if let Ok(mut flag) = TERMINAL_RAW_MODE.lock() {
        *flag = enabled;
    }
}

/// Main application entry point.
///
/// This function orchestrates the entire application lifecycle:
/// 1. Sets up panic handling for terminal safety
/// 2. Initializes logging system
/// 3. Parses command-line arguments
/// 4. Loads and validates configuration
/// 5. Creates and configures the LLM provider
/// 6. Initializes the terminal interface
/// 7. Runs the main UI loop
/// 8. Ensures proper cleanup on exit
///
/// # Returns
///
/// * `Result<()>` - Success or error details if the application fails to start
///
/// # Errors
///
/// This function can return errors for:
/// - Invalid command-line arguments
/// - Configuration file parsing failures
/// - LLM provider validation failures
/// - Terminal initialization failures
/// - Network connectivity issues
///
/// # Examples
///
/// The application supports various usage patterns:
///
/// ```bash
/// # Default usage with OpenAI
/// perspt
///
/// # Custom provider and model
/// perspt --provider-type anthropic --model-name claude-3-sonnet-20240229
///
/// # Custom configuration
/// perspt --config ./my-config.json --api-key sk-...
/// ```
#[tokio::main]
async fn main() -> Result<()> {
    // Set up panic hook before doing anything else
    setup_panic_hook();

    // Initialize logging - set to error level only to avoid TUI interference
    // Logs will only show critical errors
    env_logger::Builder::from_default_env()
        .filter_level(LevelFilter::Error)
        .init();

    // Parse CLI arguments
    let matches = Command::new("Perspt - Performance LLM Chat CLI")
        .version("0.4.0")
        .author("Vikrant Rathore")
        .about(
            "A performant CLI for talking to LLMs using the allms crate with unified API support",
        )
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path"),
        )
        .arg(
            Arg::new("api-key")
                .short('k')
                .long("api-key")
                .value_name("KEY")
                .help("API key for the LLM provider"),
        )
        .arg(
            Arg::new("model-name")
                .short('m')
                .long("model")
                .value_name("MODEL")
                .help("Model name to use"),
        )
        .arg(
            Arg::new("provider-type")
                .short('p')
                .long("provider-type")
                .value_name("TYPE")
                .help(
                    "Provider type: openai, anthropic, gemini, groq, cohere, xai, deepseek, ollama",
                )
                .value_parser([
                    "openai",
                    "anthropic",
                    "gemini",
                    "groq",
                    "cohere",
                    "xai",
                    "deepseek",
                    "ollama",
                ]),
        )
        .arg(
            Arg::new("provider")
                .long("provider")
                .value_name("PROFILE")
                .help("Provider profile name from config"),
        )
        .arg(
            Arg::new("list-models")
                .short('l')
                .long("list-models")
                .help("List available models for the configured provider")
                .action(clap::ArgAction::SetTrue),
        )
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
        .get_matches();

    let config_path = matches.get_one::<String>("config");
    let cli_api_key = matches.get_one::<String>("api-key");
    let cli_model_name = matches.get_one::<String>("model-name");
    let cli_provider_profile = matches.get_one::<String>("provider");
    let cli_provider_type = matches.get_one::<String>("provider-type");
    let list_models = matches.get_flag("list-models");
    let simple_cli_mode = matches.get_flag("simple-cli");
    let log_file = matches.get_one::<String>("log-file").cloned();

    // Load configuration
    let mut config = config::load_config(config_path)
        .await
        .context("Failed to load configuration")?;

    // Apply CLI overrides BEFORE checking provider configuration
    if let Some(key) = cli_api_key {
        config.api_key = Some(key.clone());
        log::info!("Using API key from command line argument");
    }

    if let Some(ptype) = cli_provider_type {
        config.provider_type = Some(ptype.clone());
        log::info!("Using provider type from command line: {ptype}");
    }

    if let Some(profile_name) = cli_provider_profile {
        config.default_provider = Some(profile_name.clone());
        log::info!("Using provider profile from command line: {profile_name}");
    }

    if let Some(model_val) = cli_model_name {
        config.default_model = Some(model_val.clone());
        log::info!("Using model from command line: {model_val}");
    }

    // Check if we have a valid provider configuration (after CLI overrides)
    if config.provider_type.is_none() {
        eprintln!("❌ No LLM provider configured!");
        eprintln!();
        eprintln!("To get started, either:");
        eprintln!("  1. Set an environment variable for a supported provider:");
        eprintln!("     • OPENAI_API_KEY=sk-your-key");
        eprintln!("     • ANTHROPIC_API_KEY=sk-ant-your-key");
        eprintln!("     • GEMINI_API_KEY=your-key");
        eprintln!("     • GROQ_API_KEY=your-key");
        eprintln!("     • COHERE_API_KEY=your-key");
        eprintln!("     • XAI_API_KEY=your-key");
        eprintln!("     • DEEPSEEK_API_KEY=your-key");
        eprintln!();
        eprintln!("  2. Use command line arguments:");
        eprintln!("     perspt --provider-type openai --api-key sk-your-key");
        eprintln!();
        eprintln!("  3. Create a config.json file with provider settings");
        eprintln!();
        return Err(anyhow::anyhow!("No provider configured"));
    }

    // Get model name for the provider - set defaults based on provider type using genai compatible names
    let model_name_for_provider =
        config
            .default_model
            .clone()
            .unwrap_or_else(|| match config.provider_type.as_deref() {
                Some("openai") => "gpt-4o-mini".to_string(),
                Some("anthropic") => "claude-3-5-sonnet-20241022".to_string(),
                Some("gemini") => "gemini-1.5-flash".to_string(),
                Some("groq") => "llama-3.1-8b-instant".to_string(),
                Some("cohere") => "command-r-plus".to_string(),
                Some("xai") => "grok-beta".to_string(),
                Some("deepseek") => "deepseek-chat".to_string(),
                Some("ollama") => "llama3.2".to_string(),
                _ => "gpt-4o-mini".to_string(),
            });

    let api_key_string = config.api_key.clone().unwrap_or_default();

    // Create the GenAI provider instance with configuration
    let provider = Arc::new(GenAIProvider::new_with_config(
        config.provider_type.as_deref(),
        config.api_key.as_deref(),
    )?);

    log::info!(
        "Created GenAI provider with provider_type: {:?}, has_api_key: {}",
        config.provider_type,
        config.api_key.is_some()
    );

    // Handle list-models command before model validation
    if list_models {
        list_available_models(&provider, &config).await?;
        return Ok(());
    }

    // Validate the model before starting UI (only if not listing models)
    let validated_model = provider
        .validate_model(&model_name_for_provider, config.provider_type.as_deref())
        .await
        .context("Failed to validate model")?;

    if validated_model != model_name_for_provider {
        log::info!(
            "Model changed from {model_name_for_provider} to {validated_model} after validation"
        );
    }

    if simple_cli_mode {
        // Run the simple CLI mode
        cli::run_simple_cli(provider, validated_model, log_file).await?;
        return Ok(());
    }

    // Initialize terminal
    let mut terminal = initialize_terminal().context("Failed to initialize terminal")?;

    // Run the UI - panic handling is done at the LLM provider level and via the global panic hook
    run_ui(
        &mut terminal,
        config,
        validated_model,
        api_key_string,
        provider,
    )
    .await
    .context("UI execution failed")?;

    // Ensure terminal cleanup
    cleanup_terminal()?;

    Ok(())
}

/// Lists all available models for the current LLM provider.
///
/// This function queries the LLM provider for its supported models and displays
/// them in a user-friendly format. Each model is listed with its identifier
/// which can be used with the `--model-name` argument.
///
/// # Arguments
///
/// * `provider` - Arc reference to the LLM provider implementation
/// * `_config` - Application configuration (currently unused but reserved for future features)
///
/// # Returns
///
/// * `Result<()>` - Success or error if model listing fails
///
/// # Errors
///
/// This function can fail if:
/// - The provider doesn't support model listing
/// - Network connectivity issues prevent API calls
/// - Authentication failures occur
///
/// # Examples
///
/// ```bash
/// # List OpenAI models
/// perspt --provider-type openai --list-models
///
/// # List Anthropic models
/// perspt --provider-type anthropic --list-models
/// ```
async fn list_available_models(provider: &Arc<GenAIProvider>, config: &AppConfig) -> Result<()> {
    // If a specific provider was configured, list models for that provider only
    if let Some(provider_type) = &config.provider_type {
        println!("Available models for {provider_type} provider:");
        match provider.get_available_models(provider_type).await {
            Ok(models) => {
                if models.is_empty() {
                    println!("  No models found or API authentication required");
                    println!("  Try setting the appropriate API key environment variable:");
                    match provider_type.as_str() {
                        "openai" => println!("  export OPENAI_API_KEY=sk-your-key"),
                        "anthropic" => println!("  export ANTHROPIC_API_KEY=sk-ant-your-key"),
                        "gemini" => println!("  export GEMINI_API_KEY=your-key"),
                        "groq" => println!("  export GROQ_API_KEY=your-key"),
                        "cohere" => println!("  export COHERE_API_KEY=your-key"),
                        "xai" => println!("  export XAI_API_KEY=your-key"),
                        "deepseek" => println!("  export DEEPSEEK_API_KEY=your-key"),
                        "ollama" => println!("  Ensure Ollama is running locally on port 11434"),
                        _ => {}
                    }
                } else {
                    for model in models {
                        println!("  - {model}");
                    }
                }
            }
            Err(e) => {
                println!("  Error fetching models: {e}");
                println!("  This usually means:");
                println!("  1. No API key is configured for this provider");
                println!("  2. The API key is invalid");
                println!("  3. Network connectivity issues");
                println!();
                println!("  Try setting the API key:");
                match provider_type.as_str() {
                    "openai" => println!("     export OPENAI_API_KEY=sk-your-key"),
                    "anthropic" => println!("     export ANTHROPIC_API_KEY=sk-ant-your-key"),
                    "gemini" => println!("     export GEMINI_API_KEY=your-key"),
                    "groq" => println!("     export GROQ_API_KEY=your-key"),
                    "cohere" => println!("     export COHERE_API_KEY=your-key"),
                    "xai" => println!("     export XAI_API_KEY=your-key"),
                    "deepseek" => println!("     export DEEPSEEK_API_KEY=your-key"),
                    "ollama" => println!("     Ensure Ollama is running: ollama serve"),
                    _ => {}
                }
            }
        }
    } else {
        // List all providers and their models if no specific provider
        let providers = provider.get_available_providers().await?;

        for provider_name in providers {
            println!("Available models for {provider_name} provider:");
            match provider.get_available_models(&provider_name).await {
                Ok(models) => {
                    if models.is_empty() {
                        println!("  No models found or authentication required");
                    } else {
                        for model in models {
                            println!("  - {model}");
                        }
                    }
                }
                Err(e) => {
                    println!("  Error fetching models: {e}");
                }
            }
            println!();
        }
    }
    Ok(())
}

/// Initializes the terminal for TUI operation.
///
/// This function prepares the terminal for the Text User Interface by:
/// 1. Enabling raw mode for direct key input handling
/// 2. Entering alternate screen mode to preserve terminal contents
/// 3. Creating the ratatui terminal backend
/// 4. Clearing the terminal display
///
/// # Returns
///
/// * `Result<Terminal<CrosstermBackend<Stdout>>>` - Configured terminal or error
///
/// # Errors
///
/// This function can fail if:
/// - Raw mode cannot be enabled (terminal doesn't support it)
/// - Alternate screen mode fails (terminal limitations)
/// - Terminal backend creation fails (I/O errors)
///
/// # Safety
///
/// This function updates the global raw mode flag to enable proper cleanup
/// in case of panics. The terminal state should always be restored using
/// `cleanup_terminal()` or the panic handler will handle it automatically.
fn initialize_terminal() -> Result<ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>>
{
    enable_raw_mode().context("Failed to enable raw mode")?;
    set_raw_mode_flag(true);
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("Failed to enter alternate screen")?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend).context("Failed to create terminal")?;
    terminal.clear().context("Failed to clear terminal")?;
    Ok(terminal)
}

/// Cleans up terminal state and restores normal operation.
///
/// This function should be called before application exit to:
/// 1. Update the global raw mode flag
/// 2. Disable raw terminal mode
/// 3. Exit alternate screen mode
/// 4. Restore the original terminal state
///
/// # Returns
///
/// * `Result<()>` - Success or error if cleanup fails
///
/// # Errors
///
/// This function can fail if:
/// - Raw mode cannot be disabled (rare terminal issues)
/// - Alternate screen exit fails (terminal state corruption)
///
/// # Note
///
/// Even if this function fails, the panic handler will attempt
/// terminal restoration, so terminal corruption should be rare.
fn cleanup_terminal() -> Result<()> {
    set_raw_mode_flag(false);
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(io::stdout(), LeaveAlternateScreen).context("Failed to leave alternate screen")?;
    Ok(())
}

/// Initiates an asynchronous LLM request with proper state management.
///
/// This function handles the complex orchestration of sending a user message
/// to the LLM provider while managing UI state and providing user feedback.
///
/// # Process Flow
///
/// 1. Sets the application to busy state (disables input)
/// 2. Updates the status message with request information
/// 3. Spawns an asynchronous task for the LLM request
/// 4. Handles streaming responses through the provided channel
/// 5. Manages error states and recovery
///
/// # Arguments
///
/// * `app` - Mutable reference to the application state
/// * `input_to_send` - The user's message to send to the LLM
/// * `provider` - Arc reference to the LLM provider implementation
/// * `model_name` - Name/identifier of the model to use
/// * `tx_llm` - Channel sender for streaming LLM responses
///
/// # State Changes
///
/// This function modifies the application state:
/// - Sets `is_llm_busy` to true
/// - Sets `is_input_disabled` to true
/// - Updates the status message
/// - May add error messages to the chat history
///
/// # Concurrency
///
/// The actual LLM request is executed in a separate tokio task to prevent
/// blocking the UI thread. This ensures the interface remains responsive
/// during potentially long-running LLM requests.
///
/// # Error Handling
///
/// Errors are handled gracefully and communicated to the user through:
/// - Status message updates
/// - Error state management
/// - Chat history error messages
async fn initiate_llm_request(
    app: &mut ui::App,
    input_to_send: String,
    provider: Arc<GenAIProvider>,
    model_name: &str,
    tx_llm: &mpsc::UnboundedSender<String>,
) {
    app.is_llm_busy = true;
    app.is_input_disabled = true;
    app.streaming_buffer.clear(); // Clear any previous streaming buffer

    log::info!("Initiating LLM request for input: '{input_to_send}'");
    app.set_status(
        format!("Sending: {}...", truncate_message(&input_to_send, 20)),
        false,
    );

    let model_name_clone = model_name.to_string();
    let _config_clone = app.config.clone();
    let tx_clone_for_provider = tx_llm.clone();
    let input_clone = input_to_send.clone();

    tokio::spawn(async move {
        // Use the provider's streaming method with proper genai streaming
        let result = provider
            .generate_response_stream_to_channel(
                &model_name_clone,
                &input_clone,
                tx_clone_for_provider.clone(),
            )
            .await;

        match result {
            Ok(()) => {
                log::debug!("Streaming completed successfully");
                // EOT signal is now sent by the provider itself, no need to send it here
            }
            Err(e) => {
                log::error!("LLM request failed: {e}");
                let error_msg = format!("Error: {e}");
                let _ = tx_clone_for_provider.send(error_msg);
                let _ = tx_clone_for_provider.send(EOT_SIGNAL.to_string());
            }
        }
    });
}

/// Truncates a message to a specified maximum length for display purposes.
///
/// This utility function is used to create abbreviated versions of messages
/// for status displays and logs where space is limited.
///
/// # Arguments
///
/// * `s` - The string to truncate
/// * `max_chars` - Maximum number of characters to include
///
/// # Returns
///
/// * `String` - The truncated string with "..." suffix if truncation occurred
///
/// # Examples
///
/// ```rust
/// let short = truncate_message("Hello world", 5);
/// assert_eq!(short, "He...");
///
/// let unchanged = truncate_message("Hi", 10);
/// assert_eq!(unchanged, "Hi");
/// ```
fn truncate_message(s: &str, max_chars: usize) -> String {
    if s.len() > max_chars {
        format!("{}...", &s[..max_chars.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

/// Handles terminal events and user input in the main application loop.
///
/// This function processes keyboard events and manages user interactions with
/// the terminal interface. It supports various input modes and application states.
///
/// # Arguments
///
/// * `app` - Mutable reference to the application state
/// * `tx_llm` - Channel sender for LLM communication
/// * `_api_key` - API key for the LLM provider (currently unused in this function)
/// * `model_name` - Name of the current LLM model
/// * `provider` - Arc reference to the LLM provider implementation
///
/// # Returns
///
/// * `Option<AppEvent>` - Returns Some(AppEvent) for significant events, None otherwise
///
/// # Supported Events
///
/// - **Enter**: Send current input to LLM (if not busy)
/// - **Escape**: Quit application or close help overlay
/// - **F1/?**: Toggle help overlay
/// - **Ctrl+C**: Force quit application
/// - **Arrow Up/Down**: Scroll chat history
/// - **Printable characters**: Add to input buffer
/// - **Backspace**: Remove last character from input
///
/// # State Management
///
/// The function manages several application states:
/// - Input text buffer
/// - LLM busy state
/// - Help overlay visibility
/// - Chat history scrolling
/// - Application quit state
///
/// # Error Handling
///
/// Terminal events that cannot be read are ignored to maintain
/// application stability. Critical errors are logged for debugging.
pub async fn handle_events(
    app: &mut ui::App,
    tx_llm: &mpsc::UnboundedSender<String>,
    _api_key: &str,
    model_name: &str,
    provider: &Arc<GenAIProvider>,
) -> Option<AppEvent> {
    if let Ok(Event::Key(key)) = event::read() {
        if key.kind == KeyEventKind::Press {
            match key.code {
                KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.should_quit = true;
                    return Some(AppEvent::Key(key));
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.should_quit = true;
                    return Some(AppEvent::Key(key));
                }
                KeyCode::F(1) => {
                    app.show_help = !app.show_help;
                    return Some(AppEvent::Key(key));
                }
                KeyCode::Esc => {
                    if app.show_help {
                        app.show_help = false;
                    } else {
                        app.should_quit = true;
                    }
                    return Some(AppEvent::Key(key));
                }
                KeyCode::Enter => {
                    if !app.is_input_disabled && !app.input_text.trim().is_empty() {
                        let input_to_send = app.input_text.trim().to_string();

                        // Check for Easter egg exact sequence "l-o-v-e"
                        if input_to_send.eq_ignore_ascii_case("l-o-v-e") {
                            app.input_text.clear();
                            app.trigger_easter_egg();
                            return Some(AppEvent::Key(key));
                        }

                        app.input_text.clear();

                        // Add user message to chat history
                        app.add_message(ui::ChatMessage {
                            message_type: ui::MessageType::User,
                            content: vec![ratatui::text::Line::from(input_to_send.clone())],
                            timestamp: ui::App::get_timestamp(),
                            raw_content: input_to_send.clone(),
                        });

                        // Clear any previous errors when starting a new request
                        app.clear_error();

                        // Start LLM request
                        initiate_llm_request(
                            app,
                            input_to_send,
                            Arc::clone(provider),
                            model_name,
                            tx_llm,
                        )
                        .await;
                    } else if app.is_input_disabled && !app.input_text.trim().is_empty() {
                        // Queue the input if LLM is busy
                        let input_to_queue = app.input_text.trim().to_string();
                        app.pending_inputs.push_back(input_to_queue);
                        app.input_text.clear();
                        app.set_status(
                            format!("Message queued (queue: {})", app.pending_inputs.len()),
                            false,
                        );
                    }
                    return Some(AppEvent::Key(key));
                }
                KeyCode::Char(c) => {
                    if !app.is_input_disabled || !app.show_help {
                        app.input_text.push(c);
                    }
                    return Some(AppEvent::Key(key));
                }
                KeyCode::Backspace => {
                    if !app.is_input_disabled || !app.show_help {
                        app.input_text.pop();
                    }
                    return Some(AppEvent::Key(key));
                }
                KeyCode::Up => {
                    app.scroll_up();
                    return Some(AppEvent::Key(key));
                }
                KeyCode::Down => {
                    app.scroll_down();
                    return Some(AppEvent::Key(key));
                }
                KeyCode::PageUp => {
                    // Scroll up by 5 lines
                    for _ in 0..5 {
                        app.scroll_up();
                    }
                    return Some(AppEvent::Key(key));
                }
                KeyCode::PageDown => {
                    // Scroll down by 5 lines
                    for _ in 0..5 {
                        app.scroll_down();
                    }
                    return Some(AppEvent::Key(key));
                }
                KeyCode::Home => {
                    // Scroll to top
                    app.scroll_position = 0;
                    app.update_scroll_state();
                    return Some(AppEvent::Key(key));
                }
                KeyCode::End => {
                    // Scroll to bottom
                    app.scroll_to_bottom();
                    return Some(AppEvent::Key(key));
                }
                _ => {
                    return Some(AppEvent::Key(key));
                }
            }
        }
    }
    None
}
