//! Chat Application for Perspt TUI
//!
//! An elegant chat interface with markdown rendering, syntax highlighting,
//! and reliable key handling. Now with async event-driven architecture.

use crate::app_event::AppEvent;
use crate::simple_input::SimpleInput;
use crate::theme::icons;
use anyhow::Result;
use crossterm::event::{
    Event as CrosstermEvent, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind,
};
use perspt_core::{GenAIProvider, EOT_SIGNAL};
use ratatui::{
    crossterm::event::{self, Event},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    DefaultTerminal, Frame,
};
use std::sync::Arc;
use throbber_widgets_tui::ThrobberState;
use tokio::sync::mpsc;
use unicode_width::UnicodeWidthStr;

/// Role of a chat message
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

pub use crate::markdown::{ContentBlock, TableAlign};

/// A single chat message
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub reasoning: Option<String>,
    pub cached_visual_lines: Vec<Line<'static>>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            reasoning: None,
            cached_visual_lines: Vec::new(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            reasoning: None,
            cached_visual_lines: Vec::new(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            reasoning: None,
            cached_visual_lines: Vec::new(),
        }
    }

    /// Parse thinking blocks from content.
    /// Returns (thought_content, remaining_content)
    pub fn parse_inline_thought(content: &str) -> (Option<String>, String) {
        if let Some(start_idx) = content.find("<think>") {
            if let Some(end_idx) = content.find("</think>") {
                if end_idx > start_idx {
                    let thought = content[start_idx + "<think>".len()..end_idx].to_string();
                    let remaining = format!(
                        "{}{}",
                        &content[..start_idx],
                        &content[end_idx + "</think>".len()..]
                    );
                    return (Some(thought), remaining);
                }
            } else {
                // Unclosed <think> tag (still streaming)
                let thought = content[start_idx + "<think>".len()..].to_string();
                let remaining = content[..start_idx].to_string();
                return (Some(thought), remaining);
            }
        }
        (None, content.to_string())
    }

    /// Rebuild this message's wrapped visual lines for one viewport width:
    /// header, optional thought block, then the markdown/math rendering
    /// pipeline in [`crate::markdown`].
    pub fn update_cache(&mut self, viewport_width: usize, show_reasoning: bool) {
        self.cached_visual_lines.clear();
        if viewport_width == 0 {
            return;
        }

        // Message header with role
        let (icon, header_style, content_style) = match self.role {
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

        // Add separator line
        self.cached_visual_lines.push(Line::from(Span::styled(
            format!(
                "━━━ {} {} ━━━",
                icon,
                match self.role {
                    MessageRole::User => "You",
                    MessageRole::Assistant => "Assistant",
                    MessageRole::System => "System",
                }
            ),
            header_style,
        )));

        // Parse inline thoughts if any
        let (inline_thought, display_content) = if self.role == MessageRole::Assistant {
            Self::parse_inline_thought(&self.content)
        } else {
            (None, self.content.clone())
        };

        let combined_thought = match (&self.reasoning, &inline_thought) {
            (Some(r), Some(i)) => Some(format!("{}\n{}", r, i)),
            (Some(r), None) => Some(r.clone()),
            (None, Some(i)) => Some(i.clone()),
            (None, None) => None,
        };

        // Render thought block first if enabled
        if show_reasoning {
            if let Some(ref thought) = combined_thought {
                if !thought.is_empty() {
                    self.cached_visual_lines.push(Line::from(Span::styled(
                        "  ⚡ Thought Process".to_string(),
                        Style::default()
                            .fg(Color::Rgb(255, 183, 77))
                            .add_modifier(Modifier::ITALIC | Modifier::BOLD),
                    )));
                    let reasoning_style = Style::default()
                        .fg(Color::Rgb(120, 144, 156))
                        .add_modifier(Modifier::ITALIC);
                    for line in thought.lines() {
                        let text = format!("    {}", line);
                        if text.width() <= viewport_width {
                            self.cached_visual_lines
                                .push(Line::from(Span::styled(text, reasoning_style)));
                        } else {
                            let wrapped = ChatApp::wrap_text_to_width(&text, viewport_width);
                            for wrapped_line in wrapped {
                                self.cached_visual_lines
                                    .push(Line::from(Span::styled(wrapped_line, reasoning_style)));
                            }
                        }
                    }
                    self.cached_visual_lines.push(Line::from(String::new()));
                }
            }
        }

        // Pre-transpile math segments in the display content
        let display_content_transpiled = crate::markdown::transpile_math_in_text(&display_content);

        // Render message content into logical lines
        if self.role == MessageRole::Assistant {
            let blocks = crate::markdown::parse_markdown_blocks(&display_content_transpiled);
            for block in blocks {
                match block {
                    ContentBlock::Markdown(text) => {
                        let rendered = tui_markdown::from_str(&text);
                        for line in rendered.lines {
                            let text: String =
                                line.spans.iter().map(|s| s.content.as_ref()).collect();
                            let parsed_line =
                                crate::markdown::parse_line_to_spans(&text, content_style);
                            let wrapped = crate::markdown::wrap_line(parsed_line, viewport_width);
                            self.cached_visual_lines.extend(wrapped);
                        }
                    }
                    ContentBlock::Table {
                        headers,
                        alignments,
                        rows,
                    } => {
                        let table_lines = crate::markdown::render_table(
                            headers,
                            alignments,
                            rows,
                            viewport_width,
                            content_style,
                        );
                        self.cached_visual_lines.extend(table_lines);
                    }
                }
            }
        } else {
            let mut logical_lines = Vec::new();
            for line in display_content_transpiled.lines() {
                logical_lines.push((format!("  {}", line), content_style));
            }
            for (text, style) in logical_lines {
                let parsed_line = crate::markdown::parse_line_to_spans(&text, style);
                let wrapped = crate::markdown::wrap_line(parsed_line, viewport_width);
                self.cached_visual_lines.extend(wrapped);
            }
        }

        // Add spacing at the end
        self.cached_visual_lines.push(Line::from(String::new()));
    }
}

