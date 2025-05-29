# Perspt API Documentation

## Module: `main.rs`

### Overview
The main entry point for the Perspt application, responsible for application initialization, CLI parsing, and lifecycle management.

### Constants

#### `EOT_SIGNAL`
```rust
pub const EOT_SIGNAL: &str = "<<EOT>>";
```
**Description**: End-of-transmission signal used to indicate completion of streaming responses from LLM providers.

**Usage**: Sent by LLM providers to signal the end of a streaming response.

### Functions

#### `main()`
```rust
#[tokio::main]
async fn main() -> Result<()>
```
**Description**: Main application entry point that orchestrates the entire application lifecycle.

**Returns**: 
- `Result<()>` - Success or error details if the application fails to start

**Errors**:
- Invalid command-line arguments
- Configuration file parsing failures
- LLM provider validation failures
- Terminal initialization failures
- Network connectivity issues

**Example**:
```bash
perspt --provider-type anthropic --model-name claude-3-sonnet-20240229
```

#### `setup_panic_hook()`
```rust
fn setup_panic_hook()
```
**Description**: Sets up a comprehensive panic hook that ensures proper terminal restoration and provides user-friendly error messages.

**Behavior**:
- Immediately disables raw terminal mode
- Exits alternate screen mode
- Clears the terminal display
- Provides context-specific error messages and recovery tips
- Exits the application cleanly

**Safety**: Must be called early in main() before any terminal operations.

#### `set_raw_mode_flag(enabled: bool)`
```rust
fn set_raw_mode_flag(enabled: bool)
```
**Description**: Thread-safe function to update the global terminal raw mode state flag.

**Parameters**:
- `enabled: bool` - Whether raw mode is currently enabled

**Thread Safety**: Uses mutex protection to prevent race conditions.

#### `initialize_terminal()`
```rust
fn initialize_terminal() -> Result<ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>>
```
**Description**: Initializes the terminal for TUI operation with proper error handling.

**Returns**:
- `Result<Terminal<...>>` - Configured terminal instance or error

**Errors**:
- Raw mode cannot be enabled
- Alternate screen mode fails
- Terminal backend creation fails

**Side Effects**: Updates global raw mode flag for panic recovery.

#### `cleanup_terminal()`
```rust
fn cleanup_terminal() -> Result<()>
```
**Description**: Cleans up terminal state and restores normal operation.

**Returns**:
- `Result<()>` - Success or error if cleanup fails

**Behavior**:
- Updates global raw mode flag
- Disables raw terminal mode
- Exits alternate screen mode
- Restores original terminal state

#### `list_available_models()`
```rust
async fn list_available_models(provider: &Arc<dyn LLMProvider + Send + Sync>, _config: &AppConfig) -> Result<()>
```
**Description**: Lists all available models for the current LLM provider.

**Parameters**:
- `provider` - Arc reference to the LLM provider implementation
- `_config` - Application configuration (reserved for future features)

**Returns**:
- `Result<()>` - Success or error if model listing fails

**Example Output**:
```
Available models for OpenAI:
• gpt-4o-mini
• gpt-4o
• gpt-4-turbo
• gpt-3.5-turbo
```

#### `initiate_llm_request()`
```rust
async fn initiate_llm_request(
    app: &mut ui::App,
    input_to_send: String,
    provider: Arc<dyn LLMProvider + Send + Sync>, 
    model_name: &str,
    tx_llm: &mpsc::UnboundedSender<String>,
)
```
**Description**: Initiates an asynchronous LLM request with proper state management and user feedback.

**Parameters**:
- `app` - Mutable reference to application state
- `input_to_send` - User's message to send to the LLM
- `provider` - Arc reference to LLM provider implementation
- `model_name` - Name/identifier of the model to use
- `tx_llm` - Channel sender for streaming LLM responses

**State Changes**:
- Sets `is_llm_busy` to true
- Sets `is_input_disabled` to true
- Updates status message
- May add error messages to chat history

**Concurrency**: Spawns separate tokio task for LLM request to maintain UI responsiveness.

#### `handle_events()`
```rust
pub async fn handle_events(
    app: &mut ui::App,
    tx_llm: &mpsc::UnboundedSender<String>, 
    _api_key: &String,
    model_name: &String,
    provider: &Arc<dyn LLMProvider + Send + Sync>, 
) -> Option<AppEvent>
```
**Description**: Handles terminal events and user input in the main application loop.

**Parameters**:
- `app` - Mutable reference to application state
- `tx_llm` - Channel sender for LLM communication
- `_api_key` - API key for LLM provider (currently unused)
- `model_name` - Name of current LLM model
- `provider` - Arc reference to LLM provider implementation

**Returns**:
- `Option<AppEvent>` - Some(AppEvent) for significant events, None otherwise

**Supported Events**:
- **Enter**: Send current input to LLM (if not busy)
- **Escape**: Quit application or close help overlay
- **F1/?**: Toggle help overlay
- **Ctrl+C**: Force quit application
- **Arrow Up/Down**: Scroll chat history
- **Printable characters**: Add to input buffer
- **Backspace**: Remove last character from input

#### `truncate_message()`
```rust
fn truncate_message(s: &str, max_chars: usize) -> String
```
**Description**: Utility function to truncate messages for display in status areas.

**Parameters**:
- `s` - String to truncate
- `max_chars` - Maximum number of characters to include

**Returns**:
- `String` - Truncated string with "..." suffix if truncation occurred

**Example**:
```rust
let short = truncate_message("Hello world", 5);
assert_eq!(short, "He...");
```

### Global State

#### `TERMINAL_RAW_MODE`
```rust
static TERMINAL_RAW_MODE: Mutex<bool> = Mutex::new(false);
```
**Description**: Mutex-protected global flag tracking terminal raw mode state for panic recovery.

**Purpose**: Enables the panic handler to determine if terminal cleanup is necessary.

**Thread Safety**: Protected by Mutex to prevent race conditions.

### Error Handling Patterns

The main module implements comprehensive error handling:

1. **Panic Recovery**: Global panic hook ensures terminal restoration
2. **Graceful Degradation**: Application continues with defaults when possible
3. **User-Friendly Messages**: Context-specific error messages and recovery tips
4. **Resource Cleanup**: Automatic terminal state restoration

### Usage Examples

#### Basic Usage
```bash
# Start with defaults
perspt

# Specify provider and model
perspt --provider-type anthropic --model-name claude-3-sonnet-20240229

# Use custom configuration
perspt --config /path/to/config.json --api-key your-key
```

#### With Environment Variables
```bash
export OPENAI_API_KEY="sk-your-key"
export RUST_LOG="debug"
perspt --model-name gpt-4
```

#### Debugging Mode
```bash
RUST_LOG=debug perspt --provider-type openai --list-models
```
