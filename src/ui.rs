// src/ui.rs
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap, ScrollbarState},
    Terminal,
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
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub message_type: MessageType,
    pub content: Vec<Line<'static>>,
}

#[derive(Debug)]
pub enum AppEvent {
    Key(KeyEvent),
    Tick,
}

pub struct App {
    pub chat_history: Vec<ChatMessage>,
    pub input_text: String,
    pub status_message: String,
    pub config: AppConfig,
    pub should_quit: bool,
    scroll_state: ScrollbarState,
    scroll_position: usize,
    pub is_input_disabled: bool, 
    pub pending_inputs: VecDeque<String>,
    pub is_llm_busy: bool,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        Self {
            chat_history: Vec::new(),
            input_text: String::new(),
            status_message: "Welcome to LLM Chat CLI".to_string(),
            config,
            should_quit: false,
            scroll_state: ScrollbarState::default(),
            scroll_position: 0,
            is_input_disabled: false,
            pending_inputs: VecDeque::new(),
            is_llm_busy: false,
        }
    }

    pub fn add_message(&mut self, message: ChatMessage) {
        self.chat_history.push(message);
        self.scroll_to_bottom();
    }

    pub fn set_status(&mut self, message: String, is_error: bool) {
        self.status_message = message;
        if is_error {
            log::error!("Status error: {}", self.status_message);
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

    fn update_scroll_state(&mut self) {
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
        // Draw UI
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(1),     // Chat area
                    Constraint::Length(3),  // Input area
                    Constraint::Length(1),  // Status line
                ])
                .split(f.area());

            // Chat history
            let chat_content: Vec<Line> = app.chat_history
                .iter()
                .flat_map(|msg| {
                    let prefix = match msg.message_type {
                        MessageType::User => "🧑 User: ",
                        MessageType::Assistant => "🤖 Assistant: ",
                        MessageType::Error => "❌ Error: ",
                    };
                    
                    let mut lines = vec![Line::from(prefix)];
                    lines.extend(msg.content.iter().cloned());
                    lines.push(Line::from(""));
                    lines
                })
                .collect();

            let chat_paragraph = Paragraph::new(chat_content)
                .block(Block::default().borders(Borders::ALL).title("Chat"))
                .wrap(Wrap { trim: true })
                .scroll((app.scroll_position as u16, 0));

            f.render_widget(chat_paragraph, chunks[0]);

            // Input area
            let input_color = if app.is_input_disabled {
                Color::DarkGray
            } else {
                Color::White
            };

            let input_paragraph = Paragraph::new(app.input_text.as_str())
                .block(Block::default().borders(Borders::ALL).title("Input (Enter to send, Ctrl+Q to quit)"))
                .style(Style::default().fg(input_color))
                .wrap(Wrap { trim: false });

            f.render_widget(input_paragraph, chunks[1]);

            // Status line
            let status_color = if app.status_message.contains("Error") {
                Color::Red
            } else if app.is_llm_busy {
                Color::Yellow
            } else {
                Color::Green
            };

            let status_paragraph = Paragraph::new(app.status_message.as_str())
                .style(Style::default().fg(status_color));

            f.render_widget(status_paragraph, chunks[2]);
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
                app.set_status("Ready".to_string(), false);
                
                // Process any pending inputs
                if let Some(pending_input) = app.pending_inputs.pop_front() {
                    log::info!("Processing pending input: {}", pending_input);
                    
                    // Add user message to chat history
                    app.add_message(ChatMessage {
                        message_type: MessageType::User,
                        content: vec![Line::from(pending_input.clone())],
                    });

                    // Start LLM request for pending input
                    crate::initiate_llm_request(&mut app, pending_input, Arc::clone(&provider), &model_name, &tx).await;
                }
            } else if message.starts_with("Error: ") {
                // Error message
                let error_content = markdown_to_lines(&message);
                app.add_message(ChatMessage {
                    message_type: MessageType::Error,
                    content: error_content,
                });
                app.set_status("Error occurred".to_string(), true);
            } else {
                // Regular response token
                if app.chat_history.is_empty() || 
                   app.chat_history.last().unwrap().message_type != MessageType::Assistant {
                    // Start new assistant message
                    app.add_message(ChatMessage {
                        message_type: MessageType::Assistant,
                        content: vec![Line::from("")],
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
                            
                            // Replace with updated content
                            *last_line = Line::from(current_text);
                        }
                    }
                }
                
                app.scroll_to_bottom();
                app.set_status("Receiving response...".to_string(), false);
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

fn markdown_to_lines(markdown: &str) -> Vec<Line<'static>> {
    let parser = Parser::new_ext(markdown, Options::all());
    let mut lines = Vec::new();
    let mut current_line = Vec::new();

    for event in parser {
        match event {
            MarkdownEvent::Text(text) => {
                current_line.push(Span::raw(text.into_string()));
            }
            MarkdownEvent::Code(code) => {
                current_line.push(Span::styled(
                    code.into_string(),
                    Style::default().fg(Color::Cyan),
                ));
            }
            MarkdownEvent::Start(Tag::Strong) => {
                // Bold text start
            }
            MarkdownEvent::End(TagEnd::Strong) => {
                // Bold text end
            }
            MarkdownEvent::Start(Tag::Emphasis) => {
                // Italic text start
            }
            MarkdownEvent::End(TagEnd::Emphasis) => {
                // Italic text end
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
