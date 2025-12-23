Main Module
===========

The ``main`` module serves as the entry point and orchestrator for the Perspt application, handling CLI argument parsing, application initialization, terminal management, and the main event loop coordination.

.. currentmodule:: main

Overview
--------

The main module is responsible for the complete application lifecycle, from startup to graceful shutdown. It implements comprehensive panic recovery, terminal state management, and coordinates between the UI, configuration, and LLM provider modules.

**Key Responsibilities:**

* **Application Bootstrap**: Initialize logging, parse CLI arguments, load configuration
* **Terminal Management**: Setup/cleanup terminal raw mode and alternate screen  
* **Event Coordination**: Manage the main event loop and message passing between components
* **Error Recovery**: Comprehensive panic handling with terminal restoration
* **Resource Cleanup**: Ensure proper cleanup of terminal state and background tasks

Constants
---------

EOT_SIGNAL
~~~~~~~~~~

.. code-block:: rust

   pub const EOT_SIGNAL: &str = "<<EOT>>";

End-of-transmission signal used throughout the application to indicate completion of streaming LLM responses.

**Usage Pattern:**

.. code-block:: rust

   // LLM provider sends this signal when response is complete
   tx.send(EOT_SIGNAL.to_string()).unwrap();
   
   // UI receives and recognizes completion
   if message == EOT_SIGNAL {
       app.finish_streaming();
   }

Global State
------------

TERMINAL_RAW_MODE
~~~~~~~~~~~~~~~~~

.. code-block:: rust

   static TERMINAL_RAW_MODE: std::sync::Mutex<bool> = std::sync::Mutex::new(false);

Thread-safe global flag tracking terminal raw mode state for proper cleanup during panics and application crashes.

**Safety Mechanism:**

This global state ensures that even if the application panics or crashes unexpectedly, the panic handler can properly restore the terminal to a usable state, preventing terminal corruption for the user.

Core Functions
--------------

main()
~~~~~~

.. code-block:: rust

   #[tokio::main]
   async fn main() -> Result<()>

Main application entry point that orchestrates the entire application lifecycle.

**Returns:**

* ``Result<()>`` - Success or application startup error

**Application Lifecycle:**

1. **Panic Hook Setup** - Configures comprehensive error recovery and terminal restoration
2. **Logging Initialization** - Sets up error-level logging for debugging
3. **CLI Argument Processing** - Parses command-line options with clap
4. **Configuration Management** - Loads config from file or generates intelligent defaults
5. **Provider Setup** - Initializes LLM provider with auto-configuration
6. **Model Discovery** - Optionally lists available models and exits early
7. **Terminal Initialization** - Sets up TUI with proper raw mode and alternate screen
8. **Event Loop Execution** - Runs the main UI loop with real-time responsiveness
9. **Graceful Cleanup** - Restores terminal state and releases resources

**CLI Arguments Supported:**

.. code-block:: bash

   # Basic usage with auto-configuration
   perspt
   
   # Specify provider and model
   perspt --provider-type anthropic --model-name claude-3-sonnet-20240229
   
   # Use custom configuration file
   perspt --config /path/to/config.json
   
   # List available models for current provider
   perspt --list-models
   
   # Override API key from command line
   perspt --api-key sk-your-key-here

**Error Scenarios:**

* **Configuration Errors**: Invalid JSON, missing required fields
* **Provider Failures**: Invalid API keys, network connectivity issues
* **Terminal Issues**: Raw mode setup failures, insufficient permissions
* **Resource Constraints**: Memory limitations, file system errors

Terminal Management
-------------------

setup_panic_hook()
~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   fn setup_panic_hook()

Configures a comprehensive panic handler that ensures terminal integrity and provides helpful error messages with recovery guidance.

**Recovery Actions:**

1. **Immediate Terminal Restoration**: Disables raw mode and exits alternate screen
2. **Screen Cleanup**: Clears display and positions cursor appropriately  
3. **Contextual Error Messages**: Provides specific guidance based on error type
4. **Clean Application Exit**: Prevents zombie processes and terminal corruption

