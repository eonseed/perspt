API Reference
=============

Complete API documentation for Perspt, automatically generated from source code comments and organized by module.

.. toctree::
   :maxdepth: 2
   :caption: API Reference

   modules
   config
   llm-provider
   ui
   main

Overview
--------

The Perspt API is organized into four main modules, each with a specific responsibility:

.. grid:: 2
   :gutter: 3

   .. grid-item-card:: 📋 Configuration (config.rs)
      :link: config
      :link-type: doc

      Configuration management, file parsing, and environment variable handling.

   .. grid-item-card:: 🤖 LLM Provider (llm_provider.rs)
      :link: llm-provider
      :link-type: doc

      Unified interface to multiple AI providers with automatic model discovery.

   .. grid-item-card:: 🎨 User Interface (ui.rs)
      :link: ui
      :link-type: doc

      Terminal-based chat interface with real-time rendering and event handling.

   .. grid-item-card:: 🚀 Main Application (main.rs)
      :link: main
      :link-type: doc

      Application entry point, CLI parsing, and lifecycle management.

Architecture Overview
---------------------

.. code-block:: text

   ┌─────────────────────────────────────────────────────┐
   │                     main.rs                         │
   │              (Application Entry)                    │
   ├─────────────────────────────────────────────────────┤
   │  • CLI argument parsing with clap                   │
   │  • Application initialization & config loading      │
   │  • Comprehensive panic handling & recovery          │
   │  • Terminal setup, cleanup & state management       │
   │  • Event loop coordination                          │
   └─────────────────┬───────────────────────────────────┘
                     │
       ┌─────────────┼─────────────┐
       ▼             ▼             ▼
   ┌───────────┐ ┌───────────┐ ┌───────────────┐
   │ config.rs │ │   ui.rs   │ │ llm_provider  │
   │           │ │           │ │     .rs       │
   ├───────────┤ ├───────────┤ ├───────────────┤
   │ • Multi-  │ │ • Ratatui │ │ • GenAI       │
   │   provider│ │   TUI     │ │   client      │
   │ • Smart   │ │ • Real-   │ │ • Multi-      │
   │   defaults│ │   time    │ │   provider    │
   │ • Type    │ │   markdown│ │ • Streaming   │
   │  inference│ │ • Scroll- │ │ • Auto-config │
   │ • JSON    │ │   able    │ │ • Error       │
   │   config  │ │   history │ │   handling    │
   └───────────┘ └───────────┘ └───────────────┘

Module Dependencies
-------------------

The modules have clear dependency relationships:

**main.rs**
   - Application orchestrator and entry point
   - Uses all other modules for complete functionality
   - Handles panic recovery and terminal state management
   - Coordinates event loop and user interactions

**config.rs**
   - Standalone configuration management
   - Supports 8+ LLM providers with intelligent defaults
   - JSON-based configuration with environment variable integration
   - Provider type inference and validation

**llm_provider.rs**
   - Uses modern `genai` crate for unified provider interface
   - Supports OpenAI, Anthropic, Google, Groq, Cohere, XAI
   - Auto-configuration via environment variables
   - Streaming response handling and model discovery

**ui.rs**
   - Rich terminal UI using Ratatui framework
   - Real-time markdown rendering and streaming support
   - Scrollable chat history with responsive event handling
   - Enhanced input management with cursor positioning

Key Structures and Interfaces
-----------------------------

GenAIProvider Struct
~~~~~~~~~~~~~~~~~~~~~

The modern unified provider implementation using the genai crate:

.. code-block:: rust

   pub struct GenAIProvider {
       client: Client,
   }

   impl GenAIProvider {
       /// Creates provider with auto-configuration
       pub fn new() -> Result<Self>
       
       /// Creates provider with explicit configuration
       pub fn new_with_config(
           provider_type: Option<&str>, 
           api_key: Option<&str>
       ) -> Result<Self>
       
       /// Generates simple text response
       pub async fn generate_response_simple(
           &self,
           model: &str,
           message: &str
       ) -> Result<String>
       
       /// Generates streaming response to channel
       pub async fn generate_response_stream_to_channel(
           &self,
           model: &str,
           message: &str,
           sender: mpsc::UnboundedSender<String>
       ) -> Result<()>
       
       /// Lists available models for current provider
       pub async fn list_models(&self) -> Result<Vec<String>>
   }

Supported Providers
~~~~~~~~~~~~~~~~~~~

The GenAI provider supports multiple LLM services:

