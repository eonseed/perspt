.. _workspace-crates:

Workspace Crates
================

Perspt is organized as a **6-crate Cargo workspace** for modularity and maintainability.

Crate Overview
--------------

.. graphviz::
   :align: center
   :caption: Crate Dependency Graph

   digraph workspace {
       rankdir=TB;
       node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=10];
       edge [color="#666666"];
       
       cli [label="perspt-cli\n━━━━━━━━━\nCLI Entry Point\n8 Subcommands", fillcolor="#4ECDC4"];
       core [label="perspt-core\n━━━━━━━━━\nGenAIProvider\nConfig, Memory", fillcolor="#45B7D1"];
       tui [label="perspt-tui\n━━━━━━━━━\nAgentApp\nDashboard", fillcolor="#96CEB4"];
       agent [label="perspt-agent\n━━━━━━━━━\nOrchestrator\nLSP, Tools", fillcolor="#FFEAA7"];
       policy [label="perspt-policy\n━━━━━━━━━\nPolicyEngine\nSanitizer", fillcolor="#DDA0DD"];
       sandbox [label="perspt-sandbox\n━━━━━━━━━\nSandboxedCommand", fillcolor="#F8B739"];
       
       cli -> core;
       cli -> tui;
       cli -> agent;
       agent -> core;
       agent -> policy;
       agent -> sandbox;
   }

Crate Details
-------------

perspt-cli
~~~~~~~~~~

**Purpose**: Command-line interface entry point

**Location**: ``crates/perspt-cli/``

**Key Components**:

- ``main.rs`` — CLI argument parsing with clap
- ``commands/`` — Subcommand implementations

**Subcommands**:

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Command
     - Function
   * - ``chat``
     - Launch interactive TUI
   * - ``agent``
     - Run SRBN orchestrator
   * - ``init``
     - Initialize project config
   * - ``config``
     - Manage configuration
   * - ``ledger``
     - Query Merkle ledger
   * - ``status``
     - Show agent status
   * - ``abort``
     - Cancel current session
   * - ``resume``
     - Resume interrupted session

perspt-core
~~~~~~~~~~~

**Purpose**: Core abstractions for LLM interaction

**Location**: ``crates/perspt-core/``

**Key Components**:

- ``config.rs`` — Simple Config struct
- ``llm_provider.rs`` — Thread-safe GenAIProvider
- ``memory.rs`` — Conversation memory

**Thread Safety**: GenAIProvider uses ``Arc<RwLock<SharedState>>`` for concurrent access.

perspt-agent
~~~~~~~~~~~~

**Purpose**: SRBN autonomous coding engine

**Location**: ``crates/perspt-agent/``

**Key Components**:

.. list-table::
   :header-rows: 1
   :widths: 25 10 65

   * - Module
     - Size
     - Description
   * - ``orchestrator.rs``
     - 34KB
     - SRBN control loop, model tiers
   * - ``lsp.rs``
     - 28KB
     - LSP client for Python (``ty``)
   * - ``tools.rs``
     - 12KB
     - Agent tools (read, write, search, shell)
   * - ``types.rs``
     - 24KB
     - TaskPlan, Node, Energy types
   * - ``ledger.rs``
     - 6KB
     - Merkle change tracking
   * - ``test_runner.rs``
     - 15KB
     - pytest integration
   * - ``context_retriever.rs``
     - 10KB
     - Code context extraction

perspt-tui
~~~~~~~~~~

**Purpose**: Terminal UI components

**Location**: ``crates/perspt-tui/``

**Key Components**:

- ``agent_app.rs`` — Main agent TUI
- ``dashboard.rs`` — Status metrics
- ``diff_viewer.rs`` — File diff display
- ``review_modal.rs`` — Change approval
- ``task_tree.rs`` — Task hierarchy

perspt-policy
~~~~~~~~~~~~~

**Purpose**: Security policy enforcement

**Location**: ``crates/perspt-policy/``

**Key Components**:

- ``engine.rs`` — Starlark policy evaluator
- ``sanitize.rs`` — Command sanitization

perspt-sandbox
~~~~~~~~~~~~~~

**Purpose**: Process isolation

**Location**: ``crates/perspt-sandbox/``

**Key Component**: ``command.rs`` — Sandboxed command execution with resource limits

Building Individual Crates
--------------------------

.. code-block:: bash

   # Build specific crate
   cargo build -p perspt-cli
   cargo build -p perspt-agent

   # Run tests for crate
   cargo test -p perspt-core

   # Generate docs for crate
   cargo doc -p perspt-agent --open

Adding a New Crate
------------------

1. Create directory: ``crates/perspt-newcrate/``
2. Add ``Cargo.toml`` with package metadata
3. Register in root ``Cargo.toml``:

   .. code-block:: toml

      [workspace]
      members = [
          "crates/perspt-core",
          "crates/perspt-newcrate",  # Add here
          ...
      ]

4. Add dependencies to consuming crates

See Also
--------

- :doc:`../developer-guide/architecture` - Architecture overview
- :doc:`../developer-guide/extending` - Extension guide
- :doc:`../api/index` - Per-crate API reference