**Error Context Detection:**

The panic hook intelligently detects common error scenarios:

* **Missing Environment Variables**: API keys, required configuration settings
* **Authentication Failures**: Invalid or expired API keys
* **Network Connectivity**: Connection timeouts, DNS resolution failures
* **Provider-Specific Issues**: Service outages, rate limiting

**Example Error Output:**

.. code-block:: text

   🚨 Application Error: External Library Panic
   ═══════════════════════════════════════════
   
   ❌ Missing Google Cloud Configuration:
      Please set the PROJECT_ID environment variable
      Example: export PROJECT_ID=your-project-id
   
   💡 Troubleshooting Tips:
      - Check your provider configuration
      - Verify all required environment variables are set
      - Try a different provider (e.g., --provider-type openai)

set_raw_mode_flag()
~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   fn set_raw_mode_flag(enabled: bool)

Thread-safe function to update the global terminal raw mode state flag.

**Parameters:**

* ``enabled`` - Whether raw mode is currently enabled

**Thread Safety:** Uses mutex protection to prevent race conditions during concurrent access.

initialize_terminal()
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

set_raw_mode_flag()
~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   fn set_raw_mode_flag(enabled: bool)

Thread-safe function to update the global terminal raw mode state flag.

**Parameters:**

* ``enabled`` - Boolean indicating whether raw mode is currently enabled

**Thread Safety:** 

Uses mutex protection to prevent race conditions during concurrent access. This function is called from multiple contexts:

* Main thread during terminal setup/cleanup
* Panic handler for emergency restoration
* Signal handlers for graceful shutdown

initialize_terminal()
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   fn initialize_terminal() -> Result<ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>>

Initializes the terminal interface for TUI operation with comprehensive error handling and state tracking.

**Returns:**

* ``Result<Terminal<...>>`` - Configured terminal instance or initialization error

**Initialization Sequence:**

1. **Raw Mode Activation**: Enables character-by-character input without buffering
2. **Alternate Screen Entry**: Preserves user's current terminal session
3. **Backend Creation**: Sets up crossterm backend for ratatui compatibility
4. **State Registration**: Updates global raw mode flag for panic recovery

**Error Recovery:**

If any step fails, the function automatically cleans up partial initialization to prevent terminal corruption.

cleanup_terminal()
~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   fn cleanup_terminal() -> Result<()>

Performs comprehensive terminal cleanup and restoration to original state.

**Returns:**

* ``Result<()>`` - Success indication or cleanup error details

**Restoration Process:**

1. **State Flag Reset**: Updates global raw mode tracking to false
2. **Raw Mode Disable**: Restores normal terminal input behavior
3. **Alternate Screen Exit**: Returns to user's original terminal session
4. **Cursor Restoration**: Ensures cursor visibility and proper positioning

**Fault Tolerance:** 

Each cleanup step is executed independently - if one fails, others continue to maximize terminal restoration.

Event Handling
--------------

handle_events()
~~~~~~~~~~~~~~~

.. code-block:: rust

   pub async fn handle_events(
       app: &mut ui::App,
       tx_llm: &mpsc::UnboundedSender<String>, 
       _api_key: &String,
       model_name: &String,
       provider: &Arc<GenAIProvider>, 
   ) -> Option<AppEvent>

Processes terminal events and user input in the main application loop with real-time responsiveness.

**Parameters:**

* ``app`` - Mutable reference to application state for immediate updates
* ``tx_llm`` - Channel sender for LLM communication and streaming
* ``_api_key`` - API key for provider authentication (reserved)
* ``model_name`` - Current model identifier for requests
* ``provider`` - Arc reference to configured LLM provider

**Returns:**

* ``Option<AppEvent>`` - Some(event) for significant state changes, None for no-ops

