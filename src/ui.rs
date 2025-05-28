// src/ui.rs
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Alignment},
    style::{Color, Style, Stylize, Modifier},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap, ScrollbarState, BorderType, Clear, Gauge},
    Terminal, Frame,
};
use std::{collections::VecDeque, io, time::Duration, sync::Arc};
use anyhow::Result;

use crate::config::AppConfig;
use crate::llm_provider::LLMProvider;
use tokio::sync::mpsc;
use pulldown_cmark::{Parser, Options, Tag, Event as MarkdownEvent, TagEnd};
use crossterm::event::KeyEvent;

#[derive(Debug, Clone, PartialEq)]
pub enum MessageType {
    User,
    Assistant,
    Error,
    System,
    Warning,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub message_type: MessageType,
    pub content: Vec<Line<'static>>,
    pub timestamp: String,
}

#[derive(Debug)]
pub enum AppEvent {
    Key(KeyEvent),
    Tick,
}

#[derive(Debug, Clone)]
pub struct ErrorState {
    pub message: String,
    pub details: Option<String>,
    pub error_type: ErrorType,
}

#[derive(Debug, Clone)]
pub enum ErrorType {
    Network,
    Authentication,
    RateLimit,
    InvalidModel,
    ServerError,
    Unknown,
}

pub struct App {
    pub chat_history: Vec<ChatMessage>,
    pub input_text: String,
    pub status_message: String,
    pub config: AppConfig,
    pub should_quit: bool,
    scroll_state: ScrollbarState,
    pub scroll_position: usize,
    pub is_input_disabled: bool, 
    pub pending_inputs: VecDeque<String>,
    pub is_llm_busy: bool,
    pub current_error: Option<ErrorState>,
    pub show_help: bool,
    pub typing_indicator: String,
    pub response_progress: f64,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        let welcome_msg = ChatMessage {
            message_type: MessageType::System,
            content: vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("🌟 Welcome to ", Style::default().fg(Color::Cyan)),
                    Span::styled("Perspt", Style::default().fg(Color::Magenta).bold()),
                    Span::styled(" - Your AI Chat Terminal", Style::default().fg(Color::Cyan)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("💡 Quick Help:", Style::default().fg(Color::Yellow).bold()),
                ]),
                Line::from(vec![
                    Span::styled("  • ", Style::default().fg(Color::Green)),
                    Span::styled("Enter", Style::default().fg(Color::White).bold()),
                    Span::styled(" - Send message", Style::default().fg(Color::Gray)),
                ]),
                Line::from(vec![
                    Span::styled("  • ", Style::default().fg(Color::Green)),
                    Span::styled("↑/↓", Style::default().fg(Color::White).bold()),
                    Span::styled(" - Scroll chat history", Style::default().fg(Color::Gray)),
                ]),
                Line::from(vec![
                    Span::styled("  • ", Style::default().fg(Color::Green)),
                    Span::styled("Ctrl+C/Ctrl+Q", Style::default().fg(Color::White).bold()),
                    Span::styled(" - Exit", Style::default().fg(Color::Gray)),
                ]),
                Line::from(vec![
                    Span::styled("  • ", Style::default().fg(Color::Green)),
                    Span::styled("F1", Style::default().fg(Color::White).bold()),
                    Span::styled(" - Toggle help", Style::default().fg(Color::Gray)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Ready to chat! Type your message below...", Style::default().fg(Color::Green).italic()),
                ]),
                Line::from(""),
            ],
            timestamp: Self::get_timestamp(),
        };

        Self {
            chat_history: vec![welcome_msg],
            input_text: String::new(),
            status_message: "Ready".to_string(),
            config,
            should_quit: false,
            scroll_state: ScrollbarState::default(),
            scroll_position: 0,
            is_input_disabled: false,
            pending_inputs: VecDeque::new(),
            is_llm_busy: false,
            current_error: None,
            show_help: false,
            typing_indicator: String::new(),
            response_progress: 0.0,
        }
    }

    pub fn get_timestamp() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // Format as HH:MM
        let hours = (timestamp / 3600) % 24;
        let minutes = (timestamp / 60) % 60;
        format!("{:02}:{:02}", hours, minutes)
    }

    pub fn add_message(&mut self, mut message: ChatMessage) {
        message.timestamp = Self::get_timestamp();
        self.chat_history.push(message);
        self.scroll_to_bottom();
    }

    pub fn add_error(&mut self, error: ErrorState) {
        self.current_error = Some(error.clone());
        
        let error_content = vec![
            Line::from(vec![
                Span::styled("❌ Error: ", Style::default().fg(Color::Red).bold()),
                Span::styled(error.message.clone(), Style::default().fg(Color::Red)),
            ]),
        ];

        let mut full_content = error_content;
        if let Some(details) = &error.details {
            full_content.push(Line::from(vec![
                Span::styled("   Details: ", Style::default().fg(Color::Yellow)),
                Span::styled(details.clone(), Style::default().fg(Color::Gray)),
            ]));
        }

        self.add_message(ChatMessage {
            message_type: MessageType::Error,
            content: full_content,
            timestamp: Self::get_timestamp(),
        });
    }

    pub fn clear_error(&mut self) {
        self.current_error = None;
    }

    pub fn set_status(&mut self, message: String, is_error: bool) {
        self.status_message = message;
        if is_error {
            log::error!("Status error: {}", self.status_message);
        }
    }

    pub fn update_typing_indicator(&mut self) {
        if self.is_llm_busy {
            let indicators = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis();
            let index = (current_time / 100) % indicators.len() as u128;
            self.typing_indicator = indicators[index as usize].to_string();
        } else {
            self.typing_indicator.clear();
        }
    }

    pub fn scroll_up(&mut self) {
        if self.scroll_position > 0 {
            self.scroll_position -= 1;
            self.update_scroll_state();
        }
    }

    pub fn scroll_down(&mut self) {
        if self.scroll_position < self.max_scroll() {
            self.scroll_position += 1;
            self.update_scroll_state();
        }
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_position = self.max_scroll();
        self.update_scroll_state();
    }

    fn max_scroll(&self) -> usize {
        let content_height: usize = self.chat_history
            .iter()
            .flat_map(|msg| msg.content.iter())
            .count();
        if content_height > 0 {
            content_height.saturating_sub(1)
        } else {
            0
        }
    }

    pub fn update_scroll_state(&mut self) {
        let _max_scroll = self.max_scroll();
        self.scroll_state = self.scroll_state.position(self.scroll_position);
    }
}