/// Elegant Chat application state
pub struct ChatApp {
    /// Chat message history
    pub(crate) messages: Vec<ChatMessage>,
    /// Simple input widget
    pub(crate) input: SimpleInput,
    /// Scroll offset for message display
    pub(crate) scroll_offset: usize,
    /// Buffer for streaming response
    pub(crate) streaming_buffer: String,
    /// Buffer for streaming reasoning response
    pub(crate) streaming_reasoning: String,
    /// Toggle to show/hide reasoning tokens
    pub(crate) show_reasoning: bool,
    /// Whether currently streaming a response
    pub(crate) is_streaming: bool,
    /// LLM provider
    pub(crate) provider: Arc<GenAIProvider>,
    /// Model to use
    pub(crate) model: String,
    /// Throbber state for loading animation
    pub(crate) throbber_state: ThrobberState,
    /// Should quit the application
    pub(crate) should_quit: bool,
    /// Receiver for streaming chunks
    pub(crate) stream_rx: Option<mpsc::UnboundedReceiver<String>>,
    /// Total visual lines in messages (for scrolling) - after wrapping
    pub(crate) total_visual_lines: usize,
    /// Auto-scroll to bottom flag (set during streaming)
    pub(crate) auto_scroll: bool,
    /// Visible height of message area (updated during render)
    pub(crate) visible_height: usize,
    /// Flag to indicate a message send is pending (for async handling)
    pub(crate) pending_send: bool,
    /// Chat-scoped MCP tools (read-only admission); None keeps the plain
    /// streaming path.
    pub(crate) chat_tools: Option<perspt_agent::external_tools::chat::ChatToolSession>,
    /// Whether at least one MCP server selected the chat mode, including a
    /// server that connected but had no tools admitted.
    pub(crate) mcp_configured: bool,
    /// Namespaced MCP tools admitted into this chat lifecycle.
    pub(crate) mcp_tool_names: Vec<String>,
    /// Discovery/admission summary retained even when `/clear` removes the
    /// startup messages, so `/mcp` remains a useful diagnostic.
    pub(crate) mcp_notices: Vec<String>,
    /// One server-initiated elicitation currently awaiting an explicit user
    /// response. The model/tool task remains paused while the TUI stays live.
    pub(crate) pending_mcp_elicitation: Option<perspt_agent::McpPendingElicitation>,
    /// Last viewport width used for wrapping (to detect resize)
    pub(crate) last_viewport_width: usize,
    /// Shared history loaded from data_dir/history.txt
    pub(crate) history: Vec<String>,
    /// Index of the current traversed history item
    pub(crate) history_index: Option<usize>,
    /// Current input draft when traversing history
    pub(crate) history_draft: String,
    /// Whether the local dedication command has already been shown.
    pub(crate) love_triggered: bool,
}