**Supported Keyboard Events:**

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Key Combination
     - Action
   * - ``Enter``
     - Send current input to LLM (queues if busy)
   * - ``Ctrl+C, Ctrl+Q``
     - Quit application gracefully
   * - ``F1``
     - Toggle help overlay display
   * - ``Esc``
     - Close help overlay or exit application
   * - ``↑/↓``
     - Scroll chat history up/down
   * - ``Page Up/Down``
     - Scroll chat history by 5 lines
   * - ``Home/End``
     - Jump to start/end of chat history
   * - ``Backspace``
     - Delete character before cursor
   * - ``Left/Right``
     - Move cursor in input field
   * - ``Printable chars``
     - Insert character at cursor position

**Input Queuing System:**

When the LLM is busy generating a response, user input is automatically queued and processed when the current response completes, ensuring no user input is lost.

Model and Provider Management
-----------------------------

list_available_models()
~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   async fn list_available_models(provider: &Arc<GenAIProvider>, _config: &AppConfig) -> Result<()>

Discovers and displays all available models for the configured LLM provider, then exits the application.

**Parameters:**

* ``provider`` - Arc reference to the initialized LLM provider
* ``_config`` - Application configuration (reserved for filtering features)

**Returns:**

* ``Result<()>`` - Success or model discovery error

**Output Format:**

.. code-block:: text

   Available models for OpenAI:
   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
   ✓ gpt-4o-mini                Latest GPT-4 Optimized Mini
   ✓ gpt-4o                     GPT-4 Optimized
   ✓ gpt-4-turbo                GPT-4 Turbo with Vision
   ✓ gpt-4                      Standard GPT-4
   ✓ gpt-3.5-turbo             GPT-3.5 Turbo
   ✓ o1-mini                    Reasoning Model Mini
   ✓ o1-preview                 Reasoning Model Preview
   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

**Provider Discovery:**

Uses the ``genai`` crate's automatic model discovery to provide up-to-date model lists without manual maintenance.
   • gpt-4o-mini
   • o1-preview
   • o1-mini
   • o3-mini
   • gpt-4-turbo

**Example Usage:**

.. code-block:: bash

   perspt --list-models --provider-type anthropic

Event Handling
--------------

handle_events()
~~~~~~~~~~~~~~~

.. code-block:: rust

   pub async fn handle_events(
       app: &mut ui::App,
       tx_llm: &mpsc::UnboundedSender<String>, 
       _api_key: &String,
       model_name: &String,
       provider: &Arc<dyn LLMProvider + Send + Sync>, 
   ) -> Option<AppEvent>

Handles terminal events and user input in the main application loop.

**Parameters:**

* ``app`` - Mutable reference to application state
* ``tx_llm`` - Channel sender for LLM communication
* ``_api_key`` - API key for LLM provider (currently unused)
* ``model_name`` - Name of current LLM model
* ``provider`` - Arc reference to LLM provider implementation

**Returns:**

* ``Option<AppEvent>`` - Some(AppEvent) for significant events, None otherwise

**Supported Events:**

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Input
     - Action
   * - ``Enter``
     - Send current input to LLM (if not busy and input not empty)
   * - ``Escape``
     - Quit application or close help overlay
   * - ``F1``, ``?``
     - Toggle help overlay display
   * - ``Ctrl+C``
     - Force quit application immediately
   * - ``Ctrl+L``
     - Clear chat history
   * - ``Arrow Up/Down``
     - Scroll chat history
   * - ``Page Up/Down``
     - Scroll chat history by page
   * - ``Home/End``
     - Scroll to top/bottom of chat
   * - ``Printable chars``
     - Add to input buffer
   * - ``Backspace``
     - Remove last character from input

**Event Processing Flow:**

1. Check for available terminal events
2. Process keyboard input through app.handle_input()
3. Handle specific application events (send message, quit, etc.)
4. Update UI state based on events
5. Return significant events to main loop

LLM Integration
---------------

initiate_llm_request()
~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   async fn initiate_llm_request(
       app: &mut ui::App,
       input_to_send: String,
       provider: Arc<dyn LLMProvider + Send + Sync>, 
       model_name: &str,
       tx_llm: &mpsc::UnboundedSender<String>,
   )

Initiates an asynchronous LLM request with proper state management and user feedback.

**Parameters:**

