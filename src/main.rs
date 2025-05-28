// src/main.rs
use clap::{Arg, Command};
use anyhow::{Context, Result};
use std::io;
use std::sync::Arc;
use tokio::sync::mpsc;

// Define EOT_SIGNAL
pub const EOT_SIGNAL: &str = "<<EOT>>";

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{enable_raw_mode, EnterAlternateScreen},
};
use env_logger;
use log::LevelFilter;

use crate::config::AppConfig;
use crate::llm_provider::LLMProvider;
use crate::local_llm_provider::LocalLlmProvider;
use crate::openai_llm::OpenAIProviderLlm;
use crate::gemini_llm::GeminiProviderLlm;
use crate::ui::{run_ui, AppEvent};

mod config;
mod gemini_llm;
mod llm_provider;
mod local_llm_provider;
mod openai_llm;
mod ui;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(LevelFilter::Info)
        .init();

    // Parse CLI arguments
    let matches = Command::new("Perspt - Performance LLM Chat CLI")
        .version("0.3.0")
        .author("Vikrant Rathore")
        .about("A performant CLI for talking to LLMs using modern APIs")
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
                .help("Model name or path to use")
        )
        .arg(
            Arg::new("provider-type")
                .short('p')
                .long("provider-type")
                .value_name("TYPE")
                .help("Provider type: local, openai, gemini")
                .value_parser(["local", "openai", "gemini"])
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
    
    // Ensure we have a default provider if local type was set
    if config.provider_type.as_deref() == Some("local") {
        if config.default_provider.is_none() {
            config.default_provider = Some("local".to_string());
        }
    }

    // Get model name for the provider
    let model_name_for_provider = config.default_model.clone()
        .unwrap_or_else(|| {
            match config.provider_type.as_deref() {
                Some("openai") => "gpt-3.5-turbo".to_string(),
                Some("gemini") => "gemini-pro".to_string(),
                Some("local") => "/path/to/model.gguf".to_string(),
                _ => "gpt-3.5-turbo".to_string(),
            }
        });
    
    let api_key_string = config.api_key.clone().unwrap_or_default();

    // Create the LLM provider instance
    let provider: Arc<dyn LLMProvider + Send + Sync> = match config.provider_type.as_deref() {
        Some("local") => Arc::new(LocalLlmProvider::new()),
        Some("openai") => Arc::new(OpenAIProviderLlm::new()),
        Some("gemini") => Arc::new(GeminiProviderLlm::new()),
        _ => {
            log::warn!("Unknown or missing provider type, defaulting to Gemini");
            Arc::new(GeminiProviderLlm::new())
        }
    };

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

    // Run the UI
    run_ui(&mut terminal, config, model_name_for_provider, api_key_string, provider).await
        .context("UI execution failed")?;
    
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
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("Failed to enter alternate screen")?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend).context("Failed to create terminal")?;
    terminal.clear().context("Failed to clear terminal")?;
    Ok(terminal)
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
        let result = provider.send_chat_request(
            &input_clone,
            &model_name_clone,
            &config_clone,
            &tx_clone_for_provider,
        ).await;

        if let Err(e) = result {
            log::error!("LLM request failed: {}", e);
            let _ = tx_clone_for_provider.send(format!("Error: {}", e));
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
                        KeyCode::Enter => {
                            if !app.is_input_disabled && !app.input_text.trim().is_empty() {
                                let input_to_send = app.input_text.trim().to_string();
                                app.input_text.clear();

                                // Add user message to chat history
                                app.add_message(ui::ChatMessage {
                                    message_type: ui::MessageType::User,
                                    content: vec![ratatui::text::Line::from(input_to_send.clone())],
                                });

                                // Start LLM request
                                initiate_llm_request(app, input_to_send, Arc::clone(provider), model_name, tx_llm).await;
                            }
                            return Some(AppEvent::Key(key));
                        }
                        KeyCode::Char(c) => {
                            if !app.is_input_disabled {
                                app.input_text.push(c);
                            }
                            return Some(AppEvent::Key(key));
                        }
                        KeyCode::Backspace => {
                            if !app.is_input_disabled {
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
