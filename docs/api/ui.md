# UI Module API Documentation

## Overview

The `ui.rs` module implements the terminal-based user interface for the Perspt chat application using the Ratatui TUI framework. It provides a rich, interactive chat experience with real-time markdown rendering, enhanced scrollable chat history, and comprehensive error handling.

**Recent Improvements:**
- **Enhanced Scroll System**: Accurate text wrapping calculations for long responses
- **Unicode Text Support**: Proper character counting using `.chars().count()`
- **Content Protection**: Conservative buffering prevents content cutoff at bottom
- **Consistent Rendering**: Unified logic between scroll calculations and display

```
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
```

## Core Components

### MessageType

Represents the type of message in the chat interface, determining visual styling and behavior.

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum MessageType {
    User,      // Blue styling for user input
    Assistant, // Green styling for AI responses  
    Error,     // Red styling for error messages
    System,    // Gray styling for system notifications
    Warning,   // Yellow styling for warnings
}
```

**Usage Example:**
```rust
use perspt::ui::MessageType;

let user_msg = MessageType::User;      // Blue styling
let ai_msg = MessageType::Assistant;   // Green styling  
let error_msg = MessageType::Error;    // Red styling
```

### ChatMessage

Represents a single message in the chat interface with content, styling, and metadata.

```rust
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub message_type: MessageType,
    pub content: Vec<Line<'static>>,
    pub timestamp: String,
}
```

**Fields:**
- `message_type`: Determines styling and visual treatment
- `content`: Formatted content as styled lines (supports markdown)
- `timestamp`: When the message was created (HH:MM format)

**Usage Example:**
```rust
use perspt::ui::{ChatMessage, MessageType};
use ratatui::text::Line;

let message = ChatMessage {
    message_type: MessageType::User,
    content: vec![Line::from("Hello, AI!")],
    timestamp: "14:30".to_string(),
};
```

### ErrorState

Comprehensive error information for user display with categorization and details.

```rust
#[derive(Debug, Clone)]
pub struct ErrorState {
    pub message: String,
    pub details: Option<String>,
    pub error_type: ErrorType,
}
```

**Fields:**
- `message`: Primary error message for display
- `details`: Optional additional debugging information
- `error_type`: Category for appropriate handling and styling

**Error Types:**
```rust
#[derive(Debug, Clone)]
pub enum ErrorType {
    Network,        // Connectivity issues
    Authentication, // Provider auth failures
    RateLimit,      // API rate limiting
    InvalidModel,   // Unsupported model requests
    ServerError,    // Provider server errors
    Unknown,        // Unclassified errors
}
```

**Usage Example:**
```rust
use perspt::ui::{ErrorState, ErrorType};

let error = ErrorState {
    message: "Network connection failed".to_string(),
    details: Some("Check your internet connection".to_string()),
    error_type: ErrorType::Network,
};
```

### App (Main Controller)

Central application state and controller managing the entire chat interface.

```rust
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
```

**Key State Fields:**
- `chat_history`: Complete conversation history
- `input_text`: Current user input buffer
- `status_message`: Bottom status bar content
- `is_llm_busy`: Whether AI response is being generated
- `current_error`: Active error state for display
- `scroll_position`: Current view position in chat history

## Core Methods

### App Creation and Setup

#### `new(config: AppConfig) -> Self`

Creates a new App instance with welcome message and default state.

**Parameters:**
- `config`: Application configuration with LLM provider settings

**Returns:** Initialized App instance

**Example:**
```rust
use perspt::ui::App;
use perspt::config::AppConfig;

let config = AppConfig::load().unwrap();
let app = App::new(config);
assert!(!app.should_quit);
assert!(!app.chat_history.is_empty()); // Contains welcome message
```

### Message Management

#### `add_message(&mut self, message: ChatMessage)`

Adds a new message to chat history with automatic timestamping and scroll-to-bottom.

**Parameters:**
- `message`: ChatMessage to add (timestamp will be set automatically)

**Example:**
```rust
let message = ChatMessage {
    message_type: MessageType::User,
    content: vec![Line::from("Hello!")],
    timestamp: String::new(), // Will be set automatically
};

