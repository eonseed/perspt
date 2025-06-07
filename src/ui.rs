//! # User Interface Module (ui.rs)
//!
//! This module implements the terminal-based user interface for the Perspt chat application using
//! the Ratatui TUI framework. It provides a rich, interactive chat experience with real-time
//! markdown rendering, scrollable chat history, and comprehensive error handling.
//!
//! ## Features
//!
//! * **Rich Terminal UI**: Modern terminal interface with colors, borders, and layouts
//! * **Real-time Markdown Rendering**: Live rendering of LLM responses with markdown formatting
//! * **Scrollable Chat History**: Full chat history with keyboard navigation
//! * **Progress Indicators**: Visual feedback for LLM response generation
//! * **Error Display**: Comprehensive error handling with user-friendly messages
//! * **Help System**: Built-in help overlay with keyboard shortcuts
//! * **Responsive Layout**: Adaptive layout that works across different terminal sizes
//!
//! ## Architecture
//!
//! The UI follows a component-based architecture:
//! * `App` - Main application state and controller
//! * `ChatMessage` - Individual message representation with styling
//! * `ErrorState` - Error handling and display logic
//! * Event handling system for keyboard inputs and timers
//!
//! ## Usage Example
//!
//! ```rust
//! use perspt::ui::{App, run_app};
//! use perspt::config::AppConfig;
//!
//! let config = AppConfig::load().unwrap();
//! let mut app = App::new(config);
//! run_app(&mut app).await?;
//! ```

// src/ui.rs
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Alignment, Rect},
    style::{Color, Style, Stylize, Modifier},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap, ScrollbarState, BorderType, Clear, Gauge, Scrollbar, ScrollbarOrientation},
    Terminal, Frame,
};
use std::{collections::VecDeque, io, time::{Duration, Instant}, sync::Arc};
use anyhow::Result;

use crate::config::AppConfig;
use crate::llm_provider::GenAIProvider;
use tokio::sync::mpsc;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, KeyEvent},
    terminal::{self, LeaveAlternateScreen},
    ExecutableCommand,
};

// Buffer management constants - optimized for responsiveness
const MAX_STREAMING_BUFFER_SIZE: usize = 1_000_000; // 1MB limit for streaming buffer
const UI_UPDATE_INTERVAL: usize = 500; // Update UI every 500 characters (more responsive)
const SMALL_BUFFER_THRESHOLD: usize = 500; // Always update UI for small content (more responsive)

/// Represents the type of message in the chat interface.
///
/// This enum is used to determine the visual styling and behavior of messages
/// in the chat history, allowing for different color schemes and formatting
/// based on the message source.
///
/// # Examples
///
/// ```rust
/// use perspt::ui::MessageType;
///
/// let user_msg = MessageType::User;      // Blue styling for user input
/// let ai_msg = MessageType::Assistant;   // Green styling for AI responses
/// let error_msg = MessageType::Error;    // Red styling for error messages
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum MessageType {
    /// Messages sent by the user
    User,
    /// Responses from the AI assistant
    Assistant,
    /// Error messages and warnings
    Error,
    /// System notifications and status updates
    System,
    /// Warning messages that don't halt operation
    Warning,
}

/// Represents a single message in the chat interface.
///
/// Contains all the information needed to display a message including its content,
/// styling, timestamp, and message type for proper visual rendering.
///
/// # Fields
///
/// * `message_type` - The type of message (User, Assistant, Error, etc.)
/// * `content` - The formatted content as a vector of styled lines
/// * `timestamp` - When the message was created (formatted string)
/// * `response_id` - Optional ID to associate message with a specific response stream
///
/// # Examples
///
/// ```rust
/// use perspt::ui::{ChatMessage, MessageType};
/// use ratatui::text::Line;
///
/// let message = ChatMessage {
///     message_type: MessageType::User,
///     content: vec![Line::from("Hello, AI!")],
///     timestamp: "2024-01-01 12:00:00".to_string(),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub message_type: MessageType,
    pub content: Vec<Line<'static>>,
    pub timestamp: String,
    pub response_id: Option<usize>,
}

/// Events that can occur in the application.
///
/// These events drive the main event loop and determine how the application
/// responds to user input and system events.
#[derive(Debug)]
pub enum AppEvent {
    /// Application should quit
    Quit,
    /// UI needs to be redrawn
    Redraw,
    /// Keyboard input events
    Key(KeyEvent),
    /// Timer tick events for periodic updates
    Tick,
}

/// Represents an error state with detailed information for user display.
///
/// Provides structured error information that can be displayed to users
/// with appropriate styling and context for troubleshooting.
///
/// # Fields
///
/// * `message` - Primary error message for display
/// * `details` - Optional additional details for debugging
/// * `error_type` - Category of error for appropriate styling and handling
#[derive(Debug, Clone)]
pub struct ErrorState {
    pub message: String,
    pub details: Option<String>,
    pub error_type: ErrorType,
}

/// Categories of errors that can occur in the application.
///
/// Used to determine appropriate error handling, styling, and user guidance
/// for different types of failures.
#[derive(Debug, Clone)]
pub enum ErrorType {
    /// Network connectivity issues
    Network,
    /// Authentication failures with LLM providers
    Authentication,
    /// API rate limiting responses
    RateLimit,
    /// Invalid or unsupported model requests
    InvalidModel,
    /// Server-side errors from LLM providers
    ServerError,
    /// Unknown or unclassified errors
    Unknown,
}

/// Main application state and controller.
///
/// The `App` struct contains all the state needed to run the chat interface,
/// including chat history, user input, configuration, and UI state management.
/// Enhanced with real-time responsiveness and proper cursor management.
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
    pub streaming_buffer: String,
    pub current_response_id: Option<usize>,
    pub next_response_id: usize,
    // Enhanced input handling
    pub cursor_position: usize,
    pub input_scroll_offset: usize,
    pub last_animation_tick: Instant,
    // UI state
    pub needs_redraw: bool,
    pub input_width: usize,
    pub cursor_blink_state: bool,
    pub last_cursor_blink: Instant,
    pub terminal_height: usize,
    pub terminal_width: usize,
}