.. list-table::
   :header-rows: 1
   :widths: 20 30 50

   * - Provider
     - Environment Variable
     - Supported Models
   * - OpenAI
     - ``OPENAI_API_KEY``
     - GPT-4o, GPT-4o-mini, GPT-4, GPT-3.5, o1-preview, o1-mini
   * - Anthropic
     - ``ANTHROPIC_API_KEY``
     - Claude 3.5 Sonnet, Claude 3 Opus/Sonnet/Haiku
   * - Google
     - ``GEMINI_API_KEY``
     - Gemini 1.5 Pro/Flash, Gemini 2.0 Flash
   * - Groq
     - ``GROQ_API_KEY``
     - Llama 3.x models with ultra-fast inference
   * - Cohere
     - ``COHERE_API_KEY``
     - Command R, Command R+
   * - XAI
     - ``XAI_API_KEY``
     - Grok models

Error Handling
--------------

Perspt uses comprehensive error handling with proper context and user-friendly messages:

.. code-block:: rust

   use anyhow::{Context, Result};

   // All functions return Result<T> with proper error context
   pub async fn load_config(config_path: Option<&String>) -> Result<AppConfig> {
       // Configuration loading with detailed error context
   }

   pub async fn generate_response_simple(
       &self,
       model: &str,
       message: &str
   ) -> Result<String> {
       // Provider communication with error handling
   }
Configuration System
--------------------

The configuration system supports multiple sources with intelligent defaults:

1. **JSON Configuration Files** (explicit configuration)
2. **Environment Variables** (for API keys and credentials)
3. **Intelligent Defaults** (comprehensive provider endpoints)
4. **Provider Type Inference** (automatic detection)

.. code-block:: rust

   #[derive(Debug, Clone, Deserialize, PartialEq)]
   pub struct AppConfig {
       pub providers: HashMap<String, String>,
       pub api_key: Option<String>,
       pub default_model: Option<String>,
       pub default_provider: Option<String>,
       pub provider_type: Option<String>,
   }

   // Load configuration with smart defaults
   pub async fn load_config(config_path: Option<&String>) -> Result<AppConfig>
   
   // Process configuration with provider type inference
   pub fn process_loaded_config(mut config: AppConfig) -> AppConfig

Provider Type Inference
~~~~~~~~~~~~~~~~~~~~~~~

The configuration system automatically infers provider types from provider names:

.. list-table::
   :header-rows: 1
   :widths: 30 30 40

   * - Provider Name
     - Inferred Type
     - Notes
   * - ``openai``
     - ``openai``
     - Direct mapping
   * - ``anthropic``
     - ``anthropic``
     - Direct mapping
   * - ``google``, ``gemini``
     - ``google``
     - Multiple aliases supported
   * - ``groq``
     - ``groq``
     - Fast inference provider
   * - ``cohere``
     - ``cohere``
     - Command models
   * - ``xai``
     - ``xai``
     - Grok models
   * - Unknown
     - ``openai``
     - Fallback default

Async Architecture
------------------

Perspt is built on Tokio's async runtime for high-performance concurrent operations:

**Streaming Responses**
   Real-time display of AI responses as they're generated using async channels

**Non-blocking UI**
   User can continue typing while AI responses stream in real-time

**Concurrent Operations**
   Multiple API calls and UI updates happen simultaneously without blocking

**Resource Efficiency**
   Minimal memory footprint with efficient async/await patterns

Type Safety
-----------

Rust's type system ensures correctness throughout the codebase:

**Option Types**
   Explicit handling of optional values prevents null pointer errors

**Result Types**
   All fallible operations return Result for explicit error handling

**Strong Typing**
   Configuration, messages, and provider types are strongly typed

**Compile-time Guarantees**
   Many errors are caught at compile time rather than runtime

Performance Considerations
--------------------------

Memory Management
~~~~~~~~~~~~~~~~~

- **Streaming buffers** with configurable size limits (1MB max)
- **Efficient VecDeque** for chat history with automatic cleanup
- **RAII patterns** for automatic resource cleanup
- **Minimal allocations** in hot paths for better performance

Network Efficiency
~~~~~~~~~~~~~~~~~~

- **GenAI client pooling** handles connection reuse automatically
- **Streaming responses** reduce memory usage for long responses
- **Timeout handling** with proper error recovery
- **Environment-based auth** avoids credential storage

UI Performance
~~~~~~~~~~~~~~

- **Real-time rendering** with responsive update intervals (500 chars)
- **Efficient scrolling** with proper state management
- **Markdown rendering** using optimized terminal formatting
- **Non-blocking input** with cursor position management
- **Progress indicators** for better user feedback

Terminal Integration
~~~~~~~~~~~~~~~~~~~~

- **Crossterm compatibility** across platforms (Windows, macOS, Linux)
- **Raw mode management** with proper cleanup on panic
- **Alternate screen** support for clean terminal experience
- **Unicode support** for international characters and emojis

API Stability
-------------

Version Compatibility
~~~~~~~~~~~~~~~~~~~~~

Perspt follows semantic versioning:

