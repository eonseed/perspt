Changelog
=========

All notable changes to Perspt will be documented in this file.

The format is based on `Keep a Changelog <https://keepachangelog.com/en/1.0.0/>`_,
and this project adheres to `Semantic Versioning <https://semver.org/spec/v2.0.0.html>`_.

.. contents:: Versions
   :local:
   :depth: 1

[Unreleased]
------------

### Added
- Enhanced documentation with Sphinx
- Comprehensive API reference
- Developer guide for contributors

### Changed
- Improved error messages for better user experience
- Optimized memory usage for large conversations

### Fixed
- Fixed terminal cleanup on panic
- Resolved configuration file parsing edge cases

[0.4.0] - 2025-05-29
--------------------

### Added
- **Multi-provider support**: OpenAI, Anthropic, Google, AWS Bedrock, and more
- **Dynamic model discovery**: Automatic detection of available models
- **Input queuing**: Type new messages while AI is responding
- **Markdown rendering**: Rich text formatting in terminal
- **Streaming responses**: Real-time display of AI responses
- **Comprehensive configuration**: JSON files and environment variables
- **Beautiful terminal UI**: Powered by Ratatui with modern design
- **Graceful error handling**: User-friendly error messages and recovery

### Technical Highlights
- Built with Rust for maximum performance and safety
- Leverages `allms` crate for unified LLM access
- Async/await architecture with Tokio
- Comprehensive test suite with unit and integration tests
- Memory-safe with zero-copy operations where possible

### Supported Providers
- **OpenAI**: GPT-4, GPT-4-turbo, GPT-4o series, GPT-3.5-turbo
- **AWS Bedrock**: Amazon Nova models and more
- **Anthropic**: Claude 3 models (via allms)
- **Google**: Gemini models (via allms)
- **Mistral**: Mistral AI models (via allms)
- **Others**: Perplexity, DeepSeek, and more

### Configuration Features
- Multiple configuration file locations
- Environment variable support
- Command-line argument overrides
- Provider-specific settings
- UI customization options

### User Interface Features
- Real-time chat interface
- Syntax highlighting for code blocks
- Scrollable message history
- Keyboard shortcuts for productivity
- Status indicators and progress feedback
- Responsive design that adapts to terminal size

[0.3.0] - 2025-05-15
--------------------

### Added
- Initial AWS Bedrock support
- Configuration file validation
- Improved error categorization

### Changed
- Refactored provider architecture for extensibility
- Enhanced UI responsiveness
- Better handling of long responses

### Fixed
- Terminal state cleanup on unexpected exit
- Configuration merging precedence
- Memory leaks in streaming responses

[0.2.0] - 2025-05-01
--------------------

### Added
- Streaming response support
- Basic configuration file support
- Terminal UI with Ratatui
- OpenAI provider implementation

### Changed
- Migrated from simple CLI to TUI interface
- Improved async architecture
- Better error handling patterns

### Fixed
- Terminal rendering issues
- API request timeout handling
- Configuration loading edge cases

[0.1.0] - 2025-04-15
--------------------

### Added
- Initial release
- Basic OpenAI integration
- Simple command-line interface
- Environment variable configuration
- Basic chat functionality

### Features
- Support for GPT-3.5 and GPT-4 models
- API key authentication
- Simple text-based conversations
- Basic error handling

Migration Guides
----------------

Upgrading from 0.3.x to 0.4.0
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**Configuration Changes:**

The configuration format has been enhanced. Old configurations will continue to work, but consider updating:

.. code-block:: json

   // Old format (still supported)
   {
     "api_key": "sk-...",
     "model": "gpt-4"
   }

   // New format (recommended)
   {
     "api_key": "sk-...",
     "default_model": "gpt-4o-mini",
     "provider_type": "openai",
     "providers": {
       "openai": "https://api.openai.com/v1"
     }
   }

**Command Line Changes:**

Some command-line flags have been updated:

.. code-block:: bash

   # Old
   perspt --model gpt-4

   # New
   perspt --model-name gpt-4

**API Changes:**

If you're using Perspt as a library, some function signatures have changed:

.. code-block:: rust

   // Old
   provider.send_request(message, model).await?;

   // New
   provider.send_chat_request(message, model, &config, &tx).await?;

Upgrading from 0.2.x to 0.3.0
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**New Dependencies:**

Update your `Cargo.toml` if building from source:

.. code-block:: toml

   [dependencies]
   tokio = { version = "1.0", features = ["full"] }
   # ... other dependencies updated

**Configuration Location:**

Configuration files now support multiple locations. Move your config file to:

