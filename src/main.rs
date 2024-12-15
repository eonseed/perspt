use clap::{Arg, Command};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style, Modifier},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};
use reqwest::{Client, header, Response};
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
use pulldown_cmark::{Parser, Options, HeadingLevel};
use futures::StreamExt;

#[derive(Debug, Clone, Deserialize)]
struct AppConfig {
    providers: HashMap<String, String>,
    api_key: Option<String>,
    default_model: Option<String>,
    default_provider: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum MessageType {
    User,
    Assistant,
    Error,
}

#[derive(Debug, Clone)]
struct ChatMessage {
    message_type: MessageType,
    content: Vec<Line<'static>>,
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
                content: vec![Line::from(Span::styled(format!("System Error: {}", self.status_message), Style::default().fg(Color::Red)))],
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
                    msg.content.clone()
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
                    content: vec![Line::from(Span::styled(message.clone(), Style::default().fg(Color::Red)))],
                });
                app.set_status(message, true);
            } else {
                if let Some(last_message) = app.chat_history.last_mut() {
                     if last_message.message_type == MessageType::Assistant {
                        let styled_text = markdown_to_lines(&message);
                        last_message.content.extend(styled_text);
                     } else {
                         let styled_text = markdown_to_lines(&message);
                         app.add_message(ChatMessage {
                            message_type: MessageType::Assistant,
                            content: styled_text,
                        });
                     }
                } else {
                    let styled_text = markdown_to_lines(&message);
                    app.add_message(ChatMessage {
                        message_type: MessageType::Assistant,
                        content: styled_text,
                    });
                }
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
                                    content: vec![Line::from(Span::styled(input.clone(), Style::default().fg(Color::Green)))]
                                });
                                let tx_clone = tx.clone();
                                let api_url_clone = api_url.to_string();
                                let api_key_clone = api_key.to_string();
                                let model_name_clone = model_name.clone();
                                let input_clone = input.clone();
                                tokio::spawn(async move {
                                    match send_chat_request(&api_url_clone, &input_clone, &model_name_clone, &api_key_clone, &tx_clone).await {
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
    api_key: &str,
    tx: &UnboundedSender<String>
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
                stream_response(res, tx).await
            } else {
                let body = res.text().await.unwrap_or_else(|_| "No body".to_string());
                Err(format!("API Error: {} {}", status, body))
            }
        },
        Err(e) => Err(format!("Failed to send request: {}", e)),
    }
}

async fn stream_response(response: Response, tx: &UnboundedSender<String>) -> Result<String, String> {
    let mut stream = response.bytes_stream();
    let mut full_content = String::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(bytes) => {
                let chunk = String::from_utf8_lossy(&bytes).to_string();
                log::info!("Response Chunk: {}", chunk);
                let json_val: serde_json::Value = serde_json::from_str(&chunk)
                    .map_err(|e| format!("Failed to parse JSON: {}", e))?;
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
                 full_content.push_str(&content);
                 tx.send(content).expect("Failed to send response chunk");
            }
            Err(e) => {
                return Err(format!("Error reading response: {}", e));
            }
        }
    }
    Ok(full_content)
}


fn markdown_to_lines(markdown: &str) -> Vec<Line<'static>> {
    let parser = Parser::new_ext(markdown, Options::all());
    let mut lines = Vec::new();
    let mut current_line = Vec::new();

    for event in parser {
        match event {
            pulldown_cmark::Event::Text(text) => {
                current_line.push(Span::raw(text.to_string()));
            }
            pulldown_cmark::Event::Code(code) => {
                 current_line.push(Span::styled(code.to_string(), Style::default().fg(Color::Cyan)));
            }
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Paragraph) => {
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
            }
            pulldown_cmark::Event::End(pulldown_cmark::Tag::Paragraph) => {
                 if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
            }
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Heading { level, .. }) => {
                 if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
                let style = match level {
                    pulldown_cmark::HeadingLevel::H1 => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    pulldown_cmark::HeadingLevel::H2 => Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    pulldown_cmark::HeadingLevel::H3 => Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    _ => Style::default().add_modifier(Modifier::BOLD),
                };
                current_line.push(Span::styled(" ".to_string(), style));
            }
            pulldown_cmark::Event::End(pulldown_cmark::Tag::Heading(_)) => {
                 if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
            }
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::List(_)) => {
                 if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
            }
             pulldown_cmark::Event::End(pulldown_cmark::Tag::List) => {
                 if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
            }
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Item) => {
                 if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
                current_line.push(Span::raw("- ".to_string()));
            }
           pulldown_cmark::Event::End(pulldown_cmark::Tag::Item) => {
                 if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
            }
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::BlockQuote(_)) => {
                 if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
                current_line.push(Span::styled("> ".to_string(), Style::default().fg(Color::Gray)));
            }
            pulldown_cmark::Event::End(pulldown_cmark::Tag::BlockQuote) => {
                 if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
            }
            pulldown_cmark::Event::HardBreak => {
                 if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
            }
            pulldown_cmark::Event::Rule => {
                 if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
                lines.push(Line::from(Span::raw("---".to_string())));
            }
            _ => {}
        }
    }
     if !current_line.is_empty() {
        lines.push(Line::from(current_line));
    }
    lines
}