impl App {
    /// Creates a new App instance with the given configuration.
    ///
    /// Initializes the application with a welcome message, empty chat history,
    /// and default UI state. The welcome message includes quick help information
    /// to get users started.
    ///
    /// # Arguments
    ///
    /// * `config` - Application configuration containing LLM provider settings
    ///
    /// # Returns
    ///
    /// A new `App` instance ready for use
    ///
    /// # Examples
    ///
    /// ```rust
    /// use perspt::ui::App;
    /// use perspt::config::AppConfig;
    ///
    /// let config = AppConfig::load().unwrap();
    /// let app = App::new(config);
    /// assert!(!app.should_quit);
    /// assert!(app.chat_history.len() > 0); // Welcome message
    /// ```
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
            response_id: None,
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
            streaming_buffer: String::new(),
            current_response_id: None,
            next_response_id: 1,
            cursor_position: 0,
            input_scroll_offset: 0,
            last_animation_tick: Instant::now(),
            needs_redraw: true,
            input_width: 80, // Default width, will be updated during render
            cursor_blink_state: true,
            last_cursor_blink: Instant::now(),
            terminal_height: 24, // Default height, will be updated during render
            terminal_width: 80, // Default width, will be updated during render
        }
    }

    /// Generates a formatted timestamp string for message display.
    ///
    /// Creates a timestamp in HH:MM format based on the current system time.
    /// Used for timestamping chat messages to help users track conversation flow.
    ///
    /// # Returns
    ///
    /// A formatted timestamp string in "HH:MM" format
    ///
    /// # Examples
    ///
    /// ```rust
    /// use perspt::ui::App;
    ///
    /// let timestamp = App::get_timestamp();
    /// assert!(timestamp.len() == 5); // "HH:MM" format
    /// assert!(timestamp.contains(':'));
    /// ```
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

    /// Adds a new message to the chat history.
    ///
    /// Automatically timestamps the message and scrolls the view to the bottom
    /// to show the new message. This is the primary method for adding any type
    /// of message to the chat interface.
    ///
    /// # Arguments
    ///
    /// * `message` - The chat message to add (will be timestamped automatically)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use perspt::ui::{App, ChatMessage, MessageType};
    /// use perspt::config::AppConfig;
    /// use ratatui::text::Line;
    ///
    /// let config = AppConfig::load().unwrap();
    /// let mut app = App::new(config);
    ///
    /// let message = ChatMessage {
    ///     message_type: MessageType::User,
    ///     content: vec![Line::from("Hello!")],
    ///     timestamp: String::new(), // Will be set automatically
    ///     response_id: None,
    /// };
    /// 
    /// app.add_message(message);
    /// ```
    pub fn add_message(&mut self, mut message: ChatMessage) {
        message.timestamp = Self::get_timestamp();
        // Ensure response_id is handled if not already set by caller
        if message.response_id.is_none() && (message.message_type == MessageType::User || message.message_type == MessageType::Error || message.message_type == MessageType::System) {
            // User messages and general system/error messages not tied to a specific response stream get None
        }
        self.chat_history.push(message);
        self.scroll_to_bottom();
        self.needs_redraw = true;
    }

    /// Adds an error to both the error state and chat history.
    ///
    /// Creates a formatted error message that appears in the chat history
    /// and sets the current error state for display in the status bar.
    /// Errors are styled with red coloring and error icons.
    ///
    /// # Arguments
    ///
    /// * `error` - The error state to display
    ///
    /// # Examples
    ///
    /// ```rust
    /// use perspt::ui::{App, ErrorState, ErrorType};
    /// use perspt::config::AppConfig;
    ///
    /// let config = AppConfig::load().unwrap();
    /// let mut app = App::new(config);
    ///
    /// let error = ErrorState {
    ///     message: "Network connection failed".to_string(),
    ///     details: Some("Check your internet connection".to_string()),
    ///     error_type: ErrorType::Network,
    /// };
    ///
    /// app.add_error(error);
    /// ```
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
            response_id: None,
        });
    }

    /// Clears the current error state.
    ///
    /// Removes any active error from the status bar display, allowing normal
    /// status messages to be shown again.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use perspt::ui::App;
    /// use perspt::config::AppConfig;
    ///
    /// let config = AppConfig::load().unwrap();
    /// let mut app = App::new(config);
    ///
    /// // After adding an error...
    /// app.clear_error();
    /// assert!(app.current_error.is_none());
    /// ```
    pub fn clear_error(&mut self) {
        self.current_error = None;
    }

    /// Sets the status bar message.
    ///
    /// Updates the status message displayed at the bottom of the interface.
    /// Can be used for both informational messages and error notifications.
    ///
    /// # Arguments
    ///
    /// * `message` - The status message to display
    /// * `is_error` - Whether this is an error message (affects logging)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use perspt::ui::App;
    /// use perspt::config::AppConfig;
    ///
    /// let config = AppConfig::load().unwrap();
    /// let mut app = App::new(config);
    ///
    /// app.set_status("Processing request...".to_string(), false);
    /// app.set_status("Connection failed".to_string(), true);
    /// ```
    pub fn set_status(&mut self, message: String, is_error: bool) {
        self.status_message = message;
        if is_error {
            log::error!("Status error: {}", self.status_message);
        }
    }

    /// Updates the animated typing indicator.
    ///
    /// Creates a spinning animation to show when the LLM is generating a response.
    /// The animation cycles through different Unicode spinner characters based on
    /// system time to create smooth motion.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use perspt::ui::App;
    /// use perspt::config::AppConfig;
    ///
    /// let config = AppConfig::load().unwrap();
    /// let mut app = App::new(config);
    ///
    /// app.is_llm_busy = true;
    /// app.update_typing_indicator();
    /// assert!(!app.typing_indicator.is_empty());
    /// ```
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

    /// Scrolls the chat view up by one position.
    ///
    /// Allows users to view earlier messages in the chat history.
    /// Updates the scroll state and position for proper display.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use perspt::ui::App;
    /// use perspt::config::AppConfig;
    ///
    /// let config = AppConfig::load().unwrap();
    /// let mut app = App::new(config);
    ///
    /// let initial_pos = app.scroll_position;
    /// app.scroll_up();
    /// // Position may change depending on chat history
    /// ```
    pub fn scroll_up(&mut self) {
        if self.scroll_position > 0 {
            self.scroll_position -= 1;
            self.update_scroll_state();
        }
    }

    /// Scrolls the chat view down by one position.
    ///
    /// Allows users to move toward more recent messages in the chat history.
    /// Will not scroll past the last message in the history.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use perspt::ui::App;
    /// use perspt::config::AppConfig;
    ///
    /// let config = AppConfig::load().unwrap();
    /// let mut app = App::new(config);
    ///
    /// app.scroll_down();
    /// // Position updated based on available content
    /// ```
    pub fn scroll_down(&mut self) {
        if self.scroll_position < self.max_scroll() {
            self.scroll_position += 1;
            self.update_scroll_state();
        }
    }

    /// Scrolls the chat view to the bottom (most recent messages).
    ///
    /// Automatically called when new messages are added to ensure users
    /// see the latest content. Can be manually called to return to the
    /// bottom of the conversation.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use perspt::ui::App;
    /// use perspt::config::AppConfig;
    ///
    /// let config = AppConfig::load().unwrap();
    /// let mut app = App::new(config);
    ///
    /// app.scroll_to_bottom();
    /// assert_eq!(app.scroll_position, app.max_scroll());
    /// ```
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_position = self.max_scroll();
        self.update_scroll_state();
    }

    /// Calculates the maximum scroll position based on content height and terminal height.
    ///
    /// Determines how far the user can scroll based on the total number
    /// of lines in the chat history and the available display area.
    ///
    /// # Returns
    ///
    /// The maximum valid scroll position
    fn max_scroll(&self) -> usize {
        // Calculate total content lines more accurately
        let total_content_lines: usize = self.chat_history
            .iter()
            .map(|msg| {
                // Header line (1) + content lines + empty separator line (1)
                2 + msg.content.len()
            })
            .sum();
        
        // Account for the visible height of the chat area
        // Terminal height minus header(3) + input(5) + status(3) = 11 reserved lines
        let visible_height = self.terminal_height.saturating_sub(11).max(1);
        
        if total_content_lines > visible_height {
            total_content_lines.saturating_sub(visible_height)
        } else {
            0
        }
    }

    /// Updates the internal scroll state for display.
    ///
    /// Synchronizes the scroll position with the UI scrollbar state.
    /// Called automatically by scroll methods to maintain consistency.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use perspt::ui::App;
    /// use perspt::config::AppConfig;
    ///
    /// let config = AppConfig::load().unwrap();
    /// let mut app = App::new(config);
    ///
    /// app.scroll_position = 5;
    /// app.update_scroll_state();
    /// // Internal state updated
    /// ```
    pub fn update_scroll_state(&mut self) {
        let _max_scroll = self.max_scroll();
        self.scroll_state = self.scroll_state.position(self.scroll_position);
    }

    /// Insert character at cursor position with immediate feedback
    pub fn insert_char(&mut self, ch: char) {
        if !self.is_input_disabled {
            self.input_text.insert(self.cursor_position, ch);
            self.cursor_position += 1;
            self.update_input_scroll();
            self.needs_redraw = true;
            // Reset cursor blink when typing
            self.cursor_blink_state = true;
            self.last_cursor_blink = Instant::now();
        }
    }

    /// Delete character before cursor (backspace) with immediate feedback
    pub fn delete_char_before(&mut self) {
        if !self.is_input_disabled && self.cursor_position > 0 {
            self.input_text.remove(self.cursor_position - 1);
            self.cursor_position -= 1;
            self.update_input_scroll();
            self.needs_redraw = true;
            // Reset cursor blink when editing
            self.cursor_blink_state = true;
            self.last_cursor_blink = Instant::now();
        }
    }

    /// Delete character at cursor (delete key)
    pub fn delete_char_at(&mut self) {
        if !self.is_input_disabled && self.cursor_position < self.input_text.len() {
            self.input_text.remove(self.cursor_position);
            self.update_input_scroll();
            self.needs_redraw = true;
        }
    }

    /// Move cursor left
    pub fn move_cursor_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
            self.update_input_scroll();
            self.needs_redraw = true;
        }
    }

    /// Move cursor right
    pub fn move_cursor_right(&mut self) {
        if self.cursor_position < self.input_text.len() {
            self.cursor_position += 1;
            self.update_input_scroll();
            self.needs_redraw = true;
        }
    }

    /// Move cursor to start of line
    pub fn move_cursor_to_start(&mut self) {
        self.cursor_position = 0;
        self.input_scroll_offset = 0;
        self.needs_redraw = true;
    }

    /// Move cursor to end of line
    pub fn move_cursor_to_end(&mut self) {
        self.cursor_position = self.input_text.len();
        self.update_input_scroll();
        self.needs_redraw = true;
    }

    /// Update input scroll to keep cursor visible
    fn update_input_scroll(&mut self) {
        let visible_width = self.input_width.saturating_sub(4); // Account for borders and padding
        
        if self.cursor_position < self.input_scroll_offset {
            self.input_scroll_offset = self.cursor_position;
        } else if self.cursor_position >= self.input_scroll_offset + visible_width {
            self.input_scroll_offset = self.cursor_position.saturating_sub(visible_width) + 1;
        }
    }

    /// Get visible portion of input text
    pub fn get_visible_input(&self) -> (&str, usize) {
        let visible_width = self.input_width.saturating_sub(4);
        let start = self.input_scroll_offset;
        let end = (start + visible_width).min(self.input_text.len());
        let visible_text = &self.input_text[start..end];
        let cursor_pos = self.cursor_position.saturating_sub(start);
        (visible_text, cursor_pos)
    }

    /// Clear input and reset cursor
    pub fn clear_input(&mut self) {
        self.input_text.clear();
        self.cursor_position = 0;
        self.input_scroll_offset = 0;
        self.needs_redraw = true;
    }

    /// Get input text for sending (trims and clears)
    pub fn take_input(&mut self) -> Option<String> {
        let text = self.input_text.trim().to_string();
        if !text.is_empty() && !self.is_input_disabled {
            self.clear_input();
            Some(text)
        } else {
            None
        }
    }

    /// Start streaming response with immediate feedback and state protection
    pub fn start_streaming(&mut self) {
        log::debug!("start_streaming: Called. Current state: is_llm_busy={}, current_response_id={:?}, streaming_buffer_len={}",
                   self.is_llm_busy, self.current_response_id, self.streaming_buffer.len());
        // Ensure we're in a clean state before starting new stream
        if self.is_llm_busy {
            log::warn!("Starting new stream while already busy - forcing clean state");
            // The call to self.finish_streaming() here needs an origin_id.
            // This situation implies a logic error or an unexpected state.
            // For now, we'll use a placeholder or perhaps the current_response_id if it exists.
            // This part needs careful consideration if this state is reachable.
            // Assuming finish_streaming should be called with the ID it's trying to finish.
            if let Some(id_to_finish) = self.current_response_id {
                self.finish_streaming(id_to_finish);
            } else {
                // If there's no current_response_id, what to do?
                // For now, log and proceed with setting up new stream.
                log::warn!("start_streaming: Was busy, but no current_response_id to pass to finish_streaming.");
            }
        }
        
        self.current_response_id = Some(self.next_response_id);
        self.next_response_id += 1;

        self.is_llm_busy = true;
        self.is_input_disabled = true;
        self.response_progress = 0.0;
        self.streaming_buffer.clear(); // Clear buffer for the new stream
        log::debug!("start_streaming: Set up for new stream. current_response_id={:?}, streaming_buffer_len={} (should be 0). Placeholder will be added.",
                   self.current_response_id, self.streaming_buffer.len());

        // Create a new assistant message immediately with the current response ID
        let initial_message = ChatMessage {
            message_type: MessageType::Assistant,
            content: vec![Line::from("...")], // Placeholder content
            timestamp: Self::get_timestamp(),
            response_id: self.current_response_id,
        };
        self.chat_history.push(initial_message);
        
        self.needs_redraw = true;
        self.typing_indicator = "⠋".to_string(); // Start with first spinner frame
        self.set_status("🚀 Sending request...".to_string(), false);
        log::debug!("Started streaming mode with new assistant message");
    }

    /// Finish streaming response with clean state reset and final content preservation
    pub fn finish_streaming(&mut self, origin_id: usize) { // Added origin_id
        log::debug!("finish_streaming: Called with origin_id={}, current_ui_response_id={:?}, streaming_buffer_len={}",
                   origin_id, self.current_response_id, self.streaming_buffer.len());
        
        // CRITICAL: If the EOT is for the stream currently active in the UI,
        // the streaming_buffer is relevant to it. Otherwise, the buffer might contain
        // content from the *new* current_response_id if a new request started quickly.
        // We should only flush buffer to message with `origin_id` if `Some(origin_id) == self.current_response_id`
        // OR if we decide that buffer is *always* for the ID that just sent EOT.
        // For now, let's assume buffer is for the EOT-ing stream *if that stream was the last one writing to it*.
        // This is tricky. A safer way: buffer is *only* for self.current_response_id.
        // If an EOT for an *old* stream comes, its content is already in chat_history. We just clean up its placeholder.

        if !self.streaming_buffer.is_empty() {
            // If this EOT (origin_id) matches the stream the UI is currently tracking (current_response_id)
            // then the buffer content is for this message.
            if Some(origin_id) == self.current_response_id {
                let mut found_message_to_update = false;
                for msg in self.chat_history.iter_mut().rev() {
                    if msg.message_type == MessageType::Assistant && msg.response_id == Some(origin_id) {
                        msg.content = markdown_to_lines(&self.streaming_buffer);
                        msg.timestamp = Self::get_timestamp();
                        log::debug!("finish_streaming: Flushed buffer to message with response_id {:?}.", msg.response_id);
                        log::debug!("FINAL BUFFER FLUSH: Assistant message (ID: {}) updated with {} chars from buffer.",
                                   origin_id, self.streaming_buffer.len());
                        found_message_to_update = true;
                        break;
                    }
                }
                if !found_message_to_update {
                    log::warn!("finish_streaming: Buffer not empty, EOT for active stream ID {}, but no matching message found to flush to.", origin_id);
                }
                self.streaming_buffer.clear();
                log::debug!("finish_streaming: streaming_buffer cleared.");
            } else {
                // EOT is for an older stream. The buffer is likely for a *newer* stream. Don't touch it here.
                // The content for the older stream (origin_id) should already be finalized in its message.
                log::debug!("finish_streaming: EOT for non-active stream ID {}. Buffer content ({} chars) preserved for current active stream ({:?}).",
                           origin_id, self.streaming_buffer.len(), self.current_response_id);
            }
        } else { // Streaming buffer is empty
            // If no content in buffer, check if we have a placeholder for *this specific origin_id* and remove it.
            if let Some(pos) = self.chat_history.iter().rposition(|msg| msg.message_type == MessageType::Assistant && msg.response_id == Some(origin_id)) {
                let msg = &self.chat_history[pos];
                let is_placeholder = msg.content.is_empty() ||
                    (msg.content.len() == 1 &&
                     msg.content[0].spans.len() == 1 &&
                     (msg.content[0].spans[0].content == "..." ||
                      msg.content[0].spans[0].content.trim().is_empty()));
                if is_placeholder {
                    self.chat_history.remove(pos);
                    log::debug!("Removed placeholder assistant message (ID: {}) as no content was received for it.", origin_id);
                }
            }
            log::debug!("No content in streaming buffer to finalize for response_id: {}.", origin_id);
        }
        
        // Only reset main app state if the EOT corresponds to the *currently active* UI stream
        if Some(origin_id) == self.current_response_id {
            self.is_llm_busy = false;
            self.is_input_disabled = false;
            // Store the ID that caused this finish, for handle_llm_response logic, then clear current_response_id
            CURRENT_RESPONSE_ID_BEFORE_FINISH_FOR_EOT.with(|cell| cell.set(self.current_response_id));
            self.current_response_id = None;
            log::debug!("Cleared current_response_id due to EOT for active stream {}", origin_id);

            // Update status only if this was the active stream ending
            self.set_status("✅ Ready".to_string(), false);
            self.clear_error();
        } else {
            log::debug!("Received EOT for non-active stream_id: {} (current_id is {:?}). Chat history updated, but active UI state (busy, input_disabled, current_id) unchanged.", origin_id, self.current_response_id);
        }
        
        // These can be reset more generally or also conditioned if needed
        self.response_progress = 1.0; // Show completion
        self.typing_indicator.clear();
        
        self.scroll_to_bottom(); // Ensure visibility of any changes
        self.needs_redraw = true;
        
        log::debug!("finish_streaming for ID {} completed. Chat history has {} messages. Current active UI stream ID is now {:?}.",
                   origin_id, self.chat_history.len(), self.current_response_id);
    }

    /// Update streaming content with optimized rendering and immediate feedback
    pub fn update_streaming_content(&mut self, content: &str, origin_id: usize) { // Added origin_id
        // Only process non-empty content
        if content.is_empty() {
            return;
        }
        log::debug!("update_streaming_content: Called with origin_id={}, current_ui_response_id={:?}, incoming_chunk_len={}, current_streaming_buffer_len={}",
                   origin_id, self.current_response_id, content.len(), self.streaming_buffer.len());

        if Some(origin_id) == self.current_response_id {
            // This chunk is for the currently active UI stream. Proceed.
            log::debug!("update_streaming_content: Matched origin_id ({}) with current_ui_response_id. Appending chunk to buffer.", origin_id);
            if self.streaming_buffer.len() + content.len() > MAX_STREAMING_BUFFER_SIZE {
                log::warn!("Streaming buffer approaching limit for active stream {}, truncating old content", origin_id);
                let keep_from = self.streaming_buffer.len() / 5;
                self.streaming_buffer = self.streaming_buffer[keep_from..].to_string();
            }
            self.streaming_buffer.push_str(content);

            let mut found_message_to_update = false;
            // current_id here will be Some(origin_id) due to the outer check
            if let Some(current_id) = self.current_response_id {
                for msg in self.chat_history.iter_mut().rev() {
                    if msg.message_type == MessageType::Assistant && msg.response_id == Some(current_id) {
                        msg.content = markdown_to_lines(&self.streaming_buffer);
                        msg.timestamp = Self::get_timestamp();
                        found_message_to_update = true;
                        break;
                    }
                }
            }
            if !found_message_to_update {
                log::error!("update_streaming_content: Expected assistant message for active ID {} not found in chat_history.", origin_id);
                // This case is problematic, as we've already added to buffer.
                // For now, we log and proceed to UI update logic.
            }
        } else {
            // Chunk is for an older/unexpected stream, or UI isn't expecting any stream.
            log::warn!("update_streaming_content: Received chunk for response_id: {} but current_response_id is {:?}. Discarding chunk: '{}'",
                       origin_id, self.current_response_id, content.chars().take(50).collect::<String>());
            return; // DO NOT append to streaming_buffer or update any message.
        }
        
        // FIXED: More responsive UI update strategy that prevents freezing during long responses
        // Balance performance with responsiveness by using multiple update triggers
        let buffer_len = self.streaming_buffer.len();
        
        // Track when we last updated the UI to ensure regular updates
        thread_local! {
            static LAST_UI_UPDATE_SIZE: std::cell::Cell<usize> = std::cell::Cell::new(0);
        }
        let last_update_size = LAST_UI_UPDATE_SIZE.with(|c| c.get());
        let size_since_last_update = buffer_len.saturating_sub(last_update_size);
        
        let should_redraw_ui = 
            // Always update for small content (responsive for short responses)
            buffer_len < SMALL_BUFFER_THRESHOLD ||
            
            // Regular interval updates (every 250 chars for better responsiveness)
            size_since_last_update >= (UI_UPDATE_INTERVAL / 2) ||
            
            // Content-based triggers for better UX
            content.contains('\n') ||      // Line breaks
            content.contains("```") ||     // Code blocks
            content.contains("##") ||      // Headers
            content.contains("**") ||      // Bold text
            content.contains("*") ||       // Italic text or bullet points
            content.contains("- ") ||      // List items
            content.contains(". ") ||      // Sentence endings
            content.contains("? ") ||      // Questions
            content.contains("! ") ||      // Exclamations
            
            // Time-based fallback: ensure UI updates at least every few chunks
            // This prevents long freezes even if content doesn't match patterns
            size_since_last_update >= 200; // Force update every 200 chars minimum
        
        if should_redraw_ui {
            // Update our tracking of when we last updated
            LAST_UI_UPDATE_SIZE.with(|c| c.set(buffer_len));
            
            // Update progress and ensure visibility
            self.response_progress = (self.response_progress + 0.05).min(0.95);
            self.scroll_to_bottom(); // Ensure latest content is visible
            self.needs_redraw = true;
            
            // Force immediate status update for better feedback
            self.set_status(format!("{}  Receiving response... ({} chars, {}% complete)", 
                self.typing_indicator, 
                buffer_len,
                (self.response_progress * 100.0) as u8
            ), false);
        } else {
            // Even if we don't redraw UI, still update the progress indicator
            self.response_progress = (self.response_progress + 0.01).min(0.95);
        }
    }

    /// Tick for animations and periodic updates with smoother frame rate
    pub fn tick(&mut self) {
        let now = Instant::now();
        
        // Handle cursor blinking
        if now.duration_since(self.last_cursor_blink) >= Duration::from_millis(500) {
            self.cursor_blink_state = !self.cursor_blink_state;
            self.last_cursor_blink = now;
            if !self.is_input_disabled {
                self.needs_redraw = true;
            }
        }
        
        // Increase animation frequency for smoother experience
        if now.duration_since(self.last_animation_tick) >= Duration::from_millis(50) {
            if self.is_llm_busy {
                self.update_typing_indicator();
                self.needs_redraw = true;
            }
            self.last_animation_tick = now;
        }
    }
}

