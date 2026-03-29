.. _changelog:

Changelog
=========

Version 0.5.4
-------------

*PSP-5 Compliance Release*

**SRBN Agent Enhancements:**

- **Per-node error recovery** — Retry logic respects ``ErrorType`` classification
  (``Compilation``, ``ToolFailure``, ``ReviewRejection``) with separate counters
- **Multi-file extraction** — Actuator reliably extracts artifact bundles with
  multiple files from LLM responses
- **Multi-artifact bundles** — Bundle protocol correctly handles write, diff, and
  command artifacts in a single node
- **Plugin-driven project initialization** — All plugins use ``uv init --lib``
  (Python), ``cargo init`` (Rust), ``npm init`` (JS) for proper project scaffolding
- **Degraded verification mode** — When tool binaries are missing, falls back to
  heuristic verification with clear warnings
- **Sheaf validation** — 7 validator classes for cross-node contract verification

**Bug Fixes:**

- Fixed ``uv init`` to ``uv init --lib`` in ``plugin.rs`` (2 locations) and
  ``test_runner.rs`` (1 location) for correct ``src/`` layout with ``[build-system]``
- Removed dead ``test_check_workspace_requirement`` test from ``orchestrator.rs``

**Documentation:**

- Complete rewrite of the Perspt Book for PSP-5 accuracy
- Updated developer guide with full type inventory and architecture diagrams
- Added tutorials for headless mode, Python ETL, Rust CLI, and scientific computing
- Updated all model names and provider defaults to current versions


Version 0.5.3
-------------

- SRBN energy convergence improvements
- Provisional branch lifecycle management
- Interface seal and flush cascade support
- DuckDB migration from SQLite


Version 0.5.2
-------------

- Initial SRBN orchestrator implementation
- Per-tier model selection (``--architect-model``, etc.)
- Merkle ledger with DuckDB backend
- Plugin system for Python, Rust, and JavaScript
- Agent TUI with dashboard, task tree, and review modal
- Starlark policy engine
- Basic sandbox command execution
- 10 CLI subcommands (chat, agent, simple-chat, init, config, ledger, status,
  abort, resume, logs)


Version 0.5.1
-------------

- Multi-provider chat support
- TUI markdown rendering with code blocks
- Simple CLI mode for scripting
- Streaming response protocol with EOT signal


Version 0.5.0
-------------

- Initial release
- Basic chat interface with OpenAI
- Configuration auto-detection from environment
