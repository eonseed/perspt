.. _developer-guide:

Developer Guide
===============

Welcome to the Perspt developer guide! This section is for developers who want to understand Perspt's architecture, contribute to the project, or extend its functionality.

.. toctree::
   :maxdepth: 2
   :caption: Developer Guide Contents

   architecture
   contributing
   extending
   testing

.. contents:: Quick Navigation
   :local:
   :depth: 2

Overview
--------

Perspt is built with modern Rust practices, emphasizing performance, safety, and maintainability. The codebase is designed to be modular, testable, and easy to extend.

.. grid:: 2
   :gutter: 3

   .. grid-item-card:: 🏗️ Architecture
      :link: architecture
      :link-type: doc

      Deep dive into Perspt's design patterns, module structure, and core principles.

   .. grid-item-card:: 🤝 Contributing
      :link: contributing
      :link-type: doc

      Guidelines for contributing code, documentation, and reporting issues.

   .. grid-item-card:: 🔧 Extending
      :link: extending
      :link-type: doc

      How to add new providers, features, and customize Perspt for your needs.

   .. grid-item-card:: 🧪 Testing
      :link: testing
      :link-type: doc

      Testing strategies, test writing guidelines, and continuous integration.

Project Structure
-----------------

.. code-block:: text

   perspt/
   ├── src/
   │   ├── main.rs          # Entry point, CLI parsing, panic handling
   │   ├── config.rs        # Configuration management and validation
   │   ├── llm_provider.rs  # GenAI provider abstraction and implementation
   │   └── ui.rs            # Terminal UI with Ratatui and real-time streaming
   ├── tests/
   │   └── panic_handling_test.rs  # Integration tests
   ├── docs/
   │   ├── perspt_book/     # Sphinx documentation
   │   └── *.html           # Asset library and design system
   ├── Cargo.toml           # Dependencies and project metadata
   └── config.json.example  # Sample configuration

Core Technologies
-----------------

Technology Stack
~~~~~~~~~~~~~~~~

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Component
     - Technology & Purpose
   * - **LLM Integration**
     - `genai v0.3.5 <https://crates.io/crates/genai>`_ - Unified interface for multiple LLM providers
   * - **Async Runtime**
     - `tokio v1.42 <https://crates.io/crates/tokio>`_ - High-performance async runtime
   * - **Terminal UI**
     - `ratatui v0.29 <https://crates.io/crates/ratatui>`_ - Modern terminal user interface framework
   * - **Cross-platform Terminal**
     - `crossterm v0.28 <https://crates.io/crates/crossterm>`_ - Cross-platform terminal manipulation
   * - **CLI Framework**
     - `clap v4.5 <https://crates.io/crates/clap>`_ - Command line argument parser
   * - **Configuration**
     - `serde v1.0 <https://crates.io/crates/serde>`_ + `serde_json v1.0 <https://crates.io/crates/serde_json>`_ - Serialization framework
   * - **Markdown Rendering**
     - `pulldown-cmark v0.12 <https://crates.io/crates/pulldown-cmark>`_ - CommonMark markdown parser
   * - **Error Handling**
     - `anyhow v1.0 <https://crates.io/crates/anyhow>`_ - Flexible error handling
   * - **Async Traits**
     - `async-trait v0.1.88 <https://crates.io/crates/async-trait>`_ - Async functions in traits
   * - **Logging**
     - `log v0.4 <https://crates.io/crates/log>`_ + `env_logger v0.11 <https://crates.io/crates/env_logger>`_ - Structured logging
   * - **Streaming**
     - `futures v0.3 <https://crates.io/crates/futures>`_ - Utilities for async programming

Key Dependencies
~~~~~~~~~~~~~~~~

