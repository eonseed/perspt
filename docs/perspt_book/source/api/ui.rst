User Interface Module
======================

The ``ui`` module implements the terminal-based user interface for Perspt using the Ratatui TUI framework. It provides a modern, responsive chat experience with real-time streaming responses, enhanced cursor navigation, markdown rendering, and comprehensive state management.

.. currentmodule:: ui

Overview
--------

The UI module is the core interactive component of Perspt, providing a rich terminal-based chat interface. It handles everything from user input and cursor management to real-time streaming display and markdown rendering.

**Key Capabilities:**

* **Real-time Streaming UI**: Immediate, responsive rendering during LLM response generation with intelligent buffering
* **Enhanced Input System**: Full cursor movement, editing capabilities, and visual feedback with blinking cursor
* **Smart Content Management**: Optimized streaming buffer preventing memory overflow while maintaining responsiveness  
* **Rich Markdown Rendering**: Live formatting with syntax highlighting, code blocks, lists, and emphasis
* **Intelligent Error Handling**: Categorized error types with user-friendly messages and recovery suggestions
* **Smooth Animations**: Typing indicators, progress bars, and cursor blinking for better user experience
* **Input Queuing**: Seamless message queuing while AI is responding to maintain conversation flow

Architecture Overview
---------------------

The UI follows a layered, event-driven architecture designed for responsiveness and maintainability:

.. code-block:: text

   ┌─────────────────────────────────────────────────────────────┐
   │                    Perspt UI Architecture                   │
   ├─────────────────────────────────────────────────────────────┤
   │                                                             │
   │  ┌─────────────┐    ┌──────────────┐    ┌─────────────────┐ │
   │  │     App     │────│ ChatMessage  │────│ MessageType     │ │
   │  │(Controller) │    │   (Data)     │    │  (Styling)      │ │
   │  │             │    │              │    │                 │ │
   │  │ + State     │    │ + Content    │    │ + User          │ │
   │  │ + Streaming │    │ + Timestamp  │    │ + Assistant     │ │
   │  │ + Cursor    │    │ + Markdown   │    │ + Error/System  │ │
   │  └─────────────┘    └──────────────┘    └─────────────────┘ │
   │           │                                       │         │
   │           ▼                                       ▼         │
   │  ┌─────────────┐    ┌──────────────┐    ┌─────────────────┐ │
   │  │ ErrorState  │    │   AppEvent   │    │Event Processing │ │
   │  │ + Categories│    │ + Key Events │    │ + Async Loop    │ │
   │  │ + Recovery  │    │ + UI Updates │    │ + Priorities    │ │
   │  │ + Messages  │    │ + Timers     │    │ + Non-blocking  │ │
   │  └─────────────┘    └──────────────┘    └─────────────────┘ │
   │                                                             │
   │  ┌─────────────┐    ┌──────────────┐    ┌─────────────────┐ │
   │  │ Rendering   │    │  Animation   │    │   Markdown      │ │
   │  │ + Layout    │    │ + Spinners   │    │ + Parsing       │ │
   │  │ + Cursor    │    │ + Progress   │    │ + Highlighting  │ │
   │  │ + Scrolling │    │ + Blinking   │    │ + Code Blocks   │ │
   │  └─────────────┘    └──────────────┘    └─────────────────┘ │
   │                                                             │
   └─────────────────────────────────────────────────────────────┘

**Key Design Principles:**

1. **Responsiveness**: Immediate feedback for all user actions with optimized rendering
2. **State Consistency**: Centralized state management in the App struct prevents race conditions  
3. **Memory Efficiency**: Smart buffer management prevents overflow during long responses
4. **User Experience**: Visual feedback, animations, and clear error messages guide the user

Core Types and Data Structures
------------------------------

MessageType
~~~~~~~~~~~

.. code-block:: rust

   #[derive(Debug, Clone, PartialEq)]
   pub enum MessageType {
       User,      // Blue styling for user input
       Assistant, // Green styling for AI responses  
       Error,     // Red styling for error messages
       System,    // Cyan styling for system notifications
       Warning,   // Yellow styling for warnings
   }

Determines the visual appearance and behavior of messages in the chat interface. Each type has distinct styling to help users quickly identify message sources.

**Message Styling:**

.. list-table::
   :header-rows: 1
   :widths: 20 15 15 50

   * - Type
     - Color
     - Icon
     - Purpose
   * - ``User``
     - Blue
     - 👤
     - User input messages and questions
   * - ``Assistant``
     - Green
     - 🤖
     - AI responses and assistance
   * - ``Error``
     - Red
     - ❌
     - Error notifications and failures
   * - ``System``
     - Cyan
     - ℹ️
     - System status and welcome messages
   * - ``Warning``
     - Yellow
     - ⚠️
     - Warning messages and alerts

**Example:**

.. code-block:: rust

   use perspt::ui::MessageType;

   let user_msg = MessageType::User;      // Blue with 👤 icon
   let ai_msg = MessageType::Assistant;   // Green with 🤖 icon  
   let error_msg = MessageType::Error;    // Red with ❌ icon

ChatMessage
~~~~~~~~~~~

.. code-block:: rust

   #[derive(Debug, Clone)]
   pub struct ChatMessage {
       pub message_type: MessageType,
       pub content: Vec<Line<'static>>,
       pub timestamp: String,
   }

Core data structure for chat messages with rich formatting support and automatic timestamp management.

**Fields:**

* ``message_type`` - Determines styling, color, and icon display
* ``content`` - Pre-formatted content as styled Ratatui lines with full markdown support
* ``timestamp`` - Creation time in HH:MM format (automatically set by ``App::add_message()``)

**Features:**

* **Rich Markdown Support**: Automatic parsing of markdown with syntax highlighting
* **Responsive Formatting**: Content adapts to terminal width changes
* **Icon Integration**: Automatic icon assignment based on message type
* **Timestamp Management**: Automatic timestamping when added to chat history

**Example:**

.. code-block:: rust

   use perspt::ui::{ChatMessage, MessageType};
   use ratatui::text::Line;

   // Simple text message (timestamp will be auto-generated)
   let message = ChatMessage {
       message_type: MessageType::User,
       content: vec![Line::from("Hello, AI!")],
       timestamp: String::new(), // Auto-populated by App::add_message()
   };

   // Rich content with markdown (automatically parsed)
   let ai_response = ChatMessage {
       message_type: MessageType::Assistant,
       content: markdown_to_lines("Here's some **bold** text and `code`"),
       timestamp: App::get_timestamp(),
   };