app.add_message(message);
```

#### `add_error(&mut self, error: ErrorState)`

Adds an error to both error state and chat history with proper formatting.

**Parameters:**
- `error`: ErrorState with message, details, and type

**Example:**
```rust
let error = ErrorState {
    message: "API request failed".to_string(),
    details: Some("Rate limit exceeded".to_string()),
    error_type: ErrorType::RateLimit,
};

app.add_error(error);
```

### Status and Error Management

#### `set_status(&mut self, message: String, is_error: bool)`

Updates the status bar message with optional error logging.

**Parameters:**
- `message`: Status text to display
- `is_error`: Whether to log as error

**Example:**
```rust
app.set_status("Processing request...".to_string(), false);
app.set_status("Connection failed".to_string(), true);
```

#### `clear_error(&mut self)`

Clears the current error state, allowing normal status messages.

**Example:**
```rust
app.clear_error();
assert!(app.current_error.is_none());
```

### Scroll Management

The scroll system has been enhanced to handle long responses and text wrapping accurately:

#### Enhanced Features
- **Accurate Text Wrapping**: Uses `.chars().count()` for proper Unicode character counting
- **Consistent Calculations**: Unified logic between scroll position calculation and rendering
- **Content Protection**: Conservative buffer ensures no content is cut off at the bottom
- **Separator Line Handling**: Properly accounts for separator lines in scroll calculations
- **Debug Support**: Comprehensive logging for troubleshooting scroll issues

#### `scroll_up(&mut self)` / `scroll_down(&mut self)`

Navigate through chat history by single positions with accurate boundary checking.

**Example:**
```rust
app.scroll_up();   // Move toward older messages
app.scroll_down(); // Move toward newer messages
```

#### `scroll_to_bottom(&mut self)`

Jump to the most recent messages (bottom of history) with guaranteed content visibility.

**Example:**
```rust
app.scroll_to_bottom();
assert_eq!(app.scroll_position, app.max_scroll());
```

#### `max_scroll(&self) -> usize`

Calculates the maximum scroll position with accurate text wrapping consideration.

**Features:**
- Accounts for text wrapping using character count (not byte length)
- Includes separator lines and headers in calculations
- Applies conservative buffer to prevent content cutoff
- Handles Unicode text correctly

**Example:**
```rust
let max_pos = app.max_scroll();
// Safe to scroll up to max_pos without losing content
```

#### `update_scroll_state(&mut self)`

Synchronizes internal scroll state with UI scrollbar using consistent calculations.

**Implementation:**
- Uses identical wrapping logic as `max_scroll()` for consistency
- Updates scrollbar content length and position
- Called automatically by other scroll methods

### Visual Feedback

#### `update_typing_indicator(&mut self)`

Updates animated spinner when LLM is generating responses.

**Example:**
```rust
app.is_llm_busy = true;
app.update_typing_indicator();
assert!(!app.typing_indicator.is_empty()); // Contains spinner character
```

## Event Handling

### AppEvent

Events that drive the main application loop.

```rust
#[derive(Debug)]
pub enum AppEvent {
    Key(KeyEvent), // Keyboard input events
    Tick,          // Timer events for animations
}
```

**Event Loop Pattern:**
```rust
use crossterm::event::{self, Event, KeyCode};
use perspt::ui::{App, AppEvent};

loop {
    match event::read()? {
        Event::Key(key) => {
            match key.code {
                KeyCode::Enter => {
                    // Process user input
                    let input = app.input_text.clone();
                    app.input_text.clear();
                    // Send to LLM...
                }
                KeyCode::Up => app.scroll_up(),
                KeyCode::Down => app.scroll_down(),
                KeyCode::Char('q') => app.should_quit = true,
                _ => {}
            }
        }
        _ => {}
    }
    
    if app.should_quit {
        break;
    }
}
```

## Markdown Rendering

The UI module includes sophisticated markdown rendering for AI responses:

### Features
- **Headers**: H1-H4 with size and color differentiation
- **Code blocks**: Syntax highlighting and proper formatting
- **Lists**: Bullet points and numbered lists
- **Emphasis**: Bold, italic, and combined formatting
- **Links**: Clickable URLs (when supported by terminal)
- **Blockquotes**: Indented with visual indicators

### Rendering Pipeline

```
Markdown Text → pulldown_cmark Parser → Styled Lines → Ratatui Display
     ↓                    ↓                   ↓             ↓