/// Runs the enhanced UI event loop with real-time responsiveness.
///
/// This improved version separates event handling from rendering and provides
/// immediate feedback for user input without blocking timeouts.
pub async fn run_ui(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, 
    config: AppConfig,
    model_name: String, 
    api_key: String,
    provider: Arc<GenAIProvider>
) -> Result<()> {
    let mut app = App::new(config);
    let (tx, mut rx) = mpsc::unbounded_channel::<(usize, String)>(); // Changed type
    
    // Create event stream for responsive event handling
    let mut event_stream = EventStream::new();
    let mut tick_interval = tokio::time::interval(Duration::from_millis(50)); // Slower tick for efficiency
    let mut render_interval = tokio::time::interval(Duration::from_millis(16)); // ~60 FPS for smooth rendering

    loop {
        // Use tokio::select! with proper priorities for responsive UI
        tokio::select! {
            // Highest priority: Handle LLM responses immediately for real-time streaming
            // Process ALL available messages to prevent backlog and prioritize EOT signals
            llm_message = rx.recv() => {
                if let Some(tagged_message) = llm_message { // tagged_message is (usize, String)
                    // Collect all messages from the channel first to prioritize EOT signals
                    let mut all_tagged_messages = vec![tagged_message];
                    while let Ok(additional_tagged_message) = rx.try_recv() {
                        all_tagged_messages.push(additional_tagged_message);
                    }
                    
                    log::info!("=== UI PROCESSING === {} tagged messages received", all_tagged_messages.len());
                    
                    // Process EOT signals FIRST to prevent state confusion
                    // let mut content_messages: Vec<String> = Vec::new(); // Old
                    let mut content_tagged_messages: Vec<(usize, String)> = Vec::new(); // New
                    let mut eot_count = 0;
                    // let mut total_content_chars = 0; // Can be re-calculated if needed for logging
                    
                    for (i, (origin_id, msg_content)) in all_tagged_messages.iter().enumerate() {
                        if msg_content == crate::EOT_SIGNAL {
                            eot_count += 1;
                            log::info!(">>> EOT SIGNAL #{} (ID: {}) found at position {} <<<", eot_count, origin_id, i);
                            if eot_count == 1 { // Process only the first EOT in a batch for now
                                // Process all accumulated content messages before this EOT
                                log::info!("Processing {} content messages before EOT (ID: {})",
                                          content_tagged_messages.len(), origin_id);
                                for (j, tagged_content_msg) in content_tagged_messages.iter().enumerate() {
                                    log::debug!("Processing content message {}/{}: ID {}, {} chars",
                                               j+1, content_tagged_messages.len(), tagged_content_msg.0, tagged_content_msg.1.len());
                                    handle_llm_response(&mut app, tagged_content_msg.clone(), &provider, &model_name, &tx).await;
                                }
                                content_tagged_messages.clear();
                                // Now process the EOT signal itself
                                handle_llm_response(&mut app, (*origin_id, msg_content.clone()), &provider, &model_name, &tx).await;
                                // Consider if we should break here or let other EOTs for *different* IDs be processed.
                                // For now, let's process one EOT and its preceding messages per batch.
                                // This break is important to re-evaluate `all_tagged_messages` if new messages arrived during processing.
                                break;
                            } else {
                                // This EOT is for a different response_id or a duplicate for the same one within the same batch.
                                // The handle_llm_response will manage its specific origin_id.
                                log::warn!("Additional EOT signal #{} (ID: {}) in batch, will be processed if loop continues.", eot_count, origin_id);
                                // We'll let it be processed if it's the next item after the break, or in the `content_tagged_messages` loop below.
                                // To ensure it's handled if it's the *last* thing, or if we want to handle all EOTs in a batch:
                                content_tagged_messages.push((*origin_id, msg_content.clone()));
                            }
                        } else {
                            // total_content_chars += msg_content.len();
                            content_tagged_messages.push((*origin_id, msg_content.clone()));
                        }
                    }
                    
                    // If no EOT signal was processed via the eot_count == 1 path (e.g. no EOT, or multiple EOTs)
                    // process all remaining (or all if no EOT) messages.
                    if eot_count == 0 || !content_tagged_messages.is_empty() {
                        if eot_count == 0 {
                            log::info!("No EOT signal found in batch, processing {} remaining content messages",
                                content_tagged_messages.len());
                        } else {
                            log::info!("Processing {} remaining messages after first EOT (or additional EOTs)", content_tagged_messages.len());
                        }
                        for (j, tagged_msg) in content_tagged_messages.into_iter().enumerate() {
                            log::debug!("Processing remaining/additional message {}: ID {}, {} chars", j+1, tagged_msg.0, tagged_msg.1.len());
                            handle_llm_response(&mut app, tagged_msg, &provider, &model_name, &tx).await;
                        }
                    }
                    
                    app.needs_redraw = true;
                    // Force immediate redraw for streaming responses
                    terminal.draw(|f| {
                        draw_enhanced_ui(f, &mut app, &model_name);
                    })?;
                    app.needs_redraw = false;
                }
            }
            
            // Second priority: Handle terminal events for user interaction
            event_result = event_stream.next() => {
                if let Some(Ok(event)) = event_result {
                    if let Some(app_event) = handle_terminal_event(&mut app, event, &tx, &api_key, &model_name, &provider).await {
                        match app_event {
                            AppEvent::Quit => break,
                            AppEvent::Redraw => {
                                app.needs_redraw = true;
                                // Force immediate redraw for user input responsiveness
                                terminal.draw(|f| {
                                    draw_enhanced_ui(f, &mut app, &model_name);
                                })?;
                                app.needs_redraw = false;
                            }
                            _ => {}
                        }
                    }
                }
            }
            
            // Third priority: Regular rendering updates
            _ = render_interval.tick() => {
                app.tick(); // Handle animations and cursor blinking
                
                if app.needs_redraw {
                    terminal.draw(|f| {
                        draw_enhanced_ui(f, &mut app, &model_name);
                    })?;
                    app.needs_redraw = false;
                }
            }
            
            // Lowest priority: General tick for cleanup and background tasks
            _ = tick_interval.tick() => {
                // Additional background processing if needed
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Cleanup terminal
    terminal::disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    
    Ok(())
}

/// Enhanced event stream wrapper for non-blocking event handling
struct EventStream;

impl EventStream {
    fn new() -> Self {
        Self
    }

    async fn next(&mut self) -> Option<Result<Event, io::Error>> {
        // Use a small timeout to balance responsiveness and CPU usage
        // This allows the event loop to process other tasks while still being responsive
        if let Ok(true) = event::poll(Duration::from_millis(10)) {
            match event::read() {
                Ok(event) => Some(Ok(event)),
                Err(e) => Some(Err(e)),
            }
        } else {
            // Return None to allow other tasks in the select! loop to run
            None
        }
    }
}

/// Application events for the enhanced UI loop
/// (This duplicate enum has been removed - using the main AppEvent enum above)

/// Handle terminal events with immediate response
async fn handle_terminal_event(
    app: &mut App,
    event: Event,
    tx: &mpsc::UnboundedSender<(usize, String)>, // Changed
    _api_key: &str,
    model_name: &str,
    provider: &Arc<GenAIProvider>,
) -> Option<AppEvent> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            match key.code {
                // Quit commands
                KeyCode::Char('q') | KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.should_quit = true;
                    return Some(AppEvent::Quit);
                }
                
                // Help toggle
                KeyCode::F(1) => {
                    app.show_help = !app.show_help;
                    return Some(AppEvent::Redraw);
                }
                
                // Escape key
                KeyCode::Esc => {
                    if app.show_help {
                        app.show_help = false;
                        return Some(AppEvent::Redraw);
                    } else {
                        app.should_quit = true;
                        return Some(AppEvent::Quit);
                    }
                }
                
                // Send message
                KeyCode::Enter => {
                    if let Some(input) = app.take_input() {
                        // Add user message immediately for instant feedback
                        app.add_message(ChatMessage {
                            message_type: MessageType::User,
                            content: vec![Line::from(input.clone())],
                            timestamp: App::get_timestamp(),
                            response_id: None, // User messages don't have a response_id
                        });
                        
                        // Start LLM request
                        app.start_streaming();
                        let response_id = app.current_response_id.unwrap_or_else(|| {
                            log::error!("current_response_id is None after start_streaming in handle_terminal_event, using 0 as fallback");
                            0 // Fallback, though current_response_id should always be Some here
                        });
                        tokio::spawn(initiate_llm_request_enhanced(
                            input,
                            Arc::clone(provider),
                            model_name.to_string(),
                            response_id, // Pass the ID
                            tx.clone(),
                        ));
                        
                        return Some(AppEvent::Redraw);
                    } else if app.is_input_disabled && !app.input_text.trim().is_empty() {
                        // Queue input if busy
                        let input = app.input_text.trim().to_string();
                        app.pending_inputs.push_back(input);
                        app.clear_input();
                        app.set_status(format!("Message queued ({})", app.pending_inputs.len()), false);
                        return Some(AppEvent::Redraw);
                    }
                }
                
                // Character input
                KeyCode::Char(c) => {
                    if !app.show_help {
                        app.insert_char(c);
                        return Some(AppEvent::Redraw);
                    }
                }
                
                // Backspace
                KeyCode::Backspace => {
                    if !app.show_help {
                        app.delete_char_before();
                        return Some(AppEvent::Redraw);
                    }
                }
                
                // Delete
                KeyCode::Delete => {
                    if !app.show_help {
                        app.delete_char_at();
                        return Some(AppEvent::Redraw);
                    }
                }
                
                // Cursor movement
                KeyCode::Left => {
                    if !app.show_help {
                        app.move_cursor_left();
                        return Some(AppEvent::Redraw);
                    }
                }
                KeyCode::Right => {
                    if !app.show_help {
                        app.move_cursor_right();
                        return Some(AppEvent::Redraw);
                    }
                }
                KeyCode::Home => {
                    if app.show_help {
                        app.scroll_position = 0;
                        app.update_scroll_state();
                    } else {
                        app.move_cursor_to_start();
                    }
                    return Some(AppEvent::Redraw);
                }
                KeyCode::End => {
                    if app.show_help {
                        app.scroll_to_bottom();
                    } else {
                        app.move_cursor_to_end();
                    }
                    return Some(AppEvent::Redraw);
                }
                
                // Scrolling
                KeyCode::Up => {
                    app.scroll_up();
                    return Some(AppEvent::Redraw);
                }
                KeyCode::Down => {
                    app.scroll_down();
                    return Some(AppEvent::Redraw);
                }
                KeyCode::PageUp => {
                    for _ in 0..5 {
                        app.scroll_up();
                    }
                    return Some(AppEvent::Redraw);
                }
                KeyCode::PageDown => {
                    for _ in 0..5 {
                        app.scroll_down();
                    }
                    return Some(AppEvent::Redraw);
                }
                
                _ => {}
            }
        }
        
        Event::Resize(_, _) => {
            return Some(AppEvent::Redraw);
        }
        
        _ => {}
    }
    
    None
}

