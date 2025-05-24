// src/main.rs
use clap::{Arg, Command};
use std::error::Error;
use std::io;
use tokio::sync::mpsc;

// Define EOT_SIGNAL
pub const EOT_SIGNAL: &str = "<<EOT>>"; // Made public

use crossterm::{
    event::{self, KeyCode, KeyEventKind, KeyModifiers, Event},
    execute,
    terminal::{enable_raw_mode, EnterAlternateScreen},
};
use log::LevelFilter;
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Config as Log4rsConfig, Root},
    encode::pattern::PatternEncoder,
};
use crate::ui::run_ui;
use crate::ui::AppEvent;
use crate::config::AppConfig;
// Removed old provider imports
// use crate::openai::OpenAIProvider;
// use crate::gemini::GeminiProvider;

// Added new provider imports
use crate::llm_provider::LLMProvider;
use crate::openai_llm::OpenAIProviderLlm;
use crate::gemini_llm::GeminiProviderLlm;
use crate::local_llm_provider::LocalLlmProvider;
// Arc might be needed if provider is shared more complexly, Box for now.
// use std::sync::Arc;


mod ui;
mod config;
// Removed old module declarations
// mod openai;
// mod gemini;
// Add new module declarations if they are not already in lib.rs or other central place
// For now, assuming these are direct files in src/ referenced by main or lib.rs
// If src/openai_llm.rs etc. are meant to be modules, they should be declared:
mod openai_llm;
mod gemini_llm;
mod local_llm_provider;
mod llm_provider;