ErrorState and Error Handling
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

ErrorState
^^^^^^^^^^

.. code-block:: rust

   #[derive(Debug, Clone)]
   pub struct ErrorState {
       pub message: String,
       pub details: Option<String>,
       pub error_type: ErrorType,
   }

Comprehensive error information system with automatic categorization and user-friendly messaging.

**Fields:**

* ``message`` - Primary user-facing error message (concise and actionable)
* ``details`` - Optional technical details for debugging and troubleshooting
* ``error_type`` - Error category for appropriate styling, handling, and recovery suggestions

ErrorType
^^^^^^^^^

.. code-block:: rust

   #[derive(Debug, Clone)]
   pub enum ErrorType {
       Network,        // Connectivity and network issues
       Authentication, // API key and provider auth failures
       RateLimit,      // API rate limiting and quota exceeded
       InvalidModel,   // Unsupported or invalid model requests
       ServerError,    // Provider server errors and outages
       Unknown,        // Unclassified or unexpected errors
   }

Advanced error categorization system that automatically analyzes error messages and provides appropriate user guidance.

**Error Categories with Recovery Guidance:**

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Type
     - Description & Auto-Generated Recovery Guidance
   * - ``Network``
     - Connectivity issues, timeouts, DNS failures. *"Check internet connection and try again."*
   * - ``Authentication``
     - Invalid API keys, expired tokens, permission errors. *"Verify API key configuration."*
   * - ``RateLimit``
     - API quota exceeded, too many requests. *"Wait a moment before sending another request."*
   * - ``InvalidModel``
     - Unsupported models, malformed requests. *"Check model availability and request format."*
   * - ``ServerError``
     - Provider outages, internal server errors. *"Service may be temporarily unavailable."*
   * - ``Unknown``
     - Unclassified errors requiring investigation. *"Please report if this persists."*

**Automatic Error Categorization Example:**

.. code-block:: rust

   // The categorize_error() function automatically analyzes error messages
   fn categorize_error(error_msg: &str) -> ErrorState {
       let error_lower = error_msg.to_lowercase();
       
       if error_lower.contains("api key") || error_lower.contains("unauthorized") {
           ErrorState {
               message: "Authentication failed".to_string(),
               details: Some("Please check your API key is valid".to_string()),
               error_type: ErrorType::Authentication,
           }
       } else if error_lower.contains("rate limit") {
           ErrorState {
               message: "Rate limit exceeded".to_string(),
               details: Some("Please wait before sending another request".to_string()),
               error_type: ErrorType::RateLimit,
           }
       }
       // ... other categorizations
   }

App (Main Controller)
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   pub struct App {
       // Core Application State
       pub chat_history: Vec<ChatMessage>,
       pub input_text: String,
       pub status_message: String,
       pub config: AppConfig,
       pub should_quit: bool,
       
       // Navigation and Display Management
       scroll_state: ScrollbarState,
       pub scroll_position: usize,
       pub show_help: bool,
       
       // Input Processing and Queue Management
       pub is_input_disabled: bool,
       pub pending_inputs: VecDeque<String>,
       pub is_llm_busy: bool,
       pub current_error: Option<ErrorState>,
       
       // Enhanced Cursor and Input Handling
       pub cursor_position: usize,
       pub input_scroll_offset: usize,
       pub cursor_blink_state: bool,
       pub last_cursor_blink: Instant,
       
       // Real-time Streaming and Animation
       pub typing_indicator: String,
       pub response_progress: f64,
       pub streaming_buffer: String,
       pub last_animation_tick: Instant,
       
       // Performance and UI Optimization
       pub needs_redraw: bool,
       pub input_width: usize,
       pub terminal_height: usize,
       pub terminal_width: usize,
   }

Enhanced central application controller managing all aspects of the chat interface, including real-time streaming, cursor navigation, input queuing, and responsive UI updates.

**State Organization:**

**Core Application State:**
* ``chat_history`` - Complete conversation with automatic timestamps and rich markdown formatting
* ``input_text`` - Current user input with full text editing support (insert, delete, cursor movement)
* ``status_message`` - Dynamic status with contextual information and error states
* ``config`` - Application configuration and LLM provider settings
* ``should_quit`` - Clean shutdown flag for the event loop

**Enhanced Input System:**
* ``cursor_position`` - Current cursor position within input text (character-level precision)
* ``input_scroll_offset`` - Horizontal scroll offset for long input lines
* ``cursor_blink_state`` - Visual cursor blinking animation state (500ms intervals)
* ``input_width`` - Available input area width for accurate scroll calculations
* ``is_input_disabled`` - Input protection during streaming to prevent conflicts

**Real-time Streaming Management:**
* ``is_llm_busy`` - Active response generation state flag
* ``streaming_buffer`` - Real-time content accumulation from LLM (with 1MB overflow protection)
* ``response_progress`` - Visual progress indicator (0.0 to 1.0 scale)
* ``typing_indicator`` - Animated spinner for visual feedback (10-frame cycle)

**Navigation and UI State:**
* ``scroll_position`` - Current chat history view position with bounds checking
* ``scroll_state`` - Internal scrollbar state synchronized with position
* ``show_help`` - Help overlay visibility toggle
* ``needs_redraw`` - Performance optimization flag for efficient rendering

**Advanced Features:**
* ``pending_inputs`` - Message queue for seamless conversation flow while AI responds
* ``current_error`` - Active error state with categorization and recovery suggestions
* ``last_animation_tick`` - Animation timing for smooth 60fps visual effects
* ``terminal_height/width`` - Current terminal dimensions for responsive layout

**Performance Optimizations:**

* **Intelligent Redraw**: Only updates UI when ``needs_redraw`` flag is set, reducing CPU usage
* **Smart Buffer Management**: Prevents memory overflow during long responses with 1MB limit
* **Responsive Input**: Immediate character feedback with optimized cursor rendering
* **Efficient Scrolling**: Content-aware scroll calculations with proper bounds checking
* **Animation Timing**: Balanced update intervals for smooth visuals without CPU waste