pub async fn run_ui(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, 
    config: AppConfig,
    model_name: String, 
    api_key: String,
    provider: Arc<dyn LLMProvider + Send + Sync>
) -> Result<()> {
    let mut app = App::new(config);
    let (tx, mut rx) = mpsc::unbounded_channel();
    
    log::info!("Starting UI with model: {}", model_name);

    loop {
        // Update typing indicator animation
        app.update_typing_indicator();

        // Draw UI
        terminal.draw(|f| {
            draw_ui(f, &mut app, &model_name);
        })?;

        // Handle events with timeout
        if let Ok(Some(event)) = tokio::time::timeout(
            Duration::from_millis(100),
            crate::handle_events(&mut app, &tx, &api_key, &model_name, &provider)
        ).await {
            match event {
                AppEvent::Key(_) => {
                    // Event handled in handle_events
                }
                AppEvent::Tick => {
                    // Periodic update
                }
            }
        }

        // Process LLM responses
        while let Ok(message) = rx.try_recv() {
            if message == crate::EOT_SIGNAL {
                // End of response
                app.is_llm_busy = false;
                app.is_input_disabled = false;
                app.response_progress = 0.0;
                app.set_status("Ready".to_string(), false);
                app.clear_error();
                
                // Process any pending inputs
                if let Some(pending_input) = app.pending_inputs.pop_front() {
                    log::info!("Processing pending input: {}", pending_input);
                    
                    // Add user message to chat history
                    app.add_message(ChatMessage {
                        message_type: MessageType::User,
                        content: vec![Line::from(pending_input.clone())],
                        timestamp: App::get_timestamp(),
                    });

                    // Start LLM request for pending input
                    crate::initiate_llm_request(&mut app, pending_input, Arc::clone(&provider), &model_name, &tx).await;
                }
            } else if message.starts_with("Error: ") {
                // Parse and categorize the error
                let error_msg = &message[7..]; // Remove "Error: " prefix
                let error_state = categorize_error(error_msg);
                app.add_error(error_state);
                app.is_llm_busy = false;
                app.is_input_disabled = false;
                app.response_progress = 0.0;
                app.set_status("Error occurred".to_string(), true);
            } else {
                // Regular response token
                if app.chat_history.is_empty() || 
                   app.chat_history.last().unwrap().message_type != MessageType::Assistant {
                    // Start new assistant message
                    app.add_message(ChatMessage {
                        message_type: MessageType::Assistant,
                        content: vec![Line::from("")],
                        timestamp: App::get_timestamp(),
                    });
                }

                // Append to last assistant message
                if let Some(last_msg) = app.chat_history.last_mut() {
                    if last_msg.message_type == MessageType::Assistant {
                        if let Some(last_line) = last_msg.content.last_mut() {
                            // Append to existing line
                            let mut current_text = String::new();
                            for span in &last_line.spans {
                                current_text.push_str(&span.content);
                            }
                            current_text.push_str(&message);
                            
                            // Replace with updated content using markdown rendering
                            let rendered_lines = markdown_to_lines(&current_text);
                            last_msg.content = rendered_lines;
                        }
                    }
                }
                
                // Update progress indicator
                app.response_progress = (app.response_progress + 0.1).min(1.0);
                app.scroll_to_bottom();
                app.set_status(format!("{}  Receiving response...", app.typing_indicator), false);
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), crossterm::terminal::LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

fn draw_ui(f: &mut Frame, app: &mut App, model_name: &str) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(1),     // Chat area
            Constraint::Length(4),  // Input area
            Constraint::Length(2),  // Status line
        ])
        .split(f.area());

    // Header with model info and status
    draw_header(f, main_chunks[0], model_name, app);
    
    // Chat history
    draw_chat_area(f, main_chunks[1], app);
    
    // Input area
    draw_input_area(f, main_chunks[2], app);
    
    // Status line
    draw_status_line(f, main_chunks[3], app);

    // Help overlay if needed
    if app.show_help {
        draw_help_overlay(f, app);
    }
}