.. code-block:: toml

   [dependencies]
   # LLM unified interface - using genai for better model support
   genai = "0.3.5"
   futures = "0.3"

   # Core async and traits
   async-trait = "0.1.88"
   tokio = { version = "1.42", features = ["full"] }

   # CLI and configuration
   clap = { version = "4.5", features = ["derive"] }
   serde = { version = "1.0", features = ["derive"] }
   serde_json = "1.0"

   # UI components
   ratatui = "0.29"
   crossterm = "0.28"
   pulldown-cmark = "0.12"

   # Logging
   log = "0.4"
   env_logger = "0.11"

   # Utilities
   anyhow = "1.0"

   # CLI and configuration
   clap = { version = "4.5", features = ["derive"] }
   serde = { version = "1.0", features = ["derive"] }
   serde_json = "1.0"

   # UI components
   ratatui = "0.29"
   crossterm = "0.28"
   pulldown-cmark = "0.12"

   # Logging and error handling
   log = "0.4"
   env_logger = "0.11"
   anyhow = "1.0"

   [dependencies]
   tokio = { version = "1.0", features = ["full"] }
   ratatui = "0.26"
   crossterm = "0.27"
   serde = { version = "1.0", features = ["derive"] }
   serde_json = "1.0"
   genai = "0.3.5"
   clap = { version = "4.0", features = ["derive"] }
   anyhow = "1.0"
   thiserror = "1.0"

Design Principles
-----------------

Performance First
~~~~~~~~~~~~~~~~~

Every design decision prioritizes performance:

- **Zero-copy operations** where possible
- **Efficient memory usage** with careful allocation
- **Streaming responses** for immediate user feedback
- **Minimal dependencies** to reduce compile time and binary size

Safety and Reliability
~~~~~~~~~~~~~~~~~~~~~~

Rust's type system ensures memory safety and prevents common errors:

- **No null pointer dereferences** through Option types
- **Thread safety** with Send and Sync traits
- **Error handling** with Result types throughout
- **Resource management** with RAII patterns

Modularity and Extensibility
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The architecture supports easy extension and modification:

- **Trait-based abstractions** for provider independence
- **Configuration-driven behavior** without code changes
- **Plugin-ready architecture** for future extensions
- **Clear module boundaries** with well-defined interfaces

Development Environment Setup
-----------------------------

Prerequisites
~~~~~~~~~~~~~

.. code-block:: bash

   # Install Rust toolchain
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env

   # Install development tools
   cargo install cargo-watch cargo-edit cargo-audit

   # Install clippy and rustfmt
   rustup component add clippy rustfmt

   # Verify installation
   rustc --version
   cargo --version

Clone and Setup
~~~~~~~~~~~~~~~

.. code-block:: bash

   # Clone repository
   git clone https://github.com/eonseed/perspt.git
   cd perspt

   # Install dependencies
   cargo build

   # Run tests
   cargo test

   # Check code quality
   cargo clippy
   cargo fmt --check

Development Workflow
~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Watch mode for development
   cargo watch -x 'run -- --help'

   # Run specific tests
   cargo test test_config

   # Run with debug output
   RUST_LOG=debug cargo run

   # Profile performance
   cargo build --release
   perf record target/release/perspt
   perf report

Code Organization
-----------------

Module Structure
~~~~~~~~~~~~~~~~

Each module has a specific responsibility:

**main.rs**

- Application entry point and orchestration
- CLI argument parsing with clap derive macros
- Comprehensive panic handling with terminal restoration
- Terminal initialization and cleanup with crossterm
- Configuration loading and provider initialization
- Real-time event loop with enhanced responsiveness

**config.rs**

- JSON-based configuration system with serde
- Multi-provider support with intelligent defaults
- Automatic provider type inference
- Environment variable integration
- Configuration validation and fallbacks

**llm_provider.rs**

- GenAI crate integration for unified LLM access
- Support for OpenAI, Anthropic, Google (Gemini), Groq, Cohere, XAI, DeepSeek, Ollama
- Streaming response handling with proper event processing
- Model validation and discovery
- Comprehensive error categorization and recovery

**ui.rs**

- Ratatui-based terminal user interface
- Real-time markdown rendering with pulldown-cmark
- Responsive layout with scrollable chat history
- Enhanced keyboard input handling and cursor management
- Progress indicators and error display
- Help system with keyboard shortcuts

Design Patterns
~~~~~~~~~~~~~~~

**GenAI Provider Architecture:**