**Developer Notes:**

* The App struct uses interior mutability patterns for safe concurrent access
* All timing-related fields use ``Instant`` for high-precision animation control
* Buffer management includes overflow protection for production stability
* Input handling supports full terminal editing capabilities (Home, End, arrows, etc.)

AppEvent
~~~~~~~~

.. code-block:: rust

   #[derive(Debug)]
   pub enum AppEvent {
       Quit,           // Clean application shutdown
       Redraw,         // Immediate UI refresh needed
       Key(KeyEvent),  // User keyboard input
       Tick,           // Periodic timer for animations
   }

Event system for the responsive async UI loop, supporting immediate user feedback and smooth animations.

**Event Types:**

* ``Quit`` - Triggered by Ctrl+C/Ctrl+Q for clean application shutdown
* ``Redraw`` - Immediate UI refresh for responsive input feedback
* ``Key(KeyEvent)`` - User keyboard input with full key details and modifiers
* ``Tick`` - Periodic updates for animations, cursor blinking, and status updates

**Event Processing Priority:**

The event loop processes events with the following priority order:

1. **Highest**: LLM response chunks (real-time streaming)
2. **High**: Terminal input events (immediate user feedback)  
3. **Medium**: UI rendering updates (~60 FPS)
4. **Low**: Background tasks and periodic cleanup

Core Methods
------------

Application Lifecycle
~~~~~~~~~~~~~~~~~~~~~~

new()
^^^^^

.. code-block:: rust

   pub fn new(config: AppConfig) -> Self

Creates a new App instance with enhanced welcome message, optimized state initialization, and responsive UI setup.

**Parameters:**

* ``config`` - Application configuration with LLM provider settings

**Returns:**

* ``Self`` - Fully initialized App instance with welcome message and default state

**Features:**

* **Rich Welcome Message**: Multi-line welcome with quick help, shortcuts, and visual styling
* **State Initialization**: All cursors, buffers, and timers properly initialized to safe defaults
* **Performance Setup**: Optimized default values for responsive operation

**Implementation Details:**

The constructor creates a comprehensive welcome message that includes:

.. code-block:: rust

   // Welcome message with styling and helpful shortcuts
   let welcome_msg = ChatMessage {
       message_type: MessageType::System,
       content: vec![
           Line::from("🌟 Welcome to Perspt - Your AI Chat Terminal"),
           Line::from("💡 Quick Help:"),
           Line::from("  • Enter - Send message"),
           Line::from("  • ↑/↓ - Scroll chat history"),
           Line::from("  • Ctrl+C/Ctrl+Q - Exit"),
           Line::from("  • F1 - Toggle help"),
           Line::from("Ready to chat! Type your message below..."),
       ],
       timestamp: Self::get_timestamp(),
   };

**Example:**

.. code-block:: rust

   use perspt::ui::App;
   use perspt::config::AppConfig;

   let config = AppConfig::load().unwrap();
   let app = App::new(config);
   
   assert!(!app.should_quit);
   assert!(!app.chat_history.is_empty()); // Contains rich welcome message
   assert_eq!(app.cursor_position, 0);    // Cursor at start
   assert!(!app.is_llm_busy);             // Ready for input

get_timestamp()
^^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn get_timestamp() -> String

Generates a formatted timestamp string for message display.

**Returns:**

* ``String`` - Timestamp in HH:MM format for current system time

**Usage:**

.. code-block:: rust

   let timestamp = App::get_timestamp();
   // Returns format like "14:30" for 2:30 PM

Message Management
~~~~~~~~~~~~~~~~~~

add_message()
^^^^^^^^^^^^^

.. code-block:: rust

   pub fn add_message(&mut self, mut message: ChatMessage)

Adds a message to chat history with automatic timestamping, scroll management, and immediate UI updates.

**Parameters:**

* ``message`` - ChatMessage to add (timestamp will be automatically set to current time)

**Behavior:**

1. **Automatic Timestamping**: Sets current time in HH:MM format
2. **Smart Scrolling**: Automatically scrolls to show new message
3. **Immediate Feedback**: Triggers redraw for instant visibility
4. **State Consistency**: Maintains proper scroll and display state

**Example:**

.. code-block:: rust

   let message = ChatMessage {
       message_type: MessageType::User,
       content: vec![Line::from("What's the weather like?")],
       timestamp: String::new(), // Will be set automatically
   };

   app.add_message(message);
   // Message immediately visible with current timestamp

add_error()
^^^^^^^^^^^

.. code-block:: rust

   pub fn add_error(&mut self, error: ErrorState)

Adds an enhanced error message with automatic categorization, recovery suggestions, and visual prominence.

**Parameters:**

* ``error`` - ErrorState containing error information and category

**Enhanced Behavior:**

1. **Dual Display**: Error appears in both chat history and status bar
2. **Rich Formatting**: Error icon (❌), styled text, and optional details
3. **Recovery Guidance**: Context-appropriate suggestions based on error type
4. **Visual Prominence**: Red styling and immediate scroll-to-show

**Implementation:**

.. code-block:: rust

   // Creates rich error display with icon and details
   let error_content = vec![
       Line::from(vec![
           Span::styled("❌ Error: ", Style::default().fg(Color::Red).bold()),
           Span::styled(error.message.clone(), Style::default().fg(Color::Red)),
       ]),
   ];

   // Adds optional details if available
   if let Some(details) = &error.details {
       full_content.push(Line::from(vec![
           Span::styled("   Details: ", Style::default().fg(Color::Yellow)),
           Span::styled(details.clone(), Style::default().fg(Color::Gray)),
       ]));
   }

**Example:**

.. code-block:: rust

   let error = ErrorState {
       message: "API key invalid".to_string(),
       details: Some("Check your configuration file".to_string()),
       error_type: ErrorType::Authentication,
   };

   app.add_error(error);
   // Shows: "❌ Error: API key invalid"
   //        "   Details: Check your configuration file"

clear_error()
^^^^^^^^^^^^^

.. code-block:: rust

   pub fn clear_error(&mut self)

Clears the current error state and removes error display from the status bar.

**Features:**

* **State Reset**: Removes active error from status bar display
* **Clean Recovery**: Allows normal status messages to be shown again
* **Immediate Effect**: Error clearing is instant and triggers UI update