fn draw_header(f: &mut Frame, area: ratatui::layout::Rect, model_name: &str, app: &App) {
    let header_content = vec![
        Line::from(vec![
            Span::styled("🧠 ", Style::default().fg(Color::Magenta)),
            Span::styled("Perspt", Style::default().fg(Color::Magenta).bold()),
            Span::styled(" | Model: ", Style::default().fg(Color::Gray)),
            Span::styled(model_name, Style::default().fg(Color::Cyan).bold()),
            Span::styled(" | Status: ", Style::default().fg(Color::Gray)),
            if app.is_llm_busy {
                Span::styled("Thinking...", Style::default().fg(Color::Yellow).italic())
            } else {
                Span::styled("Ready", Style::default().fg(Color::Green).bold())
            },
        ]),
    ];

    let header = Paragraph::new(header_content)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .title("┤ AI Chat Terminal ├")
            .title_style(Style::default().fg(Color::Magenta).bold()))
        .alignment(Alignment::Center);

    f.render_widget(header, area);
}

fn draw_chat_area(f: &mut Frame, area: ratatui::layout::Rect, app: &mut App) {
    let chat_content: Vec<Line> = app.chat_history
        .iter()
        .flat_map(|msg| {
            let (prefix, style) = match msg.message_type {
                MessageType::User => ("👤 You", Style::default().fg(Color::Blue).bold()),
                MessageType::Assistant => ("🤖 Assistant", Style::default().fg(Color::Green).bold()),
                MessageType::Error => ("❌ Error", Style::default().fg(Color::Red).bold()),
                MessageType::System => ("ℹ️ System", Style::default().fg(Color::Cyan).bold()),
                MessageType::Warning => ("⚠️ Warning", Style::default().fg(Color::Yellow).bold()),
            };
            
            let mut lines = vec![
                Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(format!(" ({})", msg.timestamp), Style::default().fg(Color::DarkGray)),
                ]),
            ];
            
            lines.extend(msg.content.iter().cloned());
            lines.push(Line::from(""));
            lines
        })
        .collect();

    let chat_paragraph = Paragraph::new(chat_content)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::White))
            .title("┤ Conversation ├")
            .title_style(Style::default().fg(Color::White).bold()))
        .wrap(Wrap { trim: true })
        .scroll((app.scroll_position as u16, 0));

    f.render_widget(chat_paragraph, area);
}

