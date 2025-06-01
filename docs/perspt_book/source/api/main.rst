Main Module
===========

The ``main`` module serves as the entry point for the Perspt application, responsible for application initialization, CLI parsing, and lifecycle management.

.. currentmodule:: main

Overview
--------

The main module orchestrates the entire application lifecycle, from terminal initialization to graceful shutdown. It provides comprehensive error handling, panic recovery, and manages the interaction between UI components and LLM providers.

Constants
---------

EOT_SIGNAL
~~~~~~~~~~

.. code-block:: rust

   pub const EOT_SIGNAL: &str = "<<EOT>>";

End-of-transmission signal used to indicate completion of streaming responses from LLM providers.

**Usage:** Sent by LLM providers to signal the end of a streaming response.

Global State
------------

TERMINAL_RAW_MODE
~~~~~~~~~~~~~~~~~

.. code-block:: rust

   static TERMINAL_RAW_MODE: std::sync::Mutex<bool> = std::sync::Mutex::new(false);

Thread-safe global flag tracking terminal raw mode state for proper cleanup during panics.

Core Functions
--------------

main()
~~~~~~

.. code-block:: rust

   #[tokio::main]
   async fn main() -> Result<()>

Main application entry point that orchestrates the entire application lifecycle.

**Returns:**

* ``Result<()>`` - Success or error details if the application fails to start

**Application Flow:**

1. **Panic Hook Setup** - Configures panic recovery and terminal restoration
2. **CLI Parsing** - Processes command-line arguments and options
3. **Configuration Loading** - Loads configuration from file or defaults
4. **Provider Initialization** - Sets up LLM provider based on configuration
5. **Model Listing** - Optionally lists available models and exits
6. **Terminal Setup** - Initializes TUI terminal interface
7. **Main Loop** - Runs event loop until quit signal
8. **Cleanup** - Restores terminal state and exits gracefully

**Possible Errors:**

* Invalid command-line arguments
* Configuration file parsing failures
* LLM provider validation failures
* Terminal initialization failures
* Network connectivity issues

**Example Usage:**

.. code-block:: bash

   # Basic usage
   perspt
   
   # With specific provider and model
   perspt --provider-type anthropic --model-name claude-3-sonnet-20240229
   
   # List available models
   perspt --list-models --provider-type openai

Terminal Management
-------------------

setup_panic_hook()
~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   fn setup_panic_hook()

Sets up a comprehensive panic hook that ensures proper terminal restoration and provides user-friendly error messages.

**Behavior:**

1. Immediately disables raw terminal mode
2. Exits alternate screen mode
3. Clears the terminal display
4. Provides context-specific error messages and recovery tips
5. Exits the application cleanly

**Safety:** Must be called early in main() before any terminal operations.

**Error Messages:** Provides helpful context and recovery suggestions:

.. code-block:: text

   ╭─ Perspt encountered an unexpected error ─╮
   │                                          │
   │ A critical error occurred that caused    │
   │ the application to crash. This is likely │
   │ due to a network issue or invalid        │
   │ configuration.                           │
   │                                          │
   │ Please check:                            │
   │ • Your internet connection               │
   │ • API key configuration                  │
   │ • Provider availability                  │
   │                                          │
   │ If the problem persists, please report   │
   │ this issue with the error details above. │
   ╰──────────────────────────────────────────╯

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

   fn initialize_terminal() -> Result<ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>>

Initializes the terminal for TUI operation with proper error handling.

**Returns:**

* ``Result<Terminal<...>>`` - Configured terminal instance or error

**Setup Process:**

1. Enables raw terminal mode for character-by-character input
2. Enters alternate screen mode to preserve user's terminal session
3. Creates crossterm backend for ratatui
4. Updates global raw mode flag for panic recovery

**Possible Errors:**

* Raw mode cannot be enabled (terminal not supported)
* Alternate screen mode fails (terminal limitations)
* Terminal backend creation fails (I/O errors)

cleanup_terminal()
~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   fn cleanup_terminal() -> Result<()>

Cleans up terminal state and restores normal operation.

**Returns:**

* ``Result<()>`` - Success or error if cleanup fails

**Cleanup Process:**

1. Updates global raw mode flag to false
2. Disables raw terminal mode
3. Exits alternate screen mode
4. Restores original terminal state and cursor

**Error Handling:** Continues cleanup even if individual steps fail, ensuring maximum restoration.

Model and Provider Management
-----------------------------

list_available_models()
~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   async fn list_available_models(provider: &Arc<dyn LLMProvider + Send + Sync>, _config: &AppConfig) -> Result<()>

Lists all available models for the current LLM provider and exits.

**Parameters:**

* ``provider`` - Arc reference to the LLM provider implementation
* ``_config`` - Application configuration (reserved for future features)

**Returns:**

* ``Result<()>`` - Success or error if model listing fails

**Output Format:**

.. code-block:: text

   Available models for OpenAI:
   • gpt-4.1
   • gpt-4o
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