**Usage:**

Typically called after user acknowledges an error or when starting a new operation that should clear previous error states.

**Example:**

.. code-block:: rust

   // Display an error
   let error = ErrorState {
       message: "Connection failed".to_string(),
       details: None,
       error_type: ErrorType::Network,
   };
   app.add_error(error);
   assert!(app.current_error.is_some());
   
   // Clear the error
   app.clear_error();
   assert!(app.current_error.is_none());
   // Status bar now shows normal status instead of error

set_status()
^^^^^^^^^^^^

.. code-block:: rust

   pub fn set_status(&mut self, message: String, is_error: bool)

Sets the status bar message with optional error logging.

**Parameters:**

* ``message`` - The status message to display in the status bar
* ``is_error`` - Whether this message represents an error (affects logging level)

**Features:**

* **Immediate Display**: Status message appears instantly in the status bar
* **Error Logging**: Messages marked as errors are logged appropriately
* **Flexible Usage**: Can be used for both informational and error messages

**Example:**

.. code-block:: rust

   app.set_status("Processing request...".to_string(), false);
   // Status shows: "Processing request..."
   
   app.set_status("Connection failed".to_string(), true);  
   // Status shows: "Connection failed" and logs as error

Enhanced Input System
~~~~~~~~~~~~~~~~~~~~~

insert_char()
^^^^^^^^^^^^^

.. code-block:: rust

   pub fn insert_char(&mut self, ch: char)

Inserts a character at the current cursor position with immediate visual feedback and smart scrolling.

**Parameters:**

* ``ch`` - Character to insert

**Features:**

* **Cursor-Aware Insertion**: Character inserted exactly at cursor position
* **Auto-Scroll**: Input view scrolls to keep cursor visible for long text
* **Immediate Feedback**: Instant character appearance and cursor movement
* **Blink Reset**: Cursor blink resets for better visibility during typing
* **Input Protection**: Only works when input is enabled (not disabled during streaming)

**Example:**

.. code-block:: rust

   app.insert_char('H');
   app.insert_char('i');
   // Input shows "Hi" with cursor at position 2

delete_char_before()
^^^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn delete_char_before(&mut self)

Implements backspace functionality with cursor-aware deletion and visual feedback.

**Features:**

* **Smart Deletion**: Removes character before cursor position
* **Cursor Movement**: Cursor moves back after deletion
* **Visual Update**: Immediate text and cursor position updates
* **Boundary Safety**: Safe operation at beginning of input

delete_char_at()
^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn delete_char_at(&mut self)

Implements delete key functionality, removing character at cursor position.

**Features:**

* **Forward Deletion**: Removes character at current cursor position
* **Cursor Stability**: Cursor position remains stable after deletion
* **Boundary Safety**: Safe operation at end of input

move_cursor_left() / move_cursor_right()
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn move_cursor_left(&mut self)
   pub fn move_cursor_right(&mut self)

Navigate cursor within input text with automatic view scrolling for long input.

**Features:**

* **Boundary Respect**: Cannot move beyond text boundaries
* **Auto-Scroll**: View adjusts to keep cursor visible in long text
* **Visual Feedback**: Immediate cursor position updates

move_cursor_to_start() / move_cursor_to_end()
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn move_cursor_to_start(&mut self)
   pub fn move_cursor_to_end(&mut self)

Jump cursor to beginning or end of input with view reset.

**Features:**

* **Instant Navigation**: Immediate cursor positioning
* **View Reset**: Automatically adjusts scroll to show cursor
* **Home/End Key Support**: Mapped to Home and End keys

update_input_scroll() (Internal)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   fn update_input_scroll(&mut self)

Updates input scroll offset to keep cursor visible in long input text.

**Features:**

* **Automatic Scrolling**: Keeps cursor visible when input exceeds display width
* **Smooth Navigation**: Provides seamless editing experience for long input
* **Boundary Management**: Ensures proper scroll boundaries and cursor visibility

**Algorithm:**

.. code-block:: rust

   // Ensures cursor stays visible by adjusting scroll offset
   if self.cursor_position < self.input_scroll_offset {
       // Scroll left to show cursor
       self.input_scroll_offset = self.cursor_position;
   } else if self.cursor_position >= self.input_scroll_offset + self.input_width {
       // Scroll right to show cursor
       self.input_scroll_offset = self.cursor_position - self.input_width + 1;
   }

clear_input()
^^^^^^^^^^^^^

.. code-block:: rust

   pub fn clear_input(&mut self)

Clears input text and resets all cursor and scroll state.

**Features:**

* **Complete Reset**: Clears text, cursor position, and scroll offset
* **Immediate Update**: Triggers UI redraw for instant feedback

get_visible_input()
^^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn get_visible_input(&self) -> (&str, usize)

Returns the visible portion of input text and the relative cursor position for display.

**Returns:**

* ``(&str, usize)`` - Tuple containing (visible_text_slice, relative_cursor_position)

**Features:**

* **Scroll-Aware**: Returns only the portion of text visible in the input area
* **Cursor Mapping**: Provides cursor position relative to the visible text
* **Width Adaptive**: Automatically adjusts based on available input width

**Usage:**

Used internally by the rendering system to display input text with proper scrolling for long input lines.

**Example:**

.. code-block:: rust

   app.input_text = "This is a very long input that exceeds the terminal width".to_string();
   app.cursor_position = 10;
   app.input_width = 20; // Limited display width
   
   let (visible, cursor_pos) = app.get_visible_input();
   // Returns appropriate slice and relative cursor position

take_input()
^^^^^^^^^^^^

.. code-block:: rust

   pub fn take_input(&mut self) -> Option<String>

Extracts input text for sending, with automatic trimming and state reset.

**Returns:**

* ``Option<String>`` - Trimmed input text if not empty and input enabled, None otherwise

**Behavior:**

* Returns trimmed text only if input is enabled and non-empty
* Automatically clears input and resets cursor after extraction
* Prevents input extraction during streaming or when disabled

**Example:**

.. code-block:: rust

   if let Some(input) = app.take_input() {
       // Send input to LLM
       println!("Sending: {}", input);
       // Input automatically cleared and cursor reset
   }

Streaming and Real-time Updates
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