.. code-block:: rust

   use genai::{Client, chat::{ChatRequest, ChatMessage}};
   use futures::StreamExt;

   pub struct GenAIProvider {
       client: Client,
   }

   impl GenAIProvider {
       pub fn new() -> Result<Self> {
           let client = Client::default();
           Ok(Self { client })
       }

       pub async fn generate_response_stream_to_channel(
           &self,
           model: &str,
           prompt: &str,
           tx: mpsc::UnboundedSender<String>
       ) -> Result<()> {
           let chat_req = ChatRequest::default()
               .append_message(ChatMessage::user(prompt));

           let chat_res_stream = self.client
               .exec_chat_stream(model, chat_req, None)
               .await?;

           let mut stream = chat_res_stream.stream;
           while let Some(chunk_result) = stream.next().await {
               match chunk_result? {
                   ChatStreamEvent::Chunk(chunk) => {
                       tx.send(chunk.content)?;
                   }
                   ChatStreamEvent::End(_) => break,
                   _ => {}
               }
           }
           Ok(())
       }
   }

**Error Handling Strategy:**

.. code-block:: rust

   use anyhow::{Context, Result};
   use thiserror::Error;

   #[derive(Error, Debug)]
   pub enum PersptError {
       #[error("Configuration error: {0}")]
       Config(String),
       
       #[error("Provider error: {0}")]
       Provider(#[from] genai::GenAIError),
       
       #[error("UI error: {0}")]
       Ui(String),
       
       #[error("Network error: {0}")]
       Network(String),
   }

   // Example error handling in main
   fn setup_panic_hook() {
       panic::set_hook(Box::new(move |panic_info| {
           // Force terminal restoration immediately
           let _ = disable_raw_mode();
           let _ = execute!(io::stdout(), LeaveAlternateScreen);
           
           // Provide contextual error messages
           let panic_str = format!("{}", panic_info);
           if panic_str.contains("PROJECT_ID") {
               eprintln!("💡 Tip: Set PROJECT_ID environment variable");
           }
           // ... more context-specific help
       }));
   }

   #[derive(Error, Debug)]
   pub enum ProviderError {
       #[error("Network error: {0}")]
       Network(#[from] reqwest::Error),
       
       #[error("API error: {message}")]
       Api { message: String },
       
       #[error("Configuration error: {0}")]
       Config(String),
   }

**Configuration Pattern:**

.. code-block:: rust

   use serde::Deserialize;
   use std::collections::HashMap;

   #[derive(Debug, Clone, Deserialize, PartialEq)]
   pub struct AppConfig {
       pub providers: HashMap<String, String>,
       pub api_key: Option<String>,
       pub default_model: Option<String>,
       pub default_provider: Option<String>,
       pub provider_type: Option<String>,
   }

   pub async fn load_config(config_path: Option<&String>) -> Result<AppConfig> {
       let config: AppConfig = match config_path {
           Some(path) => {
               let config_str = fs::read_to_string(path)?;
               let initial_config: AppConfig = serde_json::from_str(&config_str)?;
               process_loaded_config(initial_config)
           }
           None => {
               // Comprehensive defaults with all supported providers
               let mut providers_map = HashMap::new();
               providers_map.insert("openai".to_string(), 
                   "https://api.openai.com/v1".to_string());
               providers_map.insert("anthropic".to_string(), 
                   "https://api.anthropic.com".to_string());
               // ... more providers
               
               AppConfig {
                   providers: providers_map,
                   api_key: None,
                   default_model: Some("gpt-4o-mini".to_string()),
                   default_provider: Some("openai".to_string()),
                   provider_type: Some("openai".to_string()),
               }
           }
       };
       Ok(config)
   }

   #[derive(Debug, Deserialize)]
   pub struct AppConfig {
       #[serde(default)]
       pub api_key: Option<String>,
       
       #[serde(default = "default_model")]
       pub default_model: String,
   }

   fn default_model() -> String {
       "gpt-4o-mini".to_string()
   }

Testing Strategy
----------------

Unit Tests
~~~~~~~~~~

Each module includes comprehensive unit tests:

.. code-block:: rust

   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn test_config_parsing() {
           let json = r#"{"api_key": "test"}"#;
           let config: AppConfig = serde_json::from_str(json).unwrap();
           assert_eq!(config.api_key, Some("test".to_string()));
       }

       #[tokio::test]
       async fn test_provider_request() {
           // Mock provider tests
       }
   }

Integration Tests
~~~~~~~~~~~~~~~~~

Full end-to-end testing in the `tests/` directory:

.. code-block:: rust

   #[tokio::test]
   async fn test_full_conversation_flow() {
       // Test complete conversation workflow
   }

Performance Benchmarks
~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use criterion::{black_box, criterion_group, criterion_main, Criterion};

   fn benchmark_config_parsing(c: &mut Criterion) {
       c.bench_function("config parsing", |b| {
           b.iter(|| {
               // Benchmark configuration parsing
           });
       });
   }

Contributing Guidelines
-----------------------

Code Style
~~~~~~~~~~

We follow standard Rust conventions:

.. code-block:: bash

   # Format code
   cargo fmt

   # Check linting
   cargo clippy -- -D warnings

   # Check documentation
   cargo doc --no-deps

Git Workflow
~~~~~~~~~~~~

.. code-block:: bash

   # Create feature branch
   git checkout -b feature/new-provider

   # Make changes and commit
   git add .
   git commit -m "feat: add support for new provider"

   # Push and create PR
   git push origin feature/new-provider

Pull Request Process
~~~~~~~~~~~~~~~~~~~~

1. **Fork** the repository
2. **Create** a feature branch
3. **Write** tests for your changes
4. **Ensure** all tests pass
5. **Submit** a pull request with clear description

Release Process
---------------

Version Management
~~~~~~~~~~~~~~~~~~

We use semantic versioning (SemVer):

- **MAJOR**: Breaking changes
- **MINOR**: New features, backward compatible
- **PATCH**: Bug fixes, backward compatible

Release Checklist
~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Update version in Cargo.toml
   # Update CHANGELOG.md
   # Run full test suite
   cargo test --all

   # Build release
   cargo build --release

   # Create git tag
   git tag v0.4.0
   git push origin v0.4.0

   # Publish to crates.io
   cargo publish

Documentation
-------------

Code Documentation
~~~~~~~~~~~~~~~~~~

Use Rust doc comments extensively:

.. code-block:: rust

   /// Sends a chat request to the LLM provider.
   ///
   /// # Arguments
   ///
   /// * `input` - The user's message
   /// * `model` - The model to use for the request
   /// * `config` - Application configuration
   /// * `tx` - Channel for streaming responses
   ///
   /// # Returns
   ///
   /// A `Result` indicating success or failure
   ///
   /// # Errors
   ///
   /// Returns `ProviderError` if the request fails
   pub async fn send_chat_request(
       &self,
       input: &str,
       model: &str,
       config: &AppConfig,
       tx: &Sender<String>
   ) -> Result<()> {
       // Implementation
   }

API Documentation
~~~~~~~~~~~~~~~~~

Generate documentation:

.. code-block:: bash

   # Generate and open docs
   cargo doc --open --no-deps

   # Generate docs with private items
   cargo doc --document-private-items

Community and Support
---------------------

Getting Help
~~~~~~~~~~~~

- **GitHub Issues**: Bug reports and feature requests
- **GitHub Discussions**: Questions and community chat
- **Discord**: Real-time development discussion
- **Documentation**: This guide and API docs

Contributing Areas
~~~~~~~~~~~~~~~~~~

We welcome contributions in:

- **Code**: New features, bug fixes, optimizations
- **Documentation**: Guides, examples, API docs
- **Testing**: Unit tests, integration tests, benchmarks
- **Design**: UI/UX improvements, accessibility
- **Community**: Helping users, writing tutorials

Next Steps
----------

Ready to dive deeper? Choose your path:

.. grid:: 2
   :gutter: 3

   .. grid-item-card:: 🏗️ Architecture Deep Dive
      :link: architecture
      :link-type: doc

      Understand the internal design and implementation details.

   .. grid-item-card:: 🤝 Start Contributing
      :link: contributing
      :link-type: doc

      Learn how to contribute code, documentation, or help the community.

   .. grid-item-card:: 🔧 Extend Functionality
      :link: extending
      :link-type: doc

      Add new providers, features, or customize Perspt.

   .. grid-item-card:: 🧪 Testing Guide
      :link: testing
      :link-type: doc

      Write tests, run benchmarks, and ensure quality.

.. seealso::

   - :doc:`../api/index` - Complete API reference
   - :doc:`../user-guide/index` - User-focused documentation
   - `GitHub Repository <https://github.com/eonseed/perspt>`_ - Source code and issues