- **Major versions** may include breaking API changes
- **Minor versions** add features while maintaining compatibility
- **Patch versions** fix bugs without changing public APIs

Deprecation Policy
~~~~~~~~~~~~~~~~~~

- **Deprecated features** are marked in documentation
- **Migration guides** provided for breaking changes
- **Compatibility period** of at least one major version
- **Clear communication** about upcoming changes

Usage Examples
--------------

Usage Examples
--------------

Basic Provider Usage
~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use perspt::llm_provider::GenAIProvider;
   use tokio::sync::mpsc;

   #[tokio::main]
   async fn main() -> Result<()> {
       // Create provider with auto-configuration
       let provider = GenAIProvider::new()?;
       
       // Simple text generation
       let response = provider.generate_response_simple(
           "gpt-4o-mini",
           "Hello, how are you?"
       ).await?;
       
       println!("Response: {}", response);
       Ok(())
   }

Streaming Response Usage
~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use perspt::llm_provider::GenAIProvider;
   use tokio::sync::mpsc;

   #[tokio::main]
   async fn main() -> Result<()> {
       let provider = GenAIProvider::new()?;
       let (tx, mut rx) = mpsc::unbounded_channel();
       
       // Start streaming response
       provider.generate_response_stream_to_channel(
           "gpt-4o-mini",
           "Tell me a story",
           tx
       ).await?;
       
       // Process streaming chunks
       while let Some(chunk) = rx.recv().await {
           print!("{}", chunk);
           std::io::stdout().flush()?;
       }
       
       Ok(())
   }

Configuration Loading
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use perspt::config::{AppConfig, load_config};

   #[tokio::main]
   async fn main() -> Result<()> {
       // Load with defaults (no config file)
       let config = load_config(None).await?;
       
       // Load from specific file
       let config = load_config(Some(&"config.json".to_string())).await?;
       
       println!("Provider: {:?}", config.provider_type);
       println!("Model: {:?}", config.default_model);
       Ok(())
   }

Custom UI Events
~~~~~~~~~~~~~~~~

.. code-block:: rust

   use perspt::ui::{App, AppEvent};
   use crossterm::event::{self, Event, KeyCode};

   fn handle_events(app: &mut App) -> Result<()> {
       if event::poll(Duration::from_millis(100))? {
           if let Event::Key(key) = event::read()? {
               match key.code {
                   KeyCode::Enter => {
                       app.handle_event(AppEvent::SendMessage)?;
                   }
                   KeyCode::Char(c) => {
                       app.handle_event(AppEvent::Input(c))?;
                   }
                   _ => {}
               }
           }
       }
       Ok(())
   }

Testing APIs
------------

Testing APIs
------------

Unit Testing
~~~~~~~~~~~~

Each module includes comprehensive unit tests:

.. code-block:: rust

   #[cfg(test)]
   mod tests {
       use super::*;

       #[tokio::test]
       async fn test_load_config_defaults() {
           let config = load_config(None).await.unwrap();
           assert_eq!(config.provider_type, Some("openai".to_string()));
           assert_eq!(config.default_model, Some("gpt-4o-mini".to_string()));
       }

       #[tokio::test]
       async fn test_provider_creation() {
           let provider = GenAIProvider::new().unwrap();
           // Provider created successfully
       }
   }

Integration Testing
~~~~~~~~~~~~~~~~~~~

End-to-end tests validate complete workflows:

.. code-block:: rust

   #[tokio::test]
   async fn test_streaming_response() {
       let provider = GenAIProvider::new().unwrap();
       let (tx, mut rx) = mpsc::unbounded_channel();
       
       provider.generate_response_stream_to_channel(
           "gpt-4o-mini",
           "Hello",
           tx
       ).await.unwrap();
       
       // Verify streaming works
       let first_chunk = rx.recv().await;
       assert!(first_chunk.is_some());
   }

Documentation Generation
------------------------

API documentation is automatically generated from source code:

.. code-block:: bash

   # Generate Rust documentation
   cargo doc --open --no-deps --all-features

   # Build Sphinx documentation
   cd docs/perspt_book && uv run make html
   cargo doc --document-private-items

   # Generate for specific package
   cargo doc --package perspt

Best Practices
--------------

When using the Perspt API:

1. **Always handle errors** explicitly with Result types
2. **Use async/await** for all I/O operations
3. **Prefer streaming** for better user experience
4. **Validate configuration** before using providers
5. **Test provider connectivity** before starting conversations
6. **Handle network timeouts** gracefully
7. **Use appropriate logging** levels for debugging

.. seealso::

   - :doc:`../developer-guide/index` - Development guidelines and architecture
   - :doc:`../user-guide/index` - User-focused documentation
   - `GitHub Repository <https://github.com/eonseed/perspt>`_ - Source code and examples
