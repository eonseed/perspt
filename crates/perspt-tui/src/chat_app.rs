//! Chat Application for Perspt TUI
//!
//! An elegant chat interface with markdown rendering, syntax highlighting,
//! and reliable key handling. Now with async event-driven architecture.

use crate::app_event::AppEvent;
use crate::simple_input::SimpleInput;
use crate::theme::{icons, Theme};
use anyhow::Result;
use crossterm::event::{
    Event as CrosstermEvent, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind,
};
use perspt_core::{GenAIProvider, EOT_SIGNAL};
use ratatui::{
    crossterm::event::{self, Event},
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    DefaultTerminal, Frame,
};
use std::sync::Arc;
use throbber_widgets_tui::{Throbber, ThrobberState};
use tokio::sync::mpsc;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

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

/// Elegant Chat application state
pub struct ChatApp {
    /// Chat message history
    messages: Vec<ChatMessage>,
    /// Simple input widget
    input: SimpleInput,
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
    #[allow(dead_code)]
    theme: Theme,
    /// Should quit the application
    should_quit: bool,
    /// Receiver for streaming chunks
    stream_rx: Option<mpsc::UnboundedReceiver<String>>,
    /// Total visual lines in messages (for scrolling) - after wrapping
    total_visual_lines: usize,
    /// Auto-scroll to bottom flag (set during streaming)
    auto_scroll: bool,
    /// Visible height of message area (updated during render)
    visible_height: usize,
    /// Flag to indicate a message send is pending (for async handling)
    pending_send: bool,
    /// Last viewport width used for wrapping (to detect resize)
    last_viewport_width: usize,
}

impl ChatApp {
    /// Create a new chat application
    pub fn new(provider: GenAIProvider, model: String) -> Self {
        Self {
            messages: vec![ChatMessage::system(
                "Welcome to Perspt! Type your message and press Enter to send.",
            )],
            input: SimpleInput::new(),
            scroll_offset: 0,
            streaming_buffer: String::new(),
            is_streaming: false,
            provider: Arc::new(provider),
            model,
            throbber_state: ThrobberState::default(),
            theme: Theme::default(),
            should_quit: false,
            stream_rx: None,
            total_visual_lines: 0,
            auto_scroll: true, // Start with auto-scroll enabled
            visible_height: 20,
            pending_send: false,
            last_viewport_width: 80,
        }
    }

