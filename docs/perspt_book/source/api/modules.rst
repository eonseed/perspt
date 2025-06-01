Module Overview
===============

Perspt is organized into several focused modules, each handling specific aspects of the application. This page provides an overview of the module architecture and how they interact.

.. currentmodule:: perspt

Architecture Overview
---------------------

.. code-block:: text

   ┌─────────────────────────────────────────────────────────────┐
   │                      Perspt Architecture                    │
   ├─────────────────────────────────────────────────────────────┤
   │                                                             │
   │  ┌─────────────┐    ┌──────────────┐    ┌─────────────────┐ │
   │  │    main     │────│    config    │────│   llm_provider  │ │
   │  │ (Entry)     │    │ (Settings)   │    │   (AI APIs)     │ │
   │  └─────────────┘    └──────────────┘    └─────────────────┘ │
   │           │                                       │         │
   │           ▼                                       ▼         │
   │  ┌─────────────┐                        ┌─────────────────┐ │
   │  │     ui      │◄───────────────────────│    External     │ │
   │  │ (Interface) │                        │     APIs        │ │
   │  └─────────────┘                        └─────────────────┘ │
   │                                                             │
   └─────────────────────────────────────────────────────────────┘

Core Modules
------------

main
~~~~

.. toctree::
   :maxdepth: 1

   main

The entry point and orchestrator of the application. Handles:

* **Application Lifecycle** - Startup, runtime, and shutdown management
* **CLI Processing** - Command-line argument parsing and validation
* **Event Loop** - Main application event handling and dispatching
* **Error Recovery** - Panic handling and terminal restoration
* **Resource Management** - Terminal initialization and cleanup

**Key Components:**

* Application initialization and configuration loading
* Terminal setup and TUI framework integration
* Event handling for user input and system events
* LLM request coordination and response management
* Graceful shutdown and error recovery

config
~~~~~~

.. toctree::
   :maxdepth: 1

   config

Configuration management and provider setup. Handles:

* **Multi-Provider Support** - Configuration for all supported AI providers
* **Intelligent Defaults** - Automatic provider type inference and sensible defaults
* **Validation** - Configuration validation and error reporting
* **File Management** - JSON configuration file loading and processing

**Key Components:**

* ``AppConfig`` structure for comprehensive configuration
* Provider type inference and validation
* Default configuration generation for all supported providers
* Configuration processing and normalization

llm-provider
~~~~~~~~~~~~

.. toctree::
   :maxdepth: 1

   llm-provider

Unified interface for AI provider integration using the modern GenAI crate. Handles:

* **Provider Abstraction** - Single interface across OpenAI, Anthropic, Google, Groq, Cohere, and XAI
* **Auto-Configuration** - Environment variable detection and automatic setup
* **Streaming Support** - Real-time response streaming with proper event handling
* **Error Handling** - Comprehensive error categorization and recovery

**Key Components:**

* ``GenAIProvider`` struct using the ``genai`` crate client
* Auto-configuration via environment variables
* Model listing and validation capabilities
* Streaming response generation to channels

ui
~~

.. toctree::
   :maxdepth: 1

   ui

Rich terminal-based user interface using Ratatui framework. Handles:

* **Real-time Chat** - Interactive chat interface with live markdown rendering
* **Streaming Display** - Real-time response streaming with configurable buffer management
* **State Management** - Comprehensive application state with cursor position tracking
* **Error Handling** - User-friendly error display with categorized error types
* **Responsive Design** - Adaptive layout with scrollable history and progress indicators

**Key Components:**

* ``App`` structure with enhanced state management and cursor tracking
* ``ChatMessage`` and ``MessageType`` for styled message representation
* ``ErrorState`` and ``ErrorType`` for comprehensive error handling
* Real-time event handling with non-blocking input processing
* Streaming buffer management with configurable update intervals

Module Interactions
-------------------

Data Flow
~~~~~~~~~

.. code-block:: text

   User Input → UI Module → Main Module → GenAI Provider → LLM API
        ↑              ↓           ↓             ↓             ↓
   Terminal ← Real-time ← Event ← Streaming ← API Response ← Provider
           Rendering    Loop     Channel

**Enhanced Flow Description:**

1. **User Input**: User types in terminal, captured by UI module with cursor tracking
2. **Event Processing**: Main module coordinates actions with comprehensive panic handling
3. **Configuration**: Config module provides auto-configured provider settings
4. **LLM Request**: GenAI provider handles API communication with environment-based auth
5. **Streaming Processing**: Real-time response streaming through unbounded channels
6. **UI Update**: UI module renders responses with markdown formatting and progress indicators

Configuration Flow
~~~~~~~~~~~~~~~~~~

.. code-block:: text

   Config File/CLI → Config Module → Provider Validation → UI Setup
        ↓                ↓                    ↓              ↓
   Defaults → Processing → Type Inference → Model List → Application Start

**Configuration Steps:**

1. **Loading**: Configuration loaded from file or generated with defaults
2. **Processing**: Provider type inference and validation
3. **Provider Setup**: LLM provider initialized with configuration
4. **Validation**: Provider configuration validated before use
5. **UI Initialization**: Application state initialized with valid configuration

Error Handling Flow
~~~~~~~~~~~~~~~~~~~

