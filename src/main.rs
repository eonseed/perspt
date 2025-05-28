// src/main.rs
use clap::{Arg, Command};
use anyhow::{Context, Result};
use std::io;
use std::sync::Arc;
use tokio::sync::mpsc;
use std::panic;
use std::sync::Mutex;

// Define EOT_SIGNAL
pub const EOT_SIGNAL: &str = "<<EOT>>";

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use env_logger;
use log::LevelFilter;

use crate::config::AppConfig;
use crate::llm_provider::{LLMProvider, UnifiedLLMProvider, ProviderType};
use crate::ui::{run_ui, AppEvent};

mod config;
mod llm_provider;
mod ui;

// Global flag to track if we're in raw mode for panic recovery
static TERMINAL_RAW_MODE: Mutex<bool> = Mutex::new(false);

/// Set up a panic hook that restores terminal state before panicking
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
        eprintln!("This is typically caused by missing environment variables or configuration issues.");
        eprintln!();
        
        // Extract useful information from panic
        let panic_str = format!("{}", panic_info);
        if panic_str.contains("PROJECT_ID") {
            eprintln!("❌ Missing Google Cloud Configuration:");
            eprintln!("   Please set the PROJECT_ID environment variable to your Google Cloud project ID");
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
            eprintln!("   {}", panic_str);
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

/// Set the global raw mode flag
fn set_raw_mode_flag(enabled: bool) {
    if let Ok(mut flag) = TERMINAL_RAW_MODE.lock() {
        *flag = enabled;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Set up panic hook before doing anything else
    setup_panic_hook();
    
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(LevelFilter::Info)
        .init();

    // Parse CLI arguments
    let matches = Command::new("Perspt - Performance LLM Chat CLI")
        .version("0.4.0")
        .author("Vikrant Rathore")
        .about("A performant CLI for talking to LLMs using the allms crate with unified API support")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
        )
        .arg(
            Arg::new("api-key")
                .short('k')
                .long("api-key")
                .value_name("KEY")
                .help("API key for the LLM provider")
        )
        .arg(
            Arg::new("model-name")
                .short('m')
                .long("model")
                .value_name("MODEL")
                .help("Model name to use")
        )
        .arg(
            Arg::new("provider-type")
                .short('p')
                .long("provider-type")
                .value_name("TYPE")
                .help("Provider type: openai, anthropic, google, mistral, perplexity, deepseek, aws-bedrock, azure-openai")
                .value_parser(["openai", "anthropic", "google", "mistral", "perplexity", "deepseek", "aws-bedrock", "azure-openai"])
        )
        .arg(
            Arg::new("provider")
                .long("provider")
                .value_name("PROFILE")
                .help("Provider profile name from config")
        )
        .arg(
            Arg::new("list-models")
                .short('l')
                .long("list-models")
                .help("List available models for the configured provider")
                .action(clap::ArgAction::SetTrue)
        )
        .get_matches();

    let config_path = matches.get_one::<String>("config");
    let cli_api_key = matches.get_one::<String>("api-key");
    let cli_model_name = matches.get_one::<String>("model-name");
    let cli_provider_profile = matches.get_one::<String>("provider");
    let cli_provider_type = matches.get_one::<String>("provider-type");
    let list_models = matches.get_flag("list-models");

    // Load configuration
    let mut config = config::load_config(config_path).await
        .context("Failed to load configuration")?;

    // Apply CLI overrides
    if let Some(key) = cli_api_key {
        config.api_key = Some(key.clone());
    }

    if let Some(ptype) = cli_provider_type {
        config.provider_type = Some(ptype.clone());
    }
    
    if let Some(profile_name) = cli_provider_profile {
        config.default_provider = Some(profile_name.clone());
    }

    if let Some(model_val) = cli_model_name {
        config.default_model = Some(model_val.clone());
    }

    // Get model name for the provider - set defaults based on provider type
    let model_name_for_provider = config.default_model.clone()
        .unwrap_or_else(|| {
            match config.provider_type.as_deref() {
                Some("openai") => "gpt-3.5-turbo".to_string(),
                Some("anthropic") => "claude-3-sonnet-20240229".to_string(),
                Some("google") => "gemini-pro".to_string(),
                Some("mistral") => "mistral-small".to_string(),
                Some("perplexity") => "llama-3.1-sonar-small-128k-online".to_string(),
                Some("deepseek") => "deepseek-chat".to_string(),
                Some("aws-bedrock") => "anthropic.claude-v2".to_string(),
                Some("azure-openai") => "gpt-35-turbo".to_string(),
                _ => "gpt-3.5-turbo".to_string(),
            }
        });
    
    let api_key_string = config.api_key.clone().unwrap_or_default();

    // Create the unified LLM provider instance
    let provider_type = ProviderType::from_string(
        config.provider_type.as_deref().unwrap_or("openai")
    ).unwrap_or(ProviderType::OpenAI);

    let provider: Arc<dyn LLMProvider + Send + Sync> = Arc::new(
        UnifiedLLMProvider::new(provider_type)
    );

    // Validate configuration for the provider
    if let Err(e) = provider.validate_config(&config).await {
        log::error!("Configuration validation failed: {}", e);
        return Err(e);
    }

    if list_models {
        list_available_models(&provider, &config).await?;
        return Ok(());
    }

    // Initialize terminal
    let mut terminal = initialize_terminal()
        .context("Failed to initialize terminal")?;

    // Run the UI - panic handling is done at the LLM provider level and via the global panic hook
    run_ui(&mut terminal, config, model_name_for_provider, api_key_string, provider).await
        .context("UI execution failed")?;

    // Ensure terminal cleanup
    cleanup_terminal()?;
    
    Ok(())
}

async fn list_available_models(
    provider: &Arc<dyn LLMProvider + Send + Sync>,
    _config: &AppConfig,
) -> Result<()> {
    match provider.list_models().await {
        Ok(models) => {
            println!("Available models for {:?} provider:", provider.provider_type());
            for model in models {
                println!("  - {}", model);
            }
        }
        Err(e) => {
            log::error!("Failed to list models: {}", e);
            return Err(e);
        }
    }
    Ok(())
}

fn initialize_terminal() -> Result<ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>> {
    enable_raw_mode().context("Failed to enable raw mode")?;
    set_raw_mode_flag(true);
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("Failed to enter alternate screen")?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend).context("Failed to create terminal")?;
    terminal.clear().context("Failed to clear terminal")?;
    Ok(terminal)
}