start_streaming()
^^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn start_streaming(&mut self)

Initiates streaming mode with state protection, immediate feedback, and clean initialization.

**Enhanced Features:**

* **State Protection**: Ensures clean state before starting new stream by calling finish_streaming() if already busy
* **Immediate Placeholder**: Creates assistant message with "..." placeholder for streaming content
* **Visual Feedback**: Shows animated spinner (⠋ frame) and progress indicator starting at 0%
* **Input Management**: Disables input during streaming to prevent conflicts and state corruption
* **Clean Initialization**: Clears streaming buffer and resets progress tracking

**Implementation Details:**

.. code-block:: rust

   // Clean state enforcement
   if self.is_llm_busy {
       log::warn!("Starting new stream while already busy - forcing clean state");
       self.finish_streaming();
   }
   
   // Set streaming flags
   self.is_llm_busy = true;
   self.is_input_disabled = true;
   self.response_progress = 0.0;
   self.streaming_buffer.clear();
   
   // Create placeholder message
   let initial_message = ChatMessage {
       message_type: MessageType::Assistant,
       content: vec![Line::from("...")],
       timestamp: Self::get_timestamp(),
   };
   self.chat_history.push(initial_message);

**Example:**

.. code-block:: rust

   app.start_streaming();
   // UI shows: "🚀 Sending request..." with animated spinner
   // Input disabled, progress bar appears
   // New assistant message with "..." placeholder added

update_streaming_content()
^^^^^^^^^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn update_streaming_content(&mut self, content: &str)

Updates streaming content with intelligent rendering optimization, memory management, and real-time UI updates.

**Parameters:**

* ``content`` - New content chunk from LLM response

**Advanced Features:**

* **Buffer Management**: Prevents memory overflow with 1MB limit and intelligent truncation (keeps last 80% of content for context)
* **Smart Rendering**: Content-aware update frequency based on content characteristics and patterns
* **Memory Safety**: Thread-local size tracking prevents buffer overflow and performance degradation
* **Progress Tracking**: Dynamic progress calculation with visual feedback (0-95% during streaming)
* **Real-time Updates**: Always updates message content regardless of UI throttling for data consistency

**Optimization Strategy:**

.. code-block:: rust

   // Buffer overflow protection
   if self.streaming_buffer.len() + content.len() > MAX_STREAMING_BUFFER_SIZE {
       let keep_from = self.streaming_buffer.len() / 5;
       self.streaming_buffer = self.streaming_buffer[keep_from..].to_string();
   }
   
   // Immediate UI update triggers:
   // - Small content (< 500 chars) - always responsive
   // - Line breaks ("\n") - paragraph completion
   // - Code blocks ("```") - syntax highlighting triggers
   // - Headers ("##", "###") - section breaks
   // - Lists ("- ", "* ") - bullet points
   // - Sentence endings (". ", "? ", "! ") - natural breaks
   // - Text formatting ("**", "*") - emphasis changes
   // - Regular intervals (every 200-250 chars) - prevents freezing

**Performance Features:**

* **Thread-local Tracking**: Efficient size-based update throttling using thread-local storage
* **Content-aware Updates**: Higher frequency for structured content (code, lists, headers)
* **Progressive Enhancement**: Gradual progress indicator updates (0.01-0.05 increments)
* **Memory Optimization**: Intelligent buffer management prevents excessive memory usage

**Example:**

.. code-block:: rust

   app.update_streaming_content("Hello, this is a streaming response...\n");
   // Updates buffer, triggers UI redraw due to line break
   // Progress increases, typing indicator continues
   // Message content updated in real-time

finish_streaming()
^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn finish_streaming(&mut self)

Completes streaming with final content preservation, state cleanup, and pending input processing.

**Critical Features:**

* **Content Preservation**: Forces final UI update to transfer all buffered content to the final message
* **Intelligent Cleanup**: Removes placeholder messages if no content was received
* **State Reset**: Properly resets all streaming-related flags and progress indicators
* **Visual Completion**: Updates progress to 100% and shows ready state with success indicator
* **Message Validation**: Ensures assistant messages are properly finalized with timestamps

**Implementation Details:**

.. code-block:: rust

   // Force final content update regardless of throttling
   if !self.streaming_buffer.is_empty() {
       if let Some(last_msg) = self.chat_history.last_mut() {
           if last_msg.message_type == MessageType::Assistant {
               last_msg.content = markdown_to_lines(&self.streaming_buffer);
               last_msg.timestamp = Self::get_timestamp();
           }
       }
   } else {
       // Remove placeholder if no content received
       if let Some(last_msg) = self.chat_history.last() {
           let is_placeholder = last_msg.content.len() == 1 && 
               last_msg.content[0].spans[0].content == "...";
           if is_placeholder {
               self.chat_history.pop();
           }
       }
   }
   
   // Complete state reset
   self.streaming_buffer.clear();
   self.is_llm_busy = false;
   self.is_input_disabled = false;
   self.response_progress = 1.0;
   self.typing_indicator.clear();

**Recovery Features:**

* **Placeholder Removal**: Automatically removes empty assistant messages if no content was received
* **Error Handling**: Gracefully handles edge cases like empty chat history or wrong message types
* **Memory Cleanup**: Clears streaming buffer after content transfer to prevent memory leaks
* **UI Synchronization**: Ensures final scroll position and redraw for proper display

**Example:**

.. code-block:: rust

   app.finish_streaming();
   // All buffered content transferred to final message
   // Progress shows 100%, status shows "✅ Ready"
   // Input re-enabled, streaming flags cleared

add_streaming_message() (Internal)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   fn add_streaming_message(&mut self)

Creates a new assistant message with "..." placeholder for streaming content.

**Features:**

* **Placeholder Creation**: Adds initial assistant message with temporary content
* **Visual Feedback**: Provides immediate indication that AI is responding
* **State Preparation**: Sets up message structure for streaming content updates

**Implementation:**

.. code-block:: rust

   let assistant_message = ChatMessage {
       message_type: MessageType::Assistant,
       content: vec![Line::from("...")],
       timestamp: Self::get_timestamp(),
   };
   self.chat_history.push(assistant_message);

**Usage:**

Called internally by `start_streaming()` to prepare the chat interface for incoming AI responses.

