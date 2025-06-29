Contributing
============

Welcome to the Perspt project! This guide will help you get started with contributing to Perspt, whether you're fixing bugs, adding features, or improving documentation.

Getting Started
---------------

Prerequisites
~~~~~~~~~~~~~

Before contributing, ensure you have:

- **Rust** (latest stable version)
- **Git** for version control
- **A GitHub account** for pull requests
- **Code editor** with Rust support (VS Code with rust-analyzer recommended)

Development Environment Setup
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

1. **Fork and Clone**:

   .. code-block:: bash

      # Fork the repository on GitHub, then:
      git clone https://github.com/YOUR_USERNAME/perspt.git
      cd perspt

2. **Set up the development environment**:

   .. code-block:: bash

      # Install Rust if not already installed
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

      # Install additional components
      rustup component add clippy rustfmt

      # Install development dependencies (optional but recommended)
      cargo install cargo-watch cargo-nextest

3. **Set up API keys for testing**:

   .. code-block:: bash

      # Copy example config
      cp config.json.example config.json
      
      # Edit config.json with your API keys (optional for basic development)
      # Or set environment variables:
      export OPENAI_API_KEY="your-key-here"
      export ANTHROPIC_API_KEY="your-key-here"

4. **Verify the setup**:

   .. code-block:: bash

      # Build the project
      cargo build

      # Run tests (some may be skipped without API keys)
      cargo test

      # Check formatting and linting
      cargo fmt --check
      cargo clippy -- -D warnings

      # Test the application
      cargo run -- "Hello, can you help me?"

Development Workflow
--------------------

Branch Strategy
~~~~~~~~~~~~~~~

We follow a simplified Git flow:

