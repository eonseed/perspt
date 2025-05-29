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

      # Install development dependencies
      cargo install cargo-watch cargo-nextest

3. **Verify the setup**:

   .. code-block:: bash

      # Build the project
      cargo build

      # Run tests
      cargo test

      # Check formatting and linting
      cargo fmt --check
      cargo clippy -- -D warnings

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
~~~~~~~~~~~~~~~~~

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

   pub struct MessageProcessor;

   impl MessageProcessor {
       pub fn process(&self, input: &str) -> Result<String, ProcessError> {
           // Implementation
       }
   }

   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn test_basic_processing() {
           let processor = MessageProcessor;
           let result = processor.process("test input");
           assert!(result.is_ok());
       }

       #[tokio::test]
       async fn test_async_processing() {
           // Async test implementation
       }
   }

Integration Tests
~~~~~~~~~~~~~~~~

Place integration tests in the `tests/` directory:

.. code-block:: rust

   // tests/integration_test.rs
   use perspt::config::Config;
   use perspt::Application;

   #[tokio::test]
   async fn test_full_application_flow() {
       let config = Config::test_config();
       let app = Application::new(config).await.unwrap();
       
       let response = app.process_message("Hello").await.unwrap();
       assert!(!response.is_empty());
   }

Mocking and Test Utilities
~~~~~~~~~~~~~~~~~~~~~~~~~~

Use `mockall` for mocking external dependencies:

.. code-block:: rust

   use mockall::{automock, predicate::*};

   #[automock]
   #[async_trait]
   pub trait HttpClient {
       async fn post(&self, url: &str, body: &str) -> Result<String, HttpError>;
   }

   #[cfg(test)]
   mod tests {
       use super::*;

       #[tokio::test]
       async fn test_with_mock() {
           let mut mock_client = MockHttpClient::new();
           mock_client
               .expect_post()
               .with(eq("https://api.example.com"), eq("test"))
               .times(1)
               .returning(|_, _| Ok("response".to_string()));

           // Use mock_client in test
       }
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
~~~~~~~~~~~~~~~~~~~~~~

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

- Documentation improvements
- Small bug fixes
- Code formatting and cleanup
- Test coverage improvements
- Example additions

Feature Development
~~~~~~~~~~~~~~~~~~

Major areas where contributions are welcome:

**New AI Providers**:

.. code-block:: rust

   // Implement the LLMProvider trait for a new provider
   pub struct NewProvider {
       client: reqwest::Client,
       config: NewProviderConfig,
   }

   #[async_trait]
   impl LLMProvider for NewProvider {
       async fn chat_completion(
           &self,
           messages: &[Message],
           options: &ChatOptions,
       ) -> Result<ChatResponse, LLMError> {
           // Implementation
       }
   }

**Plugin System**:

.. code-block:: rust

   // Create new plugins
   pub struct MyPlugin;

   #[async_trait]
   impl Plugin for MyPlugin {
       fn name(&self) -> &str { "my-plugin" }
       
       async fn handle_command(&self, command: &str, args: &[String]) -> Result<PluginResponse, PluginError> {
           // Plugin implementation
       }
   }

**UI Improvements**:

- Better terminal UI components
- Enhanced formatting options
- Accessibility improvements

**Performance Optimizations**:

- Caching improvements
- Memory usage optimizations
- Network request optimizations

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

- **API documentation**: Rust doc comments
- **User guides**: Markdown files in `docs/`
- **Developer guides**: Architecture and contribution docs
- **Examples**: Sample configurations and use cases

Documentation Standards
~~~~~~~~~~~~~~~~~~~~~~

- Use clear, concise language
- Include code examples where appropriate
- Keep examples up-to-date with current API
- Cross-reference related sections

Building Documentation
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Generate Rust documentation
   cargo doc --open

   # Build Sphinx documentation
   cd docs/perspt_book
   pip install -r requirements.txt
   make html

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
~~~~~~~~~~~~~~~~~~~~~

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
~~~~~~~~~~~~~~~~

We follow Semantic Versioning (SemVer):

- **MAJOR**: Breaking changes
- **MINOR**: New features (backward compatible)
- **PATCH**: Bug fixes (backward compatible)

Release Cycle
~~~~~~~~~~~~

- **Major releases**: Every 6-12 months
- **Minor releases**: Every 1-3 months
- **Patch releases**: As needed for critical fixes

Next Steps
----------

Ready to contribute? Here's what to do next:

1. **Find an issue** to work on (check `good first issue` label)
2. **Set up your development environment**
3. **Read the relevant documentation**:
   - :doc:`architecture` - Understand the codebase
   - :doc:`extending` - Learn about plugins and extensions
   - :doc:`testing` - Testing guidelines and best practices
4. **Start coding** and don't hesitate to ask questions!

Thank you for contributing to Perspt! 🚀