/// Enhanced LLM request initiation with proper error handling
async fn initiate_llm_request_enhanced(
    input: String,
    provider: Arc<GenAIProvider>,
    model_name: String,
    response_id_for_provider: usize, // Added
    tx: mpsc::UnboundedSender<(usize, String)>, // Changed
) {
    // log::info!("Starting enhanced LLM request: {}", input); // Removed to prevent TUI interference
    
    let result = provider.generate_response_stream_to_channel(
        &model_name,
        &input,
        response_id_for_provider, // Added
        tx.clone(),
    ).await;
    
    match result {
        Ok(()) => {
            log::debug!("Streaming completed successfully");
            // EOT signal is now sent by the provider itself, no need to send it here
        }
        Err(e) => {
            log::error!("LLM request failed: {}", e);
            let _ = tx.send(format!("Error: {}", e));
            let _ = tx.send(crate::EOT_SIGNAL.to_string());
        }
    }
}

/// Handle LLM responses with immediate UI updates
async fn handle_llm_response(
    app: &mut App,
    tagged_message: (usize, String), // Changed
    provider: &Arc<GenAIProvider>,
    model_name: &str,
    tx: &mpsc::UnboundedSender<(usize, String)>, // Changed
) {
    let (origin_id, message_content) = tagged_message; // Unpack

    if message_content == crate::EOT_SIGNAL {
        // Existing log is good:
        log::info!(">>> RECEIVED EOT SIGNAL for response_id: {} (current active UI stream ID: {:?}, buffer: {} chars) <<<",
                   origin_id, app.current_response_id, app.streaming_buffer.len());
        
        let was_active_stream = Some(origin_id) == app.current_response_id;
        app.finish_streaming(origin_id); // Pass origin_id. This might clear app.current_response_id
        
        app.needs_redraw = true;
        
        // Process pending inputs if the EOT was for the stream the UI considered active,
        // and the app is now confirmed not busy (i.e., finish_streaming cleared the busy state).
        if was_active_stream && !app.is_llm_busy {
            if !app.pending_inputs.is_empty() {
                let pending_input = app.pending_inputs.pop_front().unwrap();
                log::info!("Processing pending input after EOT for active stream ID {}: {} chars", origin_id, pending_input.len());

                app.add_message(ChatMessage {
                    message_type: MessageType::User,
                    content: vec![Line::from(pending_input.clone())],
                    timestamp: App::get_timestamp(),
                    response_id: None,
                });

                app.start_streaming();
                let new_response_id = app.current_response_id.unwrap_or_else(|| {
                    log::error!("current_response_id is None after start_streaming for pending input, using 0 as fallback");
                    0
                });
                tokio::spawn(initiate_llm_request_enhanced(
                    pending_input,
                    Arc::clone(provider),
                    model_name.to_string(),
                    new_response_id,
                    tx.clone(),
                ));
            } else {
                log::debug!("No pending inputs to process after EOT for active stream ID {}", origin_id);
            }
        } else {
            if !was_active_stream {
                log::info!("EOT for ID {} received, but it was not the current active UI stream (active: {:?}). Pending inputs not processed based on this EOT.", origin_id, app.current_response_id);
            }
            if app.is_llm_busy && was_active_stream { // Should not happen if finish_streaming worked correctly for active stream
                 log::warn!("EOT for active ID {} received, but app is STILL busy. Pending inputs will wait.", origin_id);
            }
        }

    } else if message_content.starts_with("Error: ") && message_content.len() > 7 {
        let error_msg_content = &message_content[7..];
        log::error!("Received error message for ID {}: {}", origin_id, error_msg_content);
        let error_state = categorize_error(error_msg_content);
        app.add_error(error_state);
        app.finish_streaming(origin_id);
    } else {
        // Regular streaming content
        log::debug!("handle_llm_response: Received content chunk for origin_id={}, len={}. Forwarding to update_streaming_content.",
                   origin_id, message_content.len());
        app.update_streaming_content(&message_content, origin_id);
        if Some(origin_id) == app.current_response_id {
             app.set_status(format!("{}  Receiving response...", app.typing_indicator), false);
        }
    }
}

