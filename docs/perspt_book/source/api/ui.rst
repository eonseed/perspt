User Interface Module
======================

The ``ui`` module implements the terminal-based user interface for Perspt using the Ratatui TUI framework. It provides a rich, interactive chat experience with real-time markdown rendering, scrollable chat history, and comprehensive error handling.

.. currentmodule:: ui

Architecture Overview
---------------------

.. code-block:: text

   ┌─────────────────────────────────────────────────────────────┐
   │                      Perspt UI Architecture                 │
   ├─────────────────────────────────────────────────────────────┤
   │                                                             │
   │  ┌─────────────┐    ┌──────────────┐    ┌─────────────────┐ │
   │  │   App       │────│ ChatMessage  │────│ MessageType     │ │
   │  │ (Controller)│    │   (Data)     │    │  (Styling)      │ │
   │  └─────────────┘    └──────────────┘    └─────────────────┘ │
   │           │                                       │         │
   │           ▼                                       ▼         │
   │  ┌─────────────┐    ┌──────────────┐    ┌─────────────────┐ │
   │  │ ErrorState  │    │   AppEvent   │    │   Ratatui       │ │
   │  │ (Errors)    │    │ (Input)      │    │  (Rendering)    │ │
   │  └─────────────┘    └──────────────┘    └─────────────────┘ │
   │                                                             │
   └─────────────────────────────────────────────────────────────┘

Core Types
----------

MessageType
~~~~~~~~~~~

.. code-block:: rust

   #[derive(Debug, Clone, PartialEq)]
   pub enum MessageType {
       User,      // Blue styling for user input
       Assistant, // Green styling for AI responses  
       Error,     // Red styling for error messages
       System,    // Gray styling for system notifications
       Warning,   // Yellow styling for warnings
   }

Represents the type of message in the chat interface, determining visual styling and behavior.

**Message Types:**

.. list-table::
   :header-rows: 1
   :widths: 20 20 60

   * - Type
     - Color
     - Purpose
   * - ``User``
     - Blue
     - User input messages
   * - ``Assistant``
     - Green
     - AI response messages
   * - ``Error``
     - Red
     - Error and failure notifications
   * - ``System``
     - Gray
     - System status and notifications
   * - ``Warning``
     - Yellow
     - Warning messages and alerts

**Example:**

.. code-block:: rust

   use perspt::ui::MessageType;

   let user_msg = MessageType::User;      // Blue styling
   let ai_msg = MessageType::Assistant;   // Green styling  
   let error_msg = MessageType::Error;    // Red styling

ChatMessage
~~~~~~~~~~~

.. code-block:: rust

   #[derive(Debug, Clone)]
   pub struct ChatMessage {
       pub message_type: MessageType,
       pub content: Vec<Line<'static>>,
       pub timestamp: String,
   }

Represents a single message in the chat interface with content, styling, and metadata.

**Fields:**

* ``message_type`` - Determines styling and visual treatment
* ``content`` - Formatted content as styled lines (supports markdown)
* ``timestamp`` - When the message was created (HH:MM format)

**Example:**

.. code-block:: rust

   use perspt::ui::{ChatMessage, MessageType};
   use ratatui::text::Line;

   let message = ChatMessage {
       message_type: MessageType::User,
       content: vec![Line::from("Hello, AI!")],
       timestamp: "14:30".to_string(),
   };

ErrorState
~~~~~~~~~~

.. code-block:: rust

   #[derive(Debug, Clone)]
   pub struct ErrorState {
       pub message: String,
       pub details: Option<String>,
       pub error_type: ErrorType,
   }

Comprehensive error information for user display with categorization and details.

**Fields:**

* ``message`` - Primary error message for display
* ``details`` - Optional additional debugging information
* ``error_type`` - Category for appropriate handling and styling

ErrorType
~~~~~~~~~

.. code-block:: rust

   #[derive(Debug, Clone)]
   pub enum ErrorType {
       Network,        // Connectivity issues
       Authentication, // Provider auth failures
       RateLimit,      // API rate limiting
       InvalidModel,   // Unsupported model requests
       ServerError,    // Provider server errors
       Unknown,        // Unclassified errors
   }

**Error Categories:**

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Type
     - Description
   * - ``Network``
     - Connectivity issues and network failures
   * - ``Authentication``
     - Provider authentication failures
   * - ``RateLimit``
     - API rate limiting and quota exceeded
   * - ``InvalidModel``
     - Unsupported or invalid model requests
   * - ``ServerError``
     - Provider server errors and outages
   * - ``Unknown``
     - Unclassified or unexpected errors

**Example:**

.. code-block:: rust

   use perspt::ui::{ErrorState, ErrorType};

   let error = ErrorState {
       message: "Network connection failed".to_string(),
       details: Some("Check your internet connection".to_string()),
       error_type: ErrorType::Network,
   };

App (Main Controller)
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

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

Central application state and controller managing the entire chat interface.

**Key State Fields:**

