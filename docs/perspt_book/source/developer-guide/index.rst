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
   ├── src/                    # Source code
   │   ├── main.rs            # Application entry point
   │   ├── config.rs          # Configuration management
   │   ├── llm_provider.rs    # LLM provider abstraction
   │   └── ui.rs              # Terminal user interface
   ├── tests/                 # Integration tests
   ├── docs/                  # Documentation
   ├── Cargo.toml             # Project metadata and dependencies
   └── README.md              # Project overview

Core Technologies
-----------------

Technology Stack
~~~~~~~~~~~~~~~~

.. list-table::
   :widths: 25 25 50
   :header-rows: 1

   * - Technology
     - Version
     - Purpose
   * - **Rust**
     - 1.70+
     - Core language for performance and safety
   * - **Tokio**
     - 1.0+
     - Async runtime for concurrent operations
   * - **Ratatui**
     - 0.26+
     - Terminal user interface framework
   * - **Serde**
     - 1.0+
     - JSON serialization and configuration
   * - **genai**
     - 0.1+
     - Unified LLM provider interface
   * - **clap**
     - 4.0+
     - Command-line argument parsing

Key Dependencies
~~~~~~~~~~~~~~~~

.. code-block:: toml

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
   - Application entry point
   - CLI argument parsing
   - Error handling and panic recovery
   - Terminal initialization and cleanup

**config.rs**
   - Configuration file parsing
   - Environment variable handling
   - Default value management
   - Configuration validation

**llm_provider.rs**
   - Provider abstraction layer
   - Model discovery and validation
   - Request/response handling
   - Error categorization

**ui.rs**
   - Terminal interface rendering
   - Event handling and input processing
   - Message formatting and display
   - Real-time updates and streaming

Design Patterns
~~~~~~~~~~~~~~~

**Trait Objects for Providers:**

.. code-block:: rust

   pub trait LLMProvider {
       async fn send_chat_request(
           &self,
           input: &str,
           model: &str,
           config: &AppConfig,
           tx: &Sender<String>
       ) -> Result<()>;
       
       fn provider_type(&self) -> ProviderType;
   }

**Error Handling Strategy:**

.. code-block:: rust

   use thiserror::Error;

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
