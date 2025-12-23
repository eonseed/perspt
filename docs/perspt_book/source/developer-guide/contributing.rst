.. _developer-guide-contributing:

Contributing
============

How to contribute to Perspt.

Getting Started
---------------

1. **Fork** the repository on GitHub
2. **Clone** your fork:

   .. code-block:: bash

      git clone https://github.com/YOUR_USERNAME/perspt.git
      cd perspt

3. **Create a branch**:

   .. code-block:: bash

      git checkout -b feat/your-feature

Development Setup
-----------------

.. code-block:: bash

   # Install Rust (if needed)
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

   # Build
   cargo build

   # Run tests
   cargo test

   # Run with debug
   cargo run -- chat

Code Style
----------

- **Rust**: Follow ``rustfmt`` and ``clippy``
- **Commit messages**: Conventional Commits format
- **Documentation**: Update docs for API changes

.. code-block:: bash

   # Format code
   cargo fmt

   # Run linter
   cargo clippy --all-targets

Commit Messages
---------------

Use `Conventional Commits <https://www.conventionalcommits.org/>`_:

.. code-block:: text

   feat: add new agent tool
   fix: correct energy calculation
   docs: update SRBN documentation
   refactor: simplify orchestrator
   test: add integration tests
   chore: update dependencies

Pull Request Process
--------------------

1. Ensure all tests pass:

   .. code-block:: bash

      cargo test --all

2. Update documentation if needed
3. Create a PR with clear description
4. Address review feedback
5. Squash and merge when approved

PSP Process
-----------

For significant changes, create a PSP (Perspt Specification Proposal):

1. Get PSP number from maintainers
2. Create ``docs/psps/source/psp-00000N.rst``
3. Submit for review
4. Implement after acceptance

See :doc:`../concepts/psp-process` for details.

Crate Structure
---------------

.. code-block:: text

   crates/
   ├── perspt-cli/     # CLI entry point
   ├── perspt-core/    # LLM provider
   ├── perspt-tui/     # Terminal UI
   ├── perspt-agent/   # SRBN engine
   ├── perspt-policy/  # Security
   └── perspt-sandbox/ # Isolation

When contributing, add to the appropriate crate.

See Also
--------

- :doc:`architecture` - Crate design
- :doc:`testing` - Testing guide
- :doc:`../concepts/psp-process` - PSP process