* ``chat_history`` - Complete conversation history
* ``input_text`` - Current user input buffer
* ``status_message`` - Bottom status bar content
* ``is_llm_busy`` - Whether AI response is being generated
* ``current_error`` - Active error state for display
* ``scroll_position`` - Current view position in chat history

Core Methods
------------

App Creation and Setup
~~~~~~~~~~~~~~~~~~~~~~~

new()
^^^^^

.. code-block:: rust

   pub fn new(config: AppConfig) -> Self

Creates a new App instance with welcome message and default state.

**Parameters:**

* ``config`` - Application configuration with LLM provider settings

**Returns:**

* ``Self`` - Initialized App instance

**Example:**

.. code-block:: rust

   use perspt::ui::App;
   use perspt::config::AppConfig;

   let config = AppConfig::load().unwrap();
   let app = App::new(config);
   assert!(!app.should_quit);
   assert!(!app.chat_history.is_empty()); // Contains welcome message

Message Management
~~~~~~~~~~~~~~~~~~

add_message()
^^^^^^^^^^^^^

.. code-block:: rust

   pub fn add_message(&mut self, message: ChatMessage)

Adds a new message to chat history with automatic timestamping and scroll-to-bottom.

**Parameters:**

* ``message`` - ChatMessage to add (timestamp will be set automatically)

**Example:**

.. code-block:: rust

   let message = ChatMessage {
       message_type: MessageType::User,
       content: vec![Line::from("Hello!")],
       timestamp: String::new(), // Will be set automatically
   };

   app.add_message(message);

add_error()
^^^^^^^^^^^

.. code-block:: rust

   pub fn add_error(&mut self, error: ErrorState)

Adds an error message to chat history and sets current error state.

**Parameters:**

* ``error`` - ErrorState containing error information

**Behavior:**

1. Creates error message with red styling
2. Sets ``current_error`` for status display
3. Adds message to chat history
4. Scrolls to show the error

**Example:**

.. code-block:: rust

   let error = ErrorState {
       message: "API key invalid".to_string(),
       details: Some("Check your configuration".to_string()),
       error_type: ErrorType::Authentication,
   };

   app.add_error(error);

add_user_message()
^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn add_user_message(&mut self, content: &str)

Convenience method to add a user message from plain text.

**Parameters:**

* ``content`` - User message text

**Example:**

.. code-block:: rust

   app.add_user_message("What is the weather today?");

add_assistant_message()
^^^^^^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn add_assistant_message(&mut self, content: &str)

Convenience method to add an assistant response with markdown parsing.

**Parameters:**

* ``content`` - Assistant response text (supports markdown)

**Features:**

* Automatic markdown parsing
* Code block highlighting
* Link detection
* Emphasis and strong text support

**Example:**

.. code-block:: rust

   app.add_assistant_message("Here's some **bold** text and `code`");

Input Handling
~~~~~~~~~~~~~~

handle_input()
^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn handle_input(&mut self, key: KeyEvent) -> AppEvent

Handles keyboard input and returns appropriate application events.

**Parameters:**

* ``key`` - Keyboard event from terminal

**Returns:**

* ``AppEvent`` - Event to be processed by main loop

**Key Bindings:**

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Key
     - Action
   * - ``Enter``
     - Send message (if not empty and LLM not busy)
   * - ``Ctrl+C``, ``Ctrl+D``
     - Quit application
   * - ``Escape``
     - Clear input or dismiss help
   * - ``Ctrl+L``
     - Clear chat history
   * - ``F1``, ``?``
     - Toggle help screen
   * - ``Page Up/Down``
     - Scroll chat history
   * - ``Home/End``
     - Scroll to top/bottom
   * - ``Printable chars``
     - Add to input buffer
   * - ``Backspace``
     - Delete character

**Example:**

.. code-block:: rust

   use crossterm::event::{KeyEvent, KeyCode};

   let key_event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
   let event = app.handle_input(key_event);

   match event {
       AppEvent::SendMessage(msg) => {
           // Send message to LLM
       },
       AppEvent::Quit => {
           // Exit application
       },
       _ => {}
   }

Navigation and Scrolling
~~~~~~~~~~~~~~~~~~~~~~~~

scroll_up()
^^^^^^^^^^^

.. code-block:: rust

   pub fn scroll_up(&mut self, lines: usize)

Scrolls chat history up by specified number of lines.

**Parameters:**

* ``lines`` - Number of lines to scroll

scroll_down()
^^^^^^^^^^^^^

.. code-block:: rust

   pub fn scroll_down(&mut self, lines: usize)

Scrolls chat history down by specified number of lines.

**Parameters:**

* ``lines`` - Number of lines to scroll

scroll_to_bottom()
^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn scroll_to_bottom(&mut self)

Scrolls to the bottom of chat history (most recent messages).

scroll_to_top()
^^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn scroll_to_top(&mut self)

Scrolls to the top of chat history (oldest messages).

clear_history()
^^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn clear_history(&mut self)

Clears all chat history and adds a new welcome message.

**Example:**

.. code-block:: rust

   app.clear_history();
   assert_eq!(app.chat_history.len(), 1); // Only welcome message