    /// Run the chat application main loop
    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        loop {
            // Render
            terminal.draw(|frame| self.render(frame))?;

            // Handle streaming updates - drain ALL pending chunks before rendering
            let mut just_finalized = false;
            if let Some(ref mut rx) = self.stream_rx {
                loop {
                    match rx.try_recv() {
                        Ok(chunk) => {
                            if chunk == EOT_SIGNAL {
                                self.finalize_streaming();
                                just_finalized = true;
                                break;
                            } else {
                                self.streaming_buffer.push_str(&chunk);
                            }
                        }
                        Err(mpsc::error::TryRecvError::Empty) => break,
                        Err(mpsc::error::TryRecvError::Disconnected) => {
                            self.finalize_streaming();
                            just_finalized = true;
                            break;
                        }
                    }
                }
            }

            // Immediate re-render after finalization to show final content without delay
            if just_finalized {
                terminal.draw(|frame| self.render(frame))?;
            }

            // Event handling
            let timeout = if self.is_streaming {
                std::time::Duration::from_millis(16) // ~60fps for smooth streaming
            } else {
                std::time::Duration::from_millis(100)
            };

            if event::poll(timeout)? {
                match event::read()? {
                    Event::Key(key) => {
                        if key.kind != KeyEventKind::Press {
                            continue;
                        }

                        match key.code {
                            // Quit
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.should_quit = true;
                            }
                            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.should_quit = true;
                            }
                            // Send message on Enter
                            KeyCode::Enter if !self.is_streaming => {
                                if !self.input.is_empty() {
                                    self.send_message().await?;
                                }
                            }
                            // Newline with Ctrl+J (reliable across terminals)
                            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                if !self.is_streaming {
                                    self.input.insert_newline();
                                }
                            }
                            // Also support Ctrl+Enter for newline
                            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                if !self.is_streaming {
                                    self.input.insert_newline();
                                }
                            }
                            // Scroll
                            KeyCode::PageUp => self.scroll_up(10),
                            KeyCode::PageDown => self.scroll_down(10),
                            KeyCode::Up if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.scroll_up(1)
                            }
                            KeyCode::Down if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.scroll_down(1)
                            }
                            // Input navigation
                            KeyCode::Left => self.input.move_left(),
                            KeyCode::Right => self.input.move_right(),
                            KeyCode::Up => self.input.move_up(),
                            KeyCode::Down => self.input.move_down(),
                            KeyCode::Home => self.input.move_home(),
                            KeyCode::End => self.input.move_end(),
                            // Text editing
                            KeyCode::Backspace => self.input.backspace(),
                            KeyCode::Delete => self.input.delete(),
                            KeyCode::Char(c) => {
                                if !self.is_streaming {
                                    self.input.insert_char(c);
                                }
                            }
                            _ => {}
                        }
                    }
                    Event::Mouse(mouse) => match mouse.kind {
                        MouseEventKind::ScrollUp => self.scroll_up(3),
                        MouseEventKind::ScrollDown => self.scroll_down(3),
                        _ => {}
                    },
                    _ => {}
                }
            }

            // Update throbber
            if self.is_streaming {
                self.throbber_state.calc_next();
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    /// Handle an AppEvent from the async event loop
    ///
    /// Returns `true` to continue running, `false` to quit.
    pub fn handle_app_event(&mut self, event: AppEvent) -> bool {
        match event {
            AppEvent::Terminal(crossterm_event) => self.handle_terminal_event(crossterm_event),
            AppEvent::StreamChunk(chunk) => {
                self.streaming_buffer.push_str(&chunk);
                true
            }
            AppEvent::StreamComplete => {
                self.finalize_streaming();
                true
            }
            AppEvent::Tick => {
                if self.is_streaming {
                    self.throbber_state.calc_next();
                }
                true
            }
            AppEvent::Quit => false,
            AppEvent::Error(e) => {
                // Log error but continue
                log::error!("App error: {}", e);
                true
            }
            AppEvent::AgentUpdate(_) => true, // Not used in chat mode
        }
    }

    /// Handle a terminal event (key press, mouse, resize)
    fn handle_terminal_event(&mut self, event: CrosstermEvent) -> bool {
        match event {
            CrosstermEvent::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    return true;
                }

                match key.code {
                    // Quit
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return false;
                    }
                    KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return false;
                    }
                    // Send message on Enter (needs special handling - sets pending_send flag)
                    KeyCode::Enter if !self.is_streaming => {
                        if !self.input.is_empty() {
                            self.pending_send = true;
                        }
                    }
                    // Newline with Ctrl+J
                    KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if !self.is_streaming {
                            self.input.insert_newline();
                        }
                    }
                    // Ctrl+Enter for newline
                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if !self.is_streaming {
                            self.input.insert_newline();
                        }
                    }
                    // Scroll
                    KeyCode::PageUp => self.scroll_up(10),
                    KeyCode::PageDown => self.scroll_down(10),
                    KeyCode::Up if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.scroll_up(1)
                    }
                    KeyCode::Down if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.scroll_down(1)
                    }
                    // Input navigation
                    KeyCode::Left => self.input.move_left(),
                    KeyCode::Right => self.input.move_right(),
                    KeyCode::Up => self.input.move_up(),
                    KeyCode::Down => self.input.move_down(),
                    KeyCode::Home => self.input.move_home(),
                    KeyCode::End => self.input.move_end(),
                    // Text editing
                    KeyCode::Backspace => self.input.backspace(),
                    KeyCode::Delete => self.input.delete(),
                    KeyCode::Char(c) => {
                        if !self.is_streaming {
                            self.input.insert_char(c);
                        }
                    }
                    _ => {}
                }
            }
            CrosstermEvent::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => self.scroll_up(3),
                MouseEventKind::ScrollDown => self.scroll_down(3),
                _ => {}
            },
            CrosstermEvent::Resize(_, _) => {
                // Terminal resize - render will handle it
            }
            _ => {}
        }
        true
    }

    /// Check if a message send is pending (set by Enter key in handle_terminal_event)
    pub fn is_send_pending(&self) -> bool {
        self.pending_send
    }

    /// Clear the pending send flag
    pub fn clear_pending_send(&mut self) {
        self.pending_send = false;
    }

    /// Check and process pending stream chunks
    pub fn process_stream_chunks(&mut self) {
        if let Some(ref mut rx) = self.stream_rx {
            loop {
                match rx.try_recv() {
                    Ok(chunk) => {
                        if chunk == EOT_SIGNAL {
                            self.finalize_streaming();
                            break;
                        } else {
                            self.streaming_buffer.push_str(&chunk);
                        }
                    }
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        self.finalize_streaming();
                        break;
                    }
                }
            }
        }
    }

    /// Check if a render is needed
    pub fn needs_render(&self) -> bool {
        self.is_streaming || self.pending_send
    }

    /// Send the current message to the LLM
    async fn send_message(&mut self) -> Result<()> {
        let user_message = self.input.text().trim().to_string();
        if user_message.is_empty() {
            return Ok(());
        }

        // Add user message
        self.messages.push(ChatMessage::user(user_message.clone()));
        self.input.clear();

        // Build context
        let context: Vec<String> = self
            .messages
            .iter()
            .filter(|m| m.role != MessageRole::System)
            .map(|m| {
                format!(
                    "{}: {}",
                    match m.role {
                        MessageRole::User => "User",
                        MessageRole::Assistant => "Assistant",
                        MessageRole::System => "System",
                    },
                    m.content
                )
            })
            .collect();

        // Start streaming
        self.is_streaming = true;
        self.streaming_buffer.clear();
        self.scroll_to_bottom();

        let (tx, rx) = mpsc::unbounded_channel();
        self.stream_rx = Some(rx);

        let provider = Arc::clone(&self.provider);
        let model = self.model.clone();

        tokio::spawn(async move {
            let _ = provider
                .generate_response_stream_to_channel(&model, &context.join("\n"), tx)
                .await;
        });

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

    /// Scroll up (disables auto-scroll)
    fn scroll_up(&mut self, n: usize) {
        self.auto_scroll = false; // User is manually scrolling
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    /// Scroll down
    fn scroll_down(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(n);
        let max = self.total_visual_lines.saturating_sub(self.visible_height);
        if self.scroll_offset >= max {
            self.scroll_offset = max;
            self.auto_scroll = true; // Re-enable auto-scroll when at bottom
        }
    }

    /// Enable auto-scroll to bottom (actual scroll happens in render)
    fn scroll_to_bottom(&mut self) {
        self.auto_scroll = true;
    }

    /// Wrap a single line of text to fit within the given width.
    /// Returns a vector of wrapped lines (as owned Strings).
    fn wrap_text_to_width(text: &str, width: usize) -> Vec<String> {
        if width == 0 {
            return vec![text.to_string()];
        }

        let mut result = Vec::new();
        let mut current_line = String::new();
        let mut current_width = 0;

        for word in text.split_inclusive(|c: char| c.is_whitespace()) {
            let word_width = word.width();

            if current_width + word_width > width && !current_line.is_empty() {
                // Push current line and start new one
                result.push(std::mem::take(&mut current_line));
                current_width = 0;
            }

            // Handle very long words that exceed width
            if word_width > width {
                // Split the word character by character
                for ch in word.chars() {
                    let ch_width = ch.width().unwrap_or(1);
                    if current_width + ch_width > width && !current_line.is_empty() {
                        result.push(std::mem::take(&mut current_line));
                        current_width = 0;
                    }
                    current_line.push(ch);
                    current_width += ch_width;
                }
            } else {
                current_line.push_str(word);
                current_width += word_width;
            }
        }

        if !current_line.is_empty() {
            result.push(current_line);
        }

        if result.is_empty() {
            result.push(String::new());
        }

        result
    }

    /// Render the chat application
    fn render(&mut self, frame: &mut Frame) {
        let size = frame.area();

        // Calculate input height dynamically
        let input_height = (self.input.line_count() as u16 + 2).clamp(3, 10);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),            // Header
                Constraint::Min(10),              // Messages
                Constraint::Length(input_height), // Input
            ])
            .split(size);

        self.render_header(frame, chunks[0]);
        self.render_messages(frame, chunks[1]);
        self.render_input(frame, chunks[2]);
    }

    /// Render elegant header
    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let header = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(96, 125, 139)))
            .title(Span::styled(
                format!(" {} Perspt Chat ", icons::ROCKET),
                Style::default()
                    .fg(Color::Rgb(129, 199, 132))
                    .add_modifier(Modifier::BOLD),
            ))
            .title_alignment(ratatui::layout::HorizontalAlignment::Left);

        let model_display = format!(" {} ", self.model);
        let model_span = Span::styled(
            model_display,
            Style::default()
                .fg(Color::Rgb(176, 190, 197))
                .add_modifier(Modifier::ITALIC),
        );

        // Render block
        frame.render_widget(header, area);

        // Render model name on right side
        let model_area = Rect {
            x: area.x + area.width - self.model.len() as u16 - 4,
            y: area.y,
            width: self.model.len() as u16 + 3,
            height: 1,
        };
        frame.render_widget(Paragraph::new(model_span), model_area);
    }

    /// Render messages with markdown support and virtual scrolling
    fn render_messages(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(96, 125, 139)))
            .title(Span::styled(
                " Messages ",
                Style::default().fg(Color::Rgb(176, 190, 197)),
            ));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let viewport_width = inner.width as usize;
        let viewport_height = inner.height as usize;

        // Update cached width for resize detection
        self.last_viewport_width = viewport_width;
        self.visible_height = viewport_height;

        // Step 1: Collect all logical lines as (text, style) tuples
        let mut logical_lines: Vec<(String, Style)> = Vec::new();

        for msg in &self.messages {
            // Message header with role
            let (icon, header_style, content_style) = match msg.role {
                MessageRole::User => (
                    icons::USER,
                    Style::default()
                        .fg(Color::Rgb(129, 199, 132))
                        .add_modifier(Modifier::BOLD),
                    Style::default().fg(Color::Rgb(224, 247, 250)),
                ),
                MessageRole::Assistant => (
                    icons::ASSISTANT,
                    Style::default()
                        .fg(Color::Rgb(144, 202, 249))
                        .add_modifier(Modifier::BOLD),
                    Style::default().fg(Color::Rgb(189, 189, 189)),
                ),
                MessageRole::System => (
                    icons::SYSTEM,
                    Style::default()
                        .fg(Color::Rgb(176, 190, 197))
                        .add_modifier(Modifier::ITALIC),
                    Style::default().fg(Color::Rgb(158, 158, 158)),
                ),
            };

            // Add separator line (headers don't wrap - they're short)
            logical_lines.push((
                format!(
                    "━━━ {} {} ━━━",
                    icon,
                    match msg.role {
                        MessageRole::User => "You",
                        MessageRole::Assistant => "Assistant",
                        MessageRole::System => "System",
                    }
                ),
                header_style,
            ));

            // Render message content
            if msg.role == MessageRole::Assistant {
                // For assistant messages, extract text from tui-markdown rendered output
                let rendered = tui_markdown::from_str(&msg.content);
                for line in rendered.lines {
                    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                    logical_lines.push((text, content_style));
                }
            } else {
                // Plain text for user/system
                for line in msg.content.lines() {
                    logical_lines.push((format!("  {}", line), content_style));
                }
            }

            logical_lines.push((String::new(), Style::default())); // Spacing
        }

        // Add streaming content
        if self.is_streaming && !self.streaming_buffer.is_empty() {
            let header_style = Style::default()
                .fg(Color::Rgb(144, 202, 249))
                .add_modifier(Modifier::BOLD);
            let content_style = Style::default().fg(Color::Rgb(189, 189, 189));

            logical_lines.push((
                format!("━━━ {} Assistant ━━━", icons::ASSISTANT),
                header_style,
            ));

            let rendered = tui_markdown::from_str(&self.streaming_buffer);
            for line in rendered.lines {
                let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                logical_lines.push((text, content_style));
            }

            // Streaming cursor
            logical_lines.push((
                "▌".to_string(),
                Style::default()
                    .fg(Color::Rgb(129, 212, 250))
                    .add_modifier(Modifier::SLOW_BLINK),
            ));
        }

        // Step 2: Wrap each logical line to visual lines
        let mut visual_lines: Vec<(String, Style)> = Vec::new();

        for (text, style) in logical_lines {
            if text.is_empty() {
                // Empty line stays as is
                visual_lines.push((text, style));
            } else if text.width() <= viewport_width {
                // Line fits, no wrapping needed
                visual_lines.push((text, style));
            } else {
                // Need to wrap - create new lines from wrapped text
                let wrapped = Self::wrap_text_to_width(&text, viewport_width);
                for wrapped_line in wrapped {
                    visual_lines.push((wrapped_line, style));
                }
            }
        }

        // Handle throbber when loading with empty buffer
        if self.is_streaming && self.streaming_buffer.is_empty() {
            let throbber = Throbber::default()
                .label(" Thinking...")
                .style(Style::default().fg(Color::Rgb(255, 183, 77)));
            frame.render_stateful_widget(
                throbber,
                Rect::new(inner.x + 1, inner.y + 1, 20, 1),
                &mut self.throbber_state.clone(),
            );
        }

        // Step 3: Calculate scroll position using visual line count
        let total_visual = visual_lines.len();
        self.total_visual_lines = total_visual;

        let max_scroll = total_visual.saturating_sub(viewport_height);

        let scroll_pos = if self.auto_scroll {
            max_scroll
        } else {
            self.scroll_offset.min(max_scroll)
        };

        // Update scroll_offset to actual position
        self.scroll_offset = scroll_pos;

        // Step 4: Slice visible range and convert to Lines (virtual scrolling - key fix!)
        let visible_lines: Vec<Line> = visual_lines
            .into_iter()
            .skip(scroll_pos)
            .take(viewport_height)
            .map(|(text, style)| Line::from(Span::styled(text, style)))
            .collect();

        // Step 5: Render only the visible slice (NO Paragraph::scroll needed!)
        let paragraph = Paragraph::new(Text::from(visible_lines));
        frame.render_widget(paragraph, inner);

        // Scrollbar with accurate visual line count
        if total_visual > viewport_height {
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .thumb_style(Style::default().fg(Color::Rgb(96, 125, 139)));
            let mut state = ScrollbarState::new(total_visual).position(scroll_pos);
            frame.render_stateful_widget(scrollbar, area.inner(Margin::new(0, 1)), &mut state);
        }
    }

    /// Render input area
    fn render_input(&self, frame: &mut Frame, area: Rect) {
        if self.is_streaming {
            // Show streaming indicator
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(96, 125, 139)))
                .title(Span::styled(
                    " Receiving response... ",
                    Style::default().fg(Color::Rgb(255, 183, 77)),
                ));
            let inner = block.inner(area);
            frame.render_widget(block, area);

            let text = Paragraph::new("Press Ctrl+C to cancel")
                .style(Style::default().fg(Color::Rgb(120, 144, 156)));
            frame.render_widget(text, inner);
        } else {
            // Render input with hint
            self.input
                .render(frame, area, "Enter=send │ Ctrl+J=newline");
        }
    }
}

/// Run the chat TUI
pub async fn run_chat_tui(provider: GenAIProvider, model: String) -> Result<()> {
    use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
    use ratatui::crossterm::execute;
    use std::io::stdout;

    // Enable mouse capture
    execute!(stdout(), EnableMouseCapture)?;

    let mut terminal = ratatui::init();
    let mut app = ChatApp::new(provider, model);

    let result = app.run(&mut terminal).await;

    // Restore terminal
    ratatui::restore();
    execute!(stdout(), DisableMouseCapture)?;

    result
}
