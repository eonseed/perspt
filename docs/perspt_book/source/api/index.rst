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
   │                   main.rs                           │
   │              (Application Entry)                    │
   ├─────────────────────────────────────────────────────┤
   │  • CLI argument parsing                             │
   │  • Application initialization                       │
   │  • Error handling and recovery                      │
   │  • Terminal setup and cleanup                       │
   └─────────────────┬───────────────────────────────────┘
                     │
       ┌─────────────┼─────────────┐
       ▼             ▼             ▼
   ┌─────────┐  ┌─────────┐  ┌─────────────┐
   │config.rs│  │  ui.rs  │  │llm_provider │
   │         │  │         │  │    .rs      │
   ├─────────┤  ├─────────┤  ├─────────────┤
   │• Config │  │• TUI    │  │• Provider   │
   │  parsing│  │  render │  │  abstraction│
   │• Env    │  │• Events │  │• Model      │
   │  vars   │  │• Input  │  │  discovery  │
   │• Validation     │  handling   │• Streaming  │
   └─────────┘  └─────────┘  └─────────────┘

Module Dependencies
-------------------

The modules have clear dependency relationships:

**main.rs**
   - Uses all other modules
   - Orchestrates application flow
   - Handles top-level error recovery

**config.rs**
   - No dependencies on other modules
   - Pure configuration logic
   - Standalone and testable

**llm_provider.rs**
   - Depends on config.rs for configuration
   - Independent of UI concerns
   - Provider-agnostic interface

**ui.rs**
   - Depends on config.rs for UI settings
   - Uses llm_provider.rs for AI communication
   - Handles all user interaction

Key Traits and Interfaces
-------------------------

LLMProvider Trait
~~~~~~~~~~~~~~~~~

The core abstraction for AI providers:

.. code-block:: rust

   #[async_trait]
   pub trait LLMProvider {
       /// Send a chat request to the LLM provider
       async fn send_chat_request(
           &self,
           input: &str,
           model: &str,
           config: &AppConfig,
           tx: &Sender<String>
       ) -> Result<()>;

       /// Get the provider type
       fn provider_type(&self) -> ProviderType;

       /// Get available models for this provider
       async fn get_available_models(&self) -> Result<Vec<String>>;

       /// Validate model availability
       async fn validate_model(&self, model: &str) -> Result<bool>;
   }

Error Handling
--------------

Perspt uses a comprehensive error handling strategy with custom error types:

.. code-block:: rust

   use thiserror::Error;

   #[derive(Error, Debug)]
   pub enum PersptError {
       #[error("Configuration error: {0}")]
       Config(#[from] ConfigError),

       #[error("Provider error: {0}")]
       Provider(#[from] ProviderError),

       #[error("UI error: {0}")]
       Ui(#[from] UiError),

       #[error("Network error: {0}")]
       Network(#[from] NetworkError),
   }

Configuration System
--------------------

The configuration system supports multiple sources with clear precedence:

1. **Command-line arguments** (highest priority)
2. **Configuration files**
3. **Environment variables**
4. **Default values** (lowest priority)

.. code-block:: rust

   #[derive(Debug, Clone, Deserialize)]
   pub struct AppConfig {
       pub api_key: Option<String>,
       pub default_model: String,
       pub provider_type: String,
       pub providers: HashMap<String, String>,
       // ... additional fields
   }

Async Architecture
------------------

Perspt is built on Tokio's async runtime for high-performance concurrent operations:

**Streaming Responses**
   Real-time display of AI responses as they're generated

**Non-blocking UI**
   User can type new messages while AI is responding

**Concurrent Operations**
   Multiple API calls and UI updates can happen simultaneously

**Resource Efficiency**
   Minimal thread usage with async/await patterns

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

- **Zero-copy operations** where possible
- **String interning** for frequently used values
- **Efficient collection usage** with appropriate data structures
- **RAII patterns** for automatic resource cleanup

Network Efficiency
~~~~~~~~~~~~~~~~~~

- **Connection pooling** for repeated requests
- **Request batching** where supported by providers
- **Timeout handling** with configurable values
- **Retry logic** with exponential backoff

UI Performance
~~~~~~~~~~~~~~

- **Incremental rendering** only updates changed content
- **Efficient text processing** with optimized algorithms
- **Smooth scrolling** with virtual scrolling for large histories
- **Responsive input** with non-blocking event handling

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

Basic Provider Usage
~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use perspt::llm_provider::{UnifiedLLMProvider, ProviderType};
   use perspt::config::AppConfig;

   #[tokio::main]
   async fn main() -> Result<()> {
       let config = AppConfig::load(None).await?;
       let provider = UnifiedLLMProvider::new(ProviderType::OpenAI);
       
       let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
       
       provider.send_chat_request(
           "Hello, AI!",
           "gpt-4o-mini",
           &config,
           &tx
       ).await?;
       
       while let Some(chunk) = rx.recv().await {
           print!("{}", chunk);
       }
       
       Ok(())
   }

Configuration Loading
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use perspt::config::{AppConfig, load_config};

   #[tokio::main]
   async fn main() -> Result<()> {
       // Load from default locations
       let config = load_config(None).await?;
       
       // Load from specific file
       let config = load_config(Some(&"custom.json".to_string())).await?;
       
       // Process and validate
       let config = process_loaded_config(config);
       
       println!("Using model: {}", config.default_model);
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

Unit Testing
~~~~~~~~~~~~

Each module includes comprehensive unit tests:

.. code-block:: rust

   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn test_config_defaults() {
           let config = AppConfig::default();
           assert_eq!(config.default_model, "gpt-4o-mini");
       }

       #[tokio::test]
       async fn test_provider_validation() {
           let provider = UnifiedLLMProvider::new(ProviderType::OpenAI);
           assert!(provider.validate_model("gpt-4").await.unwrap());
       }
   }

Integration Testing
~~~~~~~~~~~~~~~~~~~

Full end-to-end tests validate complete workflows:

.. code-block:: rust

   #[tokio::test]
   async fn test_full_conversation() {
       let config = test_config().await;
       let app = App::new(config).await?;
       
       // Simulate user input
       app.send_message("Test message").await?;
       
       // Verify response
       let response = app.get_last_response().await?;
       assert!(!response.is_empty());
   }

Documentation Generation
------------------------

API documentation is automatically generated from source code:

.. code-block:: bash

   # Generate full documentation
   cargo doc --open --no-deps

   # Include private items
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