State Management
~~~~~~~~~~~~~~~~

set_llm_busy()
^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn set_llm_busy(&mut self, busy: bool)

Sets the LLM busy state, affecting input handling and UI indicators.

**Parameters:**

* ``busy`` - Whether LLM is currently processing

**Effects:**

* Disables input when busy
* Shows typing indicator
* Updates status message

clear_error()
^^^^^^^^^^^^^

.. code-block:: rust

   pub fn clear_error(&mut self)

Clears current error state.

**Example:**

.. code-block:: rust

   app.clear_error();
   assert!(app.current_error.is_none());

update_progress()
^^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn update_progress(&mut self, progress: f64)

Updates response generation progress (0.0 to 1.0).

**Parameters:**

* ``progress`` - Progress value between 0.0 and 1.0

Events
------

AppEvent
~~~~~~~~

.. code-block:: rust

   #[derive(Debug)]
   pub enum AppEvent {
       SendMessage(String),
       Quit,
       ClearHistory,
       ShowHelp,
       None,
   }

Events generated by user interactions for processing by the main application loop.

**Event Types:**

* ``SendMessage(String)`` - User wants to send a message
* ``Quit`` - User wants to exit application
* ``ClearHistory`` - User wants to clear chat history
* ``ShowHelp`` - User wants to toggle help screen
* ``None`` - No action required

Rendering Functions
-------------------

render_app()
~~~~~~~~~~~~

.. code-block:: rust

   pub fn render_app<B: Backend>(f: &mut Frame<B>, app: &mut App)

Main rendering function that draws the complete UI.

**Parameters:**

* ``f`` - Ratatui frame for drawing
* ``app`` - Application state to render

**Layout:**

.. code-block:: text

   ┌─────────────────────────────────────┐
   │            Chat History             │
   │  [User] Hello!                      │
   │  [AI] Hi there! How can I help?     │
   │  [User] What's the weather?         │
   │  [AI] I can't access weather data   │
   │                                     │
   ├─────────────────────────────────────┤
   │ > Type your message here...         │
   ├─────────────────────────────────────┤
   │ Status: Ready | Model: gpt-4o-mini  │
   └─────────────────────────────────────┘

render_help()
~~~~~~~~~~~~~

.. code-block:: rust

   pub fn render_help<B: Backend>(f: &mut Frame<B>)

Renders the help screen overlay with keyboard shortcuts and usage information.

**Features:**

* Semi-transparent overlay
* Comprehensive key bindings
* Usage instructions
* Provider information

Utilities
---------

format_content()
~~~~~~~~~~~~~~~~

.. code-block:: rust

   pub fn format_content(content: &str, message_type: MessageType) -> Vec<Line<'static>>

Formats message content with appropriate styling and markdown support.

**Parameters:**

* ``content`` - Raw message text
* ``message_type`` - Type for styling decisions

**Returns:**

* ``Vec<Line<'static>>`` - Formatted lines for rendering

**Features:**

* Markdown parsing (bold, italic, code)
* Syntax highlighting for code blocks
* Link detection and styling
* Word wrapping

get_current_time()
~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   pub fn get_current_time() -> String

Returns current time in HH:MM format for message timestamps.

**Returns:**

* ``String`` - Formatted time string

**Example:**

.. code-block:: rust

   let timestamp = get_current_time();
   // Returns: "14:30"

Constants
---------

UI Colors and Styling
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   pub const USER_COLOR: Color = Color::Blue;
   pub const ASSISTANT_COLOR: Color = Color::Green;
   pub const ERROR_COLOR: Color = Color::Red;
   pub const SYSTEM_COLOR: Color = Color::Gray;
   pub const WARNING_COLOR: Color = Color::Yellow;

Default colors for different message types.

Layout Constants
~~~~~~~~~~~~~~~~

.. code-block:: rust

   pub const CHAT_AREA_HEIGHT: u16 = 3; // Minimum height for chat area
   pub const INPUT_AREA_HEIGHT: u16 = 3; // Height of input area
   pub const STATUS_AREA_HEIGHT: u16 = 1; // Height of status bar

Layout dimensions for UI components.

Error Handling
--------------

The UI module provides comprehensive error handling and user feedback:

**Error Display:**

* Inline error messages in chat history
* Status bar error indicators
* Detailed error information in help
* Recovery suggestions

**Error Recovery:**

* Automatic error dismissal after user action
* Input validation and sanitization
* Graceful degradation for rendering errors
* Connection retry mechanisms

**Example Error Handling:**

.. code-block:: rust

   match llm_response {
       Ok(response) => app.add_assistant_message(&response),
       Err(e) => {
           let error_state = ErrorState {
               message: "Failed to get AI response".to_string(),
               details: Some(e.to_string()),
               error_type: ErrorType::Network,
           };
           app.add_error(error_state);
       }
   }

See Also
--------

* :doc:`../user-guide/basic-usage` - Basic usage guide
* :doc:`../developer-guide/extending` - UI extension guide
* :doc:`config` - Configuration module
* :doc:`llm-provider` - LLM provider integration
