use clap::{Arg, Command};
use std::error::Error;
use std::io;
use tokio::sync::mpsc;
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
use crate::openai::OpenAIProvider;
use crate::gemini::GeminiProvider;
use tokio::sync::watch;

mod ui;
mod config;
mod openai;
mod gemini;

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
                .value_name("MODEL")
                .help("Model to use (e.g., gpt-4)"),
        )
        .arg(
            Arg::new("provider")
                .short('p')
                .long("provider")
                .value_name("PROVIDER")
                .help("Choose the LLM provider (e.g., openai, gemini)"),
        )
         .arg(
            Arg::new("list-models")
                .long("list-models")
                .help("List available models for the provider")
                .action(clap::ArgAction::SetTrue)
        )
        .get_matches();

    let config_path = matches.get_one::<String>("config");
    let api_key = matches.get_one::<String>("api-key");
    let model_name = matches.get_one::<String>("model-name");
    let provider_name = matches.get_one::<String>("provider");
    let list_models = matches.get_flag("list-models");

    // Load configuration
    let mut config = config::load_config(config_path).await?;

    if let Some(key) = api_key {
        config.api_key = Some(key.clone());
    }
    if let Some(provider) = provider_name {
         config.default_provider = Some(provider.clone());
    }
    let model_name = match model_name {
         Some(model) => model.clone(),
        None => config.default_model.clone().unwrap_or("gemini-pro".to_string()),
    };
    let api_key = match &config.api_key {
        Some(key) => key.clone(),
        None => String::new(),
    };

    if list_models {
        list_available_models(&config).await?;
        return Ok(());
    }

    // Initialize Ratatui
    let mut terminal = initialize_terminal()?;

    // Run the UI
    run_ui(&mut terminal, config, model_name, api_key).await?;
    Ok(())
}

async fn list_available_models(config: &AppConfig) -> Result<(), Box<dyn Error>> {
    if let Some(provider) = &config.default_provider {
        let provider_url = config.providers.get(provider).ok_or("Invalid provider")?;
         let api_key = config.api_key.as_ref().ok_or("API Key is required")?;
        match provider.as_str() {
            "openai" => {
                let openai_provider = OpenAIProvider::new(provider_url, api_key.clone());
                openai_provider.list_models().await?;
            }
            "gemini" => {
                 let gemini_provider = GeminiProvider::new(provider_url, api_key.clone());
                 gemini_provider.list_models().await?;
            }
            _ => {
                println!("Unsupported provider: {}", provider);
            }
        }
    } else {
        println!("Please provide provider name");
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

pub async fn handle_events(
    app: &mut ui::App,
    tx: &mpsc::UnboundedSender<String>,
    api_url: &str,
    api_key: &String,
    model_name: &String
) -> Option<AppEvent> {
    if let Ok(event) = event::read() {
        match event {
            Event::Key(key) => {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Enter => {
                            if !app.input_text.is_empty() && !app.is_processing {
                                app.is_processing = true;
                                let input = app.input_text.clone();
                                 app.add_message(ui::ChatMessage{
                                    message_type: ui::MessageType::User,
                                    content: vec![ratatui::text::Line::from(ratatui::text::Span::styled(std::borrow::Cow::from(input.clone()), ratatui::style::Style::default().fg(ratatui::style::Color::Green)))]
                                });
                                let tx_clone = tx.clone();
                                let api_url_clone = api_url.to_string();
                                let api_key_clone = api_key.clone();
                                let model_name_clone = model_name.clone();
                                let input_clone = input.clone();
                                 let provider = app.config.default_provider.clone().unwrap_or_else(|| "gemini".to_string());

                                let (interrupt_tx, interrupt_rx) = watch::channel(false);
                                tokio::spawn(async move {
                                    match provider.as_str() {
                                        "openai" => {
                                              let openai_provider = OpenAIProvider::new(&api_url_clone, api_key_clone);
                                              match openai_provider.send_chat_request(&input_clone, &model_name_clone, &tx_clone, &interrupt_rx).await {
                                                    Ok(_) => {},
                                                    Err(err) => {
                                                        tx_clone.send(format!("Error: {}", err)).expect("Failed to send error");
                                                    }
                                               };
                                        }
                                        "gemini" => {
                                             let gemini_provider = GeminiProvider::new(&api_url_clone, api_key_clone);
                                             match gemini_provider.send_chat_request(&input_clone, &model_name_clone, &tx_clone, &interrupt_rx).await {
                                                    Ok(_) => {},
                                                    Err(err) => {
                                                          tx_clone.send(format!("Error: {}", err)).expect("Failed to send error");
                                                    }
                                               };
                                        }
                                        _ => {
                                            tx_clone.send(format!("Error: Unsupported provider: {}", provider)).expect("Failed to send error");
                                        }
                                    }
                                    app.is_processing = false;
                                });
                                app.input_text.clear();
                                return Some(AppEvent::Tick);
                            }
                        },
                        KeyCode::Char(c) => {
                            if key.modifiers.contains(KeyModifiers::CONTROL) {
                                match c {
                                    'c' => {
                                        if app.is_processing {
                                            interrupt_tx.send(true).expect("Failed to send interrupt signal");
                                            app.is_processing = false;
                                            return Some(AppEvent::Tick);
                                        } else {
                                            app.should_quit = true;
                                            return Some(AppEvent::Tick);
                                        }
                                    },
                                    'd' => {
                                        app.should_quit = true;
                                        return Some(AppEvent::Tick);
                                    },
                                    _=> app.input_text.push(c),
                                }
                            } else {
                                app.input_text.push(c);
                            }
                        },
                        KeyCode::Backspace => {
                            app.input_text.pop();
                        },
                         KeyCode::Esc => {
                             return Some(AppEvent::Key(key));
                        },
                        _ => {}
                    }
                }
                return  Some(AppEvent::Key(key));
            },
             _ => {}
        }
    }
    None
}