"# Header"     →    Header Event    →    Bold+Color   →   Terminal
"**bold**"     →    Strong Event    →    Bold Style   →   Terminal  
"- item"       →    List Event      →    Bullet+Text  →   Terminal
```

### Custom Styling

The module applies consistent color schemes:
- **Headers**: Magenta with bold formatting
- **Code**: Cyan background with white text
- **Lists**: Green bullet points
- **Emphasis**: Bold and italic combinations
- **Errors**: Red text with warning icons
- **Success**: Green text with check marks

## Layout Management

The UI uses a responsive layout system:

```
┌─────────────────────────────────────────────────────────────┐
│                     Chat History                            │
│  ┌─────────────────────────────────────────────────────────┐│
│  │ [14:30] User: Hello, AI!                               ││
│  │ [14:31] Assistant: Hello! How can I help you today?   ││
│  │ [14:32] User: What's the weather like?                ││
│  │ [14:33] Assistant: I don't have access to current...  ││
│  │                                              ▲ ▼ ◄ ►  ││
│  └─────────────────────────────────────────────────────────┘│
├─────────────────────────────────────────────────────────────┤
│ Input: Type your message here...                   [ENTER] │
├─────────────────────────────────────────────────────────────┤
│ Status: Ready | Provider: OpenAI | Model: gpt-4 | ? Help   │
└─────────────────────────────────────────────────────────────┘
```

### Layout Components
1. **Chat Area**: Scrollable message history (main content)
2. **Input Area**: User text input with prompt indicator  
3. **Status Bar**: Provider info, status messages, and help indicator

## Integration Examples

### Basic Usage

```rust
use perspt::ui::{App, run_app};
use perspt::config::AppConfig;

#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::load()?;
    let mut app = App::new(config);
    
    // Run the main UI loop
    run_app(&mut app).await?;
    
    Ok(())
}
```

### Adding Custom Messages

```rust
use perspt::ui::{App, ChatMessage, MessageType};
use ratatui::text::{Line, Span};
use ratatui::style::{Color, Style};

// Add a styled system message
let message = ChatMessage {
    message_type: MessageType::System,
    content: vec![
        Line::from(vec![
            Span::styled("🔄 ", Style::default().fg(Color::Blue)),
            Span::styled("Switching to GPT-4...", Style::default().fg(Color::Gray)),
        ])
    ],
    timestamp: String::new(),
};

app.add_message(message);
```

### Error Handling

```rust
use perspt::ui::{ErrorState, ErrorType};

// Handle different error types
match error_type {
    "network" => {
        let error = ErrorState {
            message: "Network connection failed".to_string(),
            details: Some("Check your internet connection".to_string()),
            error_type: ErrorType::Network,
        };
        app.add_error(error);
    }
    "auth" => {
        let error = ErrorState {
            message: "Authentication failed".to_string(),
            details: Some("Please check your API key".to_string()),
            error_type: ErrorType::Authentication,
        };
        app.add_error(error);
    }
    _ => {
        let error = ErrorState {
            message: "Unknown error occurred".to_string(),
            details: None,
            error_type: ErrorType::Unknown,
        };
        app.add_error(error);
    }
}
```

## Performance Considerations

### Memory Management
- Chat history is kept in memory for the session duration
- Large conversations may consume significant memory
- Consider implementing history pruning for long sessions

### Rendering Optimization
- Uses efficient diff-based rendering via Ratatui
- Markdown parsing is cached per message
- Scroll state updates are minimal and targeted

### Responsiveness
- Non-blocking event handling
- Background LLM requests with progress indicators
- Smooth animations with configurable timing

## Dependencies

The UI module relies on several key crates:

- **Ratatui**: Terminal UI framework for layout and rendering
- **Crossterm**: Cross-platform terminal control
- **Pulldown-cmark**: Markdown parsing and processing
- **Tokio**: Async runtime for non-blocking operations
- **Anyhow**: Error handling and propagation

These provide a robust foundation for the terminal-based chat interface with modern UI patterns and reliable cross-platform support.
