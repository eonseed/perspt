// src/ui.rs
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap, Scrollbar, ScrollbarState},
    Terminal,
    prelude::Margin,
};
use std::{borrow::Cow, collections::VecDeque, io, sync::Arc}; // Added VecDeque
use crate::config::AppConfig;
use crate::llm_provider::LLMProvider; // Added LLMProvider import
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
    pub pending_inputs: VecDeque<String>, // Added field
    pub is_llm_busy: bool, // Added field
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
            pending_inputs: VecDeque::new(), // Initialize
            is_llm_busy: false, // Initialize
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

// Updated run_ui function signature to accept the provider
pub async fn run_ui(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, 
    config: AppConfig, // Pass AppConfig by value as it's owned by App
    model_name: String, 
    api_key: String, // Still passed, though AppConfig inside App also has it
    provider: Box<dyn LLMProvider + Send + Sync> // Accept the boxed trait object
) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new(config); // App now owns its config
    let (tx, mut rx) = mpsc::unbounded_channel();
    // api_url is no longer needed here as the provider itself will get it from AppConfig.
    // log::info!("API URL: {}", api_url); // Removed
    log::info!("Model Name (in run_ui): {}", model_name);
    // log::info!("API Key (in run_ui): {}", api_key); // api_key is in app.config

    // If provider needs to be shared with async tasks spawned by handle_events
    // and also used elsewhere, Arc might be better. For now, Box is passed.
    // If handle_events needs to own a part of it or clone it for spawning,
    // then Arc<dyn LLMProvider...> would be passed to run_ui.
    // For now, run_ui owns the Box, and passes a reference to handle_events.

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
            // Conditional styling for input paragraph
            let input_style = if app.is_input_disabled {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            };
            let input_text_display = if app.is_input_disabled {
                format!("{} [Processing...]", app.input_text)
            } else {
                app.input_text.clone()
            };
            let input_paragraph = Paragraph::new(input_text_display)
                .style(input_style) // Apply the conditional style
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
        // Pass reference to provider to handle_events.
        // api_url is removed from handle_events call.
        if let Ok(Some(event)) = tokio::time::timeout(
            std::time::Duration::from_millis(50), // Reduced timeout for snappier feel
            crate::handle_events(&mut app, &tx, &api_key, &model_name, &provider) // Pass provider by reference
        ).await
        {
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
        if let Ok(message) = rx.try_recv() { // Changed to if let for single message processing per tick
            if message == crate::EOT_SIGNAL {
                if let Some(next_input) = app.pending_inputs.pop_front() {
                    log::info!("Processing next input from queue: '{}'", next_input);
                    // Call initiate_llm_request directly.
                    // initiate_llm_request is async, so we need to spawn it or run_ui needs to be able to await it.
                    // Since run_ui is already async, we can await it here.
                    // Note: This will block run_ui's loop until initiate_llm_request (which spawns) returns.
                    // This is fine as initiate_llm_request itself is quick (just sets flags and spawns).
                    crate::initiate_llm_request(&mut app, next_input, &**provider, &model_name, &tx).await;
                    // app.is_llm_busy is true (set by initiate_llm_request)
                    // app.is_input_disabled is true (set by initiate_llm_request)
                    app.set_status(format!("Processing queue. {} remaining.", app.pending_inputs.len()), false);

                } else {
                    // No more pending inputs, LLM is now idle.
                    app.is_llm_busy = false;
                    app.is_input_disabled = false; // Re-enable input field
                    app.set_status("Ready.".to_string(), false);
                    log::info!("LLM idle. Input enabled.");
                }
            } else if message.starts_with("Error:") {
                app.add_message(ChatMessage {
                    message_type: MessageType::Error,
                    content: vec![Line::from(Span::styled(Cow::from(message.clone()), Style::default().fg(Color::Red)))],
                });
                app.set_status(message, true);
                // Error implies the current request is done. Check queue.
                // This logic is now handled by providers sending EOT even on error.
                // If an error occurs, provider sends error message, then EOT.
                // The EOT signal will then trigger the queue check / idle state.
                // So, no need to explicitly set is_llm_busy/is_input_disabled here for errors.
            } else {
                // Handle content message
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
                app.set_status("Receiving response...".to_string(), false);
                // is_input_disabled and is_llm_busy are not changed here.
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