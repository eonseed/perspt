//! Chat Application for Perspt TUI
//!
//! Provides an interactive chat interface with streaming LLM responses,
//! markdown rendering, and syntax-highlighted code blocks.

use crate::theme::{icons, Theme};
use anyhow::Result;
use perspt_core::{GenAIProvider, EOT_SIGNAL};
use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    DefaultTerminal, Frame,
};
use std::sync::Arc;
use throbber_widgets_tui::{Throbber, ThrobberState};
use tokio::sync::mpsc;
use tui_textarea::TextArea;

/// Role of a chat message
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// A single chat message
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
        }
    }
}

/// Chat application state
pub struct ChatApp {
    /// Chat message history
    messages: Vec<ChatMessage>,
    /// Text input area
    input: TextArea<'static>,
    /// Scroll offset for message display
    scroll_offset: usize,
    /// Buffer for streaming response
    streaming_buffer: String,
    /// Whether currently streaming a response
    is_streaming: bool,
    /// LLM provider
    provider: Arc<GenAIProvider>,
    /// Model to use
    model: String,
    /// Throbber state for loading animation
    throbber_state: ThrobberState,
    /// Theme for styling
    theme: Theme,
    /// Should quit the application
    should_quit: bool,
    /// Receiver for streaming chunks
    stream_rx: Option<mpsc::UnboundedReceiver<String>>,
    /// Total lines in messages (for scrolling)
    total_lines: usize,
}

impl ChatApp {
    /// Create a new chat application
    pub fn new(provider: GenAIProvider, model: String) -> Self {
        let mut input = TextArea::default();
        input.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Message (Esc to send) "),
        );
        input.set_cursor_line_style(Style::default());

