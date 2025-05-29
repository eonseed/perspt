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

Unified interface for AI provider integration. Handles:

* **Provider Abstraction** - Unified API across different AI services
* **Model Discovery** - Automatic model enumeration using the ``allms`` crate
* **Request Management** - Streaming request handling and response processing
* **Error Handling** - Provider-specific error handling and recovery

**Key Components:**

* ``UnifiedLLMProvider`` for consistent provider interaction
* ``LLMProvider`` trait defining the common interface
* Provider type enumeration and string conversion
* Model validation and availability checking

ui
~~

.. toctree::
   :maxdepth: 1

   ui

Terminal-based user interface using Ratatui. Handles:

* **Interactive Chat** - Real-time chat interface with markdown support
* **Message Management** - Chat history, scrolling, and message formatting
* **State Management** - Application state, input handling, and UI updates
* **Error Display** - User-friendly error presentation and recovery options

**Key Components:**

* ``App`` structure managing complete application state
* ``ChatMessage`` and ``MessageType`` for message representation
* Event handling for keyboard input and navigation
* Comprehensive rendering system with markdown support

Module Interactions
-------------------

Data Flow
~~~~~~~~~

.. code-block:: text

   User Input → UI Module → Main Module → LLM Provider → External API
        ↑                      ↓              ↓              ↓
   Terminal ← UI Rendering ← Event Loop ← Response ← API Response

**Flow Description:**

1. **User Input**: User types in terminal, captured by UI module
2. **Event Processing**: Main module processes events and coordinates actions
3. **Configuration**: Config module provides provider settings and validation
4. **LLM Request**: LLM provider module handles API communication
5. **Response Processing**: Streaming responses processed and formatted
6. **UI Update**: UI module renders responses with appropriate formatting

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
       └── allms (LLM provider APIs)

**Dependency Relationships:**

* ``main`` depends on all other modules
* ``ui`` uses ``config`` for application state
* ``llm_provider`` uses ``config`` for provider settings
* All modules use common external dependencies

External Integrations
~~~~~~~~~~~~~~~~~~~~~

**AI Provider APIs:**

* OpenAI GPT models
* Anthropic Claude models
* Google Gemini models
* Mistral AI models
* Perplexity AI models
* DeepSeek models
* AWS Bedrock service

**Terminal and System:**

* Cross-platform terminal control via ``crossterm``
* Unicode and markdown support
* Async I/O and event handling

Module Testing
--------------

Each module includes comprehensive testing:

**Unit Tests:**

* Configuration parsing and validation
* Provider type inference
* Message formatting and display
* Error handling and recovery

**Integration Tests:**

* End-to-end configuration flow
* Provider initialization and validation
* UI event handling and state management
* Error propagation and display

**Example Test Structure:**

.. code-block:: rust

   #[cfg(test)]
   mod tests {
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

* Use ``load_config()`` for consistent configuration loading
* Leverage automatic provider type inference when possible
* Validate configuration before provider initialization

**LLM Provider:**

* Use the unified provider interface for consistency
* Handle errors gracefully with appropriate user feedback
* Leverage streaming responses for better user experience

**UI Development:**

* Follow the established message type conventions
* Implement proper error display and recovery options
* Maintain responsive UI during long-running operations

**Error Handling:**

* Use ``anyhow::Result`` for comprehensive error context
* Provide user-friendly error messages with recovery suggestions
* Implement proper cleanup in error conditions

See Also
--------

* :doc:`../developer-guide/architecture` - Detailed architecture guide
* :doc:`../developer-guide/extending` - Module extension guide
* :doc:`../user-guide/troubleshooting` - Common issues and solutions