.. code-block:: text

   Error Source → Module Handler → Error State → UI Display → User Action
        ↓              ↓              ↓            ↓            ↓
   Network ─────→ LLM Provider ───→ Main ────→ UI ─────→ Recovery

**Error Handling:**

* **Network Errors**: Handled by LLM provider with retry logic
* **Configuration Errors**: Caught by config module with helpful messages
* **UI Errors**: Managed by UI module with graceful degradation
* **System Errors**: Handled by main module with proper cleanup

Module Dependencies
-------------------

Dependency Graph
~~~~~~~~~~~~~~~~

.. code-block:: text

   main
   ├── config (configuration management)
   ├── llm_provider (AI integration)
   ├── ui (user interface)
   └── External Dependencies:
       ├── tokio (async runtime)
       ├── ratatui (TUI framework)
       ├── crossterm (terminal control)
       ├── anyhow (error handling)
       ├── serde (serialization)
       └── genai (LLM provider APIs)

**Dependency Relationships:**

* ``main`` depends on all other modules
* ``ui`` uses ``config`` for application state
* ``llm_provider`` uses ``config`` for provider settings
* All modules use common external dependencies

External Integrations
~~~~~~~~~~~~~~~~~~~~~

**AI Provider APIs (via GenAI crate):**

* OpenAI GPT models (GPT-4o, GPT-4o-mini, o1-preview, o1-mini)
* Anthropic Claude models (Claude 3.5 Sonnet, Claude 3 Opus/Sonnet/Haiku)
* Google Gemini models (Gemini 1.5 Pro/Flash, Gemini 2.0 Flash)
* Groq models (Llama 3.x with ultra-fast inference)
* Cohere models (Command R, Command R+)
* XAI models (Grok)

**Terminal and System:**

* Cross-platform terminal control via ``crossterm`` (Windows, macOS, Linux)
* Real-time markdown rendering with ``ratatui``
* Async I/O with ``tokio`` runtime
* Environment variable integration for secure authentication

Module Testing
--------------

Each module includes comprehensive testing aligned with current implementation:

**Unit Tests:**

* Configuration loading with intelligent defaults (``test_load_config_defaults``)
* Provider type inference and validation (``test_load_config_from_json_string_infer_provider_type_*``)
* GenAI provider creation and model listing
* UI state management and error handling
* Panic handling and terminal restoration

**Integration Tests:**

* End-to-end configuration flow with all supported providers
* GenAI provider initialization with environment variables
* Streaming response handling with channel communication
* UI event processing and state updates
* Error propagation and user-friendly display

**Current Test Examples:**

.. code-block:: rust

   #[cfg(test)]
   mod tests {
       use super::*;

       #[tokio::test]
       async fn test_load_config_defaults() {
           let config = load_config(None).await.unwrap();
           assert_eq!(config.provider_type, Some("openai".to_string()));
           assert_eq!(config.default_model, Some("gpt-4o-mini".to_string()));
           assert!(config.providers.contains_key("openai"));
           assert!(config.providers.contains_key("groq"));
       }

       #[tokio::test]
       async fn test_genai_provider_creation() {
           let provider = GenAIProvider::new();
           assert!(provider.is_ok());
       }
   }
       use super::*;
       
       #[test]
       fn test_config_provider_inference() {
           // Test automatic provider type inference
       }
       
       #[tokio::test]
       async fn test_llm_provider_integration() {
           // Test LLM provider functionality
       }
       
       #[test]
       fn test_ui_message_formatting() {
           // Test message display and formatting
       }
   }

Best Practices
--------------

When working with Perspt modules:

**Configuration:**

* Use ``load_config(None)`` for defaults or ``load_config(Some(&path))`` for custom files
* Leverage automatic provider type inference with ``process_loaded_config()``
* Validate configuration before GenAI provider initialization
* Use environment variables for secure API key management

**GenAI Provider:**

* Use ``GenAIProvider::new()`` for auto-configuration via environment variables
* Use ``GenAIProvider::new_with_config()`` for explicit provider/key setup
* Handle streaming responses with unbounded channels for real-time display
* Implement proper error handling with ``anyhow::Result`` for detailed context

**UI Development:**

* Follow established ``MessageType`` conventions (User, Assistant, Error, etc.)
* Use ``ErrorState`` and ``ErrorType`` for categorized error display
* Maintain responsive UI with configurable streaming buffer intervals
* Implement proper cursor position tracking for enhanced user experience

**Error Handling:**

* Use ``anyhow::Result`` throughout for comprehensive error context
* Implement panic hooks for terminal state restoration
* Provide user-friendly error messages with recovery suggestions
* Use ``ErrorType`` categories for appropriate styling and user guidance

**Performance:**

* Use streaming buffers with size limits (``MAX_STREAMING_BUFFER_SIZE``)
* Update UI responsively with configurable intervals (``UI_UPDATE_INTERVAL``)
* Leverage async/await patterns for non-blocking operations
* Properly manage terminal raw mode state for clean shutdown

See Also
--------

* :doc:`../developer-guide/architecture` - Detailed architecture guide
* :doc:`../developer-guide/extending` - Module extension guide
* :doc:`../user-guide/troubleshooting` - Common issues and solutions