        Self {
            messages: vec![ChatMessage::system(
                "Welcome to Perspt! Type your message and press Ctrl+Enter to send.",
            )],
            input,
            scroll_offset: 0,
            streaming_buffer: String::new(),
            is_streaming: false,
            provider: Arc::new(provider),
            model,
            throbber_state: ThrobberState::default(),
            theme: Theme::default(),
            should_quit: false,
            stream_rx: None,
            total_lines: 0,
        }
    }

    /// Run the chat application main loop
    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        loop {
            // Render
            terminal.draw(|frame| self.render(frame))?;

            // Handle streaming updates
            if let Some(ref mut rx) = self.stream_rx {
                // Non-blocking check for stream data
                match rx.try_recv() {
                    Ok(chunk) => {
                        if chunk == EOT_SIGNAL {
                            // Streaming complete
                            self.finalize_streaming();
                        } else {
                            self.streaming_buffer.push_str(&chunk);
                        }
                        continue; // Immediately re-render
                    }
                    Err(mpsc::error::TryRecvError::Empty) => {
                        // No data yet, continue to event handling
                    }
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        // Channel closed unexpectedly
                        self.finalize_streaming();
                    }
                }
            }

            // Handle input events with timeout for streaming updates
            let timeout = if self.is_streaming {
                std::time::Duration::from_millis(50)
            } else {
                std::time::Duration::from_millis(100)
            };

            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    // Handle special keys
                    match (key.modifiers, key.code) {
                        // Quit
                        (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                            self.should_quit = true;
                        }
                        (KeyModifiers::CONTROL, KeyCode::Char('q')) => {
                            self.should_quit = true;
                        }
                        // Send message: Ctrl+Enter, Alt+Enter, or Escape
                        (KeyModifiers::CONTROL, KeyCode::Enter)
                        | (KeyModifiers::ALT, KeyCode::Enter) => {
                            if !self.is_streaming {
                                self.send_message().await?;
                            }
                        }
                        // Escape key to send (common pattern)
                        (_, KeyCode::Esc) => {
                            if !self.is_streaming && !self.input.lines().join("").trim().is_empty()
                            {
                                self.send_message().await?;
                            }
                        }
                        // Scroll
                        (_, KeyCode::PageUp) => {
                            self.scroll_up(10);
                        }
                        (_, KeyCode::PageDown) => {
                            self.scroll_down(10);
                        }
                        _ => {
                            // Forward to textarea if not streaming
                            if !self.is_streaming {
                                self.input.input(Event::Key(key));
                            }
                        }
                    }
                }
            }

            // Update throbber animation
            if self.is_streaming {
                self.throbber_state.calc_next();
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    /// Send the current input as a message
    async fn send_message(&mut self) -> Result<()> {
        let content = self.input.lines().join("\n").trim().to_string();
        if content.is_empty() {
            return Ok(());
        }

        // Add user message
        self.messages.push(ChatMessage::user(content.clone()));

        // Clear input
        self.input = TextArea::default();
        self.input.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Message (Esc to send) "),
        );

        // Start streaming
        self.is_streaming = true;
        self.streaming_buffer.clear();

        // Create channel for streaming
        let (tx, rx) = mpsc::unbounded_channel();
        self.stream_rx = Some(rx);

        // Spawn streaming task
        let provider = Arc::clone(&self.provider);
        let model = self.model.clone();

        tokio::spawn(async move {
            if let Err(e) = provider
                .generate_response_stream_to_channel(&model, &content, tx)
                .await
            {
                log::error!("Streaming error: {}", e);
            }
        });

        // Scroll to bottom
        self.scroll_to_bottom();

        Ok(())
    }

    /// Finalize streaming and add assistant message
    fn finalize_streaming(&mut self) {
        if !self.streaming_buffer.is_empty() {
            self.messages
                .push(ChatMessage::assistant(self.streaming_buffer.clone()));
        }
        self.streaming_buffer.clear();
        self.is_streaming = false;
        self.stream_rx = None;
        self.scroll_to_bottom();
    }

    /// Scroll up by n lines
    fn scroll_up(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    /// Scroll down by n lines
    fn scroll_down(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(n);
        // Cap at total lines
        if self.scroll_offset > self.total_lines.saturating_sub(10) {
            self.scroll_offset = self.total_lines.saturating_sub(10);
        }
    }

    /// Scroll to bottom of messages
    fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.total_lines.saturating_sub(10);
    }

    /// Render the chat application
    fn render(&mut self, frame: &mut Frame) {
        let size = frame.area();

        // Main layout: Header, Messages, Input
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(10),   // Messages
                Constraint::Length(5), // Input
            ])
            .split(size);

        self.render_header(frame, chunks[0]);
        self.render_messages(frame, chunks[1]);
        self.render_input(frame, chunks[2]);
    }

    /// Render the header bar
    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let header_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(area);

        // Title
        let title = Paragraph::new(format!("{} Perspt Chat", icons::ROCKET))
            .style(self.theme.user_message)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(title, header_chunks[0]);

        // Model info / status
        let status_text = if self.is_streaming {
            format!("Streaming... {}", self.model)
        } else {
            self.model.clone()
        };

        let status = Paragraph::new(status_text)
            .style(self.theme.muted)
            .block(Block::default().borders(Borders::ALL).title("Model"));
        frame.render_widget(status, header_chunks[1]);
    }

    /// Render the messages area
    fn render_messages(&mut self, frame: &mut Frame, area: Rect) {
        let inner_area = area.inner(ratatui::layout::Margin {
            vertical: 1,
            horizontal: 1,
        });

        // Build message display
        let mut lines: Vec<Line> = Vec::new();

        for msg in &self.messages {
            let (icon, style) = match msg.role {
                MessageRole::User => (icons::USER, self.theme.user_message),
                MessageRole::Assistant => (icons::ASSISTANT, self.theme.assistant_message),
                MessageRole::System => (icons::SYSTEM, self.theme.system_message),
            };

            // Message header
            lines.push(Line::from(vec![
                Span::styled(format!("─── {} ", icon), style),
                Span::styled(
                    match msg.role {
                        MessageRole::User => "You",
                        MessageRole::Assistant => "Assistant",
                        MessageRole::System => "System",
                    },
                    style.add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ───", style),
            ]));

            // Message content
            for line in msg.content.lines() {
                lines.push(Line::from(Span::styled(
                    format!("  {}", line),
                    self.theme.assistant_message,
                )));
            }

            lines.push(Line::default()); // Spacing
        }

        // Add streaming buffer if active
        if self.is_streaming && !self.streaming_buffer.is_empty() {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("─── {} ", icons::ASSISTANT),
                    self.theme.assistant_message,
                ),
                Span::styled(
                    "Assistant",
                    self.theme.assistant_message.add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ───", self.theme.assistant_message),
            ]));

            for line in self.streaming_buffer.lines() {
                lines.push(Line::from(Span::styled(
                    format!("  {}", line),
                    self.theme.assistant_message,
                )));
            }

            // Cursor indicator
            lines.push(Line::from(Span::styled(
                format!("  {}", icons::CURSOR),
                self.theme.cursor,
            )));
        }

        // Add throbber if streaming
        if self.is_streaming && self.streaming_buffer.is_empty() {
            let throbber = Throbber::default()
                .label("Thinking...")
                .style(self.theme.warning);
            frame.render_stateful_widget(
                throbber,
                Rect::new(area.x + 2, area.y + area.height - 2, 20, 1),
                &mut self.throbber_state,
            );
        }

        self.total_lines = lines.len();

        // Apply scroll
        let visible_lines = (inner_area.height as usize).saturating_sub(2);
        let max_scroll = self.total_lines.saturating_sub(visible_lines);
        let scroll = self.scroll_offset.min(max_scroll);

        let paragraph = Paragraph::new(Text::from(lines))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Chat History "),
            )
            .wrap(Wrap { trim: false })
            .scroll((scroll as u16, 0));

        frame.render_widget(paragraph, area);

        // Scrollbar
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));
        let mut scrollbar_state = ScrollbarState::new(self.total_lines).position(scroll);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }

    /// Render the input area
    fn render_input(&self, frame: &mut Frame, area: Rect) {
        if self.is_streaming {
            // Show streaming indicator instead of input
            let streaming_indicator = Paragraph::new("⏳ Receiving response... (Ctrl+C to cancel)")
                .style(self.theme.muted)
                .block(Block::default().borders(Borders::ALL).title(" Input "));
            frame.render_widget(streaming_indicator, area);
        } else {
            frame.render_widget(&self.input, area);
        }
    }
}

/// Run the chat TUI with streaming support
pub async fn run_chat_tui(provider: GenAIProvider, model: String) -> Result<()> {
    let mut terminal = ratatui::init();
    let mut app = ChatApp::new(provider, model);

    let result = app.run(&mut terminal).await;
    ratatui::restore();
    result
}
