Acknowledgments
===============

Perspt is built on the shoulders of giants. We extend our gratitude to the many open-source projects, libraries, and communities that made this project possible.

Core Dependencies
-----------------

AI and LLM Integration
~~~~~~~~~~~~~~~~~~~~~~

**allms**
  The foundation of Perspt's multi-provider support. This exceptional crate provides unified interfaces to multiple AI providers and automatically stays up-to-date with new models and capabilities.
  
  * **Project**: `allms <https://crates.io/crates/allms>`_
  * **License**: MIT/Apache 2.0
  * **Impact**: Enables seamless integration with OpenAI, Anthropic, Google, Mistral, and other providers

**serde & serde_json**
  Rust's premier serialization framework, powering Perspt's configuration management and API communication.
  
  * **Project**: `serde <https://serde.rs/>`_
  * **License**: MIT/Apache 2.0
  * **Impact**: JSON configuration parsing, API request/response handling

User Interface and Terminal
~~~~~~~~~~~~~~~~~~~~~~~~~~~

**ratatui**
  The modern, feature-rich TUI framework that powers Perspt's interactive terminal interface.
  
  * **Project**: `ratatui <https://ratatui.rs/>`_
  * **License**: MIT
  * **Impact**: Rich terminal UI, markdown rendering, scrollable chat interface

**crossterm**
  Cross-platform terminal manipulation library enabling consistent behavior across operating systems.
  
  * **Project**: `crossterm <https://github.com/crossterm-rs/crossterm>`_
  * **License**: MIT
  * **Impact**: Keyboard input handling, terminal control, cross-platform compatibility

Async Runtime and Concurrency
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**tokio**
  The asynchronous runtime that enables Perspt's responsive, non-blocking architecture.
  
  * **Project**: `tokio <https://tokio.rs/>`_
  * **License**: MIT
  * **Impact**: Async/await support, concurrent LLM requests, responsive UI

Error Handling and Utilities
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**anyhow**
  Elegant error handling that makes Perspt's error messages helpful and actionable.
  
  * **Project**: `anyhow <https://github.com/dtolnay/anyhow>`_
  * **License**: MIT/Apache 2.0
  * **Impact**: Comprehensive error context, user-friendly error messages

**clap**
  Command-line argument parsing that makes Perspt easy to use and configure.
  
  * **Project**: `clap <https://clap.rs/>`_
  * **License**: MIT/Apache 2.0
  * **Impact**: CLI interface, help generation, argument validation

Documentation Tools
--------------------

**Sphinx**
  The documentation generator that created this beautiful book-style documentation.
  
  * **Project**: `Sphinx <https://www.sphinx-doc.org/>`_
  * **License**: BSD
  * **Impact**: Professional documentation, PDF generation, cross-references

**Furo Theme**
  The modern, accessible Sphinx theme that makes this documentation a pleasure to read.
  
  * **Project**: `Furo <https://pradyunsg.me/furo/>`_
  * **License**: MIT
  * **Impact**: Beautiful documentation design, responsive layout, accessibility

Development Tools
-----------------

**Rust Language**
  The systems programming language that makes Perspt fast, safe, and reliable.
  
  * **Project**: `Rust <https://www.rust-lang.org/>`_
  * **License**: MIT/Apache 2.0
  * **Impact**: Memory safety, performance, excellent tooling ecosystem

**cargo**
  Rust's package manager and build system that makes development smooth and dependency management effortless.
  
  * **Project**: Part of Rust toolchain
  * **License**: MIT/Apache 2.0
  * **Impact**: Dependency management, build automation, testing framework

Community and Inspiration
--------------------------

AI Provider Communities
~~~~~~~~~~~~~~~~~~~~~~~

**OpenAI**
  For creating GPT models and establishing many of the patterns that define modern AI interaction.

**Anthropic**
  For Claude models and their pioneering work in AI safety and helpful, harmless, and honest AI systems.

**Google**
  For Gemini models and their contributions to accessible AI technology.

**Mistral AI**
  For their excellent open-source and commercial models.

**Perplexity AI**
  For innovative approaches to AI-powered search and information retrieval.

**DeepSeek**
  For their contributions to the open-source AI ecosystem.

Open Source Ecosystem
~~~~~~~~~~~~~~~~~~~~~~

**GitHub**
  For providing the platform that enables collaborative development and open-source sharing.

**crates.io**
  Rust's package registry that makes sharing and discovering Rust libraries effortless.

**docs.rs**
  For automatically generating and hosting documentation for Rust crates.

Terminal and CLI Inspiration
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The terminal and CLI interface draws inspiration from many excellent tools:

* **htop** - For showing how terminal UIs can be both beautiful and functional
* **tmux** - For terminal multiplexing concepts and keyboard navigation patterns
* **vim/neovim** - For modal editing concepts and efficient keyboard shortcuts
* **fzf** - For demonstrating responsive, interactive terminal interfaces

Rust Community Projects
~~~~~~~~~~~~~~~~~~~~~~~~

Many patterns and approaches in Perspt were learned from studying excellent Rust projects:

* **ripgrep** - For performance optimization and user experience design
* **bat** - For beautiful terminal output and syntax highlighting
* **exa/eza** - For modern CLI design and colored output
* **gitui** - For TUI application architecture and event handling

Testing and Quality Assurance
------------------------------

**Users and Beta Testers**
  The early adopters and users who provided feedback, reported bugs, and suggested improvements.

**Security Researchers**
  For responsible disclosure of security issues and helping make Perspt more secure.

**Documentation Reviewers**
  For helping improve the clarity and completeness of this documentation.

Special Thanks
--------------

**AI Safety Research Community**
  For ongoing work to make AI systems more reliable, interpretable, and aligned with human values.

**Open Source Contributors**
  To everyone who contributes to open-source projects, from major features to documentation fixes.

**Rust Community**
  For creating and maintaining an inclusive, helpful community that makes Rust development a joy.

**Terminal Enthusiasts**
  For keeping the art of terminal-based applications alive and pushing the boundaries of what's possible in text-based interfaces.

Contributing Back
-----------------

Perspt aims to be a good citizen of the open-source ecosystem. We contribute back by:

**Open Source Release**
  Perspt itself is released under the MIT license, allowing anyone to use, modify, and distribute it.

**Documentation Standards**
  This comprehensive documentation serves as an example of thorough project documentation.

**Best Practices Sharing**
  Through blog posts, talks, and code examples, we share what we've learned building Perspt.

**Upstream Contributions**
  When we find bugs or missing features in dependencies, we contribute fixes and improvements back to those projects.

License Information
-------------------

Perspt is licensed under the MIT License. For complete license information, see :doc:`license`.

All dependencies are used in accordance with their respective licenses. We are grateful to all the authors and maintainers who choose to share their work under permissive open-source licenses.

Get Involved
------------

Want to contribute to Perspt or the broader ecosystem?

**Report Issues**
  Help improve Perspt by reporting bugs, suggesting features, or improving documentation.

**Contribute Code**
  See our :doc:`developer-guide/contributing` guide for how to contribute code improvements.

**Share Knowledge**
  Write blog posts, create tutorials, or give talks about your experience with Perspt.

**Support Dependencies**
  Consider contributing to the open-source projects that Perspt depends on.

**Spread the Word**
  Help others discover Perspt and the amazing ecosystem of Rust and AI tools.

---

*Thank you to everyone who makes open-source software development possible. Your contributions, large and small, make projects like Perspt possible.*