/// Clean up terminal state
fn cleanup_terminal() -> Result<()> {
    set_raw_mode_flag(false);
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(io::stdout(), LeaveAlternateScreen).context("Failed to leave alternate screen")?;
    Ok(())
}

async fn initiate_llm_request(
    app: &mut ui::App,
    input_to_send: String,
    provider: Arc<dyn LLMProvider + Send + Sync>, 
    model_name: &str,
    tx_llm: &mpsc::UnboundedSender<String>,
) {
    app.is_llm_busy = true;
    app.is_input_disabled = true;

    log::info!("Initiating LLM request for input: '{}'", input_to_send);
    app.set_status(format!("Sending: {}...", truncate_message(&input_to_send, 20)), false);

    let model_name_clone = model_name.to_string();
    let config_clone = app.config.clone(); 
    let tx_clone_for_provider = tx_llm.clone();
    let input_clone = input_to_send.clone();

    tokio::spawn(async move {
        // The panic protection is now handled inside the LLM provider's get_completion_response
        let result = provider.send_chat_request(
            &input_clone,
            &model_name_clone,
            &config_clone,
            &tx_clone_for_provider,
        ).await;

        if let Err(e) = result {
            log::error!("LLM request failed: {}", e);
            let error_msg = format!("Error: {}", e);
            let _ = tx_clone_for_provider.send(error_msg);
            let _ = tx_clone_for_provider.send(EOT_SIGNAL.to_string());
        }
    });
}

fn truncate_message(s: &str, max_chars: usize) -> String {
    if s.len() > max_chars {
        format!("{}...", &s[..max_chars.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

pub async fn handle_events(
    app: &mut ui::App,
    tx_llm: &mpsc::UnboundedSender<String>, 
    _api_key: &String,
    model_name: &String,
    provider: &Arc<dyn LLMProvider + Send + Sync>, 
) -> Option<AppEvent> {
    if let Ok(event) = event::read() {
        match event {
            Event::Key(key) => {
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
                                app.input_text.clear();

                                // Add user message to chat history
                                app.add_message(ui::ChatMessage {
                                    message_type: ui::MessageType::User,
                                    content: vec![ratatui::text::Line::from(input_to_send.clone())],
                                    timestamp: ui::App::get_timestamp(),
                                });

                                // Clear any previous errors when starting a new request
                                app.clear_error();

                                // Start LLM request
                                initiate_llm_request(app, input_to_send, Arc::clone(provider), model_name, tx_llm).await;
                            } else if app.is_input_disabled && !app.input_text.trim().is_empty() {
                                // Queue the input if LLM is busy
                                let input_to_queue = app.input_text.trim().to_string();
                                app.pending_inputs.push_back(input_to_queue);
                                app.input_text.clear();
                                app.set_status(format!("Message queued (queue: {})", app.pending_inputs.len()), false);
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
            _ => {}
        }
    }
    None
}