Navigation and Display
~~~~~~~~~~~~~~~~~~~~~~

scroll_up() / scroll_down()
^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn scroll_up(&mut self)
   pub fn scroll_down(&mut self)

Enhanced scrolling with content-aware positioning, proper bounds checking, and automatic state synchronization.

**Features:**

* **Bounds Checking**: Prevents scrolling beyond valid content range (0 to max_scroll())
* **State Synchronization**: Automatically updates internal scroll_state for scrollbar display
* **Performance Optimized**: Single-position increments for precise navigation
* **Content Awareness**: Respects actual content height and terminal dimensions

**Implementation:**

.. code-block:: rust

   // scroll_up()
   if self.scroll_position > 0 {
       self.scroll_position -= 1;
       self.update_scroll_state();
   }
   
   // scroll_down()  
   if self.scroll_position < self.max_scroll() {
       self.scroll_position += 1;
       self.update_scroll_state();
   }

**Usage Examples:**

.. code-block:: rust

   // Navigate chat history
   app.scroll_up();    // Move toward older messages
   app.scroll_down();  // Move toward newer messages
   
   // Fast scrolling (5 positions at once)
   for _ in 0..5 { app.scroll_up(); }    // Page Up behavior
   for _ in 0..5 { app.scroll_down(); }  // Page Down behavior

scroll_to_bottom()
^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn scroll_to_bottom(&mut self)

Instantly scrolls to the most recent messages with optimized positioning calculations.

**Features:**

* **Instant Navigation**: Jumps directly to the latest messages without animation
* **Automatic Calculation**: Uses max_scroll() to determine proper bottom position
* **State Synchronization**: Updates scroll_state for consistent scrollbar display
* **Content Tracking**: Automatically adjusts for changing chat history length

**Auto-called when:**

* New messages added to chat history
* Streaming responses complete
* User sends message
* Content updates require visibility

**Implementation:**

.. code-block:: rust

   pub fn scroll_to_bottom(&mut self) {
       self.scroll_position = self.max_scroll();
       self.update_scroll_state();
   }

**Example:**

.. code-block:: rust

   app.scroll_to_bottom();
   assert_eq!(app.scroll_position, app.max_scroll());
   // User now sees the most recent messages

update_scroll_state()
^^^^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn update_scroll_state(&mut self)

Synchronizes internal scroll state with UI scrollbar for consistent display and user feedback.

**Features:**

* **Scrollbar Synchronization**: Updates ScrollbarState position to match current scroll_position
* **Automatic Calls**: Called by all scroll methods to maintain consistency
* **UI Integration**: Ensures scrollbar thumb position accurately reflects content position
* **State Management**: Maintains internal state consistency across navigation operations

**Implementation:**

.. code-block:: rust

   pub fn update_scroll_state(&mut self) {
       self.scroll_state = self.scroll_state.position(self.scroll_position);
   }

max_scroll() (Internal)
^^^^^^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   fn max_scroll(&self) -> usize

Calculates the maximum valid scroll position based on content height and terminal dimensions.

**Algorithm:**

.. code-block:: rust

   // Calculate total content lines accurately
   let total_content_lines: usize = self.chat_history
       .iter()
       .map(|msg| 2 + msg.content.len()) // Header + content + separator
       .sum();
   
   // Account for terminal UI space (header + input + status = 11 lines)
   let visible_height = self.terminal_height.saturating_sub(11).max(1);
   
   // Return scroll range
   if total_content_lines > visible_height {
       total_content_lines.saturating_sub(visible_height)
   } else {
       0
   }

Animation and Visual Feedback
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

update_typing_indicator()
^^^^^^^^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn update_typing_indicator(&mut self)

Updates animated typing indicator with smooth character transitions and context-aware animation.

**Features:**

* **Smooth Animation**: 10-frame Unicode spinner animation cycle with 100ms timing
* **Context Aware**: Only animates when LLM is actively generating responses (is_llm_busy)
* **Performance Optimized**: Efficient time-based frame selection using system time
* **Memory Efficient**: Clears indicator when not in use to prevent unnecessary updates

**Animation Frames:**

.. code-block:: rust

   let indicators = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
   let current_time = SystemTime::now()
       .duration_since(UNIX_EPOCH)
       .unwrap()
       .as_millis();
   let index = (current_time / 100) % indicators.len() as u128;

**Implementation Details:**

* **Time-based**: Uses system time for consistent animation speed across different systems
* **Frame Rate**: 10 FPS (100ms per frame) for smooth visual experience without CPU waste
* **State Management**: Automatically clears when streaming stops to save resources

**Example:**

.. code-block:: rust

   app.is_llm_busy = true;
   app.update_typing_indicator();
   assert!(!app.typing_indicator.is_empty()); // Contains current spinner frame
   
   app.is_llm_busy = false;
   app.update_typing_indicator();
   assert!(app.typing_indicator.is_empty()); // Cleared when not busy

tick()
^^^^^^

.. code-block:: rust

   pub fn tick(&mut self)

Handles periodic updates for animations, cursor blinking, and visual effects with optimized timing.

**Timing System:**

* **Cursor Blink**: 500ms intervals for natural text cursor blinking
* **Animation Updates**: 50ms intervals for smooth spinner transitions (20 FPS)
* **Performance Balanced**: Optimized timing to prevent CPU waste while maintaining smooth visuals
* **State-based Updates**: Only triggers redraws when visual state actually changes

**Implementation:**

.. code-block:: rust

   let now = Instant::now();
   
   // Cursor blinking (500ms cycle)
   if now.duration_since(self.last_cursor_blink) >= Duration::from_millis(500) {
       self.cursor_blink_state = !self.cursor_blink_state;
       self.last_cursor_blink = now;
       if !self.is_input_disabled {
           self.needs_redraw = true;
       }
   }
   
   // Animation updates (50ms cycle)
   if now.duration_since(self.last_animation_tick) >= Duration::from_millis(50) {
       if self.is_llm_busy {
           self.update_typing_indicator();
           self.needs_redraw = true;
       }
       self.last_animation_tick = now;
   }

**Features:**

* **Conditional Updates**: Only updates cursor when input is enabled
* **Animation Management**: Handles typing indicator updates during LLM processing
* **Efficient Timing**: Uses separate timers for different visual elements
* **Resource Optimization**: Prevents unnecessary redraws when not needed

