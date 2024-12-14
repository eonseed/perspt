use clap::{Arg, Command};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};
use reqwest::{Client, header};
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::HashMap,
    error::Error,
    io,
    time::Duration,
};
use tokio::sync::mpsc::{self, UnboundedSender};
use crossterm::{
    event::{self, KeyCode, KeyEventKind, KeyModifiers, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::fs;
use log::LevelFilter;
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Config as Log4rsConfig, Root},
    encode::pattern::PatternEncoder,
};

#[derive(Debug, Clone, Deserialize)]
struct AppConfig {
    providers: HashMap<String, String>,
    api_key: Option<String>,
    default_model: Option<String>,
    default_provider: Option<String>,
}

#[derive(Debug, Clone)]
enum MessageType {
    User,
    Assistant,
    Error,
}

#[derive(Debug, Clone)]
struct ChatMessage {
    message_type: MessageType,
    content: String,
}

#[derive(Debug)]
enum AppEvent {
    Key(crossterm::event::KeyEvent),
    Tick,
}

struct App {
    chat_history: Vec<ChatMessage>,
    input_text: String,
    status_message: String,
    config: AppConfig,
    should_quit: bool,
}

impl App {
    fn new(config: AppConfig) -> Self {
        Self {
            chat_history: Vec::new(),
            input_text: String::new(),
            status_message: "Welcome to LLM Chat CLI".to_string(),
            config,
            should_quit: false,
        }
    }

    fn add_message(&mut self, message: ChatMessage) {
        self.chat_history.push(message);
    }

    fn set_status(&mut self, message: String, is_error: bool) {
        self.status_message = message;
        if is_error {
            self.add_message(ChatMessage {
                message_type: MessageType::Error,
                content: format!("System Error: {}", self.status_message),
            });
        }
    }
}

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
    let mut config = load_config(config_path).await?;
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
    let api_key = match api_key {
        Some(key) => key.clone(),
        None => config.api_key.clone().unwrap_or_default(),
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

async fn load_config(config_path: Option<&String>) -> Result<AppConfig, Box<dyn Error>> {
    let config: AppConfig = match config_path {
        Some(path) => {
            let config_str = fs::read_to_string(path)?;
            let config: AppConfig = serde_json::from_str(&config_str)?;
            config
        }
        None => {
            AppConfig {
                providers: {
                    let mut map = HashMap::new();
                     map.insert("gemini".to_string(), "https://generativelanguage.googleapis.com/v1beta".to_string());
                    map.insert("openai".to_string(), "https://api.openai.com/v1".to_string());
                    map
                },
                api_key: None,
                default_model: Some("gemini-pro".to_string()),
                default_provider: Some("gemini".to_string()),
            }
        }
    };
    Ok(config)
}

async fn list_available_models(config: &AppConfig) -> Result<(), Box<dyn Error>> {
    if let Some(provider) = &config.default_provider {
        let provider_url = config.providers.get(provider).ok_or("Invalid provider")?;
        let client = Client::new();
        let request_url = format!("{}/models", provider_url);
        let api_key = config.api_key.as_ref().ok_or("API Key is required")?;
        let request = client
            .get(&request_url)
            .header("Authorization", format!("Bearer {}", api_key));
        let response = request.send().await?;
        if response.status().is_success() {
            let body = response.text().await?;
            let json_value: serde_json::Value = serde_json::from_str(&body)?;
            if let Some(models) = json_value["models"].as_array() {
                println!("Available models:");
                for model in models {
                    if let Some(id) = model["name"].as_str() {
                        println!("- {}", id);
                    }
                }
            } else {
                println!("No models found");
            }
        } else {
            println!("Failed to fetch models: {}", response.status());
        }
    } else {
        println!("Please provide provider name");
    }
    Ok(())
}

fn initialize_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>, Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    Ok(terminal)
}