#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d(%Y-%m-%d %H:%M:%S)} [{l}] - {m}\n")))
        .build("perspt.log")?;

    let log_config = Log4rsConfig::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(Root::builder().appender("logfile").build(LevelFilter::Info))?;

    log4rs::init_config(log_config)?;

    // Parse CLI arguments
    let matches = Command::new("LLM Chat CLI")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Path to the configuration file"),
        )
        .arg(
            Arg::new("api-key")
                .short('k')
                .long("api-key")
                .value_name("API_KEY")
                .help("API key to use for the provider"),
        )
        .arg(
            Arg::new("model-name")
                .short('m')
                .long("model-name")
                .value_name("MODEL/PATH") // Changed from MODEL
                .help("Model to use (e.g., gpt-4, or path to local model file if provider-type is local_llm)"),
        )
        .arg(
            Arg::new("provider") // This can be used to select a pre-configured provider profile from config.providers
                .short('p')
                .long("provider")
                .value_name("PROVIDER_PROFILE")
                .help("Choose a configured LLM provider profile (e.g., openai, gemini)"),
        )
        .arg(
            Arg::new("provider-type")
                .short('t')
                .long("provider-type")
                .value_name("TYPE")
                .help("Specify the type of LLM provider (e.g., openai, gemini, local_llm)"),
        )
         .arg(
            Arg::new("list-models")
                .long("list-models")
                .help("List available models for the provider")
                .action(clap::ArgAction::SetTrue)
        )
        .get_matches();

    let config_path = matches.get_one::<String>("config");
    let cli_api_key = matches.get_one::<String>("api-key");
    let cli_model_name = matches.get_one::<String>("model-name");
    let cli_provider_profile = matches.get_one::<String>("provider"); // Profile name like "openai_default"
    let cli_provider_type = matches.get_one::<String>("provider-type"); // Type like "openai", "local_llm"
    let list_models = matches.get_flag("list-models");

    // Load configuration
    let mut config = config::load_config(config_path).await?;

    // Apply CLI overrides
    if let Some(key) = cli_api_key {
        config.api_key = Some(key.clone());
        log::info!("Overriding API key from CLI.");
    }

    if let Some(ptype) = cli_provider_type {
        config.provider_type = Some(ptype.clone());
        log::info!("Overriding provider type from CLI: {}", ptype);
        // If local_llm is chosen via CLI, adjust default_provider to avoid URL lookup issues.
        // LocalLlmProvider uses config.default_model as path and doesn't need a provider URL.
        if ptype == "local_llm" {
            // Set default_provider to a non-API specific name.
            // This ensures that if other logic tries to use default_provider to get a URL, it won't pick an API one.
            config.default_provider = Some("local_llm_cli".to_string());
        }
    }
    
    if let Some(profile_name) = cli_provider_profile {
        // If --provider (profile) is specified, it might imply a provider_type
        // For example, if profile "openai_custom" is chosen, provider_type should be "openai".
        // This logic might need refinement based on how profiles in config.providers are structured.
        // For now, assume that if --provider is used, it sets both default_provider and implies provider_type.
        config.default_provider = Some(profile_name.clone());
        log::info!("Overriding default provider profile from CLI: {}", profile_name);
        // Infer provider_type from profile if not explicitly set by --provider-type
        if config.provider_type.is_none() {
            if profile_name.contains("openai") { config.provider_type = Some("openai".to_string()); }
            else if profile_name.contains("gemini") { config.provider_type = Some("gemini".to_string()); }
            // local_llm profiles might exist but are less common if path is primary identifier.
        }
    }

    if let Some(model_val) = cli_model_name {
        config.default_model = Some(model_val.clone());
        log::info!("Overriding model name/path from CLI: {}", model_val);
    }
    
    // Ensure config.default_provider is sensible if provider_type was set to local_llm by CLI
    // and no explicit --provider profile was given that might have already set it.
    if config.provider_type.as_deref() == Some("local_llm") {
        // If default_provider is still pointing to an API provider (e.g. from config file default),
        // change it to something generic for local_llm.
        if config.default_provider.as_deref() == Some("openai") || config.default_provider.as_deref() == Some("gemini") {
             config.default_provider = Some("local_llm_instance".to_string());
             log::info!("Adjusted default_provider to 'local_llm_instance' due to provider_type being 'local_llm'.");
        } else if config.default_provider.is_none() {
            config.default_provider = Some("local_llm_instance".to_string());
            log::info!("Set default_provider to 'local_llm_instance' for provider_type 'local_llm'.");
        }
    }


    // Final model_name to be used by the provider
    // For local_llm, this must be the path to the model file.
    // For API providers, this is the model identifier (e.g., "gpt-4").
    let model_name_for_provider = config.default_model.clone().unwrap_or_else(|| {
        match config.provider_type.as_deref() {
            Some("openai") => "gpt-3.5-turbo".to_string(),
            Some("gemini") => "gemini-pro".to_string(),
            Some("local_llm") => {
                log::warn!("Local LLM provider selected, but no model path provided via --model-name or in config.default_model. This will likely fail.");
                "".to_string() // Empty path will cause error in LocalLlmProvider
            }
            _ => "gemini-pro".to_string(), // Fallback default
        }
    });
    
    let api_key_string = config.api_key.clone().unwrap_or_default();

    // Create the LLM provider instance
    let provider: Box<dyn LLMProvider + Send + Sync> = match config.provider_type.as_deref() {
        Some("openai") => Box::new(OpenAIProviderLlm::new()),
        Some("gemini") => Box::new(GeminiProviderLlm::new()),
        Some("local_llm") => Box::new(LocalLlmProvider::new()),
        // Default if provider_type is None or unrecognized
        None | Some(_) => {
            log::warn!(
                "Provider type '{}' is not set or unrecognized. Defaulting to GeminiProviderLlm.",
                config.provider_type.as_deref().unwrap_or("not set")
            );
            // Before defaulting, ensure config reflects this choice if it was None
            if config.provider_type.is_none() {
                config.provider_type = Some("gemini".to_string());
                config.default_provider = Some("gemini".to_string()); // Assuming a "gemini" profile in providers map
            }
            Box::new(GeminiProviderLlm::new())
        }
    };

    if list_models {
        list_available_models(&provider, &config).await?;
        return Ok(());
    }

    // Initialize Ratatui
    let mut terminal = initialize_terminal()?;

    // Run the UI, passing the provider and the resolved model_name_for_provider
    run_ui(&mut terminal, config, model_name_for_provider, api_key_string, provider).await?;
    Ok(())
}