/// Categorizes error messages into specific error types with helpful details
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

/// Enhanced UI rendering with cursor support and better performance
fn draw_enhanced_ui(f: &mut Frame, app: &mut App, model_name: &str) {
    // Update terminal dimensions
    app.terminal_height = f.area().height as usize;
    app.terminal_width = f.area().width as usize;
    
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(1),     // Chat area (flexible)
            Constraint::Length(5),  // Input area (fixed size for better visibility)
            Constraint::Length(3),  // Status line (increased to prevent overlap)
        ])
        .split(f.area());

    // Update input width for proper scrolling calculations
    app.input_width = main_chunks[2].width as usize;

    // Header with enhanced styling
    draw_enhanced_header(f, main_chunks[0], model_name, app);
    
    // Chat history with scrollbar
    draw_enhanced_chat_area(f, main_chunks[1], app);
    
    // Enhanced input area with cursor
    draw_enhanced_input_area(f, main_chunks[2], app);
    
    // Status line with progress
    draw_enhanced_status_line(f, main_chunks[3], app);

    // Help overlay if needed
    if app.show_help {
        draw_enhanced_help_overlay(f, app);
    }
}

/// Enhanced header with better visual hierarchy
fn draw_enhanced_header(f: &mut Frame, area: Rect, model_name: &str, app: &App) {
    let status_text = if app.is_llm_busy {
        "🤔 Thinking..."
    } else if !app.pending_inputs.is_empty() {
        "⏳ Queued"
    } else {
        "✅ Ready"
    };

    let status_color = if app.is_llm_busy {
        Color::Yellow
    } else if !app.pending_inputs.is_empty() {
        Color::Blue
    } else {
        Color::Green
    };

    let header_content = vec![
        Line::from(vec![
            Span::styled("🧠 ", Style::default().fg(Color::Magenta)),
            Span::styled("Perspt", Style::default().fg(Color::Magenta).bold()),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled("Model: ", Style::default().fg(Color::Gray)),
            Span::styled(model_name, Style::default().fg(Color::Cyan).bold()),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled("Status: ", Style::default().fg(Color::Gray)),
            Span::styled(status_text, Style::default().fg(status_color).bold()),
        ]),
    ];

    let header = Paragraph::new(header_content)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" AI Chat Terminal ")
            .title_style(Style::default().fg(Color::Magenta).bold()))
        .alignment(Alignment::Center);

    f.render_widget(header, area);
}