fn draw_input_area(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let input_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Input field
            Constraint::Length(1),  // Progress bar (if busy)
        ])
        .split(area);

    // Input field styling
    let (input_color, input_style, title) = if app.is_input_disabled {
        (
            Color::DarkGray,
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            "┤ Input (Disabled - AI is thinking...) ├"
        )
    } else {
        (
            Color::White,
            Style::default().fg(Color::White),
            "┤ Type your message (Enter to send, F1 for help) ├"
        )
    };

    let input_text = if app.is_input_disabled && app.input_text.is_empty() {
        "Waiting for AI response..."
    } else {
        &app.input_text
    };

    let input_paragraph = Paragraph::new(input_text)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(input_color))
            .title(title)
            .title_style(Style::default().fg(input_color)))
        .style(input_style)
        .wrap(Wrap { trim: false });

    f.render_widget(input_paragraph, input_chunks[0]);

    // Progress bar when AI is working
    if app.is_llm_busy {
        let progress = Gauge::default()
            .block(Block::default().borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM))
            .gauge_style(Style::default().fg(Color::Yellow))
            .ratio(app.response_progress)
            .label(format!("{}  Processing...", app.typing_indicator));
        
        f.render_widget(progress, input_chunks[1]);
    }
}

fn draw_status_line(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let status_content = if let Some(error) = &app.current_error {
        vec![Line::from(vec![
            Span::styled("❌ ", Style::default().fg(Color::Red)),
            Span::styled(error.message.clone(), Style::default().fg(Color::Red)),
            Span::styled(" | Press F1 for help", Style::default().fg(Color::Gray)),
        ])]
    } else {
        let queue_info = if !app.pending_inputs.is_empty() {
            format!(" | Queued: {}", app.pending_inputs.len())
        } else {
            String::new()
        };

        vec![Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::Gray)),
            Span::styled(app.status_message.clone(), 
                if app.is_llm_busy { 
                    Style::default().fg(Color::Yellow) 
                } else { 
                    Style::default().fg(Color::Green) 
                }),
            Span::styled(queue_info, Style::default().fg(Color::Blue)),
            Span::styled(" | Ctrl+C to exit", Style::default().fg(Color::Gray)),
        ])]
    };

    let status_paragraph = Paragraph::new(status_content)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Gray)));

    f.render_widget(status_paragraph, area);
}

fn draw_help_overlay(f: &mut Frame, _app: &App) {
    let popup_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(f.area())[1];

    let popup_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(popup_area)[1];

    f.render_widget(Clear, popup_area);

    let help_content = vec![
        Line::from(""),
        Line::from(vec![Span::styled("📖 Help & Shortcuts", Style::default().fg(Color::Magenta).bold())]),
        Line::from(""),
        Line::from(vec![Span::styled("Navigation:", Style::default().fg(Color::Yellow).bold())]),
        Line::from(vec![Span::styled("  ↑/↓     ", Style::default().fg(Color::Cyan)), Span::styled("Scroll chat history", Style::default())]),
        Line::from(""),
        Line::from(vec![Span::styled("Input:", Style::default().fg(Color::Yellow).bold())]),
        Line::from(vec![Span::styled("  Enter   ", Style::default().fg(Color::Cyan)), Span::styled("Send message", Style::default())]),
        Line::from(vec![Span::styled("  F1      ", Style::default().fg(Color::Cyan)), Span::styled("Toggle this help", Style::default())]),
        Line::from(""),
        Line::from(vec![Span::styled("Exit:", Style::default().fg(Color::Yellow).bold())]),
        Line::from(vec![Span::styled("  Ctrl+C  ", Style::default().fg(Color::Cyan)), Span::styled("Exit application", Style::default())]),
        Line::from(vec![Span::styled("  Ctrl+Q  ", Style::default().fg(Color::Cyan)), Span::styled("Exit application", Style::default())]),
        Line::from(""),
        Line::from(vec![Span::styled("Features:", Style::default().fg(Color::Yellow).bold())]),
        Line::from(vec![Span::styled("  • Input queuing while AI responds", Style::default().fg(Color::Green))]),
        Line::from(vec![Span::styled("  • Markdown rendering support", Style::default().fg(Color::Green))]),
        Line::from(vec![Span::styled("  • Automatic scrolling", Style::default().fg(Color::Green))]),
        Line::from(""),
        Line::from(vec![Span::styled("Press F1 again to close", Style::default().fg(Color::Gray).italic())]),
    ];

    let help_popup = Paragraph::new(help_content)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(Color::Magenta))
            .title("┤ Help ├")
            .title_style(Style::default().fg(Color::Magenta).bold()))
        .wrap(Wrap { trim: true });

    f.render_widget(help_popup, popup_area);
}

