.. _workspace-crates:

Workspace Crates
================

Perspt is organized as a **7-crate Rust workspace** under the ``crates/`` directory,
plus a meta-crate (``perspt``) that re-exports all libraries.

.. graphviz::
   :align: center
   :caption: Crate Dependencies

   digraph deps {
       rankdir=TB;
       node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=10];

       cli [label="perspt-cli\n10 commands", fillcolor="#4ECDC4"];
       tui [label="perspt-tui\nRatatui UI", fillcolor="#96CEB4"];
       agent [label="perspt-agent\nSRBN Engine", fillcolor="#FFEAA7"];
       core [label="perspt-core\nLLM + Types", fillcolor="#45B7D1"];
       store [label="perspt-store\nDuckDB", fillcolor="#87CEEB"];
       policy [label="perspt-policy\nStarlark", fillcolor="#DDA0DD"];
       sandbox [label="perspt-sandbox\nIsolation", fillcolor="#F8B739"];

       cli -> tui;
       cli -> agent;
       cli -> core;
       cli -> store;
       agent -> core;
       agent -> store;
       agent -> policy;
       agent -> sandbox;
   }

Crate Summary
-------------

.. list-table::
   :header-rows: 1
   :widths: 18 42 40

   * - Crate
     - Purpose
     - Key Types
   * - **perspt-cli**
     - Binary entry point with 10 subcommands
     - ``Cli``, ``Commands``
   * - **perspt-core**
     - LLM provider abstraction, config, types, events, plugins
     - ``GenAIProvider``, ``Config``, ``SRBNNode``, ``AgentContext``, ``AgentEvent``
   * - **perspt-agent**
     - SRBN orchestrator, agents, LSP client, tools, test runner, ledger
     - ``SRBNOrchestrator``, ``Agent`` trait, ``LspClient``, ``AgentTools``, ``MerkleLedger``
   * - **perspt-tui**
     - Ratatui-based terminal UI for chat and agent monitoring
     - ``ChatApp``, ``AgentApp``, ``DiffViewer``, ``ReviewModal``, ``TaskTree``
   * - **perspt-store**
     - DuckDB persistence for sessions, energy, LLM logs
     - ``SessionStore``, ``EnergyRecord``, ``LlmRequestRecord``
   * - **perspt-policy**
     - Starlark-based policy engine and command sanitization
     - ``PolicyEngine``, ``sanitize_command``, ``SanitizeResult``
   * - **perspt-sandbox**
     - Sandboxed command execution with resource limits
     - ``SandboxedCommand``, ``CommandResult``

perspt-cli
~~~~~~~~~~

The CLI crate parses arguments with ``clap`` and dispatches to the appropriate
handler. Subcommands: ``chat``, ``agent``, ``simple-chat``, ``init``, ``config``,
``ledger``, ``status``, ``abort``, ``resume``, ``logs``.

perspt-core
~~~~~~~~~~~

Contains all shared types used across the workspace:

- **Types**: ``SRBNNode``, ``NodeState``, ``NodeClass``, ``ModelTier``, ``TaskPlan``,
  ``BehavioralContract``, ``StabilityMonitor``, ``EnergyComponents``, ``AgentContext``,
  ``TokenBudget``, ``RetryPolicy``, ``OwnershipManifest``, ``PlanningPolicy``,
  ``FeatureCharter``, ``BudgetEnvelope``, ``RepairFootprint``
- **Events**: ``AgentEvent`` enum with 40+ lifecycle event variants (PSP-5)
- **Plugins**: Language plugin registry (Rust, Python, JS, Go) with LSP config,
  test runner, init commands, and required binaries
- **Config**: ``Config`` struct with provider, model, and API key
- **LLM Provider**: ``GenAIProvider`` — thread-safe wrapper around the ``genai``
  crate with streaming support and retry logic

perspt-agent
~~~~~~~~~~~~

The heart of SRBN:

- **Orchestrator**: ``SRBNOrchestrator`` manages the DAG (``petgraph``), drives the
  7-phase control loop, manages LSP clients, and integrates TUI events. Split into
  submodules: ``mod.rs``, ``bundle.rs``, ``commit.rs``, ``convergence.rs``,
  ``init.rs``, ``planning.rs``, ``repair.rs``, ``solo.rs``, ``verification.rs``
- **Agents**: ``Agent`` trait with four implementations — ``ArchitectAgent``,
  ``ActuatorAgent``, ``VerifierAgent``, ``SpeculatorAgent``
- **Tools**: ``AgentTools`` — ``read_file``, ``write_file``, ``apply_patch``,
  ``apply_diff``, ``run_command``, ``search_code``, ``list_files``,
  ``sed_replace``, ``awk_filter``, ``diff_files``
- **LSP Client**: Native LSP client supporting ``rust-analyzer``, ``ty``,
  ``pyright``, ``typescript-language-server``, ``gopls``
- **Test Runner**: ``PythonTestRunner`` (pytest) with V_log calculation
- **Ledger**: ``MerkleLedger`` backed by ``perspt-store``
- **Context Retriever**: ``ripgrep``-based code search for context injection
- **Prompts**: Centralized prompt templates in ``prompts.rs`` with constants and
  ``render_*`` helper functions for all agent tiers

perspt-tui
~~~~~~~~~~

Ratatui components:

- **ChatApp** — Interactive chat with markdown rendering and virtual scrolling
- **AgentApp** — SRBN monitoring with dashboard, task tree, and diff viewer
- **ReviewModal** — Approval UI with grouped diff view and verification gates
- **LogsViewer** — LLM request/response log inspector
- **Dashboard** — Status display with energy breakdown
- **TaskTree** — Node execution tree with state indicators

perspt-store
~~~~~~~~~~~~

DuckDB-based persistence layer. Tables include sessions, node states, energy
records, verification results, LLM request logs, provisional branches, interface
seals, artifact bundles, escalation reports, and sheaf validations.

perspt-policy
~~~~~~~~~~~~~

Starlark-based execution policy:

- ``sanitize_command()`` validates commands before execution
- ``validate_workspace_bound()`` ensures file operations stay within the project
- ``is_safe_for_auto_exec()`` checks if a command can be auto-approved

perspt-sandbox
~~~~~~~~~~~~~~

Sandboxed command execution with resource limits. Used for running build and
test commands in controlled environments during provisional branch verification.