/// Enhanced chat area with better scrolling and formatting
fn draw_enhanced_chat_area(f: &mut Frame, area: Rect, app: &mut App) {
    let chat_content: Vec<Line> = app.chat_history
        .iter()
        .flat_map(|msg| {
            let (icon, style) = match msg.message_type {
                MessageType::User => ("👤", Style::default().fg(Color::Blue).bold()),
                MessageType::Assistant => ("🤖", Style::default().fg(Color::Green).bold()),
                MessageType::Error => ("❌", Style::default().fg(Color::Red).bold()),
                MessageType::System => ("ℹ️", Style::default().fg(Color::Cyan).bold()),
                MessageType::Warning => ("⚠️", Style::default().fg(Color::Yellow).bold()),
            };
            
            let mut lines = vec![
                Line::from(vec![
                    Span::styled(icon, style),
                    Span::styled(" ", Style::default()),
                    Span::styled(
                        match msg.message_type {
                            MessageType::User => "You",
                            MessageType::Assistant => "Assistant",
                            MessageType::Error => "Error",
                            MessageType::System => "System",
                            MessageType::Warning => "Warning",
                        },
                        style
                    ),
                    Span::styled(format!(" • {}", msg.timestamp), Style::default().fg(Color::DarkGray)),
                ]),
            ];
            
            // Use the existing message content directly to avoid formatting issues
            lines.extend(msg.content.iter().cloned());
            lines.push(Line::from("")); // Separator line
            lines
        })
        .collect();

    // Calculate proper scroll position relative to content
    let content_height = chat_content.len();
    let visible_height = area.height.saturating_sub(2) as usize; // Account for borders
    
    // Update scroll state for content - ensure proper bounds
    app.scroll_state = app.scroll_state.content_length(content_height.max(1).saturating_sub(1));
    
    // Ensure scroll position is within valid bounds when content changes
    let max_scroll = if content_height > visible_height {
        content_height.saturating_sub(visible_height)
    } else {
        0
    };
    
    if app.scroll_position > max_scroll {
        app.scroll_position = max_scroll;
        app.update_scroll_state();
    }

    // Create layout for chat and scrollbar
    let chat_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let chat_paragraph = Paragraph::new(chat_content)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::White))
            .title(" Conversation ")
            .title_style(Style::default().fg(Color::White).bold()))
        .wrap(Wrap { trim: true })
        .scroll((app.scroll_position as u16, 0));

    f.render_widget(chat_paragraph, chat_chunks[0]);

    // Render scrollbar
    let scrollbar = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"));
    
    f.render_stateful_widget(scrollbar, chat_chunks[1], &mut app.scroll_state);
}