// list_available_models remains the same
async fn list_available_models(provider: &Box<dyn LLMProvider + Send + Sync>, _config: &AppConfig) -> Result<(), Box<dyn Error>> {
    // _config might be used if list_models needed details not in provider, but current trait doesn't pass it.
    match provider.list_models().await {
        Ok(models) => {
            if models.is_empty() {
                println!("No models listed by the provider, or model is pre-loaded (e.g., local model path).");
            } else {
                println!("Available models/info from provider:");
                for model_info in models {
                    println!("- {}", model_info);
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to list models: {}", e); // Use eprintln for errors
        }
    }
    Ok(())
}


fn initialize_terminal() -> Result<ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>, Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;
    terminal.clear()?;
    Ok(terminal)
}

// (Existing main, list_available_models, initialize_terminal functions are above this.
// The SEARCH block will start from the existing initiate_llm_request or where it should be inserted,
// and cover the existing handle_events function.)

// Ensure initiate_llm_request and truncate_message are defined as per the prompt.
// (If they exist from a previous step, they will be overwritten if different; if not, created)

async fn initiate_llm_request(
    app: &mut ui::App,
    input_to_send: String,
    provider: &(dyn LLMProvider + Send + Sync), 
    model_name: &str,
    tx_llm: &mpsc::UnboundedSender<String>,
) {
    app.is_llm_busy = true;
    app.is_input_disabled = true; // Visually disable input when a request starts

    // User message is added to chat_history in handle_events before this call.
    
    log::info!("Initiating LLM request for input: '{}'", input_to_send);
    app.set_status(format!("Sending: {}...", truncate_message(&input_to_send, 20)), false);

    let model_name_clone = model_name.to_string();
    let config_clone = app.config.clone(); 
    let tx_clone_for_provider = tx_llm.clone();

    tokio::spawn(async move {
        log::info!("LLM Task: Sending request for '{}' with model '{}'", input_to_send, model_name_clone);
        match provider.send_chat_request(&input_to_send, &model_name_clone, &config_clone, &tx_clone_for_provider).await {
            Ok(_) => {
                log::info!("LLM Task: Request for '{}' processed successfully by provider. Provider should send EOT.", input_to_send);
            }
            Err(err) => {
                log::error!("LLM Task: Error for input '{}': {}", input_to_send, err);
                // Provider should send EOT, but if it failed before that, we might need to ensure UI knows.
                // However, providers were modified to always send EOT. If an error occurs here,
                // it means the provider's send_chat_request itself returned Err.
                // The provider's EOT should still be sent in its own error handling logic.
                // We still send the error message to the UI via the channel.
                if tx_clone_for_provider.send(format!("Error: {}", err)).is_err() {
                    log::error!("LLM Task: Failed to send error to UI for input '{}'", input_to_send);
                }
                // Defensive EOT from here if provider might fail before its own EOT.
                // (Commented out as providers should handle their own EOT)
                // if tx_clone_for_provider.send(EOT_SIGNAL.to_string()).is_err() {
                //     log::error!("LLM Task: Failed to send defensive EOT after error for input '{}'", input_to_send);
                // }
            }
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

// Replacing the entire handle_events function with the new logic.
pub async fn handle_events(
    app: &mut ui::App,
    tx_llm: &mpsc::UnboundedSender<String>, 
    _api_key: &String, // Unused, as AppConfig within app is used by provider
    model_name: &String, // This is model_name_for_provider from main
    provider: &Box<dyn LLMProvider + Send + Sync>, 
) -> Option<AppEvent> {
    if let Ok(event) = event::read() {
        match event {
            Event::Key(key) => {
                if key.kind == KeyEventKind::Press {
                    // Global keybindings (Ctrl+C/D, Esc)
                    if key.modifiers.contains(KeyModifiers::CONTROL) && 
                       (key.code == KeyCode::Char('c') || key.code == KeyCode::Char('d')) {
                        app.should_quit = true;
                        return Some(AppEvent::Tick);
                    }
                    if key.code == KeyCode::Esc {
                        return Some(AppEvent::Key(key)); // Propagate Esc for run_ui to handle
                    }

                    // Handle text input keys (Char, Backspace, Enter)
                    // These are allowed even if app.is_llm_busy (for queuing)
                    // but not if app.is_input_disabled (active request processing).
                    match key.code {
                        KeyCode::Enter => {
                            if !app.input_text.is_empty() {
                                let current_input = app.input_text.drain(..).collect::<String>();
                                app.add_message(ui::ChatMessage {
                                    message_type: ui::MessageType::User,
                                    content: vec![ratatui::text::Line::from(ratatui::text::Span::styled(
                                        std::borrow::Cow::from(current_input.clone()),
                                        ratatui::style::Style::default().fg(ratatui::style::Color::Green),
                                    ))],
                                });

                                if app.is_llm_busy {
                                    log::info!("LLM is busy, queuing input: '{}'", current_input);
                                    app.pending_inputs.push_back(current_input);
                                    app.set_status(format!("Request queued. {} in queue.", app.pending_inputs.len()), false);
                                } else {
                                    // Not busy, so initiate the request immediately.
                                    // Pass &**provider to convert &Box<dyn T> to &dyn T
                                    initiate_llm_request(app, current_input, &**provider, model_name, tx_llm).await;
                                }
                                return Some(AppEvent::Tick); // Input processed
                            }
                        },
                        KeyCode::Char(c) => {
                            // Only allow typing if input is not visually/logically disabled by an active request
                            if !app.is_input_disabled {
                                app.input_text.push(c);
                            }
                        },
                        KeyCode::Backspace => {
                            if !app.is_input_disabled {
                                app.input_text.pop();
                            }
                        },
                        // Up and Down keys are handled by run_ui for scrolling if not captured here.
                        // If is_input_disabled is true, they are passed through.
                        _ => {} 
                    }
                }
                // Propagate all key events for other handlers (like scrolling in run_ui)
                return Some(AppEvent::Key(key));
            }
            _ => {} // Other event types like Mouse, Resize
        }
    }
    None // No event read
}
