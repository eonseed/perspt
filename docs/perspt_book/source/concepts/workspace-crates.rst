.. _workspace-crates:

Workspace & Crates
==================

Perspt is organized as a Cargo workspace with modular crates for maintainability.

Crate Overview
--------------

.. code-block:: text

   perspt/
   ├── Cargo.toml              # Workspace root
   └── crates/
       ├── perspt-cli/         # Binary entry point
       ├── perspt-core/        # Shared configuration & LLM
       ├── perspt-tui/         # Terminal UI
       ├── perspt-agent/       # SRBN engine
       ├── perspt-policy/      # Security rules
       └── perspt-sandbox/     # Process isolation

perspt-cli
----------

**Purpose**: CLI entry point and mode routing.

**Key files**:

- ``src/main.rs`` - Argument parsing with clap, mode dispatch
- ``src/agent_cli.rs`` - Agent mode CLI handling

**Dependencies**: perspt-core, perspt-tui, perspt-agent

perspt-core
-----------

**Purpose**: Shared configuration and LLM provider abstraction.

**Key files**:

- ``src/config.rs`` - JSON config, env vars, provider detection
- ``src/llm_provider.rs`` - genai crate wrapper, streaming

**Features**:

- Zero-config provider detection from environment
- Model validation before connection
- Streaming response handling

perspt-tui
----------

**Purpose**: Terminal user interface.

**Key files**:

- ``src/app.rs`` - TUI application state
- ``src/ui.rs`` - Ratatui layout and rendering
- ``src/agent_app.rs`` - Agent mode dashboard
- ``src/task_tree.rs`` - Plan visualization

perspt-agent
------------

**Purpose**: SRBN autonomous coding engine.

**Key files**:

- ``src/orchestrator.rs`` - Main SRBN control loop
- ``src/agent.rs`` - Architect and Actuator LLM roles
- ``src/tools.rs`` - File, search, command tools
- ``src/lsp.rs`` - LSP client for Python (ty)
- ``src/test_runner.rs`` - pytest execution
- ``src/ledger.rs`` - Merkle change log
- ``src/types.rs`` - TaskPlan, Energy, RetryPolicy

perspt-policy
-------------

**Purpose**: Security rules and path validation.

**Key files**:

- ``src/lib.rs`` - PathPolicy for workspace restrictions

**Rules**:

- Only allow writes within workspace directory
- Block access to system paths (``/etc``, ``/usr``, etc.)
- Validate command arguments

perspt-sandbox
--------------

**Purpose**: Process isolation (future).

**Planned features**:

- WASM-based tool execution
- Container isolation for untrusted code
- Resource limits (CPU, memory, time)

Adding a New Crate
------------------

1. Create directory: ``mkdir crates/perspt-newcrate``

2. Add to workspace ``Cargo.toml``:

   .. code-block:: toml

      [workspace]
      members = [
          "crates/perspt-cli",
          "crates/perspt-newcrate",  # Add here
          # ...
      ]

3. Create ``crates/perspt-newcrate/Cargo.toml``

4. Add as dependency where needed

Build Commands
--------------

.. code-block:: bash

   # Build all crates
   cargo build --workspace

   # Build specific crate
   cargo build -p perspt-agent

   # Run tests
   cargo test --workspace

   # Check without building
   cargo check --workspace
