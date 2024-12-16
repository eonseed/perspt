use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap, Scrollbar, ScrollbarState},
    Terminal,
    prelude::Margin,
};
use std::{borrow::Cow, io};
use crate::config::AppConfig;
use tokio::sync::mpsc;
use pulldown_cmark::{Parser, Options, Tag, Event as MarkdownEvent, TagEnd};
use crossterm::event::KeyEvent;
use tokio::sync::watch;

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
    pub is_processing: bool,
    scroll_state: ScrollbarState,
    scroll_position: usize,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        Self {
            chat_history: Vec::new(),
            input_text: String::new(),
            status_message: "Welcome to LLM Chat CLI".to_string(),
            config,
            should_quit: false,
            is_processing: false,
            scroll_state: ScrollbarState::default(),
            scroll_position: 0,
        }
    }

    pub fn add_message(&mut self, message: ChatMessage) {
        self.chat_history.push(message);
        self.scroll_to_bottom();
    }

    pub fn set_status(&mut self, message: String, is_error: bool) {
        self.status_message = message;
        if is_error {
            self.add_message(ChatMessage {
                message_type: MessageType::Error,
                content: vec![Line::from(Span::styled(format!("System Error: {}", self.status_message), Style::default().fg(Color::Red)))],
            });
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
            content_height - 1
        } else {
            0
        }
    }

    fn update_scroll_state(&mut self){
        let max_scroll = self.max_scroll();
        self.scroll_state = self.scroll_state.content_length(max_scroll).position(self.scroll_position);
    }
}


pub async fn run_ui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, config: AppConfig, model_name: String, api_key: String, interrupt_tx: watch::Sender<bool>, interrupt_rx: watch::Receiver<bool>) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new(config);
    let (tx, mut rx) = mpsc::unbounded_channel();
    let provider = app.config.default_provider.clone().unwrap_or_else(|| "gemini".to_string());
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
                .wrap(Wrap { trim: true })
                .scroll((app.scroll_position as u16, 0));

            f.render_widget(chat_paragraph, layout[0]);
            // Scrollbar
            let scrollbar = Scrollbar::default()
                .orientation(ratatui::widgets::ScrollbarOrientation::VerticalRight);
            f.render_stateful_widget(
                scrollbar,
                layout[0].inner(&Margin {horizontal: 0, vertical: 0}),
                &mut app.scroll_state,
            );

            // User input
            let input_block = Block::default()
                .title("Input")
                .borders(Borders::ALL);
            let input_paragraph = if app.is_processing {
                Paragraph::new("...".to_string())
                    .block(input_block)
            } else {
                Paragraph::new(app.input_text.clone())
                    .block(input_block)
            };
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
            std::time::Duration::from_millis(50),
            crate::handle_events(&mut app, &tx, &api_url, &api_key, &model_name, &interrupt_tx, &interrupt_rx)
        ).await {
            match event {
                AppEvent::Key(key_event) => {
                    match key_event.code {
                        crossterm::event::KeyCode::Esc => {
                            app.should_quit = true;
                        },
                        crossterm::event::KeyCode::Up => {
                            app.scroll_up();
                        }
                        crossterm::event::KeyCode::Down => {
                            app.scroll_down();
                        }
                        _=> {}
                    }
                },
                _ => {}
            }
        }

        // Check for response messages
        while let Ok(message) = rx.try_recv() {
            let message_clone = message.clone();
            if message.starts_with("Error:") {
                app.add_message(ChatMessage {
                    message_type: MessageType::Error,
                    content: vec![Line::from(Span::styled(Cow::from(message_clone), Style::default().fg(Color::Red)))],
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
                current_line.push(Span::raw(Cow::from(text.to_string())));
            }
            MarkdownEvent::Code(code) => {
                current_line.push(Span::styled(Cow::from(code.to_string()), Style::default().fg(Color::Cyan)));
            }
            MarkdownEvent::Start(Tag::Paragraph) => {
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
            }
            MarkdownEvent::End(TagEnd::Paragraph) => {
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
            }
            MarkdownEvent::Start(Tag::Heading { level, .. }) => {
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
                let style = match level {
                    pulldown_cmark::HeadingLevel::H1 => Style::default().fg(Color::Red).add_modifier(ratatui::style::Modifier::BOLD),
                    pulldown_cmark::HeadingLevel::H2 => Style::default().fg(Color::Green).add_modifier(ratatui::style::Modifier::BOLD),
                    pulldown_cmark::HeadingLevel::H3 => Style::default().fg(Color::Yellow).add_modifier(ratatui::style::Modifier::BOLD),
                    _ => Style::default().add_modifier(ratatui::style::Modifier::BOLD),
                };
                current_line.push(Span::styled(Cow::from(" ".to_string()), style));
            }
            MarkdownEvent::End(TagEnd::Heading { .. }) => {
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
            }
            MarkdownEvent::Start(Tag::List(_)) => {
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
            }
            MarkdownEvent::End(TagEnd::List(_)) => {
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
            }
            MarkdownEvent::Start(Tag::Item) => {
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
                current_line.push(Span::raw(Cow::from("- ".to_string())));
            }
            MarkdownEvent::End(TagEnd::Item) => {
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
            }
            MarkdownEvent::Start(Tag::BlockQuote(_)) => {
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
                current_line.push(Span::styled(Cow::from("> ".to_string()), Style::default().fg(Color::Gray)));
            }
            MarkdownEvent::End(TagEnd::BlockQuote(_)) => {
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
            }
            MarkdownEvent::HardBreak => {
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
            }
            MarkdownEvent::Rule => {
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>()));
                }
                lines.push(Line::from(Span::raw(Cow::from("---".to_string()))));
            }
            _ => {}
        }
    }
    if !current_line.is_empty() {
        lines.push(Line::from(current_line));
    }
    lines
}
