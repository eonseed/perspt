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
//! * **Advanced Scrollable Chat History**: Intelligent scrolling with accurate text wrapping calculations
//! * **Progress Indicators**: Visual feedback for LLM response generation
//! * **Error Display**: Comprehensive error handling with user-friendly messages
//! * **Help System**: Built-in help overlay with keyboard shortcuts
//! * **Conversation Saving**: Export chat conversations to text files with `/save` command
//! * **Command Interface**: Built-in command system for app functionality
//! * **Responsive Layout**: Adaptive layout that works across different terminal sizes
//! * **Unicode Text Wrapping**: Accurate character counting for proper terminal text wrapping
//!
//! ## Scroll System Improvements
//!
//! The scroll system has been enhanced to handle long responses reliably:
//! * **Accurate Text Wrapping**: Uses `.chars().count()` for proper Unicode character counting
//! * **Consistent Calculations**: Unified logic between `max_scroll()` and `update_scroll_state()`
//! * **Content Visibility**: Conservative buffer ensures no content is cut off at the bottom
//! * **Separator Line Handling**: Properly accounts for separator lines in scroll calculations
//! * **Debug Logging**: Comprehensive logging for scroll-related troubleshooting
//!
//! ## Architecture
//!
//! The UI follows a component-based architecture:
//! * `App` - Main application state and controller with enhanced scroll management
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
use anyhow::Result;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, Gauge, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Wrap,
    },
    Frame, Terminal,
};
use std::{
    collections::VecDeque,
    io,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::config::AppConfig;
use crate::llm_provider::GenAIProvider;
use tokio::sync::mpsc;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
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
/// styling, timestamp, and message type for proper visual rendering. Also stores
/// the raw content for conversation export functionality.
///
/// # Fields
///
/// * `message_type` - The type of message (User, Assistant, Error, etc.)
/// * `content` - The formatted content as a vector of styled lines
/// * `timestamp` - When the message was created (formatted string)
/// * `raw_content` - Unformatted text content for saving to files
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
///     raw_content: "Hello, AI!".to_string(),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub message_type: MessageType,
    pub content: Vec<Line<'static>>,
    pub timestamp: String,
    /// Raw text content before markdown formatting (for saving to file)
    pub raw_content: String,
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
    pub chat_area_height: usize, // Actual chat area height from layout
    pub chat_area_width: usize,  // Actual chat area width from layout
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
                Line::from(vec![
                    Span::styled("  • ", Style::default().fg(Color::Green)),
                    Span::styled("/save", Style::default().fg(Color::White).bold()),
                    Span::styled(" - Save conversation", Style::default().fg(Color::Gray)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Ready to chat! Type your message below...", Style::default().fg(Color::Green).italic()),
                ]),
                Line::from(""),
            ],
            timestamp: Self::get_timestamp(),
            raw_content: "🌟 Welcome to Perspt - Your AI Chat Terminal\n\n💡 Quick Help:\n  • Enter - Send message\n  • ↑/↓ - Scroll chat history\n  • Ctrl+C/Ctrl+Q - Exit\n  • F1 - Toggle help\n  • /save - Save conversation\n\nReady to chat! Type your message below...".to_string(),
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
            cursor_position: 0,
            input_scroll_offset: 0,
            last_animation_tick: Instant::now(),
            needs_redraw: true,
            input_width: 80, // Default width, will be updated during render
            cursor_blink_state: true,
            last_cursor_blink: Instant::now(),
            terminal_height: 24,  // Default height, will be updated during render
            terminal_width: 80,   // Default width, will be updated during render
            chat_area_height: 10, // Default height, will be updated during render
            chat_area_width: 80,  // Default width, will be updated during render
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
        format!("{hours:02}:{minutes:02}")
    }

    /// Triggers the Easter egg by displaying the dedication message.
    pub fn trigger_easter_egg(&mut self) {
        let dedication_msg = ChatMessage {
            message_type: MessageType::System,
            content: vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("💝 ", Style::default().fg(Color::Magenta)),
                    Span::styled("Special Dedication", Style::default().fg(Color::Magenta).bold()),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("✨ ", Style::default().fg(Color::Yellow)),
                    Span::styled("This application is lovingly dedicated to", Style::default().fg(Color::Cyan)),
                ]),
                Line::from(vec![
                    Span::styled("   my wonderful mother and grandma", Style::default().fg(Color::Cyan).italic()),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("🌟 ", Style::default().fg(Color::Yellow)),
                    Span::styled("Thank you for your endless love, wisdom, and support", Style::default().fg(Color::Green)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("💖 ", Style::default().fg(Color::Red)),
                    Span::styled("With all my love and gratitude", Style::default().fg(Color::Magenta).italic()),
                ]),
                Line::from(""),
            ],
            timestamp: Self::get_timestamp(),
            raw_content: "💝 Special Dedication\n\n✨ This application is lovingly dedicated to\n   my wonderful mother and grandma\n\n🌟 Thank you for your endless love, wisdom, and support\n\n💖 With all my love and gratitude".to_string(),
        };

        self.add_message(dedication_msg);
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
    /// };
    ///
    /// app.add_message(message);
    /// ```
    pub fn add_message(&mut self, mut message: ChatMessage) {
        message.timestamp = Self::get_timestamp();
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

        let error_content = vec![Line::from(vec![
            Span::styled("❌ Error: ", Style::default().fg(Color::Red).bold()),
            Span::styled(error.message.clone(), Style::default().fg(Color::Red)),
        ])];

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
            raw_content: format!("ERROR: {}", error.message),
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
        // Calculate max scroll and add extra lines to ensure last content is visible
        let max_scroll = self.max_scroll();
        // Add 3 extra lines to compensate for any calculation inaccuracies
        self.scroll_position = max_scroll.saturating_add(3);
        self.update_scroll_state();
    }

    /// Calculates the maximum scroll position based on content height and terminal height.
    ///
    /// This method accurately determines how far the user can scroll by calculating the
    /// total rendered lines accounting for text wrapping in the terminal. It ensures that
    /// all content remains accessible and prevents content cutoff at the bottom of the
    /// conversation area.
    ///
    /// # Algorithm
    ///
    /// 1. Calculates visible height of the chat area (terminal height - UI overhead)
    /// 2. Computes total rendered lines including:
    ///    - Header lines (1 per message)
    ///    - Content lines with accurate text wrapping using character count
    ///    - Separator lines (1 per message)
    /// 3. Applies conservative buffer to prevent content cutoff
    ///
    /// # Text Wrapping
    ///
    /// Uses `.chars().count()` instead of `.len()` for accurate Unicode character
    /// counting, ensuring proper text wrapping calculations in the terminal.
    ///
    /// # Returns
    ///
    /// The maximum valid scroll position with buffer to ensure content visibility
    pub fn max_scroll(&self) -> usize {
        // Calculate visible height for the chat area
        let chat_area_height = self.terminal_height.saturating_sub(11).max(1);
        let visible_height = chat_area_height.saturating_sub(2).max(1); // Account for borders

        // Calculate terminal width for text wrapping calculations
        let chat_width = self.input_width.saturating_sub(4).max(20); // Account for borders and padding

        // Calculate the actual rendered lines accounting for text wrapping
        let total_rendered_lines: usize = self
            .chat_history
            .iter()
            .map(|msg| {
                let mut lines = 0;

                // Header line (always 1 line)
                lines += 1;

                // Content lines - account for text wrapping
                for line in &msg.content {
                    let line_text = line
                        .spans
                        .iter()
                        .map(|span| span.content.as_ref())
                        .collect::<String>();

                    if line_text.trim().is_empty() {
                        lines += 1; // Empty lines
                    } else {
                        // More accurate text wrapping calculation
                        let display_width = line_text.chars().count();
                        if display_width <= chat_width {
                            lines += 1;
                        } else {
                            // Use div_ceil for ceiling division (clippy requirement)
                            let wrapped_lines = display_width.div_ceil(chat_width);
                            lines += wrapped_lines.max(1);
                        }
                    }
                }

                // Separator line after each message (always 1 line)
                lines += 1;

                lines
            })
            .sum();

        // Return scroll position that ensures content is accessible
        // Add small buffer for long responses to ensure last lines are visible
        if total_rendered_lines > visible_height {
            let max_scroll = total_rendered_lines.saturating_sub(visible_height);
            // Subtract 1 to ensure the last lines are always visible for long responses
            max_scroll.saturating_sub(1)
        } else {
            0
        }
    }

    /// Updates the internal scroll state for display.
    ///
    /// Synchronizes the scroll position with the UI scrollbar state by recalculating
    /// the total rendered lines accounting for text wrapping in the terminal. This
    /// method ensures consistency between scroll calculations and actual rendered
    /// content.
    ///
    /// # Implementation Details
    ///
    /// * Recalculates total rendered lines using the same algorithm as `max_scroll()`
    /// * Accounts for text wrapping using accurate character counting
    /// * Updates scrollbar content length and position for proper display
    /// * Called automatically by scroll methods to maintain UI consistency
    ///
    /// # Text Wrapping Consistency
    ///
    /// This method uses identical text wrapping calculations as `max_scroll()` to
    /// ensure that the scrollbar accurately represents the actual content layout.
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
        // Calculate terminal width for text wrapping calculations
        let chat_width = self.input_width.saturating_sub(4).max(20); // Account for borders and padding

        // Calculate total rendered lines accounting for text wrapping
        let total_rendered_lines: usize = self
            .chat_history
            .iter()
            .map(|msg| {
                let mut lines = 0;

                // Header line (always 1 line)
                lines += 1;

                // Content lines - account for text wrapping
                for line in &msg.content {
                    let line_text = line
                        .spans
                        .iter()
                        .map(|span| span.content.as_ref())
                        .collect::<String>();

                    if line_text.trim().is_empty() {
                        lines += 1; // Empty lines
                    } else {
                        // More accurate text wrapping calculation
                        let display_width = line_text.chars().count();
                        if display_width <= chat_width {
                            lines += 1;
                        } else {
                            // Use div_ceil for ceiling division (clippy requirement)
                            let wrapped_lines = display_width.div_ceil(chat_width);
                            lines += wrapped_lines.max(1);
                        }
                    }
                }

                // Separator line after each message (always 1 line)
                lines += 1;

                lines
            })
            .sum();

        self.scroll_state = self
            .scroll_state
            .content_length(total_rendered_lines.max(1))
            .position(self.scroll_position);
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
        // Ensure we're in a clean state before starting new stream
        if self.is_llm_busy {
            log::warn!("Starting new stream while already busy - forcing clean state");
            self.finish_streaming();
        }

        self.is_llm_busy = true;
        self.is_input_disabled = true;
        self.response_progress = 0.0;
        self.streaming_buffer.clear();

        // Create a new assistant message immediately to ensure we have a dedicated message for this streaming session
        let initial_message = ChatMessage {
            message_type: MessageType::Assistant,
            content: vec![Line::from("...")], // Placeholder content while waiting for response
            timestamp: Self::get_timestamp(),
            raw_content: String::new(), // Will be filled as we receive chunks
        };
        self.chat_history.push(initial_message);

        self.needs_redraw = true;
        self.typing_indicator = "⠋".to_string(); // Start with first spinner frame
        self.set_status("🚀 Sending request...".to_string(), false);
        log::debug!("Started streaming mode with new assistant message");
    }

    /// Finish streaming response with clean state reset and final content preservation
    pub fn finish_streaming(&mut self) {
        log::debug!(
            "Finishing streaming mode, buffer has {} chars",
            self.streaming_buffer.len()
        );

        // CRITICAL FIX: Always force final UI update regardless of throttling
        // This ensures that all accumulated content in the buffer gets transferred to the chat message
        if !self.streaming_buffer.is_empty() {
            log::debug!(
                "Forcing final UI update with {} chars in buffer",
                self.streaming_buffer.len()
            );

            if let Some(last_msg) = self.chat_history.last_mut() {
                if last_msg.message_type == MessageType::Assistant {
                    // Force final update of the assistant message with complete content
                    last_msg.content = markdown_to_lines(&self.streaming_buffer);
                    last_msg.raw_content = self.streaming_buffer.clone();
                    last_msg.timestamp = Self::get_timestamp();
                    log::debug!(
                        "FINAL UPDATE: Assistant message now has {} lines of content",
                        last_msg.content.len()
                    );
                } else {
                    // This shouldn't happen with our new approach, but handle gracefully
                    log::warn!(
                        "Expected assistant message at end of streaming but found {:?}",
                        last_msg.message_type
                    );
                    self.add_streaming_message();
                    log::debug!("Added new assistant message with final content");
                }
            } else {
                // This shouldn't happen either, but handle gracefully
                log::warn!("No messages in chat history at end of streaming");
                self.add_streaming_message();
                log::debug!("Added assistant message to empty chat history");
            }

            // Log the final content for debugging
            if let Some(last_msg) = self.chat_history.last() {
                if last_msg.message_type == MessageType::Assistant {
                    let content_preview = last_msg
                        .content
                        .iter()
                        .map(|line| {
                            line.spans
                                .iter()
                                .map(|span| span.content.as_ref())
                                .collect::<String>()
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                        .chars()
                        .take(100)
                        .collect::<String>();
                    log::debug!("Final message content preview: {content_preview}...");
                }
            }
        } else {
            // If no content was received, check if we have a placeholder and remove it
            if let Some(last_msg) = self.chat_history.last() {
                if last_msg.message_type == MessageType::Assistant {
                    // Check if this looks like our placeholder (empty or just "...")
                    let is_placeholder = last_msg.content.is_empty()
                        || (last_msg.content.len() == 1
                            && last_msg.content[0].spans.len() == 1
                            && (last_msg.content[0].spans[0].content == "..."
                                || last_msg.content[0].spans[0].content.trim().is_empty()));

                    if is_placeholder {
                        self.chat_history.pop();
                        log::debug!("Removed placeholder assistant message (no content received)");
                    }
                }
            }
            log::debug!("No content in streaming buffer to finalize");
        }

        // Clear the buffer AFTER ensuring content is saved to prevent race conditions
        self.streaming_buffer.clear();

        // Reset streaming state
        self.is_llm_busy = false;
        self.is_input_disabled = false;
        self.response_progress = 1.0; // Show completion
        self.typing_indicator.clear();

        // CRITICAL: Ensure scroll calculations are done with final content
        self.scroll_to_bottom();
        self.needs_redraw = true;

        // Update status
        self.set_status("✅ Ready".to_string(), false);
        self.clear_error();

        log::debug!(
            "Streaming mode finished successfully, chat history has {} messages",
            self.chat_history.len()
        );
    }

    /// Update streaming content with optimized rendering and immediate feedback
    pub fn update_streaming_content(&mut self, content: &str) {
        // Only process non-empty content
        if content.is_empty() {
            return;
        }

        // Prevent buffer overflow - if buffer gets too large, start replacing old content
        if self.streaming_buffer.len() + content.len() > MAX_STREAMING_BUFFER_SIZE {
            log::warn!("Streaming buffer approaching limit, truncating old content");
            // Keep the last 80% of the buffer to maintain context
            let keep_from = self.streaming_buffer.len() / 5;
            self.streaming_buffer = self.streaming_buffer[keep_from..].to_string();
        }

        self.streaming_buffer.push_str(content);

        // Update the message content with latest streaming data
        if let Some(last_msg) = self.chat_history.last_mut() {
            if last_msg.message_type == MessageType::Assistant {
                // Always update the assistant message with the latest streaming content
                last_msg.content = markdown_to_lines(&self.streaming_buffer);
                last_msg.raw_content = self.streaming_buffer.clone();
                last_msg.timestamp = Self::get_timestamp();
            } else {
                log::warn!(
                    "Expected assistant message but found {:?}, creating new assistant message",
                    last_msg.message_type
                );
                self.add_streaming_message();
            }
        } else {
            log::warn!(
                "No messages in chat history during streaming, creating new assistant message"
            );
            self.add_streaming_message();
        }

        // Always scroll to bottom to ensure new content is visible
        self.scroll_to_bottom();

        // Mark for UI redraw - use simpler logic for better responsiveness
        let should_redraw =
            // Always update for small content (responsive for short responses)
            self.streaming_buffer.len() < SMALL_BUFFER_THRESHOLD ||
            // Regular interval updates for longer content
            self.streaming_buffer.len() % UI_UPDATE_INTERVAL == 0 ||
            // Content-based triggers for better UX
            content.contains('\n') ||      // Line breaks
            content.contains("```") ||     // Code blocks
            content.contains("##") ||      // Headers
            content.ends_with(". ") ||     // Sentence endings
            content.ends_with("? ") ||     // Questions
            content.ends_with("! "); // Exclamations

        if should_redraw {
            self.needs_redraw = true;

            // Update status with progress info
            self.set_status(
                format!(
                    "{}  Receiving response... ({} chars, {}% complete)",
                    self.typing_indicator,
                    self.streaming_buffer.len(),
                    (self.response_progress * 100.0) as u8
                ),
                false,
            );

            // Update progress
            self.response_progress = (self.response_progress + 0.05).min(0.95);
        } else {
            // Even if we don't redraw UI, still update the progress indicator
            self.response_progress = (self.response_progress + 0.01).min(0.95);
        }
    }

    /// Add new streaming assistant message
    fn add_streaming_message(&mut self) {
        let message = ChatMessage {
            message_type: MessageType::Assistant,
            content: markdown_to_lines(&self.streaming_buffer),
            timestamp: Self::get_timestamp(),
            raw_content: self.streaming_buffer.clone(),
        };
        self.chat_history.push(message);
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

    /// Save the current conversation to a text file.
    ///
    /// Exports all user and assistant messages from the chat history to a plain text file
    /// with timestamps and proper formatting. System messages are excluded from the export.
    ///
    /// # Arguments
    ///
    /// * `filename` - Optional custom filename. If None, generates a timestamped default name.
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The filename that was used for saving
    /// * `Err(anyhow::Error)` - If no conversation exists or file operations fail
    ///
    /// # Examples
    ///
    /// ```rust
    /// // Save with default timestamped filename
    /// let filename = app.save_conversation(None)?;
    ///
    /// // Save with custom filename
    /// let filename = app.save_conversation(Some("my_chat.txt".to_string()))?;
    /// ```
    pub fn save_conversation(&self, filename: Option<String>) -> Result<String> {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        // Check if there's any conversation to save (exclude system messages)
        let has_conversation = self
            .chat_history
            .iter()
            .any(|msg| matches!(msg.message_type, MessageType::User | MessageType::Assistant));

        if !has_conversation {
            return Err(anyhow::anyhow!("No conversation to save"));
        }

        let filename = filename.unwrap_or_else(|| {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            format!("conversation_{timestamp}.txt")
        });

        let mut content = String::new();
        content.push_str("Perspt Conversation\n");
        content.push_str(&"=".repeat(18));
        content.push('\n');
        content.push('\n');

        for msg in &self.chat_history {
            match msg.message_type {
                MessageType::User => {
                    content.push_str(&format!(
                        "[{}] User:\n{}\n\n",
                        msg.timestamp, msg.raw_content
                    ));
                }
                MessageType::Assistant => {
                    content.push_str(&format!(
                        "[{}] Assistant:\n{}\n\n",
                        msg.timestamp, msg.raw_content
                    ));
                }
                _ => {} // Skip system messages
            }
        }

        fs::write(&filename, content)?;
        Ok(filename)
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
    provider: Arc<GenAIProvider>,
) -> Result<()> {
    let mut app = App::new(config);
    let (tx, mut rx) = mpsc::unbounded_channel();

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
                if let Some(message) = llm_message {
                    // Collect all messages from the channel first to prioritize EOT signals
                    let mut all_messages = vec![message];
                    while let Ok(additional_message) = rx.try_recv() {
                        all_messages.push(additional_message);
                    }

                    log::info!("=== UI PROCESSING === {} messages received", all_messages.len());

                    // Process EOT signals FIRST to prevent state confusion
                    let mut content_messages: Vec<String> = Vec::new();
                    let mut eot_count = 0;
                    let mut total_content_chars = 0;

                    for (i, msg) in all_messages.iter().enumerate() {
                        if msg == crate::EOT_SIGNAL {
                            eot_count += 1;
                            log::info!(">>> EOT SIGNAL #{eot_count} found at position {i} <<<");
                            if eot_count == 1 {
                                // Process all accumulated content before first EOT
                                log::info!("Processing {} content messages before EOT ({} total chars)",
                                          content_messages.len(), total_content_chars);
                                for (j, content_msg) in content_messages.iter().enumerate() {
                                    log::debug!("Processing content message {}/{}: {} chars",
                                               j+1, content_messages.len(), content_msg.len());
                                    handle_llm_response(&mut app, content_msg.clone(), &provider, &model_name, &tx).await;
                                }
                                content_messages.clear();
                                // Now process the EOT signal
                                handle_llm_response(&mut app, msg.clone(), &provider, &model_name, &tx).await;
                                break; // Stop processing after first EOT
                            } else {
                                // Ignore duplicate EOT signals
                                log::warn!("Ignoring duplicate EOT signal #{eot_count}");
                            }
                        } else {
                            total_content_chars += msg.len();
                            content_messages.push(msg.clone());
                        }
                    }

                    // If no EOT signal, process remaining content messages
                    if eot_count == 0 {
                        log::info!("No EOT signal found, processing {} remaining content messages ({} chars)",
                                  content_messages.len(), total_content_chars);
                        for (j, content_msg) in content_messages.into_iter().enumerate() {
                            log::debug!("Processing remaining content message {}: {} chars", j+1, content_msg.len());
                            handle_llm_response(&mut app, content_msg, &provider, &model_name, &tx).await;
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
    tx: &mpsc::UnboundedSender<String>,
    _api_key: &str,
    model_name: &str,
    provider: &Arc<GenAIProvider>,
) -> Option<AppEvent> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            match key.code {
                // Quit commands
                KeyCode::Char('q') | KeyCode::Char('c')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
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
                        // Check for Easter egg exact sequence "l-o-v-e"
                        if input.eq_ignore_ascii_case("l-o-v-e") {
                            app.trigger_easter_egg();
                            return Some(AppEvent::Redraw);
                        }

                        // Handle commands starting with /
                        if input.starts_with('/') {
                            let command = input.trim_start_matches('/').trim();
                            match command {
                                "save" => match app.save_conversation(None) {
                                    Ok(filename) => {
                                        app.add_message(ChatMessage {
                                            message_type: MessageType::System,
                                            content: vec![Line::from(format!(
                                                "✅ Conversation saved to: {filename}"
                                            ))],
                                            timestamp: App::get_timestamp(),
                                            raw_content: format!(
                                                "Conversation saved to: {filename}"
                                            ),
                                        });
                                    }
                                    Err(e) => {
                                        app.add_message(ChatMessage {
                                            message_type: MessageType::Error,
                                            content: vec![Line::from(format!("❌ Error: {e}"))],
                                            timestamp: App::get_timestamp(),
                                            raw_content: format!("Error: {e}"),
                                        });
                                    }
                                },
                                _ => {
                                    app.add_message(ChatMessage {
                                        message_type: MessageType::Error,
                                        content: vec![Line::from(
                                            "❌ Unknown command. Available: /save",
                                        )],
                                        timestamp: App::get_timestamp(),
                                        raw_content: "Unknown command. Available: /save"
                                            .to_string(),
                                    });
                                }
                            }
                            return Some(AppEvent::Redraw);
                        }

                        // Add user message immediately for instant feedback
                        app.add_message(ChatMessage {
                            message_type: MessageType::User,
                            content: vec![Line::from(input.clone())],
                            timestamp: App::get_timestamp(),
                            raw_content: input.clone(),
                        });

                        // Start LLM request
                        app.start_streaming();
                        tokio::spawn(initiate_llm_request_enhanced(
                            input,
                            Arc::clone(provider),
                            model_name.to_string(),
                            tx.clone(),
                        ));

                        return Some(AppEvent::Redraw);
                    } else if app.is_input_disabled && !app.input_text.trim().is_empty() {
                        // Queue input if busy
                        let input = app.input_text.trim().to_string();
                        app.pending_inputs.push_back(input);
                        app.clear_input();
                        app.set_status(
                            format!("Message queued ({})", app.pending_inputs.len()),
                            false,
                        );
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
    tx: mpsc::UnboundedSender<String>,
) {
    // log::info!("Starting enhanced LLM request: {}", input); // Removed to prevent TUI interference

    let result = provider
        .generate_response_stream_to_channel(&model_name, &input, tx.clone())
        .await;

    match result {
        Ok(()) => {
            log::debug!("Streaming completed successfully");
            // EOT signal is now sent by the provider itself, no need to send it here
        }
        Err(e) => {
            log::error!("LLM request failed: {e}");
            let _ = tx.send(format!("Error: {e}"));
            let _ = tx.send(crate::EOT_SIGNAL.to_string());
        }
    }
}

/// Handle LLM responses with immediate UI updates
async fn handle_llm_response(
    app: &mut App,
    message: String,
    provider: &Arc<GenAIProvider>,
    model_name: &str,
    tx: &mpsc::UnboundedSender<String>,
) {
    if message == crate::EOT_SIGNAL {
        // End of response - CRITICAL: Ensure this is processed immediately
        log::info!(
            ">>> RECEIVED EOT SIGNAL - finishing streaming (busy: {}, buffer: {} chars) <<<",
            app.is_llm_busy,
            app.streaming_buffer.len()
        );

        // Always finish streaming when we get EOT, even if state seems wrong
        if app.is_llm_busy {
            log::info!("Calling finish_streaming() due to EOT");
            app.finish_streaming();
        } else {
            log::warn!("!!! Received EOT signal but not in busy state - cleaning up anyway !!!");
            app.streaming_buffer.clear();
            app.is_input_disabled = false;
            app.scroll_to_bottom(); // Ensure we're scrolled to bottom when re-enabling input
        }

        // Ensure the UI has processed the finish_streaming state change
        app.needs_redraw = true;

        // Process pending inputs ONLY after confirming we're in clean state
        if !app.is_llm_busy && !app.pending_inputs.is_empty() {
            let pending_input = app.pending_inputs.pop_front().unwrap();
            log::info!(
                "Processing pending input after EOT: {} chars",
                pending_input.len()
            );

            app.add_message(ChatMessage {
                message_type: MessageType::User,
                content: vec![Line::from(pending_input.clone())],
                timestamp: App::get_timestamp(),
                raw_content: pending_input.clone(),
            });

            // Start new streaming session with clean state
            app.start_streaming();
            tokio::spawn(initiate_llm_request_enhanced(
                pending_input,
                Arc::clone(provider),
                model_name.to_string(),
                tx.clone(),
            ));
        } else if !app.pending_inputs.is_empty() {
            log::warn!("Still have pending inputs but LLM is busy - this shouldn't happen");
        } else {
            log::debug!("No pending inputs to process after EOT");
        }
    } else if message.starts_with("Error: ") && message.len() > 7 {
        // Handle errors
        let error_msg = &message[7..];
        log::error!("Received error message: {error_msg}");
        let error_state = categorize_error(error_msg);
        app.add_error(error_state);
        app.finish_streaming();
    } else {
        // Regular streaming content - use thread-local counters for logging
        thread_local! {
            static TOTAL_CONTENT_RECEIVED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
            static CHUNK_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        }

        let chunk_count = CHUNK_COUNT.with(|c| {
            let count = c.get() + 1;
            c.set(count);
            count
        });

        let total_received = TOTAL_CONTENT_RECEIVED.with(|t| {
            let total = t.get() + message.len();
            t.set(total);
            total
        });

        // Log every 25 chunks or large content chunks
        if chunk_count % 25 == 0 || message.len() > 100 {
            log::info!(
                "STREAMING: chunk #{}, {} chars, total {} chars, buffer: {} chars",
                chunk_count,
                message.len(),
                total_received,
                app.streaming_buffer.len()
            );
        }

        app.update_streaming_content(&message);
        app.set_status(
            format!("{}  Receiving response...", app.typing_indicator),
            false,
        );
    }
}

/// Categorizes error messages into specific error types with helpful details
fn categorize_error(error_msg: &str) -> ErrorState {
    let error_lower = error_msg.to_lowercase();

    let (error_type, message, details) = if error_lower.contains("api key")
        || error_lower.contains("unauthorized")
        || error_lower.contains("authentication")
    {
        (
            ErrorType::Authentication,
            "Authentication failed".to_string(),
            Some(
                "Please check your API key is valid and has the necessary permissions.".to_string(),
            ),
        )
    } else if error_lower.contains("rate limit") || error_lower.contains("too many requests") {
        (
            ErrorType::RateLimit,
            "Rate limit exceeded".to_string(),
            Some("Please wait a moment before sending another request.".to_string()),
        )
    } else if error_lower.contains("network")
        || error_lower.contains("connection")
        || error_lower.contains("timeout")
    {
        (
            ErrorType::Network,
            "Network error".to_string(),
            Some("Please check your internet connection and try again.".to_string()),
        )
    } else if error_lower.contains("model") || error_lower.contains("invalid") {
        (
            ErrorType::InvalidModel,
            "Invalid model or request".to_string(),
            Some(
                "The specified model may not be available or the request format is incorrect."
                    .to_string(),
            ),
        )
    } else if error_lower.contains("server")
        || error_lower.contains("5")
        || error_lower.contains("internal")
    {
        (
            ErrorType::ServerError,
            "Server error".to_string(),
            Some("The AI service is experiencing issues. Please try again later.".to_string()),
        )
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
            Constraint::Length(3), // Header
            Constraint::Min(1),    // Chat area (flexible)
            Constraint::Length(5), // Input area (fixed size for better visibility)
            Constraint::Length(3), // Status line (increased to prevent overlap)
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

    let header_content = vec![Line::from(vec![
        Span::styled("🧠 ", Style::default().fg(Color::Magenta)),
        Span::styled("Perspt", Style::default().fg(Color::Magenta).bold()),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled("Model: ", Style::default().fg(Color::Gray)),
        Span::styled(model_name, Style::default().fg(Color::Cyan).bold()),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled("Status: ", Style::default().fg(Color::Gray)),
        Span::styled(status_text, Style::default().fg(status_color).bold()),
    ])];

    let header = Paragraph::new(header_content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" AI Chat Terminal ")
                .title_style(Style::default().fg(Color::Magenta).bold()),
        )
        .alignment(Alignment::Center);

    f.render_widget(header, area);
}

/// Enhanced chat area with better scrolling and formatting
fn draw_enhanced_chat_area(f: &mut Frame, area: Rect, app: &mut App) {
    let mut chat_content: Vec<Line> = Vec::new();

    for msg in app.chat_history.iter() {
        let (icon, style) = match msg.message_type {
            MessageType::User => ("👤", Style::default().fg(Color::Blue).bold()),
            MessageType::Assistant => ("🤖", Style::default().fg(Color::Green).bold()),
            MessageType::Error => ("❌", Style::default().fg(Color::Red).bold()),
            MessageType::System => ("ℹ️", Style::default().fg(Color::Cyan).bold()),
            MessageType::Warning => ("⚠️", Style::default().fg(Color::Yellow).bold()),
        };

        // Add header line
        chat_content.push(Line::from(vec![
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
                style,
            ),
            Span::styled(
                format!(" • {}", msg.timestamp),
                Style::default().fg(Color::DarkGray),
            ),
        ]));

        // Add message content
        chat_content.extend(msg.content.iter().cloned());

        // Add separator line after each message for consistency
        chat_content.push(Line::from(""));
    }

    // Update the app's input width for accurate scroll calculations
    app.input_width = area.width as usize;
    // Update the actual chat area dimensions from the layout
    app.chat_area_height = area.height as usize;
    app.chat_area_width = area.width as usize;

    // Calculate content size for scroll validation
    let total_content_lines = chat_content.len();

    // Ensure scroll position is within bounds
    let max_scroll = app.max_scroll();
    if app.scroll_position > max_scroll {
        app.scroll_position = max_scroll;
    }

    // Update scroll state
    app.update_scroll_state();

    // Debug: Log scroll information for troubleshooting
    let visible_height = area.height.saturating_sub(2) as usize; // Account for borders
    if total_content_lines > 20 {
        // Only log for substantial content
        log::debug!(
            "RENDER SCROLL DEBUG - area_height={}, calculated_visible_height={}, total_content_lines={}, scroll_pos={}, max_scroll={}, terminal_height={}",
            area.height, visible_height, total_content_lines, app.scroll_position, max_scroll, app.terminal_height
        );
    }

    // Create layout for chat and scrollbar
    let chat_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    // Use the calculated scroll position directly
    let chat_paragraph = Paragraph::new(chat_content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::White))
                .title(" Conversation ")
                .title_style(Style::default().fg(Color::White).bold()),
        )
        .wrap(Wrap { trim: false })
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
            Constraint::Length(3), // Input field
            Constraint::Length(2), // Progress bar or hint (properly sized)
        ])
        .split(area);

    // Get visible input and cursor position
    let (visible_input, cursor_pos) = app.get_visible_input();

    // Input field styling based on state
    let (border_color, title) = if app.is_input_disabled {
        (Color::DarkGray, " Input (Disabled - AI is thinking...) ")
    } else {
        (
            Color::Green,
            " Type your message (Enter to send, F1 for help) ",
        )
    };

    // Create input content with cursor
    let mut input_spans = vec![];

    if app.is_input_disabled && visible_input.is_empty() {
        input_spans.push(Span::styled(
            "Waiting for AI response...",
            Style::default().fg(Color::DarkGray).italic(),
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
            input_spans.push(Span::styled(
                before_cursor,
                Style::default().fg(Color::White),
            ));
        }

        // Cursor character with highlighting and blinking
        if !app.is_input_disabled {
            let cursor_style = if app.cursor_blink_state {
                Style::default().fg(Color::Black).bg(Color::White)
            } else {
                Style::default().fg(Color::White).bg(Color::DarkGray)
            };

            input_spans.push(Span::styled(at_cursor.to_string(), cursor_style));
        }

        if !after_cursor.is_empty() {
            input_spans.push(Span::styled(
                after_cursor,
                Style::default().fg(Color::White),
            ));
        }
    } // Close the else block

    let input_paragraph = Paragraph::new(Line::from(input_spans))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(border_color))
                .title(title)
                .title_style(Style::default().fg(border_color)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(input_paragraph, input_chunks[0]);

    // Progress bar or hint area - properly contained within its own area
    if app.is_llm_busy {
        let progress = Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .gauge_style(Style::default().fg(Color::Yellow).bg(Color::DarkGray))
            .ratio(app.response_progress)
            .label(format!("{}  Processing response...", app.typing_indicator));

        f.render_widget(progress, input_chunks[1]);
    } else if !app.pending_inputs.is_empty() {
        let queue_info = Paragraph::new(Line::from(vec![
            Span::styled("📋 Queued messages: ", Style::default().fg(Color::Blue)),
            Span::styled(
                app.pending_inputs.len().to_string(),
                Style::default().fg(Color::Blue).bold(),
            ),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .alignment(Alignment::Center);

        f.render_widget(queue_info, input_chunks[1]);
    } else {
        // Show helpful hint when idle
        let hint = Paragraph::new(Line::from(vec![
            Span::styled("💡 ", Style::default().fg(Color::Yellow)),
            Span::styled(
                "Press F1 for help • Use ↑/↓ to scroll chat history",
                Style::default().fg(Color::DarkGray).italic(),
            ),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
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
            Span::styled(
                &app.status_message,
                if app.is_llm_busy {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Green)
                },
            ),
            Span::styled(queue_info, Style::default().fg(Color::Blue)),
            Span::styled(" │ Ctrl+C to exit", Style::default().fg(Color::Gray)),
        ])]
    };

    let status_paragraph = Paragraph::new(status_content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Gray)),
    );

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
        Line::from(vec![Span::styled(
            "📖 Perspt Help & Shortcuts",
            Style::default().fg(Color::Magenta).bold(),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "🎹 Input Controls:",
            Style::default().fg(Color::Yellow).bold(),
        )]),
        Line::from(vec![
            Span::styled("  Enter     ", Style::default().fg(Color::Cyan)),
            Span::styled("Send message", Style::default()),
        ]),
        Line::from(vec![
            Span::styled("  ←/→       ", Style::default().fg(Color::Cyan)),
            Span::styled("Move cursor", Style::default()),
        ]),
        Line::from(vec![
            Span::styled("  Home/End  ", Style::default().fg(Color::Cyan)),
            Span::styled("Start/End of line", Style::default()),
        ]),
        Line::from(vec![
            Span::styled("  Backspace ", Style::default().fg(Color::Cyan)),
            Span::styled("Delete before cursor", Style::default()),
        ]),
        Line::from(vec![
            Span::styled("  Delete    ", Style::default().fg(Color::Cyan)),
            Span::styled("Delete at cursor", Style::default()),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "📜 Navigation:",
            Style::default().fg(Color::Yellow).bold(),
        )]),
        Line::from(vec![
            Span::styled("  ↑/↓       ", Style::default().fg(Color::Cyan)),
            Span::styled("Scroll chat history", Style::default()),
        ]),
        Line::from(vec![
            Span::styled("  PgUp/PgDn ", Style::default().fg(Color::Cyan)),
            Span::styled("Fast scroll", Style::default()),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "🔧 Application:",
            Style::default().fg(Color::Yellow).bold(),
        )]),
        Line::from(vec![
            Span::styled("  F1        ", Style::default().fg(Color::Cyan)),
            Span::styled("Toggle this help", Style::default()),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+C/Q  ", Style::default().fg(Color::Cyan)),
            Span::styled("Exit application", Style::default()),
        ]),
        Line::from(vec![
            Span::styled("  Esc       ", Style::default().fg(Color::Cyan)),
            Span::styled("Close help/Exit", Style::default()),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "✨ Features:",
            Style::default().fg(Color::Yellow).bold(),
        )]),
        Line::from(vec![Span::styled(
            "  • Real-time streaming responses",
            Style::default().fg(Color::Green),
        )]),
        Line::from(vec![Span::styled(
            "  • Input queuing during AI responses",
            Style::default().fg(Color::Green),
        )]),
        Line::from(vec![Span::styled(
            "  • Full cursor navigation support",
            Style::default().fg(Color::Green),
        )]),
        Line::from(vec![Span::styled(
            "  • Live markdown rendering",
            Style::default().fg(Color::Green),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Press F1 or Esc to close",
            Style::default().fg(Color::Gray).italic(),
        )]),
    ];

    let help_popup = Paragraph::new(help_content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(Color::Magenta))
                .title(" Help ")
                .title_style(Style::default().fg(Color::Magenta).bold()),
        )
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
                lines.push(Line::from(vec![Span::styled(
                    "└─────────────┘",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )]));
                code_lang.clear();
            } else {
                // Start of code block
                in_code_block = true;
                code_lang = line.trim_start_matches("```").to_string();
                lines.push(Line::from(vec![Span::styled(
                    format!(
                        "┌─ {} ─┐",
                        if code_lang.is_empty() {
                            "Code"
                        } else {
                            &code_lang
                        }
                    ),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )]));
            }
            continue;
        }

        if in_code_block {
            // Code block content
            lines.push(Line::from(vec![Span::styled(
                format!("│ {line}"),
                Style::default().fg(Color::Cyan).bg(Color::DarkGray),
            )]));
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
            lines.push(Line::from(vec![Span::styled(
                format!("{} {}", "#".repeat(level), title),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )]));
            continue;
        }

        // Handle lists
        if line.trim_start().starts_with('*') || line.trim_start().starts_with('-') {
            let indent = line.len() - line.trim_start().len();
            let content = line
                .trim_start()
                .trim_start_matches('*')
                .trim_start_matches('-')
                .trim();
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
                    Style::default()
                        .fg(Color::LightBlue)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));
            continue;
        }

        // Regular paragraph text
        if line.trim().is_empty() {
            // Only add empty line if the previous line was not empty to avoid excessive spacing
            if !lines.is_empty() {
                if let Some(last_line) = lines.last() {
                    if !last_line.spans.is_empty() && !last_line.spans[0].content.trim().is_empty()
                    {
                        lines.push(Line::from(""));
                    }
                }
            }
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
                    for ch in chars.by_ref() {
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
                for ch in chars.by_ref() {
                    if ch == '`' {
                        break;
                    }
                    code_text.push(ch);
                }
                spans.push(Span::styled(
                    format!(" {code_text} "),
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