**Example Usage:**

.. code-block:: rust

   // In main event loop (60 FPS render cycle):
   app.tick(); // Updates all animations and cursor
   if app.needs_redraw {
       terminal.draw(|f| draw_enhanced_ui(f, &mut app, &model_name))?;
       app.needs_redraw = false;
   }

Event System
------------

AppEvent
~~~~~~~~

.. code-block:: rust

   #[derive(Debug)]
   pub enum AppEvent {
       Quit,           // Application should terminate
       Redraw,         // UI needs immediate redraw
       Key(KeyEvent),  // Keyboard input event
       Tick,           // Periodic timer event
   }

Enhanced event system for the responsive async UI loop, supporting immediate user feedback and smooth animations.

**Event Types:**

* ``Quit`` - Clean application shutdown requested
* ``Redraw`` - Immediate UI refresh needed (for responsive input)
* ``Key(KeyEvent)`` - User keyboard input with full key details
* ``Tick`` - Periodic updates for animations and cursor blinking

**Event Priorities in Main Loop:**

1. **Highest**: LLM response processing (real-time streaming)
2. **High**: Terminal input events (immediate user feedback)
3. **Medium**: Rendering updates (~60 FPS for smooth UI)
4. **Low**: Background tasks and cleanup

Advanced UI Functions
---------------------

Enhanced Event Loop
~~~~~~~~~~~~~~~~~~~~

run_ui()
^^^^^^^^

.. code-block:: rust

   pub async fn run_ui(
       terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, 
       config: AppConfig,
       model_name: String, 
       api_key: String,
       provider: Arc<GenAIProvider>
   ) -> Result<()>

Runs the enhanced asynchronous UI event loop with prioritized event handling, real-time responsiveness, and optimized performance.

**Features:**

* **Async Event Processing**: Non-blocking event handling with proper priority-based processing
* **Real-time Streaming**: Immediate LLM response processing with EOT signal prioritization
* **Responsive Input**: Instant feedback for user typing and navigation (immediate redraw)
* **Smooth Rendering**: ~60 FPS updates for fluid animations and visual feedback
* **Resource Optimization**: Balanced CPU usage, battery efficiency, and memory management

**Event Processing Architecture:**

.. code-block:: rust

   // Setup event channels and intervals
   let (tx, mut rx) = mpsc::unbounded_channel();
   let mut event_stream = EventStream::new();
   let mut tick_interval = tokio::time::interval(Duration::from_millis(50));
   let mut render_interval = tokio::time::interval(Duration::from_millis(16)); // ~60 FPS
   
   loop {
       tokio::select! {
           // Priority 1: LLM responses (highest priority for real-time streaming)
           llm_message = rx.recv() => {
               // Collect all available messages to prioritize EOT signals
               let mut all_messages = vec![message];
               while let Ok(additional) = rx.try_recv() {
                   all_messages.push(additional);
               }
               
               // Process EOT signals first to prevent state confusion
               // Then process content messages in order
               for msg in all_messages {
                   handle_llm_response(&mut app, msg, &provider, &model_name, &tx).await;
               }
               
               // Force immediate redraw for streaming responses
               terminal.draw(|f| draw_enhanced_ui(f, &mut app, &model_name))?;
           }
           
           // Priority 2: User input (immediate feedback)
           event_result = event_stream.next() => {
               if let Some(app_event) = handle_terminal_event(
                   &mut app, event, &tx, &api_key, &model_name, &provider
               ).await {
                   match app_event {
                       AppEvent::Quit => break,
                       AppEvent::Redraw => {
                           // Force immediate redraw for user input responsiveness
                           terminal.draw(|f| draw_enhanced_ui(f, &mut app, &model_name))?;
                       }
                       _ => {}
                   }
               }
           }
           
           // Priority 3: Regular rendering updates (~60 FPS)
           _ = render_interval.tick() => {
               app.tick(); // Handle animations and cursor blinking
               if app.needs_redraw {
                   terminal.draw(|f| draw_enhanced_ui(f, &mut app, &model_name))?;
                   app.needs_redraw = false;
               }
           }
           
           // Priority 4: Background tasks and cleanup
           _ = tick_interval.tick() => {
               // Additional background processing if needed
           }
       }
   }

**Advanced Features:**

* **EOT Signal Prioritization**: Processes end-of-transmission signals first to prevent state confusion
* **Message Batching**: Collects multiple messages from channel to optimize processing
* **Immediate Feedback**: Forces UI updates for user input to maintain responsiveness
* **Animation Management**: Separate timing for smooth visual effects and cursor blinking
* **Memory Management**: Efficient event processing without memory leaks or buffer overflow

**Performance Optimizations:**

* **Non-blocking Events**: Uses 10ms polling for terminal events to balance responsiveness and CPU usage
* **Selective Rendering**: Only redraws when needs_redraw flag is set or immediate feedback required
* **Efficient Intervals**: Separate timers for different update frequencies (16ms render, 50ms tick)
* **Resource Cleanup**: Proper terminal cleanup on exit with error handling

**Example Usage:**

.. code-block:: rust

   let mut terminal = setup_terminal()?;
   let config = AppConfig::load()?;
   let provider = Arc::new(GenAIProvider::new(api_key.clone()));
   
   // Run the UI loop
   run_ui(&mut terminal, config, model_name, api_key, provider).await?;
   
   // Terminal automatically cleaned up on exit
           // Cleanup and maintenance
       }
   }

**Example:**

.. code-block:: rust

   let mut terminal = setup_terminal()?;
   let config = AppConfig::load()?;
   let provider = Arc::new(GenAIProvider::new()?);
   
   run_ui(&mut terminal, config, "gpt-4o-mini".to_string(), 
          api_key, provider).await?;

Enhanced Rendering
~~~~~~~~~~~~~~~~~~

draw_enhanced_ui()
^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   fn draw_enhanced_ui(f: &mut Frame, app: &mut App, model_name: &str)

Main rendering function with enhanced layout, cursor visualization, and responsive design.

**Layout Structure:**