- **main**: Stable, production-ready code
- **develop**: Integration branch for new features
- **feature/**: Feature development branches
- **fix/**: Bug fix branches
- **docs/**: Documentation improvement branches

Creating a Feature Branch
~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Ensure you're on the latest develop branch
   git checkout develop
   git pull origin develop

   # Create a new feature branch
   git checkout -b feature/your-feature-name

   # Make your changes
   # ...

   # Commit your changes
   git add .
   git commit -m "feat: add your feature description"

   # Push to your fork
   git push origin feature/your-feature-name

Code Style and Standards
------------------------

Rust Style Guide
~~~~~~~~~~~~~~~~

We follow the official Rust style guide with these additions:

**Formatting**:

.. code-block:: bash

   # Auto-format your code
   cargo fmt

**Linting**:

.. code-block:: bash

   # Check for common issues
   cargo clippy -- -D warnings

**Documentation**:

.. code-block:: rust

   /// Brief description of the function.
   ///
   /// More detailed explanation if needed.
   ///
   /// # Arguments
   ///
   /// * `param1` - Description of parameter
   /// * `param2` - Description of parameter
   ///
   /// # Returns
   ///
   /// Description of return value
   ///
   /// # Errors
   ///
   /// Description of possible errors
   ///
   /// # Examples
   ///
   /// ```
   /// let result = function_name(arg1, arg2);
   /// assert_eq!(result, expected);
   /// ```
   pub fn function_name(param1: Type1, param2: Type2) -> Result<ReturnType, Error> {
       // Implementation
   }

Naming Conventions
~~~~~~~~~~~~~~~~~~

- **Functions and variables**: `snake_case`
- **Types and traits**: `PascalCase`
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Modules**: `snake_case`

.. code-block:: rust

   // Good
   pub struct LlmProvider;
   pub trait ConfigManager;
   pub fn process_message() -> Result<String, Error>;
   pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

   // Avoid
   pub struct llmProvider;
   pub trait configManager;
   pub fn ProcessMessage() -> Result<String, Error>;

Error Handling
~~~~~~~~~~~~~~

Use the `thiserror` crate for error definitions:

.. code-block:: rust

   use thiserror::Error;

   #[derive(Error, Debug)]
   pub enum ConfigError {
       #[error("Configuration file not found: {path}")]
       FileNotFound { path: String },
       
       #[error("Invalid configuration: {reason}")]
       Invalid { reason: String },
       
       #[error("IO error: {0}")]
       Io(#[from] std::io::Error),
   }

Testing Guidelines
------------------

Test Structure
~~~~~~~~~~~~~~

Organize tests in the same file as the code they test:

.. code-block:: rust

   pub struct MessageProcessor {
       config: Config,
   }

   impl MessageProcessor {
       pub fn new(config: Config) -> Self {
           Self { config }
       }

       pub async fn process(&self, input: &str) -> Result<String, ProcessError> {
           // Implementation using GenAI crate
           validate_message(input)?;
           let response = send_message(&self.config, input, tx).await?;
           Ok(response)
       }
   }

   #[cfg(test)]
   mod tests {
       use super::*;
       use tokio::sync::mpsc;

       #[test]
       fn test_message_validation() {
           let processor = MessageProcessor::new(Config::default());
           assert!(processor.validate_message("valid message").is_ok());
           assert!(processor.validate_message("").is_err());
       }

       #[tokio::test]
       async fn test_async_processing() {
           // Skip if no API key available
           if std::env::var("OPENAI_API_KEY").is_err() {
               return;
           }

           let config = Config {
               provider: "openai".to_string(),
               api_key: std::env::var("OPENAI_API_KEY").ok(),
               model: Some("gpt-3.5-turbo".to_string()),
               ..Default::default()
           };

           let (tx, mut rx) = mpsc::unbounded_channel();
           let result = send_message(&config, "test", tx).await;
           assert!(result.is_ok());
       }
   }

Integration Tests
~~~~~~~~~~~~~~~~~

Place integration tests in the `tests/` directory:

.. code-block:: rust

   // tests/integration_test.rs
   use perspt::config::Config;
   use perspt::llm_provider::send_message;
   use std::env;
   use tokio::sync::mpsc;

   #[tokio::test]
   async fn test_full_conversation_flow() {
       // Skip if no API keys available
       if env::var("OPENAI_API_KEY").is_err() {
           return;
       }

       let config = Config {
           provider: "openai".to_string(),
           api_key: env::var("OPENAI_API_KEY").ok(),
           model: Some("gpt-3.5-turbo".to_string()),
           temperature: Some(0.7),
           max_tokens: Some(100),
           timeout_seconds: Some(30),
       };

       let (tx, mut rx) = mpsc::unbounded_channel();
       
       // Test streaming response
       let result = send_message(&config, "Hello, how are you?", tx).await;
       assert!(result.is_ok());

       // Verify we receive streaming content
       let mut received_content = String::new();
       while let Ok(content) = rx.try_recv() {
           received_content.push_str(&content);
       }
       assert!(!received_content.is_empty());
   }

   #[test]
   fn test_config_loading_hierarchy() {
       // Test config loading from different sources
       let config = Config::load();
       assert!(config.is_ok());
   }

Test Categories
~~~~~~~~~~~~~~~

We have several categories of tests:

1. **Unit Tests**: Test individual functions and methods

   .. code-block:: bash

      # Run only unit tests
      cargo test --lib

2. **Integration Tests**: Test module interactions

   .. code-block:: bash

      # Run integration tests
      cargo test --test '*'

3. **API Tests**: Test against real APIs (require API keys)

   .. code-block:: bash

      # Run with API keys set
      OPENAI_API_KEY=xxx ANTHROPIC_API_KEY=yyy cargo test

4. **UI Tests**: Test terminal UI components

   .. code-block:: bash

      # Run UI tests (may require TTY)
      cargo test ui::tests

Test Utilities
~~~~~~~~~~~~~~

Use these utilities for consistent testing:

.. code-block:: rust

   // Test configuration helper
   impl Config {
       pub fn test_config() -> Self {
           Config {
               provider: "test".to_string(),
               api_key: Some("test-key".to_string()),
               model: Some("test-model".to_string()),
               temperature: Some(0.7),
               max_tokens: Some(100),
               timeout_seconds: Some(30),
           }
       }
   }

   // Mock message sender for testing
   pub async fn mock_send_message(
       _config: &Config,
       message: &str,
       tx: tokio::sync::mpsc::UnboundedSender<String>,
   ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
       tx.send(format!("Mock response to: {}", message))?;
       Ok(())
   }

Running Tests
~~~~~~~~~~~~~

.. code-block:: bash

   # Run all tests
   cargo test

   # Run tests with output
   cargo test -- --nocapture

   # Run specific test
   cargo test test_name

   # Run tests with coverage (requires cargo-tarpaulin)
   cargo install cargo-tarpaulin
   cargo tarpaulin --out Html

Pull Request Process
--------------------

Before Submitting
~~~~~~~~~~~~~~~~~

1. **Ensure tests pass**:

   .. code-block:: bash

      cargo test
      cargo clippy -- -D warnings
      cargo fmt --check

2. **Update documentation** if needed
3. **Add tests** for new functionality
4. **Update changelog** if applicable

PR Description Template
~~~~~~~~~~~~~~~~~~~~~~~

When creating a pull request, use this template:

.. code-block:: markdown

   ## Description
   Brief description of changes made.

   ## Type of Change
   - [ ] Bug fix (non-breaking change which fixes an issue)
   - [ ] New feature (non-breaking change which adds functionality)
   - [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
   - [ ] Documentation update

   ## Testing
   - [ ] Unit tests added/updated
   - [ ] Integration tests added/updated
   - [ ] Manual testing performed

   ## Checklist
   - [ ] Code follows the project's style guidelines
   - [ ] Self-review completed
   - [ ] Comments added to hard-to-understand areas
   - [ ] Documentation updated
   - [ ] No new warnings introduced

Review Process
~~~~~~~~~~~~~~

1. **Automated checks** must pass (CI/CD pipeline)
2. **Code review** by at least one maintainer
3. **Testing** in development environment
4. **Final approval** and merge

Areas for Contribution
----------------------

Good First Issues
~~~~~~~~~~~~~~~~~

Look for issues labeled `good first issue`:

- Documentation improvements and typo fixes
- Configuration validation enhancements
- Error message improvements
- Test coverage improvements
- Code formatting and cleanup
- Example configurations for new providers

Feature Development
~~~~~~~~~~~~~~~~~~~

Major areas where contributions are welcome:

**New AI Provider Support**:

.. code-block:: rust

   // Add support for new providers in llm_provider.rs
   pub async fn send_message_custom_provider(
       config: &Config,
       message: &str,
       tx: UnboundedSender<String>,
   ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
       // Use the GenAI crate to add new provider support
       let client = genai::Client::builder()
           .with_api_key(&config.api_key.unwrap_or_default())
           .build()?;

       let chat_req = genai::chat::ChatRequest::new(vec![
           genai::chat::ChatMessage::user(message)
       ]);

       let stream = client.exec_stream(&config.model.clone().unwrap_or_default(), chat_req).await?;
       
       // Handle streaming response
       // Implementation details...
       
       Ok(())
   }

**UI Component Enhancements**:

.. code-block:: rust

   // Add new Ratatui components in ui.rs
   pub struct CustomWidget {
       content: String,
       scroll_offset: u16,
   }

   impl CustomWidget {
       pub fn render(&self, area: Rect, buf: &mut Buffer) {
           let block = Block::default()
               .borders(Borders::ALL)
               .title("Custom Feature");
           
           let inner = block.inner(area);
           block.render(area, buf);
           
           // Custom rendering logic using Ratatui
           self.render_content(inner, buf);
       }
   }

**Configuration System Extensions**:

.. code-block:: rust

   // Extend Config struct in config.rs
   #[derive(Debug, Deserialize, Serialize, Clone)]
   pub struct ExtendedConfig {
       #[serde(flatten)]
       pub base: Config,
       
       // New configuration options
       pub custom_endpoints: Option<HashMap<String, String>>,
       pub retry_config: Option<RetryConfig>,
       pub logging_config: Option<LoggingConfig>,
   }

**Performance and Reliability**:

- Streaming response optimizations
- Better error handling and recovery
- Configuration validation improvements
- Memory usage optimizations for large conversations
- Connection pooling and retry logic

**Developer Experience**:

- Better debugging tools and logging
- Enhanced error messages with suggestions
- Configuration validation with helpful feedback
- Developer-friendly CLI options

Bug Reports and Issues
----------------------

Filing Bug Reports
~~~~~~~~~~~~~~~~~~

When filing a bug report, include:

1. **Clear description** of the issue
2. **Steps to reproduce** the problem
3. **Expected behavior** vs actual behavior
4. **Environment information**:

   .. code-block:: text

      - OS: [e.g., macOS 12.0, Ubuntu 20.04]
      - Perspt version: [e.g., 1.0.0]
      - Rust version: [e.g., 1.70.0]
      - Provider: [e.g., OpenAI GPT-4]

5. **Configuration** (sanitized):

   .. code-block:: json

      {
        "provider": "openai",
        "model": "gpt-4",
        "api_key": "[REDACTED]"
      }

6. **Error messages** (full text)
7. **Log files** if available

Feature Requests
~~~~~~~~~~~~~~~~

For feature requests, provide:

1. **Clear description** of the desired feature
2. **Use case** and motivation
3. **Proposed implementation** (if you have ideas)
4. **Alternatives considered**
5. **Additional context** or examples

Documentation Contributions
---------------------------

Types of Documentation
~~~~~~~~~~~~~~~~~~~~~~

- **API documentation**: Rust doc comments in source code
- **Developer guides**: Sphinx documentation in `docs/perspt_book/`
- **README**: Project overview and quick start
- **Examples**: Sample configurations and use cases
- **Changelog**: Version history and migration guides

Documentation Standards
~~~~~~~~~~~~~~~~~~~~~~~

- Use clear, concise language
- Include working code examples that match current implementation
- Keep examples up-to-date with current API and dependencies
- Cross-reference related sections using Sphinx references
- Follow reStructuredText formatting for Sphinx docs

Building Documentation
~~~~~~~~~~~~~~~~~~~~~~

**Rust API Documentation**:

.. code-block:: bash

   # Generate and open Rust documentation
   cargo doc --open --no-deps --all-features

**Sphinx Documentation**:

.. code-block:: bash

   # Build HTML documentation
   cd docs/perspt_book
   uv run make html
   
   # Build PDF documentation  
   uv run make latexpdf
   
   # Clean and rebuild everything
   uv run make clean && uv run make html && uv run make latexpdf

**Watch Mode for Development**:

.. code-block:: bash

   # Auto-rebuild on changes
   cd docs/perspt_book
   uv run sphinx-autobuild source build/html

**Available VS Code Tasks**:

You can also use the VS Code tasks for documentation:

- "Build Sphinx HTML Documentation"
- "Build Sphinx PDF Documentation" 
- "Watch and Auto-build HTML Documentation"
- "Open Sphinx HTML Documentation"
- "Validate Documentation Links"

Writing Documentation
~~~~~~~~~~~~~~~~~~~~~

**Code Examples**: Ensure all code examples compile and work:

.. code-block:: rust

   // Good: Complete, working example
   use perspt::config::Config;
   use tokio::sync::mpsc;

   #[tokio::main]
   async fn main() -> Result<(), Box<dyn std::error::Error>> {
       let config = Config::load()?;
       let (tx, mut rx) = mpsc::unbounded_channel();
       
       perspt::llm_provider::send_message(&config, "Hello", tx).await?;
       
       while let Some(response) = rx.recv().await {
           println!("{}", response);
       }
       
       Ok(())
   }

**Configuration Examples**: Use realistic, sanitized configs:

.. code-block:: json

   {
     "provider": "openai",
     "api_key": "${OPENAI_API_KEY}",
     "model": "gpt-4",
     "temperature": 0.7,
     "max_tokens": 2000,
     "timeout_seconds": 30
   }

Community Guidelines
--------------------

Code of Conduct
~~~~~~~~~~~~~~~

We follow the Rust Code of Conduct. In summary:

- Be friendly and patient
- Be welcoming
- Be considerate
- Be respectful
- Be careful in word choice
- When we disagree, try to understand why

Communication Channels
~~~~~~~~~~~~~~~~~~~~~~

- **GitHub Issues**: Bug reports and feature requests
- **GitHub Discussions**: General questions and ideas
- **Discord/Slack**: Real-time community chat
- **Email**: Direct contact with maintainers

Recognition
-----------

Contributors are recognized in:

- **CONTRIBUTORS.md**: List of all contributors
- **Release notes**: Major contributions highlighted
- **Documentation**: Author attribution where appropriate
- **Community highlights**: Regular contributor spotlights

Release Process
---------------

Version Numbering
~~~~~~~~~~~~~~~~~

We follow Semantic Versioning (SemVer):

- **MAJOR**: Breaking changes
- **MINOR**: New features (backward compatible)
- **PATCH**: Bug fixes (backward compatible)

Release Cycle
~~~~~~~~~~~~~

- **Major releases**: Every 6-12 months
- **Minor releases**: Every 1-3 months
- **Patch releases**: As needed for critical fixes

Next Steps
----------

See the following documentation for more detailed information:

- :doc:`architecture` - Understanding Perspt's internal architecture
- :doc:`extending` - How to extend Perspt with new features
- :doc:`testing` - Testing strategies and best practices
- :doc:`../user-guide/index` - User guide for understanding the application
- :doc:`../api/index` - API reference documentation

Development Workflow Tips
-------------------------

Using VS Code Tasks
~~~~~~~~~~~~~~~~~~~

The project includes several VS Code tasks for common development activities:

.. code-block:: bash

   # Available tasks (use Ctrl+Shift+P -> "Tasks: Run Task"):
   - "Generate Documentation" (cargo doc)
   - "Build Sphinx HTML Documentation"
   - "Build Sphinx PDF Documentation"
   - "Watch and Auto-build HTML Documentation"
   - "Clean and Build All Documentation"
   - "Validate Documentation Links"

Hot Reloading During Development
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

For faster development cycles:

.. code-block:: bash

   # Watch for changes and rebuild
   cargo install cargo-watch
   cargo watch -x 'build'
   
   # Watch and run tests
   cargo watch -x 'test'
   
   # Watch and run with sample input
   cargo watch -x 'run -- "test message"'

Debugging
~~~~~~~~~

**Enable Debug Logging**:

.. code-block:: bash

   # Set environment variable for detailed logs
   export RUST_LOG=debug
   cargo run -- "your message"

**Debug Streaming Issues**:

The project includes debug scripts:

.. code-block:: bash

   # Debug long responses and streaming
   ./debug-long-response.sh

**Use Rust Debugger**:

.. code-block:: rust

   // Add debug prints in your code
   eprintln!("Debug: config = {:?}", config);
   
   // Use dbg! macro for quick debugging
   let result = dbg!(some_function());

Project Structure Understanding
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Key files and their purposes:

- ``src/main.rs``: CLI entry point, panic handling, terminal setup
- ``src/config.rs``: Configuration loading and validation
- ``src/llm_provider.rs``: GenAI integration and streaming
- ``src/ui.rs``: Ratatui terminal UI components
- ``Cargo.toml``: Dependencies and project metadata
- ``config.json.example``: Sample configuration file
- ``docs/perspt_book/``: Sphinx documentation source
- ``tests/``: Integration tests
- ``validate-docs.sh``: Documentation validation script

Common Development Patterns
~~~~~~~~~~~~~~~~~~~~~~~~~~~

**Error Handling Pattern**:

.. code-block:: rust

   use anyhow::{Context, Result};
   use thiserror::Error;

   #[derive(Error, Debug)]
   pub enum MyError {
       #[error("Configuration error: {0}")]
       Config(String),
       #[error("Network error")]
       Network(#[from] reqwest::Error),
   }

   pub fn example_function() -> Result<String> {
       let config = load_config()
           .context("Failed to load configuration")?;
       
       process_config(&config)
           .context("Failed to process configuration")
   }

**Async/Await Pattern**:

.. code-block:: rust

   use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};

   pub async fn stream_handler(
       mut rx: UnboundedReceiver<String>,
       tx: UnboundedSender<String>,
   ) -> Result<()> {
       while let Some(message) = rx.recv().await {
           let processed = process_message(&message).await?;
           tx.send(processed).context("Failed to send processed message")?;
       }
       Ok(())
   }

**Configuration Pattern**:

.. code-block:: rust

   use serde::{Deserialize, Serialize};

   #[derive(Debug, Deserialize, Serialize, Clone)]
   pub struct ModuleConfig {
       pub enabled: bool,
       pub timeout: Option<u64>,
       #[serde(default)]
       pub advanced_options: AdvancedOptions,
   }

   impl Default for ModuleConfig {
       fn default() -> Self {
           Self {
               enabled: true,
               timeout: Some(30),
               advanced_options: AdvancedOptions::default(),
           }
       }
   }

Dependency Management
~~~~~~~~~~~~~~~~~~~~~

**Adding New Dependencies**:

.. code-block:: bash

   # Add a new dependency
   cargo add serde --features derive
   
   # Add a development dependency
   cargo add --dev mockall
   
   # Add an optional dependency
   cargo add optional-dep --optional

**Dependency Guidelines**:

1. **Minimize dependencies**: Only add what's necessary
2. **Use well-maintained crates**: Check recent updates and issues
3. **Consider security**: Use ``cargo audit`` to check for vulnerabilities
4. **Version pinning**: Be specific about versions in Cargo.toml

.. code-block:: toml

   # Good: Specific versions
   serde = { version = "1.0.196", features = ["derive"] }
   tokio = { version = "1.36.0", features = ["full"] }
   
   # Avoid: Wildcard versions
   serde = "*"

**Security Auditing**:

.. code-block:: bash

   # Install cargo-audit
   cargo install cargo-audit
   
   # Run security audit
   cargo audit
   
   # Update advisories database
   cargo audit --update

Release Process
~~~~~~~~~~~~~~~

**Version Bumping**:

.. code-block:: bash

   # Update version in Cargo.toml
   # Update CHANGELOG.md with changes
   # Create release notes
   
   # Tag the release
   git tag -a v1.2.0 -m "Release version 1.2.0"
   git push origin v1.2.0

**Pre-release Checklist**:

1. All tests pass: ``cargo test``
2. Documentation builds: ``cargo doc``
3. No clippy warnings: ``cargo clippy -- -D warnings``
4. Code formatted: ``cargo fmt --check``
5. CHANGELOG.md updated
6. Version bumped in Cargo.toml
7. Security audit clean: ``cargo audit``

**Release Notes Template**:

.. code-block:: markdown

   ## Version X.Y.Z - YYYY-MM-DD

   ### Added
   - New features and enhancements

   ### Changed
   - Breaking changes and modifications

   ### Fixed
   - Bug fixes and issue resolutions

   ### Security
   - Security-related changes

   ### Dependencies
   - Updated dependencies

Performance Profiling
~~~~~~~~~~~~~~~~~~~~~

**CPU Profiling**:

.. code-block:: bash

   # Install profiling tools
   cargo install cargo-flamegraph
   
   # Profile your application
   cargo flamegraph --bin perspt -- "test message"

**Memory Profiling**:

.. code-block:: bash

   # Use valgrind (Linux/macOS)
   cargo build
   valgrind --tool=massif target/debug/perspt "test message"

**Benchmarking**:

.. code-block:: rust

   // Add to benches/benchmark.rs
   use criterion::{black_box, criterion_group, criterion_main, Criterion};

   fn benchmark_message_processing(c: &mut Criterion) {
       c.bench_function("process_message", |b| {
           b.iter(|| {
               let result = process_message(black_box("test input"));
               result
           })
       });
   }

   criterion_group!(benches, benchmark_message_processing);
   criterion_main!(benches);

Troubleshooting Common Issues
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**Build Failures**:

.. code-block:: bash

   # Clean build artifacts
   cargo clean
   
   # Update toolchain
   rustup update
   
   # Rebuild dependencies
   cargo build

**Test Failures**:

.. code-block:: bash

   # Run tests with output
   cargo test -- --nocapture
   
   # Run a specific test
   cargo test test_name -- --exact
   
   # Run ignored tests
   cargo test -- --ignored

**API Key Issues**:

.. code-block:: bash

   # Check environment variables
   env | grep -i api
   
   # Verify config file
   cat ~/.config/perspt/config.json
   
   # Test with explicit config
   echo '{"provider":"openai","api_key":"test"}' | cargo run

**Documentation Build Issues**:

.. code-block:: bash

   # Check Python/uv installation
   uv --version
   
   # Reinstall dependencies
   cd docs/perspt_book
   uv sync
   
   # Clean and rebuild
   uv run make clean && uv run make html

Getting Help
------------

If you encounter issues or need guidance:

1. **Check existing issues** on GitHub
2. **Search the documentation** for similar problems
3. **Ask in discussions** for general questions
4. **Create a detailed issue** for bugs or feature requests
5. **Join the community** chat for real-time help

**When asking for help, include**:

- Your operating system and version
- Rust version (``rustc --version``)
- Perspt version or commit hash
- Full error messages
- Steps to reproduce the issue
- Your configuration (sanitized)

Final Notes
-----------

**Code Quality**:

- Write self-documenting code with clear variable names
- Add comments for complex logic
- Keep functions small and focused
- Use meaningful error messages
- Follow Rust idioms and best practices

**Testing Philosophy**:

- Test behavior, not implementation
- Write tests before fixing bugs (TDD when possible)
- Cover edge cases and error conditions
- Use descriptive test names
- Keep tests fast and reliable

**Documentation Philosophy**:

- Document the "why", not just the "what"
- Keep examples current and working
- Use real-world scenarios in examples
- Cross-reference related concepts
- Update docs with code changes

Ready to contribute? Here's your next steps:

1. **Fork the repository** and set up your environment
2. **Find an issue** to work on or propose a new feature
3. **Read the codebase** to understand the current patterns
4. **Start small** with documentation or simple fixes
5. **Ask questions** early and often
6. **Submit your PR** with tests and documentation

Welcome to the Perspt development community! 🎉

Contributing to Simple CLI Mode
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**NEW in v0.4.6** - The Simple CLI mode offers several areas for contribution:

**Areas for Enhancement**:

1. **Command Extensions**:
   - Additional built-in commands (``/history``, ``/config``, ``/provider``)
   - Command auto-completion
   - Command history navigation

2. **Session Management**:
   - Enhanced logging formats (JSON, CSV, Markdown)
   - Session resumption capabilities
   - Multi-session management

3. **Accessibility Improvements**:
   - Screen reader optimizations
   - High contrast mode support
   - Keyboard navigation enhancements

4. **Scripting Integration**:
   - Better shell integration
   - Environment variable templating
   - Batch processing improvements

**Development Guidelines for CLI Features**:

.. code-block:: rust

   // Example: Adding a new CLI command
   // In src/cli.rs

   async fn process_cli_command(
       command: &str,
       session_log: &mut Option<SessionLogger>,
       app_state: &mut CliAppState, // New state management
   ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
       let parts: Vec<&str> = command.splitn(2, ' ').collect();
       
       match parts[0] {
           // Existing commands...
           
           "/history" => {
               // NEW: Show conversation history
               display_conversation_history(&app_state.conversation_history);
               Ok(true)
           }
           "/config" => {
               // NEW: Show current configuration
               display_current_config(&app_state.config);
               Ok(true)
           }
           "/provider" => {
               // NEW: Switch providers dynamically
               if let Some(provider_name) = parts.get(1) {
                   switch_provider(provider_name, app_state).await?;
                   println!("Switched to provider: {}", provider_name);
               } else {
                   println!("Current provider: {}", app_state.current_provider());
               }
               Ok(true)
           }
           _ => {
               println!("Unknown command: {}. Type /help for available commands.", parts[0]);
               Ok(true)
           }
       }
   }

   // State management for CLI mode
   pub struct CliAppState {
       pub config: AppConfig,
       pub conversation_history: Vec<ConversationEntry>,
       pub current_provider: String,
       pub session_stats: SessionStats,
   }

   impl CliAppState {
       pub fn new(config: AppConfig) -> Self {
           Self {
               current_provider: config.provider_type.clone().unwrap_or_default(),
               config,
               conversation_history: Vec::new(),
               session_stats: SessionStats::new(),
           }
       }

       pub fn add_conversation_entry(&mut self, user_input: String, ai_response: String) {
           self.conversation_history.push(ConversationEntry {
               timestamp: SystemTime::now(),
               user_input,
               ai_response,
           });
           self.session_stats.increment_exchange_count();
       }
   }

**Testing Requirements for CLI Contributions**:

.. code-block:: rust

   // All CLI contributions should include tests
   #[cfg(test)]
   mod cli_contribution_tests {
       use super::*;

       #[test]
       fn test_new_command_parsing() {
           // Test new command syntax
           assert!(matches!(parse_command("/newcommand arg"), Ok(Command::NewCommand(_))));
       }

       #[tokio::test]
       async fn test_new_command_execution() {
           let mut app_state = CliAppState::new(test_config());
           let result = process_cli_command("/newcommand test", &mut None, &mut app_state).await;
           assert!(result.is_ok());
       }

       #[test]
       fn test_accessibility_compliance() {
           // Ensure new features maintain accessibility
           let output = execute_command_with_screen_reader_simulation("/newcommand");
           assert!(is_screen_reader_friendly(&output));
       }
   }

**Code Style for CLI Contributions**:

.. code-block:: rust

   // Follow these patterns for CLI code
   
   // 1. Clear error messages with helpful context
   fn handle_cli_error(error: &dyn std::error::Error) -> String {
       match error.to_string().as_str() {
           msg if msg.contains("network") => {
               "❌ Network error. Check your internet connection and try again.".to_string()
           }
           msg if msg.contains("api key") => {
               "❌ API key error. Verify your API key is set correctly.".to_string()
           }
           _ => format!("❌ Error: {}", error),
       }
   }

   // 2. Consistent prompt and output formatting
   fn display_cli_prompt(provider: &str, model: &str) {
       println!("Perspt Simple CLI Mode");
       println!("Provider: {} | Model: {}", provider, model);
       println!("Type 'exit' or press Ctrl+D to quit.\n");
   }

   // 3. Graceful handling of edge cases
   fn sanitize_cli_input(input: &str) -> Result<String, CliError> {
       let trimmed = input.trim();
       
       if trimmed.is_empty() {
           return Err(CliError::EmptyInput);
       }
       
       if trimmed.len() > MAX_INPUT_LENGTH {
           return Err(CliError::InputTooLong(trimmed.len()));
       }
       
       // Remove potentially problematic characters while preserving meaning
       let sanitized = trimmed
           .chars()
           .filter(|c| c.is_ascii() && (!c.is_control() || *c == '\n' || *c == '\t'))
           .collect::<String>();
       
       Ok(sanitized)
   }

**Documentation Requirements**:

When contributing CLI features, please include:

1. **Inline Documentation**:

   .. code-block:: rust

      /// Processes user commands in Simple CLI mode
      /// 
      /// # Arguments
      /// 
      /// * `command` - The command string starting with '/'
      /// * `session_log` - Optional session logger for recording commands
      /// * `app_state` - Mutable reference to CLI application state
      /// 
      /// # Returns
      /// 
      /// * `Ok(true)` - Continue CLI session
      /// * `Ok(false)` - Exit CLI session
      /// * `Err(e)` - Command processing error
      /// 
      /// # Examples
      /// 
      /// ```rust
      /// let result = process_cli_command("/help", &mut None, &mut app_state).await?;
      /// assert_eq!(result, Ok(true));
      /// ```
      pub async fn process_cli_command(
          command: &str,
          session_log: &mut Option<SessionLogger>,
          app_state: &mut CliAppState,
      ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>

2. **User Documentation**: Update the user guide with new commands and usage examples

3. **Integration Examples**: Provide shell script examples showing how to use new features

**Pull Request Guidelines for CLI Features**:

1. **Feature Description**: Clearly describe the CLI enhancement and its use cases
2. **Backward Compatibility**: Ensure changes don't break existing CLI workflows
3. **Accessibility Testing**: Verify features work with screen readers and accessibility tools
4. **Performance Testing**: Test with large inputs and long sessions
5. **Cross-Platform Testing**: Verify functionality on Windows, macOS, and Linux

**Example Pull Request Template for CLI Features**:

.. code-block:: markdown

   ## CLI Feature: [Feature Name]

   ### Description
   Brief description of the new CLI feature or enhancement.

   ### Use Cases
   - [ ] Scripting and automation workflows
   - [ ] Accessibility improvements
   - [ ] Developer productivity enhancements
   - [ ] Integration with external tools

   ### Testing
   - [ ] Unit tests for new functionality
   - [ ] Integration tests with sample scripts
   - [ ] Accessibility testing with screen reader simulation
   - [ ] Cross-platform compatibility testing

   ### Documentation
   - [ ] Updated inline documentation
   - [ ] Updated user guide with examples
   - [ ] Shell script examples in `/examples/` directory

   ### Backward Compatibility
   - [ ] Existing CLI workflows continue to work
   - [ ] Configuration file compatibility maintained
   - [ ] Command line argument compatibility preserved