- `~/.config/perspt/config.json` (Linux)
- `~/Library/Application Support/perspt/config.json` (macOS)
- `%APPDATA%/perspt/config.json` (Windows)

Breaking Changes
----------------

Version 0.4.0
~~~~~~~~~~~~~

- **Provider trait changes**: `LLMProvider` trait now requires `async fn` methods
- **Configuration structure**: Some configuration keys renamed for consistency
- **Error types**: Custom error types replace generic error handling
- **Streaming interface**: Response handling now uses channels instead of callbacks

Version 0.3.0
~~~~~~~~~~~~~

- **Async runtime**: Switched to full async architecture
- **UI framework**: Migrated from custom rendering to Ratatui
- **Configuration format**: Enhanced JSON schema with validation

Version 0.2.0
~~~~~~~~~~~~~

- **Interface change**: Moved from CLI to TUI
- **Provider abstraction**: Introduced provider trait system
- **Async support**: Added Tokio async runtime

Deprecation Notices
-------------------

The following features are deprecated and will be removed in future versions:

Version 0.5.0 (Upcoming)
~~~~~~~~~~~~~~~~~~~~~~~~

- **Legacy configuration keys**: Old configuration format support will be removed
- **Synchronous API**: All provider methods must be async
- **Direct model specification**: Use provider + model pattern instead

Version 0.6.0 (Planned)
~~~~~~~~~~~~~~~~~~~~~~~

- **Environment variable precedence**: Will change to match command-line precedence
- **Default provider**: Will change from OpenAI to provider-agnostic selection

Known Issues
------------

Current Version (0.4.0)
~~~~~~~~~~~~~~~~~~~~~~~

- **Windows terminal compatibility**: Some Unicode characters may not display correctly on older Windows terminals
- **AWS Bedrock regions**: Limited model availability in some AWS regions
- **Large conversation history**: Memory usage increases with very long conversations (>1000 messages)
- **Network interruption**: Streaming responses may be interrupted during network issues

Workarounds:

.. code-block:: bash

   # For Windows terminal issues
   # Use Windows Terminal or enable UTF-8 support

   # For memory issues with large histories
   perspt --max-history 500

   # For network issues
   perspt --timeout 60 --max-retries 5

Planned Features
----------------

Version 0.5.0 (Next Release)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

- **Local model support**: Integration with Ollama and other local LLM servers
- **Plugin system**: Support for custom providers and UI extensions
- **Conversation persistence**: Save and restore chat sessions
- **Multi-conversation support**: Multiple chat tabs in single session
- **Enhanced markdown**: Tables, math equations, and diagrams
- **Voice input**: Speech-to-text support for hands-free operation

Version 0.6.0 (Future)
~~~~~~~~~~~~~~~~~~~~~~

- **Collaborative features**: Share conversations and collaborate with others
- **IDE integration**: VS Code extension and other editor plugins
- **Mobile companion**: Mobile app for conversation sync
- **Advanced AI features**: Function calling, tool use, and agent capabilities
- **Performance analytics**: Response time tracking and optimization suggestions

Version 1.0.0 (Stable Release)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

- **API stability guarantee**: Stable public API with semantic versioning
- **Enterprise features**: SSO, audit logging, and compliance features
- **Advanced customization**: Themes, layouts, and workflow customization
- **Comprehensive integrations**: GitHub, Slack, Discord, and more
- **Professional support**: Documentation, training, and enterprise support

Contributing
------------

We welcome contributions! Please see our :doc:`developer-guide/contributing` for guidelines.

**Types of contributions:**
- Bug reports and feature requests
- Code contributions and optimizations
- Documentation improvements
- Testing and quality assurance
- Community support and advocacy

**How to contribute:**

1. Check existing issues and discussions
2. Fork the repository
3. Create a feature branch
4. Make your changes with tests
5. Submit a pull request

Support
-------

- **GitHub Issues**: `Bug Reports <https://github.com/yourusername/perspt/issues>`_
- **Discussions**: `Community Chat <https://github.com/yourusername/perspt/discussions>`_
- **Documentation**: This guide and API reference
- **Email**: support@perspt.dev (for enterprise inquiries)

License
-------

Perspt is released under the MIT License. See :doc:`license` for details.

Acknowledgments
---------------

Special thanks to:

- The Rust community for excellent tooling and libraries
- Ratatui developers for the amazing TUI framework
- allms crate maintainers for unified LLM access
- All contributors and users who help improve Perspt

.. seealso::

   - :doc:`installation` - How to install or upgrade Perspt
   - :doc:`getting-started` - Quick start guide for new users
   - :doc:`developer-guide/contributing` - How to contribute to the project