.. code-block:: text

   ┌─────────────────────────────────────────────────────────┐
   │ 🧠 Perspt │ Model: gpt-4o-mini │ Status: ✅ Ready        │ 3 lines
   ├─────────────────────────────────────────────────────────┤
   │                Chat History Area                        │
   │ 👤 You • 14:30                                           │ flexible
   │ Hello, can you help me with Rust?                       │ (main area)
   │                                                         │
   │ 🤖 Assistant • 14:30                                     │
   │ Of course! I'd be happy to help with Rust.              │
   │ ┌─ Code ─┐                                              │
   │ │ let x = 42;                                           │
   │ └─────────┘                                             │
   ├─────────────────────────────────────────────────────────┤
   │ > Type your message here...                   ▌         │ 3 lines
   │ ┌─ Progress Bar ─┐                                      │ 2 lines
   │ │ ████████████████████████                        │     │
   │ └─────────────────────────────────────────────────┘     │
   ├─────────────────────────────────────────────────────────┤
   │ Status: Ready │ Ctrl+C to exit                          │ 3 lines
   └─────────────────────────────────────────────────────────┘

**Features:**

* **Adaptive Layout**: Responds to terminal size changes
* **Rich Header**: Model info, status, and visual indicators
* **Enhanced Chat Area**: Icons, timestamps, and markdown rendering
* **Cursor Visualization**: Blinking cursor with position indication
* **Progress Feedback**: Real-time progress bars during AI responses
* **Contextual Status**: Dynamic status information and shortcuts

draw_enhanced_input_area()
^^^^^^^^^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   fn draw_enhanced_input_area(f: &mut Frame, area: Rect, app: &App)

Advanced input area rendering with visible cursor, scrolling support, and contextual feedback.

**Features:**

* **Visible Cursor**: Blinking cursor with character-level positioning
* **Horizontal Scrolling**: Support for long input text with auto-scroll
* **State Indicators**: Visual feedback for input disabled/enabled states
* **Progress Integration**: Shows typing progress and queue status
* **Contextual Hints**: Dynamic hints based on application state

**Cursor Rendering:**

.. code-block:: rust

   // Cursor visualization with blinking
   let cursor_style = if app.cursor_blink_state {
       Style::default().fg(Color::Black).bg(Color::White)  // Visible
   } else {
       Style::default().fg(Color::White).bg(Color::DarkGray) // Dimmed
   };

Markdown Processing
~~~~~~~~~~~~~~~~~~~

markdown_to_lines()
^^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   fn markdown_to_lines(markdown: &str) -> Vec<Line<'static>>

Advanced markdown parser converting text to richly formatted terminal output with syntax highlighting and visual enhancements.

**Supported Elements:**

.. list-table::
   :header-rows: 1
   :widths: 20 30 50

   * - Element
     - Syntax
     - Terminal Rendering
   * - **Headers**
     - ``# Header``
     - Colored and bold text by level
   * - **Code Blocks**
     - ```rust\ncode\n```
     - Bordered boxes with syntax highlighting
   * - **Inline Code**
     - ```code```
     - Highlighted background color
   * - **Bold Text**
     - ``**bold**``
     - Bold terminal styling
   * - **Italic Text**
     - ``*italic*``
     - Italic terminal styling
   * - **Lists**
     - ``- item`` or ``* item``
     - Colored bullet points with proper indentation
   * - **Block Quotes**
     - ``> quote``
     - Left border with italic text
   * - **Line Breaks**
     - Empty lines
     - Proper spacing preservation

**Code Block Rendering:**

.. code-block:: text

   ┌─ rust ─┐
   │ let greeting = "Hello, World!";     │
   │ println!("{}", greeting);           │
   └─────────────┘

**Features:**

* **Syntax-Aware**: Different colors for different code languages
* **Performance Optimized**: Efficient parsing for real-time streaming
* **Terminal-Friendly**: Colors and styles optimized for terminal display
* **Robust Parsing**: Handles malformed markdown gracefully

**Example:**

.. code-block:: rust

   let markdown = r#"
   # Example Response
   
   Here's some **bold** text and `inline code`.
   
   ```rust
   fn main() {
       println!("Hello, World!");
   }
   ```
   
   - First item
   - Second item with *emphasis*
   "#;
   
   let formatted_lines = markdown_to_lines(markdown);
   // Returns fully styled lines ready for terminal display

Error Handling and Recovery
~~~~~~~~~~~~~~~~~~~~~~~~~~~

categorize_error()
^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   fn categorize_error(error_msg: &str) -> ErrorState

Intelligent error analysis and categorization with automatic recovery suggestions.

**Analysis Process:**

1. **Pattern Matching**: Analyzes error message content for known patterns
2. **Context Extraction**: Extracts relevant technical details
3. **User Translation**: Converts technical errors to user-friendly messages
4. **Recovery Guidance**: Provides specific next steps based on error type

**Recognition Patterns:**

.. code-block:: rust

   // Network errors
   if error_lower.contains("network") || error_lower.contains("connection") {
       // Suggests checking internet connection
   }
   
   // Authentication errors  
   if error_lower.contains("api key") || error_lower.contains("unauthorized") {
       // Suggests checking API key configuration
   }
   
   // Rate limiting
   if error_lower.contains("rate limit") || error_lower.contains("too many") {
       // Suggests waiting before retry
   }

Performance and Optimization
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**Buffer Management:**

.. code-block:: rust

   const MAX_STREAMING_BUFFER_SIZE: usize = 1_000_000; // 1MB limit
   const UI_UPDATE_INTERVAL: usize = 500;              // Update frequency
   const SMALL_BUFFER_THRESHOLD: usize = 500;          // Immediate updates

**Rendering Optimization:**

* **Intelligent Redraw**: Only updates when ``needs_redraw`` flag is set
* **Streaming Throttling**: Balances responsiveness with performance
* **Animation Timing**: Optimized intervals for smooth visual effects
* **Memory Management**: Prevents buffer overflow during long responses

**Event Loop Efficiency:**

* **Priority-Based Processing**: Critical events processed first
* **Non-Blocking Operations**: Prevents UI freezing during long operations
* **Resource Management**: Balanced CPU usage and battery life

See Also
--------

* :doc:`../user-guide/basic-usage` - Basic usage guide
* :doc:`../developer-guide/extending` - UI extension guide
* :doc:`config` - Configuration module
* :doc:`llm-provider` - LLM provider integration
