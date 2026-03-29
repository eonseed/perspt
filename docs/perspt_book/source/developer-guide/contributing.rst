.. _developer-guide-contributing:

Contributing
============

Development Setup
-----------------

.. code-block:: bash

   # Clone
   git clone https://github.com/eonseed/perspt.git
   cd perspt

   # Build
   cargo build

   # Run tests
   cargo test

   # Lint
   cargo clippy -- -D warnings

   # Format check
   cargo fmt -- --check


Project Structure
-----------------

.. code-block:: text

   crates/
     perspt-core/     # Types, config, LLM, events, plugins
     perspt-agent/    # Orchestrator, agents, ledger, tools
     perspt-tui/      # Chat + Agent TUI
     perspt-cli/      # CLI entry (clap)
     perspt-store/    # DuckDB persistence
     perspt-policy/   # Starlark policies
     perspt-sandbox/  # Command sandboxing
   tests/             # Integration tests
   docs/              # Sphinx documentation


Coding Standards
----------------

1. **Clippy clean** — ``cargo clippy -- -D warnings`` must pass
2. **Formatted** — ``cargo fmt`` with default settings
3. **Tests pass** — ``cargo test`` must pass all tests
4. **No ``println!`` in UI paths** — Use the event system or ``env_logger``
5. **Error types** — Use ``ErrorType`` enum for categorized errors
6. **Streaming safety** — Never block the UI select loop; spawn on tokio tasks

Commit Messages
---------------

- Describe what changed, not the sequence
- Do NOT include phase numbers or commit sequence numbers
- Keep the subject line under 72 characters

.. code-block:: text

   # Good
   Add sheaf validation for cross-language boundaries

   # Bad
   Commit 3/7: Phase 2 - Add sheaf validation


PR Workflow
-----------

1. Create a feature branch from ``main``
2. Make changes with passing tests
3. Run the full check suite:

   .. code-block:: bash

      cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt -- --check

4. Push and open a PR
5. Address review feedback


Documentation
-------------

Documentation uses Sphinx with reStructuredText:

.. code-block:: bash

   # Build HTML docs
   cd docs/perspt_book && uv run make html

   # Build PDF docs
   cd docs/perspt_book && uv run make latexpdf

   # Live preview
   cd docs/perspt_book && uv run sphinx-autobuild source build/html

See the ``Generate Documentation`` and ``Build Sphinx HTML Documentation`` VS Code
tasks for convenience.
