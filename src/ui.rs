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
}

/// Events that can occur in the application.
///
/// These events drive the main event loop and determine how the application
/// responds to user input and system events.
#[derive(Debug)]
pub enum AppEvent {
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
/// It serves as the central controller for the entire application.
///
/// # Fields
///
/// * `chat_history` - Complete history of all chat messages
/// * `input_text` - Current user input text
/// * `status_message` - Current status bar message
/// * `config` - Application configuration
/// * `should_quit` - Flag to control application shutdown
/// * `scroll_state` - State for chat history scrolling
/// * `scroll_position` - Current scroll position in chat
/// * `is_input_disabled` - Whether input is currently disabled
/// * `pending_inputs` - Queue of pending user inputs
/// * `is_llm_busy` - Whether an LLM request is in progress
/// * `current_error` - Current error state if any
/// * `show_help` - Whether help overlay is shown
/// * `typing_indicator` - Animation for response generation
/// * `response_progress` - Progress indicator for LLM responses
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
/// // Add a user message
/// app.add_user_message("Hello, AI!".to_string());
/// 
/// // Check if app should quit
/// if app.should_quit {
///     // Handle shutdown
/// }
/// ```
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
    /// };
    /// 
    /// app.add_message(message);
    /// ```
    pub fn add_message(&mut self, mut message: ChatMessage) {
        message.timestamp = Self::get_timestamp();
        self.chat_history.push(message);
        self.scroll_to_bottom();
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

    /// Calculates the maximum scroll position based on content height.
    ///
    /// Determines how far the user can scroll based on the total number
    /// of lines in the chat history. Used internally for scroll bounds checking.
    ///
    /// # Returns
    ///
    /// The maximum valid scroll position
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
}

/// Runs the main UI event loop for the chat application.
///
/// This is the primary entry point for the terminal user interface. It initializes
/// the app state, sets up event handling, and manages the main interaction loop
/// between the user and the LLM provider.
///
/// # Arguments
///
/// * `terminal` - Configured terminal instance for rendering
/// * `config` - Application configuration with provider settings
/// * `model_name` - Name of the LLM model to use
/// * `api_key` - API key for the LLM provider
/// * `provider` - LLM provider implementation for making requests
///
/// # Returns
///
/// `Result<()>` - Ok if the UI runs successfully and exits cleanly, Err for fatal errors
///
/// # Errors
///
/// Returns an error if:
/// - Terminal operations fail (rendering, input handling)
/// - Event channel communication fails
/// - Critical UI state corruption occurs
///
/// # Examples
///
/// ```rust
/// use perspt::ui::run_ui;
/// use perspt::config::AppConfig;
/// use perspt::llm_provider::create_provider;
/// use ratatui::backend::CrosstermBackend;
/// use ratatui::Terminal;
/// use std::sync::Arc;
/// use std::io;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let config = AppConfig::load()?;
///     let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
///     let provider = create_provider(&config)?;
///     
///     run_ui(
///         &mut terminal,
///         config.clone(),
///         config.model.clone(),
///         config.api_key.clone(),
///         Arc::new(provider)
///     ).await?;
///     
///     Ok(())
/// }
/// ```
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

