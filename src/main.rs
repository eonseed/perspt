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
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashMap,
    error::Error,
    io,
};
use tokio::sync::mpsc::{self, UnboundedSender, UnboundedReceiver};
use crossterm::{
    event::{self, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::fs;
use futures_util::stream::StreamExt;


#[derive(Serialize, Deserialize, Debug, Clone)]
struct Config {
    providers: HashMap<String, String>,
    api_key: Option<String>,
    default_model: Option<String>,
    default_provider: Option<String>,
}

#[derive(Debug, Clone)]
enum MessageType {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone)]
struct ChatMessage {
    message_type: MessageType,
    content: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Parse CLI arguments
    let matches = Command::new("LLM Chat CLI")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Path to the configuration file")
        )
         .arg(
            Arg::new("api-key")
            .short('k')
            .long("api-key")
            .value_name("API_KEY")
            .help("API key to use for the provider")
        )
        .arg(
            Arg::new("model-name")
                .short('m')
                .long("model-name")
                .value_name("MODEL")
                .help("Model to use (e.g., gpt-4)")
        )
        .arg(
            Arg::new("provider")
            .short('p')
            .long("provider")
            .value_name("PROVIDER")
            .help("Choose the LLM provider (e.g., openai, gemini)")
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
    
    if let Some(model) = model_name {
        config.default_model = Some(model.clone());
    }
    
    if let Some(provider) = provider_name {
         config.default_provider = Some(provider.clone());
    }

    if list_models {
        list_available_models(&config).await?;
        return Ok(());
    }

    // Initialize Ratatui
    let mut terminal = initialize_terminal()?;

    // Run the UI
    run_ui(&mut terminal, config).await;
    Ok(())
}

async fn load_config(config_path: Option<&String>) -> Result<Config, Box<dyn Error>> {
     let config: Config = match config_path {
        Some(path) => {
             let config_str = fs::read_to_string(path)?;
            let config: Config = serde_json::from_str(&config_str)?;
            config
        }
        None => {
           Config {
                providers: {
                     let mut map = HashMap::new();
                     map.insert("gemini".to_string(), "https://generativelanguage.googleapis.com/v1beta".to_string());
                     map.insert("openai".to_string(), "https://api.openai.com/v1".to_string());
                     map
                },
                api_key: None,
                default_model: None,
                default_provider: Some("openai".to_string()),
            }
        }
     };
    Ok(config)
}

async fn list_available_models(config: &Config) -> Result<(), Box<dyn Error>> {
    if let Some(provider) = &config.default_provider {
        let provider_url = config.providers.get(provider).ok_or("Invalid provider")?;
        let client = Client::new();
        let request_url = format!("{}/models", provider_url);
       let  api_key = config.api_key.as_ref().ok_or("API Key is required")?;
        let request = client
            .get(&request_url)
            .header("Authorization", format!("Bearer {}", api_key));
        let response = request.send().await?;
        if response.status().is_success() {
            let body = response.text().await?;
            let json_value: serde_json::Value = serde_json::from_str(&body)?;
            if let Some(models) = json_value["data"].as_array() {
                println!("Available models:");
                for model in models {
                    if let Some(id) = model["id"].as_str() {
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

async fn run_ui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, config: Config) {
    let mut chat_history: Vec<ChatMessage> = Vec::new();
    let mut input_text = String::new();
    let status_message = String::new();

    let (tx, mut rx): (UnboundedSender<String>, UnboundedReceiver<String>) = mpsc::unbounded_channel();
    
     let api_key = config.api_key.clone().unwrap_or("".to_string());
    let model_name = config.default_model.clone().unwrap_or("gpt-3.5-turbo".to_string());
    let provider = config.default_provider.clone().unwrap_or("openai".to_string());
    
    let provider_url = config.providers.get(&provider).map(|url| url.clone()).unwrap_or("".to_string());
    
    let api_url = format!("{}/chat/completions", provider_url);

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
            let chat_lines: Vec<Line> = chat_history
                .iter()
                .flat_map(|msg| {
                    let mut lines = Vec::new();
                    let spans: Vec<Span> = msg.content.lines().map(|line|{
                        let style = match msg.message_type {
                            MessageType::User => Style::default().fg(Color::Green),
                            MessageType::Assistant => Style::default().fg(Color::Blue),
                            MessageType::System => Style::default().fg(Color::Red),
                        };
                         Span::styled(format!("{} ", line), style)
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
            let input_paragraph = Paragraph::new(input_text.clone())
                .block(input_block);
            f.render_widget(input_paragraph, layout[1]);

            // Status message
            let status_block = Block::default()
                .borders(Borders::NONE);
             let status_paragraph = Paragraph::new(status_message.clone()).block(status_block);
            f.render_widget(status_paragraph, layout[2]);
        }).unwrap();

        if let Ok(event::Event::Key(key)) = event::read() {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Enter => {
                        let input = input_text.drain(..).collect::<String>();
                        chat_history.push(ChatMessage{message_type: MessageType::User, content: format!("User: {}\n", input)});
                        let tx_clone = tx.clone();
                        let api_url_clone = api_url.clone();
                        let api_key_clone = api_key.clone();
                        let model_name_clone = model_name.clone();
                        tokio::spawn(async move {
                            match stream_chat(&api_url_clone, &input, &model_name_clone, &api_key_clone).await {
                                Ok(res) => {
                                    tx_clone.send(format!("Assistant: {}\n", res)).expect("Failed to send response");
                                },
                                Err(err) => {
                                     tx_clone.send(format!("Error: {}\n", err)).expect("Failed to send error");
                                },
                            }
                        });
                    },
                    KeyCode::Char(c) => {
                        input_text.push(c);
                    },
                    KeyCode::Backspace => {
                        input_text.pop();
                    }
                    KeyCode::Esc => {
                         disable_raw_mode().unwrap();
                         execute!(terminal.backend_mut(), LeaveAlternateScreen).unwrap();
                         terminal.show_cursor().unwrap();
                         break;
                    }
                    _ => {}
                }
            }
        }

        //Handle response message
        match rx.try_recv() {
           Ok(message) => {
                let message_type = if message.starts_with("Error:") {
                  MessageType::System
                } else {
                   MessageType::Assistant
                };
                chat_history.push(ChatMessage{message_type, content: message});
            },
            Err(_) => {}
        }
    }
}

async fn stream_chat(api_url: &str, input: &str, model_name: &str, api_key: &str) -> Result<String, String> {
    let client = Client::new();
    let request = client.post(api_url)
        .header(header::AUTHORIZATION, format!("Bearer {}", api_key))
        .header(header::CONTENT_TYPE, "application/json")
        .json(&json!({
            "model": model_name,
            "messages": [{
                "role": "user",
                "content": input
            }],
             "stream": true
        }));

    let response = request.send().await;
     match response {
        Ok(res) => {
            let status = res.status();
            if status.is_success() {
                  let  mut stream = res.bytes_stream();
                  let mut content = String::new();
                    while let Some(chunk) = stream.next().await {
                         match chunk {
                              Ok(bytes) => {
                                  if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                                        let lines: Vec<&str> = text.split("\n").collect();
                                        for line in lines {
                                            if line.starts_with("data: ") && !line.contains("[DONE]") {
                                                 let json_str = line.replace("data: ", "");
                                                 if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                                                    if let Some(choices) = json_val.get("choices") {
                                                        if let Some(delta) = choices.get(0).and_then(|c| c.get("delta")) {
                                                            if let Some(text) = delta.get("content") {
                                                                if let Some(text_str) = text.as_str() {
                                                                  content.push_str(text_str);
                                                                }
                                                            }
                                                        }
                                                    }
                                               } else {
                                                   eprintln!("Could not parse json: {}", json_str);
                                               }
                                            }
                                        }
                                  } else {
                                     eprintln!("Could not convert bytes to utf8")
                                  }
                              },
                                Err(e) => {
                                  eprintln!("Error streaming response: {}", e)
                                },
                            }
                    }
                 Ok(content)
            } else {
               let body = res.text().await.unwrap_or_else(|_| "No body".to_string());
               Err(format!("API Error: {} {}", status, body))
           }
        },
        Err(e) => Err(format!("Failed to send request: {}", e)),
    }
}