async fn run_ui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, config: AppConfig, model_name: String, api_key: String) -> Result<(), Box<dyn Error>> {
    let mut app = App::new(config);
    let (tx, mut rx) = mpsc::unbounded_channel();
    let provider = app.config.default_provider.clone().unwrap_or("gemini".to_string());
    let provider_url = app.config.providers.get(&provider)
        .map(|url| url.clone())
        .unwrap_or_default();
    let api_url = format!("{}", provider_url);
    log::info!("API URL: {}", api_url);
    log::info!("Model Name: {}", model_name);
    log::info!("API Key: {}", api_key);

    loop {
        terminal.draw(|f| {
            let size = f.size();
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(0),
                    Constraint::Length(3),
                    Constraint::Length(1),
                ])
                .split(size);

            // Chat history
            let chat_block = Block::default()
                .title("Chat History")
                .borders(Borders::ALL);
            let chat_lines: Vec<Line> = app.chat_history
                .iter()
                .flat_map(|msg| {
                    let mut lines = Vec::new();
                    let spans: Vec<Span> = msg.content.lines().map(|line|{
                        let style = match msg.message_type {
                            MessageType::User => Span::styled(line, Style::default().fg(Color::Green)),
                            MessageType::Assistant => Span::styled(line, Style::default().fg(Color::Blue)),
                            MessageType::Error => Span::styled(line, Style::default().fg(Color::Red)),
                        };
                        style
                    }).collect();
                    lines.push(Line::from(spans));
                    lines
                })
                .collect();
            let chat_paragraph = Paragraph::new(chat_lines)
                .block(chat_block)
                .wrap(Wrap { trim: true });
            f.render_widget(chat_paragraph, layout[0]);

            // User input
            let input_block = Block::default()
                .title("Input")
                .borders(Borders::ALL);
            let input_paragraph = Paragraph::new(app.input_text.clone())
                .block(input_block);
            f.render_widget(input_paragraph, layout[1]);

            // Status message
            let status_block = Block::default()
                .borders(Borders::NONE);
            let status_paragraph = Paragraph::new(app.status_message.clone())
                .block(status_block);
            f.render_widget(status_paragraph, layout[2]);
        })?;

        // Event handling
        if let Ok(Some(event)) = tokio::time::timeout(
            Duration::from_millis(50),
            handle_events(&mut app, &tx, &api_url, &api_key, &model_name)
        ).await {
            match event {
                AppEvent::Key(key_event) => {
                    if key_event.code == KeyCode::Esc {
                        app.should_quit = true;
                    }
                },
                _ => {}
            }
        }

        // Check for response messages
         while let Ok(message) = rx.try_recv() {
            if message.starts_with("Error:") {
                app.add_message(ChatMessage {
                    message_type: MessageType::Error,
                    content: message.clone(),
                });
                app.set_status(message, true);
            } else {
                app.add_message(ChatMessage {
                    message_type: MessageType::Assistant,
                    content: message,
                });
                app.set_status("Response received successfully".to_string(), false);
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

async fn handle_events(
    app: &mut App,
    tx: &UnboundedSender<String>,
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
                            if !app.input_text.is_empty() {
                                let input = app.input_text.clone();
                                app.add_message(ChatMessage{
                                    message_type: MessageType::User,
                                    content: input.clone()
                                });
                                let tx_clone = tx.clone();
                                let api_url_clone = api_url.to_string();
                                let api_key_clone = api_key.to_string();
                                let model_name_clone = model_name.clone();
                                let input_clone = input.clone();
                                let input_clone = input.clone();
                                let tx_clone = tx.clone();
                                let api_url_clone = api_url.to_string();
                                let api_key_clone = api_key.to_string();
                                let model_name_clone = model_name.clone();
                                tokio::spawn(async move {
                                    match send_chat_request(&api_url_clone, &input_clone, &model_name_clone, &api_key_clone).await {
                                        Ok(response) => {
                                            tx_clone.send(response).expect("Failed to send response");
                                        },
                                        Err(err) => {
                                             tx_clone.send(format!("Error: {}", err)).expect("Failed to send error");
                                        }
                                    }
                                });
                                app.input_text.clear();
                                return Some(AppEvent::Tick);
                            }
                        },
                        KeyCode::Char(c) => {
                            if key.modifiers.contains(KeyModifiers::CONTROL) {
                                match c {
                                    'c' | 'd' => {
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

async fn send_chat_request(
    api_url: &str,
    input: &str,
    model_name: &str,
    api_key: &str
) -> Result<String, String> {
    let client = Client::new();
    let request_url = format!("{}/models/{}:generateContent", api_url, model_name);
    log::info!("Request URL: {}", request_url);
    let request_payload = json!({
        "contents": [{
            "parts": [{
                "text": input
            }]
        }]
    });
    log::info!("Request Payload: {}", request_payload.to_string());

    let request_url = format!("{}?key={}", request_url, api_key);
    let request = client.post(request_url)
        .header(header::CONTENT_TYPE, "application/json")
        .json(&request_payload);

    let response = request.send().await;
    match response {
        Ok(res) => {
            let status = res.status();
            log::info!("Response Status: {}", status);
            if status.is_success() {
                let body = res.text().await
                    .map_err(|e| format!("Error reading response: {}", e))?;
                log::info!("Response Body: {}", body);
                // Parse Gemini's response JSON
                let json_val: serde_json::Value = serde_json::from_str(&body)
                    .map_err(|e| format!("Failed to parse JSON: {}", e))?;
                // Extract text from Gemini's response structure
                 let content = json_val
                    .get("candidates")
                    .and_then(|candidates| candidates.get(0))
                    .and_then(|candidate| candidate.get("content"))
                     .and_then(|content| content.get("parts"))
                     .and_then(|parts| parts.get(0))
                     .and_then(|part| part.get("text"))
                    .and_then(|text| text.as_str())
                    .unwrap_or("No response")
                    .to_string();

                Ok(content)
            } else {
                let body = res.text().await.unwrap_or_else(|_| "No body".to_string());
                Err(format!("API Error: {} {}", status, body))
            }
        },
        Err(e) => Err(format!("Failed to send request: {}", e)),
    }
}