fn categorize_error(error_msg: &str) -> ErrorState {
    let error_lower = error_msg.to_lowercase();
    
    let (error_type, message, details) = if error_lower.contains("api key") || error_lower.contains("unauthorized") || error_lower.contains("authentication") {
        (ErrorType::Authentication, "Authentication failed".to_string(), Some("Please check your API key is valid and has the necessary permissions.".to_string()))
    } else if error_lower.contains("rate limit") || error_lower.contains("too many requests") {
        (ErrorType::RateLimit, "Rate limit exceeded".to_string(), Some("Please wait a moment before sending another request.".to_string()))
    } else if error_lower.contains("network") || error_lower.contains("connection") || error_lower.contains("timeout") {
        (ErrorType::Network, "Network error".to_string(), Some("Please check your internet connection and try again.".to_string()))
    } else if error_lower.contains("model") || error_lower.contains("invalid") {
        (ErrorType::InvalidModel, "Invalid model or request".to_string(), Some("The specified model may not be available or the request format is incorrect.".to_string()))
    } else if error_lower.contains("server") || error_lower.contains("5") || error_lower.contains("internal") {
        (ErrorType::ServerError, "Server error".to_string(), Some("The AI service is experiencing issues. Please try again later.".to_string()))
    } else {
        (ErrorType::Unknown, error_msg.to_string(), None)
    };

    ErrorState {
        message,
        details,
        error_type,
    }
}

fn markdown_to_lines(markdown: &str) -> Vec<Line<'static>> {
    let parser = Parser::new_ext(markdown, Options::all());
    let mut lines = Vec::new();
    let mut current_line = Vec::new();
    let mut in_code_block = false;
    let mut is_bold = false;
    let mut is_italic = false;

    for event in parser {
        match event {
            MarkdownEvent::Text(text) => {
                let mut style = Style::default();
                
                if in_code_block {
                    style = style.fg(Color::Cyan).bg(Color::DarkGray);
                } else {
                    if is_bold {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    if is_italic {
                        style = style.add_modifier(Modifier::ITALIC);
                    }
                }
                
                current_line.push(Span::styled(text.into_string(), style));
            }
            MarkdownEvent::Code(code) => {
                current_line.push(Span::styled(
                    format!(" {} ", code.into_string()),
                    Style::default().fg(Color::Cyan).bg(Color::DarkGray),
                ));
            }
            MarkdownEvent::Start(Tag::CodeBlock(_)) => {
                in_code_block = true;
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line.clone()));
                    current_line.clear();
                }
                current_line.push(Span::styled(
                    "┌─ Code Block ─┐",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ));
                lines.push(Line::from(current_line.clone()));
                current_line.clear();
            }
            MarkdownEvent::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line.clone()));
                    current_line.clear();
                }
                current_line.push(Span::styled(
                    "└─────────────┘",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ));
                lines.push(Line::from(current_line.clone()));
                current_line.clear();
            }
            MarkdownEvent::Start(Tag::Strong) => {
                is_bold = true;
            }
            MarkdownEvent::End(TagEnd::Strong) => {
                is_bold = false;
            }
            MarkdownEvent::Start(Tag::Emphasis) => {
                is_italic = true;
            }
            MarkdownEvent::End(TagEnd::Emphasis) => {
                is_italic = false;
            }
            MarkdownEvent::Start(Tag::Heading { level, .. }) => {
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line.clone()));
                    current_line.clear();
                }
                
                let prefix = match level {
                    pulldown_cmark::HeadingLevel::H1 => "# ",
                    pulldown_cmark::HeadingLevel::H2 => "## ",
                    pulldown_cmark::HeadingLevel::H3 => "### ",
                    _ => "#### ",
                };
                
                current_line.push(Span::styled(
                    prefix,
                    Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                ));
            }
            MarkdownEvent::Start(Tag::List(_)) => {
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line.clone()));
                    current_line.clear();
                }
            }
            MarkdownEvent::Start(Tag::Item) => {
                current_line.push(Span::styled(
                    "• ",
                    Style::default().fg(Color::Green),
                ));
            }
            MarkdownEvent::Start(Tag::BlockQuote(_)) => {
                current_line.push(Span::styled(
                    "▎ ",
                    Style::default().fg(Color::Blue),
                ));
            }
            MarkdownEvent::SoftBreak | MarkdownEvent::HardBreak => {
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line.clone()));
                    current_line.clear();
                } else {
                    lines.push(Line::from(""));
                }
            }
            _ => {
                // Handle other markdown events as needed
            }
        }
    }

    if !current_line.is_empty() {
        lines.push(Line::from(current_line));
    }

    // Convert to 'static
    lines.into_iter().map(|line| {
        let spans: Vec<Span<'static>> = line.spans.into_iter().map(|span| {
            Span::styled(span.content.into_owned(), span.style)
        }).collect();
        Line::from(spans)
    }).collect()
}