impl ChatApp {
    /// Create a new chat application
    pub fn new(provider: GenAIProvider, model: String) -> Self {
        // Load history from paths::history_file() if possible
        let mut history = Vec::new();
        if let Some(history_path) = perspt_core::paths::history_file() {
            if history_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&history_path) {
                    history = content.lines().map(|s| s.to_string()).collect();
                }
            }
        }

        Self {
            messages: vec![ChatMessage::system(
                "Welcome to Perspt! Type your message and press Enter to send.",
            )],
            input: SimpleInput::new(),
            scroll_offset: 0,
            streaming_buffer: String::new(),
            streaming_reasoning: String::new(),
            show_reasoning: true,
            is_streaming: false,
            provider: Arc::new(provider),
            model,
            throbber_state: ThrobberState::default(),
            should_quit: false,
            stream_rx: None,
            total_visual_lines: 0,
            auto_scroll: true, // Start with auto-scroll enabled
            visible_height: 20,
            pending_send: false,
            chat_tools: None,
            mcp_configured: false,
            mcp_tool_names: Vec::new(),
            mcp_notices: Vec::new(),
            pending_mcp_elicitation: None,
            last_viewport_width: 80,
            history,
            history_index: None,
            history_draft: String::new(),
            love_triggered: false,
        }
    }

    /// Attach chat-scoped MCP tools discovered by the composition root.
    pub fn with_chat_tools(
        mut self,
        tools: Option<perspt_agent::external_tools::chat::ChatToolSession>,
        notices: Vec<String>,
    ) -> Self {
        // A failed setup still means MCP was configured; retain that state so
        // `/mcp` reports the failure instead of incorrectly saying disabled.
        self.mcp_configured = tools.is_some() || !notices.is_empty();
        self.mcp_tool_names = tools
            .as_ref()
            .map(|session| session.tool_names())
            .unwrap_or_default();
        for notice in &notices {
            let msg = ChatMessage::system(notice.clone());
            self.push_message(msg);
        }
        self.mcp_notices = notices;
        self.chat_tools = tools.filter(|session| session.has_tools());
        self
    }

    /// Run the chat application main loop
    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        loop {
            self.poll_mcp_elicitation();
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
                            } else if let Some(content) =
                                chunk.strip_prefix("__PERSPT_REASONING__:")
                            {
                                self.streaming_reasoning.push_str(content);
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
                    Event::Paste(text)
                        if !self.is_streaming || self.pending_mcp_elicitation.is_some() =>
                    {
                        self.input.insert_text(&text);
                    }
                    Event::Key(key) => {
                        if key.kind == KeyEventKind::Release {
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
                            // Emacs navigation & editing shortcuts
                            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.input.move_home();
                            }
                            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.input.move_end();
                            }
                            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.input.move_left();
                            }
                            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.input.move_right();
                            }
                            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.input.delete();
                            }
                            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.input.backspace();
                            }
                            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.input.kill_to_end();
                            }
                            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.input.kill_to_start();
                            }
                            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.input.delete_word_before();
                            }

                            // Send message on Enter
                            KeyCode::Enter
                                if (!self.is_streaming
                                    || self.pending_mcp_elicitation.is_some())
                                    && !self.input.is_empty() =>
                            {
                                let text = self.input.text().trim().to_string();
                                if self.handle_local_command(&text)
                                    || self.handle_elicitation(&text)
                                {
                                    continue;
                                } else if text.starts_with('/') {
                                    let cmd = text.to_lowercase();
                                    if cmd == "/exit" || cmd == "/quit" {
                                        self.should_quit = true;
                                    } else if cmd == "/clear" {
                                        self.messages.clear();
                                        self.push_message(ChatMessage::system(
                                            "Conversation history cleared.",
                                        ));
                                        self.input.clear();
                                        self.scroll_offset = 0;
                                        // Explicitly clear terminal screen buffer to
                                        // remove residual artifacts
                                        let _ = terminal.clear();
                                    } else if cmd.starts_with("/model") {
                                        let parts: Vec<&str> = text.split_whitespace().collect();
                                        if parts.len() > 1 {
                                            let new_model = parts[1..].join(" ");
                                            self.model = new_model;
                                            self.push_message(ChatMessage::system(format!(
                                                "Switched model to: {}",
                                                self.model
                                            )));
                                        } else {
                                            self.push_message(ChatMessage::system(
                                                "Usage: /model <name>",
                                            ));
                                        }
                                        self.input.clear();
                                    } else if cmd.starts_with("/save") {
                                        let parts: Vec<&str> = text.split_whitespace().collect();
                                        if parts.len() > 1 {
                                            let filepath = parts[1..].join(" ");
                                            match self.save_conversation_to_file(&filepath) {
                                                Ok(_) => {
                                                    self.push_message(ChatMessage::system(
                                                        format!(
                                                        "Conversation saved successfully to: {}",
                                                        filepath
                                                    ),
                                                    ));
                                                }
                                                Err(e) => {
                                                    self.push_message(ChatMessage::system(
                                                        format!(
                                                            "Failed to save conversation: {}",
                                                            e
                                                        ),
                                                    ));
                                                }
                                            }
                                        } else {
                                            self.push_message(ChatMessage::system(
                                                "Usage: /save <file_path>\nExample: /save conversation.md",
                                            ));
                                        }
                                        self.input.clear();
                                    } else if cmd == "/mcp" {
                                        self.push_message(ChatMessage::system(
                                            self.mcp_status_text(),
                                        ));
                                        self.input.clear();
                                    } else if cmd == "/help" {
                                        self.push_message(ChatMessage::system(self.help_text()));
                                        self.input.clear();
                                    } else {
                                        self.push_message(ChatMessage::system(format!(
                                            "Unknown command: {}. Type /help for help.",
                                            text
                                        )));
                                        self.input.clear();
                                    }
                                } else {
                                    self.send_message().await?;
                                }
                            }
                            // Toggle reasoning display on Ctrl+R
                            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.show_reasoning = !self.show_reasoning;
                                for msg in &mut self.messages {
                                    msg.update_cache(self.last_viewport_width, self.show_reasoning);
                                }
                            }
                            // Newline with Ctrl+J (reliable across terminals)
                            KeyCode::Char('j')
                                if key.modifiers.contains(KeyModifiers::CONTROL)
                                    && !self.is_streaming =>
                            {
                                self.input.insert_newline();
                            }
                            // Also support Ctrl+Enter for newline
                            KeyCode::Enter
                                if key.modifiers.contains(KeyModifiers::CONTROL)
                                    && !self.is_streaming =>
                            {
                                self.input.insert_newline();
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
                            // Shift+Up/Down to scroll
                            KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
                                self.scroll_up(1);
                            }
                            KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                                self.scroll_down(1);
                            }
                            // Input navigation / history traversal
                            KeyCode::Left => self.input.move_left(),
                            KeyCode::Right => self.input.move_right(),
                            KeyCode::Up if !self.is_streaming => {
                                if self.input.cursor_line() == 0 {
                                    if !self.history.is_empty() {
                                        if self.history_index.is_none() {
                                            self.history_draft = self.input.text();
                                            self.history_index = Some(self.history.len() - 1);
                                            let item = &self.history[self.history.len() - 1];
                                            self.input.set_text(item);
                                        } else {
                                            let idx = self.history_index.unwrap();
                                            if idx > 0 {
                                                self.history_index = Some(idx - 1);
                                                let item = &self.history[idx - 1];
                                                self.input.set_text(item);
                                            }
                                        }
                                    }
                                } else {
                                    self.input.move_up();
                                }
                            }
                            KeyCode::Down if !self.is_streaming => {
                                if self.input.cursor_line() == self.input.line_count() - 1 {
                                    if let Some(idx) = self.history_index {
                                        if idx + 1 < self.history.len() {
                                            self.history_index = Some(idx + 1);
                                            let item = &self.history[idx + 1];
                                            self.input.set_text(item);
                                        } else {
                                            self.history_index = None;
                                            let draft = self.history_draft.clone();
                                            self.input.set_text(&draft);
                                        }
                                    }
                                } else {
                                    self.input.move_down();
                                }
                            }
                            KeyCode::Home => self.input.move_home(),
                            KeyCode::End => self.input.move_end(),
                            // Text editing
                            KeyCode::Backspace => self.input.backspace(),
                            KeyCode::Delete => self.input.delete(),
                            KeyCode::Char(c)
                                if !self.is_streaming || self.pending_mcp_elicitation.is_some() =>
                            {
                                self.input.insert_char(c);
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
            AppEvent::CoreEvent(_) => true,   // Not used in chat mode
        }
    }

    /// Handle a terminal event (key press, mouse, resize)
    fn handle_terminal_event(&mut self, event: CrosstermEvent) -> bool {
        match event {
            CrosstermEvent::Paste(text)
                if !self.is_streaming || self.pending_mcp_elicitation.is_some() =>
            {
                self.input.insert_text(&text);
            }
            CrosstermEvent::Key(key) => {
                if key.kind == KeyEventKind::Release {
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
                    // Emacs navigation & editing shortcuts
                    KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input.move_home();
                    }
                    KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input.move_end();
                    }
                    KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input.move_left();
                    }
                    KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input.move_right();
                    }
                    KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input.delete();
                    }
                    KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input.backspace();
                    }
                    KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input.kill_to_end();
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input.kill_to_start();
                    }
                    KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input.delete_word_before();
                    }

                    // Send message on Enter
                    KeyCode::Enter
                        if (!self.is_streaming || self.pending_mcp_elicitation.is_some())
                            && !self.input.is_empty() =>
                    {
                        let text = self.input.text().trim().to_string();
                        if self.handle_local_command(&text) || self.handle_elicitation(&text) {
                            return true;
                        } else if text.starts_with('/') {
                            let cmd = text.to_lowercase();
                            if cmd == "/exit" || cmd == "/quit" {
                                return false; // Exit TUI app
                            } else if cmd == "/clear" {
                                self.messages.clear();
                                self.push_message(ChatMessage::system(
                                    "Conversation history cleared.",
                                ));
                                self.input.clear();
                                self.scroll_offset = 0;
                            } else if cmd.starts_with("/model") {
                                let parts: Vec<&str> = text.split_whitespace().collect();
                                if parts.len() > 1 {
                                    let new_model = parts[1..].join(" ");
                                    self.model = new_model;
                                    self.push_message(ChatMessage::system(format!(
                                        "Switched model to: {}",
                                        self.model
                                    )));
                                } else {
                                    self.push_message(ChatMessage::system("Usage: /model <name>"));
                                }
                                self.input.clear();
                            } else if cmd.starts_with("/save") {
                                let parts: Vec<&str> = text.split_whitespace().collect();
                                if parts.len() > 1 {
                                    let filepath = parts[1..].join(" ");
                                    match self.save_conversation_to_file(&filepath) {
                                        Ok(_) => {
                                            self.push_message(ChatMessage::system(format!(
                                                "Conversation saved successfully to: {}",
                                                filepath
                                            )));
                                        }
                                        Err(e) => {
                                            self.push_message(ChatMessage::system(format!(
                                                "Failed to save conversation: {}",
                                                e
                                            )));
                                        }
                                    }
                                } else {
                                    self.push_message(ChatMessage::system(
                                        "Usage: /save <file_path>\nExample: /save conversation.md",
                                    ));
                                }
                                self.input.clear();
                            } else if cmd == "/mcp" {
                                self.push_message(ChatMessage::system(self.mcp_status_text()));
                                self.input.clear();
                            } else if cmd == "/help" {
                                self.push_message(ChatMessage::system(self.help_text()));
                                self.input.clear();
                            } else {
                                self.push_message(ChatMessage::system(format!(
                                    "Unknown command: {}. Type /help for help.",
                                    text
                                )));
                                self.input.clear();
                            }
                        } else {
                            self.pending_send = true;
                        }
                    }
                    // Toggle reasoning display on Ctrl+R
                    KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.show_reasoning = !self.show_reasoning;
                        for msg in &mut self.messages {
                            msg.update_cache(self.last_viewport_width, self.show_reasoning);
                        }
                    }
                    // Newline with Ctrl+J
                    KeyCode::Char('j')
                        if key.modifiers.contains(KeyModifiers::CONTROL) && !self.is_streaming =>
                    {
                        self.input.insert_newline();
                    }
                    // Ctrl+Enter for newline
                    KeyCode::Enter
                        if key.modifiers.contains(KeyModifiers::CONTROL) && !self.is_streaming =>
                    {
                        self.input.insert_newline();
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
                    // Shift+Up/Down to scroll
                    KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        self.scroll_up(1);
                    }
                    KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        self.scroll_down(1);
                    }
                    // Input navigation / history traversal
                    KeyCode::Left => self.input.move_left(),
                    KeyCode::Right => self.input.move_right(),
                    KeyCode::Up if !self.is_streaming => {
                        if self.input.cursor_line() == 0 {
                            if !self.history.is_empty() {
                                if self.history_index.is_none() {
                                    self.history_draft = self.input.text();
                                    self.history_index = Some(self.history.len() - 1);
                                    let item = &self.history[self.history.len() - 1];
                                    self.input.set_text(item);
                                } else {
                                    let idx = self.history_index.unwrap();
                                    if idx > 0 {
                                        self.history_index = Some(idx - 1);
                                        let item = &self.history[idx - 1];
                                        self.input.set_text(item);
                                    }
                                }
                            }
                        } else {
                            self.input.move_up();
                        }
                    }
                    KeyCode::Down if !self.is_streaming => {
                        if self.input.cursor_line() == self.input.line_count() - 1 {
                            if let Some(idx) = self.history_index {
                                if idx + 1 < self.history.len() {
                                    self.history_index = Some(idx + 1);
                                    let item = &self.history[idx + 1];
                                    self.input.set_text(item);
                                } else {
                                    self.history_index = None;
                                    let draft = self.history_draft.clone();
                                    self.input.set_text(&draft);
                                }
                            }
                        } else {
                            self.input.move_down();
                        }
                    }
                    KeyCode::Home => self.input.move_home(),
                    KeyCode::End => self.input.move_end(),
                    // Text editing
                    KeyCode::Backspace => self.input.backspace(),
                    KeyCode::Delete => self.input.delete(),
                    KeyCode::Char(c)
                        if !self.is_streaming || self.pending_mcp_elicitation.is_some() =>
                    {
                        self.input.insert_char(c);
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
                        } else if let Some(content) = chunk.strip_prefix("__PERSPT_REASONING__:") {
                            self.streaming_reasoning.push_str(content);
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

    /// Prune messages if they exceed character count limits (32,000 chars)
    fn prune_messages(&mut self) {
        loop {
            let total_chars: usize = self.messages.iter().map(|m| m.content.len()).sum();
            if total_chars <= 32000 {
                break;
            }

            let remove_idx = if self
                .messages
                .first()
                .map(|m| m.role == MessageRole::System)
                .unwrap_or(false)
            {
                if self.messages.len() > 1 {
                    1
                } else {
                    break;
                }
            } else {
                0
            };

            if self.messages.len() > remove_idx {
                self.messages.remove(remove_idx);
            } else {
                break;
            }
        }
    }

    /// Add a message, updating its visual cache and pruning history automatically
    pub(crate) fn push_message(&mut self, mut msg: ChatMessage) {
        msg.update_cache(self.last_viewport_width, self.show_reasoning);
        self.messages.push(msg);
        self.prune_messages();
        self.scroll_to_bottom();
    }

    /// Send the current message to the LLM
    async fn send_message(&mut self) -> Result<()> {
        let user_message = self.input.text().trim().to_string();
        if user_message.is_empty() {
            return Ok(());
        }

        // Add user message
        let msg = ChatMessage::user(user_message.clone());
        self.push_message(msg);
        self.input.clear();

        // Save to history
        self.history.push(user_message.clone());
        self.history_index = None;
        self.history_draft.clear();
        if let Some(history_path) = perspt_core::paths::history_file() {
            if let Some(parent) = history_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&history_path, self.history.join("\n"));
        }

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
        self.streaming_reasoning.clear();
        self.scroll_to_bottom();

        let (tx, rx) = mpsc::unbounded_channel();
        self.stream_rx = Some(rx);

        let provider = Arc::clone(&self.provider);
        let model = self.model.clone();

        if let Some(session) = self.chat_tools.clone() {
            // Tool-calling turn: bounded MCP rounds, then the final text.
            let messages = self.core_messages();
            tokio::spawn(async move {
                let outcome = session.run_turn(&provider, &model, messages, &tx).await;
                match outcome {
                    Ok(text) => {
                        let _ = tx.send(text);
                    }
                    Err(error) => {
                        let _ = tx.send(format!("error: {error:#}"));
                    }
                }
                let _ = tx.send(EOT_SIGNAL.to_string());
            });
            return Ok(());
        }

        tokio::spawn(async move {
            let _ = provider
                .generate_response_stream_to_channel(&model, &context.join("\n"), tx)
                .await;
        });

        Ok(())
    }

    /// The provider-neutral message log for a tool-calling turn.
    fn core_messages(&self) -> Vec<perspt_core::CoreMessage> {
        self.messages
            .iter()
            .map(|message| match message.role {
                MessageRole::User => perspt_core::CoreMessage::User {
                    content: message.content.clone(),
                },
                MessageRole::Assistant => perspt_core::CoreMessage::Assistant {
                    content: message.content.clone(),
                },
                MessageRole::System => perspt_core::CoreMessage::System {
                    content: message.content.clone(),
                },
            })
            .collect()
    }

    /// Finalize streaming and add assistant message
    fn finalize_streaming(&mut self) {
        if !self.streaming_buffer.is_empty() || !self.streaming_reasoning.is_empty() {
            let mut msg = ChatMessage::assistant(self.streaming_buffer.clone());
            if !self.streaming_reasoning.is_empty() {
                msg.reasoning = Some(self.streaming_reasoning.clone());
            }
            self.push_message(msg);
        }
        self.streaming_buffer.clear();
        self.streaming_reasoning.clear();
        self.is_streaming = false;
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
    pub(crate) fn wrap_text_to_width(text: &str, width: usize) -> Vec<String> {
        if width == 0 {
            return vec![text.to_string()];
        }

        let options = textwrap::Options::new(width).break_words(true);
        let wrapped = textwrap::wrap(text, options);
        let mut result: Vec<String> = wrapped.into_iter().map(|cow| cow.into_owned()).collect();

        if result.is_empty() {
            result.push(String::new());
        }

        result
    }

    /// Render the chat application
    fn render(&mut self, frame: &mut Frame) {
        let size = frame.area();

        // Calculate input height dynamically using visual wrapped lines
        let viewport_width = size.width.saturating_sub(2) as usize;
        let input_height = (self.input.line_count_wrapped(viewport_width) as u16 + 2).clamp(3, 10);

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

        let show_reasoning_str = if self.show_reasoning { "ON" } else { "OFF" };
        let model_display = format!(
            " {} │ Ctrl+R: Reasoning {} ",
            self.model, show_reasoning_str
        );
        let model_span = Span::styled(
            model_display.clone(),
            Style::default()
                .fg(Color::Rgb(176, 190, 197))
                .add_modifier(Modifier::ITALIC),
        );

        // Render block
        frame.render_widget(header, area);

        // Render model name and toggle on right side
        let model_area = Rect {
            x: area.x + area.width - model_display.len() as u16 - 4,
            y: area.y,
            width: model_display.len() as u16 + 3,
            height: 1,
        };
        frame.render_widget(Paragraph::new(model_span), model_area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perspt_core::GenAIProvider;
    use ratatui::crossterm::event::{KeyEvent, KeyModifiers};

    #[tokio::test]
    async fn test_slash_commands_in_tui() {
        let provider = GenAIProvider::new().unwrap_or_else(|_| {
            GenAIProvider::new_with_config(Some("openai"), Some("dummy_key")).unwrap()
        });
        let mut app = ChatApp::new(provider, "gpt-4".to_string());

        // Test /help command
        app.input.set_text("/help");
        app.handle_terminal_event(CrosstermEvent::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));
        assert_eq!(app.input.text(), "");
        assert!(app
            .messages
            .iter()
            .any(|m| m.content.contains("Available Slash Commands:")));
        assert!(app
            .messages
            .iter()
            .any(|m| m.content.contains("/mcp") && m.content.contains("MCP is disabled")));

        // `/mcp` is a local diagnostic and explains the default agent-only
        // mode when chat has no configured MCP lifecycle.
        app.input.set_text("/mcp");
        app.handle_terminal_event(CrosstermEvent::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));
        assert_eq!(app.input.text(), "");
        assert!(app.messages.iter().any(|m| {
            m.content.contains("MCP status: disabled for chat")
                && m.content.contains("modes = [\"agent\", \"chat\"]")
        }));

        // Test /model switching
        app.input.set_text("/model custom-gemma");
        app.handle_terminal_event(CrosstermEvent::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));
        assert_eq!(app.input.text(), "");
        assert_eq!(app.model, "custom-gemma");
        assert!(app
            .messages
            .iter()
            .any(|m| m.content.contains("Switched model to: custom-gemma")));

        // Test /clear command
        app.input.set_text("/clear");
        app.handle_terminal_event(CrosstermEvent::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));
        assert_eq!(app.input.text(), "");
        assert_eq!(app.messages.len(), 1);
        assert!(app.messages[0]
            .content
            .contains("Conversation history cleared."));

        // Test /save command
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_perspt_conv.md");
        let test_file_str = test_file.to_string_lossy().to_string();

        app.input.set_text(&format!("/save {}", test_file_str));
        app.handle_terminal_event(CrosstermEvent::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));
        assert_eq!(app.input.text(), "");
        assert!(test_file.exists());
        let _ = std::fs::remove_file(test_file);
    }

    #[test]
    fn test_parse_inline_thought() {
        let content = "Hello world <think>I should think about this.</think> Actual answer";
        let (thought, remaining) = ChatMessage::parse_inline_thought(content);
        assert_eq!(thought, Some("I should think about this.".to_string()));
        assert_eq!(remaining, "Hello world  Actual answer");

        let content_no_thought = "Just some content without thinking tags";
        let (thought, remaining) = ChatMessage::parse_inline_thought(content_no_thought);
        assert_eq!(thought, None);
        assert_eq!(remaining, "Just some content without thinking tags");

        let unclosed_thought = "Thinking <think>I am currently thinking...";
        let (thought, remaining) = ChatMessage::parse_inline_thought(unclosed_thought);
        assert_eq!(thought, Some("I am currently thinking...".to_string()));
        assert_eq!(remaining, "Thinking ");
    }
}