/// Enhanced input area with visible cursor and better feedback
fn draw_enhanced_input_area(f: &mut Frame, area: Rect, app: &App) {
    let input_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Input field
            Constraint::Length(2),  // Progress bar or hint (properly sized)
        ])
        .split(area);

    // Get visible input and cursor position
    let (visible_input, cursor_pos) = app.get_visible_input();
    
    // Input field styling based on state
    let (border_color, title) = if app.is_input_disabled {
        (Color::DarkGray, " Input (Disabled - AI is thinking...) ")
    } else {
        (Color::Green, " Type your message (Enter to send, F1 for help) ")
    };

    // Create input content with cursor
    let mut input_spans = vec![];
    
    if app.is_input_disabled && visible_input.is_empty() {
        input_spans.push(Span::styled(
            "Waiting for AI response...",
            Style::default().fg(Color::DarkGray).italic()
        ));
    } else {
        // Split text at cursor position for cursor rendering
        let before_cursor = &visible_input[..cursor_pos.min(visible_input.len())];
        let at_cursor = visible_input.chars().nth(cursor_pos).unwrap_or(' ');
        let after_start = cursor_pos.min(visible_input.len()).saturating_add(1);
        let after_cursor = if after_start <= visible_input.len() {
            &visible_input[after_start..]
        } else {
            ""
        };

        if !before_cursor.is_empty() {
            input_spans.push(Span::styled(before_cursor, Style::default().fg(Color::White)));
        }

        // Cursor character with highlighting and blinking
        if !app.is_input_disabled {
            let cursor_style = if app.cursor_blink_state {
                Style::default().fg(Color::Black).bg(Color::White)
            } else {
                Style::default().fg(Color::White).bg(Color::DarkGray)
            };
            
            input_spans.push(Span::styled(
                at_cursor.to_string(),
                cursor_style
            ));
        }

        if !after_cursor.is_empty() {
            input_spans.push(Span::styled(after_cursor, Style::default().fg(Color::White)));
        }
    }

    let input_paragraph = Paragraph::new(Line::from(input_spans))
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .title(title)
            .title_style(Style::default().fg(border_color)))
        .wrap(Wrap { trim: false });

    f.render_widget(input_paragraph, input_chunks[0]);

    // Progress bar or hint area - properly contained within its own area
    if app.is_llm_busy {
        let progress = Gauge::default()
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Yellow)))
            .gauge_style(Style::default().fg(Color::Yellow).bg(Color::DarkGray))
            .ratio(app.response_progress)
            .label(format!("{}  Processing response...", app.typing_indicator));
        
        f.render_widget(progress, input_chunks[1]);
    } else if !app.pending_inputs.is_empty() {
        let queue_info = Paragraph::new(Line::from(vec![
            Span::styled("📋 Queued messages: ", Style::default().fg(Color::Blue)),
            Span::styled(app.pending_inputs.len().to_string(), Style::default().fg(Color::Blue).bold()),
        ]))
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Blue)))
        .alignment(Alignment::Center);
        
        f.render_widget(queue_info, input_chunks[1]);
    } else {
        // Show helpful hint when idle
        let hint = Paragraph::new(Line::from(vec![
            Span::styled("💡 ", Style::default().fg(Color::Yellow)),
            Span::styled("Press F1 for help • Use ↑/↓ to scroll chat history", 
                        Style::default().fg(Color::DarkGray).italic()),
        ]))
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray)))
        .alignment(Alignment::Center);
        
        f.render_widget(hint, input_chunks[1]);
    }
}

/// Enhanced status line with better error handling
fn draw_enhanced_status_line(f: &mut Frame, area: Rect, app: &App) {
    let status_content = if let Some(error) = &app.current_error {
        vec![Line::from(vec![
            Span::styled("❌ ", Style::default().fg(Color::Red)),
            Span::styled(&error.message, Style::default().fg(Color::Red)),
            Span::styled(" │ Press F1 for help", Style::default().fg(Color::Gray)),
        ])]
    } else {
        let queue_info = if !app.pending_inputs.is_empty() {
            format!(" │ Queued: {}", app.pending_inputs.len())
        } else {
            String::new()
        };

        vec![Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::Gray)),
            Span::styled(&app.status_message, 
                if app.is_llm_busy { 
                    Style::default().fg(Color::Yellow) 
                } else { 
                    Style::default().fg(Color::Green) 
                }),
            Span::styled(queue_info, Style::default().fg(Color::Blue)),
            Span::styled(" │ Ctrl+C to exit", Style::default().fg(Color::Gray)),
        ])]
    };

    let status_paragraph = Paragraph::new(status_content)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Gray)));

    f.render_widget(status_paragraph, area);
}

/// Enhanced help overlay with better formatting
fn draw_enhanced_help_overlay(f: &mut Frame, _app: &App) {
    let popup_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(15),
            Constraint::Percentage(70),
            Constraint::Percentage(15),
        ])
        .split(f.area())[1];

    let popup_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(15),
            Constraint::Percentage(70),
            Constraint::Percentage(15),
        ])
        .split(popup_area)[1];

    f.render_widget(Clear, popup_area);

    let help_content = vec![
        Line::from(""),
        Line::from(vec![Span::styled("📖 Perspt Help & Shortcuts", Style::default().fg(Color::Magenta).bold())]),
        Line::from(""),
        Line::from(vec![Span::styled("🎹 Input Controls:", Style::default().fg(Color::Yellow).bold())]),
        Line::from(vec![Span::styled("  Enter     ", Style::default().fg(Color::Cyan)), Span::styled("Send message", Style::default())]),
        Line::from(vec![Span::styled("  ←/→       ", Style::default().fg(Color::Cyan)), Span::styled("Move cursor", Style::default())]),
        Line::from(vec![Span::styled("  Home/End  ", Style::default().fg(Color::Cyan)), Span::styled("Start/End of line", Style::default())]),
        Line::from(vec![Span::styled("  Backspace ", Style::default().fg(Color::Cyan)), Span::styled("Delete before cursor", Style::default())]),
        Line::from(vec![Span::styled("  Delete    ", Style::default().fg(Color::Cyan)), Span::styled("Delete at cursor", Style::default())]),
        Line::from(""),
        Line::from(vec![Span::styled("📜 Navigation:", Style::default().fg(Color::Yellow).bold())]),
        Line::from(vec![Span::styled("  ↑/↓       ", Style::default().fg(Color::Cyan)), Span::styled("Scroll chat history", Style::default())]),
        Line::from(vec![Span::styled("  PgUp/PgDn ", Style::default().fg(Color::Cyan)), Span::styled("Fast scroll", Style::default())]),
        Line::from(""),
        Line::from(vec![Span::styled("🔧 Application:", Style::default().fg(Color::Yellow).bold())]),
        Line::from(vec![Span::styled("  F1        ", Style::default().fg(Color::Cyan)), Span::styled("Toggle this help", Style::default())]),
        Line::from(vec![Span::styled("  Ctrl+C/Q  ", Style::default().fg(Color::Cyan)), Span::styled("Exit application", Style::default())]),
        Line::from(vec![Span::styled("  Esc       ", Style::default().fg(Color::Cyan)), Span::styled("Close help/Exit", Style::default())]),
        Line::from(""),
        Line::from(vec![Span::styled("✨ Features:", Style::default().fg(Color::Yellow).bold())]),
        Line::from(vec![Span::styled("  • Real-time streaming responses", Style::default().fg(Color::Green))]),
        Line::from(vec![Span::styled("  • Input queuing during AI responses", Style::default().fg(Color::Green))]),
        Line::from(vec![Span::styled("  • Full cursor navigation support", Style::default().fg(Color::Green))]),
        Line::from(vec![Span::styled("  • Live markdown rendering", Style::default().fg(Color::Green))]),
        Line::from(""),
        Line::from(vec![Span::styled("Press F1 or Esc to close", Style::default().fg(Color::Gray).italic())]),
    ];

    let help_popup = Paragraph::new(help_content)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(Color::Magenta))
            .title(" Help ")
            .title_style(Style::default().fg(Color::Magenta).bold()))
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);

    f.render_widget(help_popup, popup_area);
}