    // Restore terminal (this will be called even if there's an error thanks to our panic handling)
    // Note: The main cleanup is handled by the panic hook and cleanup_terminal() in main.rs
    crossterm::terminal::disable_raw_mode().ok(); // Use .ok() to ignore errors during cleanup
    crossterm::execute!(terminal.backend_mut(), crossterm::terminal::LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    Ok(())
}

/// Renders the complete UI layout to the terminal frame.
///
/// This function coordinates the rendering of all UI components including the header,
/// chat area, input area, and status line. It also handles the help overlay when active.
///
/// # Arguments
///
/// * `f` - The Ratatui frame to render to
/// * `app` - Current application state for rendering
/// * `model_name` - Name of the active LLM model for header display
///
/// # Layout Structure
///
/// ```text
/// ┌─────────────────────────────────────┐
/// │             Header (3 lines)        │ 
/// ├─────────────────────────────────────┤
/// │                                     │
/// │          Chat Area (flexible)       │
/// │                                     │ 
/// ├─────────────────────────────────────┤
/// │           Input Area (4 lines)      │
/// ├─────────────────────────────────────┤
/// │          Status Line (2 lines)      │
/// └─────────────────────────────────────┘
/// ```
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
///
/// Status can be:
/// - "Ready" (green) - Available for new input
/// - "Thinking..." (yellow) - Processing LLM request
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

/// Renders the scrollable chat history area.
///
/// This function displays all chat messages with appropriate styling based on
/// message type (user, assistant, error, system, warning). It handles scrolling,
/// message formatting, and visual indicators for different message sources.
///
/// # Arguments
///
/// * `f` - The Ratatui frame to render to
/// * `area` - The rectangular area allocated for the chat
/// * `app` - Current application state containing chat history and scroll position
///
/// # Message Format
///
/// Each message is displayed with:
/// - Type-specific icon and color (👤 User, 🤖 Assistant, ❌ Error, etc.)
/// - Timestamp in (HH:MM) format
/// - Full message content with markdown rendering for assistant responses
/// - Appropriate spacing between messages
///
/// # Scroll Behavior
///
/// - Automatically scrolls to bottom for new messages
/// - User can scroll up/down to view history
/// - Scroll position is preserved during message updates
/// - Visual scrollbar indicates position in long conversations
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

/// Renders the user input area with text field and progress indicators.
///
/// This function displays the text input area where users type their messages,
/// along with visual feedback about the current state (ready, disabled, thinking).
/// It also shows a progress bar when the LLM is generating responses.
///
/// # Arguments
///
/// * `f` - The Ratatui frame to render to
/// * `area` - The rectangular area allocated for the input section
/// * `app` - Current application state including input text and busy status
///
/// # Input States
///
/// - **Ready**: Green border, normal text input, "Type your message..." prompt
/// - **Disabled**: Gray border, italic text, "AI is thinking..." message
/// - **Busy**: Shows animated progress bar below input field
///
/// # Visual Elements
///
/// ```text
/// ┌─────────────────────────────────────────────────────────┐
/// │ Input: Hello, can you help me with...                  │
/// ├─────────────────────────────────────────────────────────┤
/// │ [████████████████████                    ] 75%         │
/// └─────────────────────────────────────────────────────────┘
/// ```
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

/// Renders the status line with current application state and error information.
///
/// This function displays the status bar at the bottom of the interface, showing
/// the current application state, any active errors, queue information, and
/// helpful keyboard shortcuts for the user.
///
/// # Arguments
///
/// * `f` - The Ratatui frame to render to
/// * `area` - The rectangular area allocated for the status line
/// * `app` - Current application state containing status and error information
///
/// # Status Display Modes
///
/// **Error Mode**: When an error is present
/// ```text
/// ┌─────────────────────────────────────────────────────────┐
/// │ ❌ Authentication failed | Press F1 for help            │
/// └─────────────────────────────────────────────────────────┘
/// ```
///
/// **Normal Mode**: During regular operation
/// ```text
/// ┌─────────────────────────────────────────────────────────┐
/// │ Status: Ready | Queued: 2 | Ctrl+C to exit             │
/// └─────────────────────────────────────────────────────────┘
/// ```
///
/// # Visual Indicators
///
/// - **Green**: Ready state, normal operation
/// - **Yellow**: Busy/processing state
/// - **Red**: Error conditions
/// - **Blue**: Queue information
/// - **Gray**: Help text and shortcuts
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

/// Renders a modal help overlay displaying keyboard shortcuts and features.
///
/// This function creates a centered popup window that displays comprehensive
/// help information including navigation controls, input shortcuts, and
/// application features. The overlay is rendered on top of the main UI.
///
/// # Arguments
///
/// * `f` - The Ratatui frame to render to
/// * `_app` - Current application state (unused but kept for consistency)
///
/// # Overlay Layout
///
/// The help popup is centered on screen using a three-tier layout:
/// - 20% top padding
/// - 60% content area (centered popup)
/// - 20% bottom padding
///
/// # Help Content Sections
///
/// 1. **Navigation**: Scroll controls for chat history
/// 2. **Input**: Message sending and help toggle
/// 3. **Exit**: Application termination shortcuts
/// 4. **Features**: List of available functionality
///
/// # Visual Design
///
/// ```text
/// ╔═══════════════════════════════════════════════════════════╗
/// ║                    📖 Help & Shortcuts                    ║
/// ╠═══════════════════════════════════════════════════════════╣
/// ║ Navigation:                                               ║
/// ║   ↑/↓      Scroll chat history                          ║
/// ║                                                           ║
/// ║ Input:                                                    ║
/// ║   Enter    Send message                                   ║
/// ║   F1       Toggle this help                              ║
/// ║                                                           ║
/// ║ Features:                                                 ║
/// ║   • Input queuing while AI responds                      ║
/// ║   • Markdown rendering support                           ║
/// ║   • Automatic scrolling                                  ║
/// ╚═══════════════════════════════════════════════════════════╝
/// ```
///
/// # Interaction
///
/// - Press F1 again to close the help overlay
/// - Uses double-line border for visual prominence
/// - Clears background area to ensure readability
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

/// Categorizes error messages into specific error types with helpful details.
///
/// This function analyzes error message content to determine the most likely
/// cause and provides appropriate error categorization along with user-friendly
/// suggestions for resolution.
///
/// # Arguments
///
/// * `error_msg` - The raw error message string to analyze
///
/// # Returns
///
/// Returns an `ErrorState` containing:
/// - `error_type`: The categorized error type enum
/// - `message`: A simplified, user-friendly error message
/// - `details`: Optional detailed explanation and suggested resolution
///
/// # Error Categories
///
/// | Category | Triggers | User Message | Resolution Hint |
/// |----------|----------|--------------|-----------------|
/// | **Authentication** | "api key", "unauthorized" | "Authentication failed" | Check API key validity |
/// | **RateLimit** | "rate limit", "too many requests" | "Rate limit exceeded" | Wait before retrying |
/// | **Network** | "network", "connection", "timeout" | "Network error" | Check internet connection |
/// | **InvalidModel** | "model", "invalid" | "Invalid model or request" | Verify model availability |
/// | **ServerError** | "server", "5xx", "internal" | "Server error" | Service issues, retry later |
/// | **Unknown** | All other cases | Original message | No specific hint |
///
/// # Examples
///
/// ```rust
/// let auth_error = categorize_error("Invalid API key provided");
/// assert_eq!(auth_error.error_type, ErrorType::Authentication);
/// assert_eq!(auth_error.message, "Authentication failed");
/// 
/// let rate_error = categorize_error("Rate limit exceeded for requests");
/// assert_eq!(rate_error.error_type, ErrorType::RateLimit);
/// ```
///
/// # Usage in UI
///
/// The categorized errors are displayed in the status line with appropriate
/// styling and helpful context for users to understand and resolve issues.
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
/// - Uses `pulldown_cmark` for efficient markdown parsing
/// - Converts all content to owned strings for `'static` lifetime
/// - Optimized for terminal display with appropriate color choices
/// - Handles large documents gracefully with streaming parsing
///
/// # Error Handling
///
/// This function is designed to be robust and will handle malformed markdown
/// gracefully, falling back to plain text rendering for unrecognized elements.
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