* ``app`` - Mutable reference to application state
* ``input_to_send`` - User's message to send to the LLM
* ``provider`` - Arc reference to LLM provider implementation
* ``model_name`` - Name/identifier of the model to use
* ``tx_llm`` - Channel sender for streaming LLM responses

**State Management:**

1. **Pre-request State:**
   * Sets ``is_llm_busy`` to true
   * Sets ``is_input_disabled`` to true  
   * Updates status message to show processing
   * Adds user message to chat history

2. **Request Processing:**
   * Spawns separate tokio task for LLM request
   * Maintains UI responsiveness during request
   * Handles provider-specific API calls

3. **Error Handling:**
   * Catches and displays network errors
   * Shows authentication failures
   * Handles rate limiting gracefully
   * Provides recovery suggestions

4. **Post-request State:**
   * Restores input availability
   * Updates status message
   * Adds response or error to chat history

**Concurrency:** Uses async/await and tokio tasks to prevent UI blocking during potentially slow LLM requests.

Utility Functions
-----------------

truncate_message()
~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   fn truncate_message(s: &str, max_chars: usize) -> String

Utility function to truncate messages for display in status areas and limited-width UI components.

**Parameters:**

* ``s`` - String to truncate
* ``max_chars`` - Maximum number of characters to include

**Returns:**

* ``String`` - Truncated string with "..." suffix if truncation occurred

**Behavior:**

* Returns original string if length ≤ max_chars
* Truncates to (max_chars - 3) and appends "..." if longer
* Handles Unicode characters properly
* Preserves word boundaries when possible

**Example:**

.. code-block:: rust

   let short = truncate_message("Hello world", 5);
   assert_eq!(short, "He...");
   
   let unchanged = truncate_message("Hi", 10);
   assert_eq!(unchanged, "Hi");

Error Handling
--------------

The main module implements comprehensive error handling across all application components:

**Panic Recovery:**

* Custom panic hook for terminal restoration
* User-friendly error messages with recovery suggestions
* Graceful degradation when possible

**Runtime Error Handling:**

* Configuration validation errors
* Provider authentication failures
* Network connectivity issues
* Terminal initialization failures
* LLM API errors and rate limiting

**Error Display:**

* Status bar error indicators
* Inline error messages in chat
* Detailed error information in logs
* Recovery action suggestions

**Example Error Scenarios:**

.. code-block:: rust

   // Configuration error
   if config.api_key.is_none() {
       return Err(anyhow!("API key not found. Please set your API key in config.json"));
   }
   
   // Provider error
   match provider.validate_config(&config).await {
       Err(e) => {
           eprintln!("Provider configuration invalid: {}", e);
           std::process::exit(1);
       }
       Ok(()) => {}
   }
   
   // Terminal error
   match initialize_terminal() {
       Err(e) => {
           eprintln!("Failed to initialize terminal: {}", e);
           eprintln!("Please ensure your terminal supports the required features.");
           std::process::exit(1);
       }
       Ok(terminal) => terminal
   }

Application Lifecycle
---------------------

The main function manages the complete application lifecycle:

**Startup Phase:**

1. Early panic hook setup for safety
2. Command-line argument processing
3. Configuration loading and validation
4. LLM provider initialization and validation
5. Terminal setup and UI initialization

**Runtime Phase:**

1. Main event loop with async event handling
2. Concurrent LLM request processing
3. Real-time UI updates and rendering
4. Error handling and recovery

**Shutdown Phase:**

1. Graceful termination signal handling
2. Terminal state restoration
3. Resource cleanup and deallocation
4. Exit with appropriate status code

**Signals and Interrupts:**

* ``Ctrl+C`` - Immediate termination with cleanup
* ``SIGTERM`` - Graceful shutdown (Unix systems)
* Panic conditions - Emergency terminal restoration

See Also
--------

* :doc:`ui` - User interface implementation
* :doc:`config` - Configuration management
* :doc:`llm-provider` - LLM provider integration
* :doc:`../user-guide/basic-usage` - Basic usage guide
* :doc:`../developer-guide/architecture` - Application architecture