/// Renders the application header with model information and status.
///
/// The header displays the application name, current LLM model, and real-time
/// status information including whether the AI is currently processing a request.
///
/// # Arguments
///
/// * `f` - The Ratatui frame to render to
/// * `area` - The rectangular area allocated for the header
/// * `model_name` - Name of the active LLM model
/// * `app` - Current application state for status information
///
/// # Header Content
///
/// ```text
/// ┌─────────────────────────────────────────────────────────┐
/// │ 🧠 Perspt | Model: gpt-4 | Status: Ready              │
/// └─────────────────────────────────────────────────────────┘
/// ```
/// Converts markdown text into styled terminal lines with rich formatting.
///
/// This function parses markdown content and converts it into Ratatui `Line` structures
/// with appropriate styling for display in the terminal interface. It supports various
/// markdown elements including headers, code blocks, lists, emphasis, and more.
///
/// # Arguments
///
/// * `markdown` - The markdown-formatted string to convert
///
/// # Returns
///
/// Returns a vector of `Line<'static>` objects that can be rendered by Ratatui,
/// with appropriate styling applied to different markdown elements.
///
/// # Supported Markdown Elements
///
/// | Element | Styling | Visual Example |
/// |---------|---------|----------------|
/// | **Headers** | Magenta, bold | `# Header` |
/// | **Code Blocks** | Cyan on dark gray background | `┌─ Code Block ─┐` |
/// | **Inline Code** | Cyan on dark gray | ` code ` |
/// | **Bold Text** | Bold modifier | **bold** |
/// | **Italic Text** | Italic modifier | *italic* |
/// | **Lists** | Green bullet points | `• Item` |
/// | **Block Quotes** | Blue vertical bar | `▎ Quote` |
/// | **Line Breaks** | Proper line separation | |
///
/// # Code Block Formatting
///
/// Code blocks are rendered with decorative borders:
/// ```text
/// ┌─ Code Block ─┐
/// let x = 42;
/// println!("{}", x);
/// └─────────────┘
/// ```
///
/// # Examples
///
/// ```rust
/// let markdown = "# Hello\n\nThis is **bold** and *italic* text.\n\n```rust\nlet x = 42;\n```";
/// let lines = markdown_to_lines(markdown);
/// // Returns styled lines ready for terminal rendering
/// ```
///
/// # Performance Considerations
///
/// - Uses basic markdown parsing for better compatibility with ratatui
/// - Handles code blocks, bold, italic, headers, and lists
/// - Optimized for terminal display with appropriate color choices
/// - Handles large documents gracefully
///
/// # Error Handling
///
/// This function is designed to be robust and will handle malformed markdown
/// gracefully, falling back to plain text rendering for unrecognized elements.
fn markdown_to_lines(markdown: &str) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut in_code_block = false;
    let mut code_lang = String::new();
    
    for line in markdown.lines() {
        if line.starts_with("```") {
            if in_code_block {
                // End of code block
                in_code_block = false;
                lines.push(Line::from(vec![
                    Span::styled(
                        "└─────────────┘",
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    )
                ]));
                code_lang.clear();
            } else {
                // Start of code block
                in_code_block = true;
                code_lang = line.trim_start_matches("```").to_string();
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("┌─ {} ─┐", if code_lang.is_empty() { "Code" } else { &code_lang }),
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    )
                ]));
            }
            continue;
        }
        
        if in_code_block {
            // Code block content
            lines.push(Line::from(vec![
                Span::styled(
                    format!("│ {}", line),
                    Style::default().fg(Color::Cyan).bg(Color::DarkGray),
                )
            ]));
            continue;
        }
        
        // Handle headers
        if line.starts_with('#') {
            let level = line.chars().take_while(|c| *c == '#').count();
            let title = line.trim_start_matches('#').trim();
            let color = match level {
                1 => Color::Magenta,
                2 => Color::Yellow,
                3 => Color::Blue,
                _ => Color::Cyan,
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{} {}", "#".repeat(level), title),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                )
            ]));
            continue;
        }
        
        // Handle lists
        if line.trim_start().starts_with('*') || line.trim_start().starts_with('-') {
            let indent = line.len() - line.trim_start().len();
            let content = line.trim_start().trim_start_matches('*').trim_start_matches('-').trim();
            lines.push(Line::from(vec![
                Span::raw(" ".repeat(indent)),
                Span::styled("• ", Style::default().fg(Color::Green)),
                Span::raw(parse_inline_markdown(content)),
            ]));
            continue;
        }
        
        // Handle blockquotes
        if line.trim_start().starts_with('>') {
            let content = line.trim_start().trim_start_matches('>').trim();
            lines.push(Line::from(vec![
                Span::styled("▎ ", Style::default().fg(Color::Blue)),
                Span::styled(
                    parse_inline_markdown(content),
                    Style::default().fg(Color::LightBlue).add_modifier(Modifier::ITALIC),
                ),
            ]));
            continue;
        }
        
        // Regular paragraph text
        if line.trim().is_empty() {
            lines.push(Line::from(""));
        } else {
            lines.push(Line::from(parse_inline_markdown_to_spans(line)));
        }
    }
    
    lines
}

/// Parse inline markdown elements like **bold**, *italic*, and `code`
fn parse_inline_markdown(text: &str) -> String {
    let mut result = text.to_string();
    
    // Remove markdown formatting for plain text
    result = result.replace("**", "");
    result = result.replace("*", "");
    result = result.replace("`", "");
    
    result
}

/// Parse inline markdown elements and return styled spans
fn parse_inline_markdown_to_spans(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut chars = text.chars().peekable();
    let mut current_text = String::new();
    
    while let Some(ch) = chars.next() {
        match ch {
            '*' => {
                if chars.peek() == Some(&'*') {
                    // Bold text **bold**
                    chars.next(); // consume second *
                    if !current_text.is_empty() {
                        spans.push(Span::raw(current_text.clone()));
                        current_text.clear();
                    }
                    
                    let mut bold_text = String::new();
                    while let Some(ch) = chars.next() {
                        if ch == '*' && chars.peek() == Some(&'*') {
                            chars.next(); // consume second *
                            break;
                        }
                        bold_text.push(ch);
                    }
                    spans.push(Span::styled(
                        bold_text,
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                } else {
                    // Italic text *italic*
                    if !current_text.is_empty() {
                        spans.push(Span::raw(current_text.clone()));
                        current_text.clear();
                    }
                    
                    let mut italic_text = String::new();
                    while let Some(ch) = chars.next() {
                        if ch == '*' {
                            break;
                        }
                        italic_text.push(ch);
                    }
                    spans.push(Span::styled(
                        italic_text,
                        Style::default().add_modifier(Modifier::ITALIC),
                    ));
                }
            }
            '`' => {
                // Inline code `code`
                if !current_text.is_empty() {
                    spans.push(Span::raw(current_text.clone()));
                    current_text.clear();
                }
                
                let mut code_text = String::new();
                while let Some(ch) = chars.next() {
                    if ch == '`' {
                        break;
                    }
                    code_text.push(ch);
                }
                spans.push(Span::styled(
                    format!(" {} ", code_text),
                    Style::default().fg(Color::Green).bg(Color::DarkGray),
                ));
            }
            _ => {
                current_text.push(ch);
            }
        }
    }
    
    if !current_text.is_empty() {
        spans.push(Span::raw(current_text));
    }
    
    if spans.is_empty() {
        spans.push(Span::raw(text.to_string()));
    }
    
    spans
}
