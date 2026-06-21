.. _workspace-crates:

Workspace Crates
================

Perspt is organized as a **twelve-crate Rust workspace** under the ``crates/``
directory. Eight crates make up the running program: they read your commands,
draw the interface, call the language model, run the agent, store the history,
and keep the work safe. Three more crates - ``perspt-sdk``, ``perspt-coding``,
and ``perspt-research`` - form a reusable platform layer that a new field of
work can build on. The last crate, ``perspt``, is a meta-crate that re-exports
the libraries so a single dependency pulls in the whole workspace.

.. graphviz::
   :align: center
   :caption: Crate Dependencies

   digraph deps {
       rankdir=TB;
       node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=10];

       cli [label="perspt-cli\n11 commands", fillcolor="#4ECDC4"];
       tui [label="perspt-tui\nRatatui UI", fillcolor="#96CEB4"];
       agent [label="perspt-agent\nSRBN Engine", fillcolor="#FFEAA7"];
       core [label="perspt-core\nLLM + Types", fillcolor="#45B7D1"];
       store [label="perspt-store\nDuckDB", fillcolor="#87CEEB"];
       policy [label="perspt-policy\nStarlark", fillcolor="#DDA0DD"];
       sandbox [label="perspt-sandbox\nIsolation", fillcolor="#F8B739"];
       dashboard [label="perspt-dashboard\nWeb UI", fillcolor="#FFB6C1"];

       subgraph cluster_sdk {
           label="Reusable SDK Platform";
           style=dashed;
           sdk [label="perspt-sdk\nStability Contract", fillcolor="#AED9E0"];
           coding [label="perspt-coding\nCoding Domain", fillcolor="#C7CEEA"];
           research [label="perspt-research\nResearch Domain", fillcolor="#E2C2C6"];
       }

       cli -> tui;
       cli -> agent;
       cli -> core;
       cli -> store;
       cli -> dashboard;
       agent -> core;
       agent -> store;
       agent -> policy;
       agent -> sandbox;
       agent -> sdk [style=dotted, label="PSP-8"];
       coding -> sdk;
       research -> sdk;
       dashboard -> store;
       dashboard -> core;
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
     - Binary entry point with 11 subcommands
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
   * - **perspt-dashboard**
     - Browser-based agent monitoring (Axum + Askama + HTMX)
     - ``build_router``, ``AppState``, SSE stream
   * - **perspt-sdk**
     - Domain-neutral SRBN platform: residuals, energy, acceptance gate
     - ``ResidualEvent``, ``EnergyModel``, ``AgentDomainPackage``, ``ResidualCertificate``
   * - **perspt-coding**
     - First domain package: coding residual schema and correction directions
     - ``CodingDomain``, ``CodingLanguage``
   * - **perspt-research**
     - Second domain skeleton proving the SDK admits a new field of work
     - ``ResearchDomain``

perspt-cli
~~~~~~~~~~

The CLI crate parses arguments with ``clap`` and dispatches to the appropriate
handler. Subcommands: ``chat``, ``agent``, ``simple-chat``, ``init``, ``config``,
``ledger``, ``status``, ``abort``, ``resume``, ``logs``, ``dashboard``.

perspt-core
~~~~~~~~~~~

Contains all shared types used across the workspace:

- **Types**: ``SRBNNode``, ``NodeState``, ``NodeClass``, ``ModelTier``, ``TaskPlan``,
  ``BehavioralContract``, ``StabilityMonitor``, ``EnergyComponents``, ``AgentContext``,
  ``TokenBudget``, ``RetryPolicy``, ``OwnershipManifest``, ``PlanningPolicy``,
  ``FeatureCharter``, ``BudgetEnvelope``, ``RepairFootprint``, ``SessionOutcome``,
  ``NodeOutcome``
- **Events**: ``AgentEvent`` enum with 40+ lifecycle event variants (PSP-5)
- **Plugins**: Language plugin registry (Rust, Python, JS, Go) with LSP config,
  test runner, init commands, and required binaries
- **Config**: ``Config`` struct with provider, model, and API key
- **LLM Provider**: ``GenAIProvider`` - thread-safe wrapper around the ``genai``
  crate with streaming support and retry logic

perspt-agent
~~~~~~~~~~~~

The heart of SRBN:

- **Orchestrator**: ``SRBNOrchestrator`` manages the DAG (``petgraph``), drives the
  7-phase control loop, manages LSP clients, and integrates TUI events. Split into
  submodules: ``mod.rs``, ``bundle.rs``, ``commit.rs``, ``convergence.rs``,
  ``init.rs``, ``planning.rs``, ``repair.rs``, ``solo.rs``, ``verification.rs``
- **Agents**: ``Agent`` trait with four implementations - ``ArchitectAgent``,
  ``ActuatorAgent``, ``VerifierAgent``, ``SpeculatorAgent``
- **Tools**: ``AgentTools`` - ``read_file``, ``write_file``, ``apply_patch``,
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

- **ChatApp** - Interactive chat with markdown rendering and virtual scrolling
- **AgentApp** - SRBN monitoring with dashboard, task tree, and diff viewer
- **ReviewModal** - Approval UI with grouped diff view and verification gates
- **LogsViewer** - LLM request/response log inspector
- **Dashboard** - Status display with energy breakdown
- **TaskTree** - Node execution tree with state indicators

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
- ``validate_artifact_mutation()`` protects root project files from delete/move

perspt-sandbox
~~~~~~~~~~~~~~

Sandboxed command execution with resource limits. Used for running build and
test commands in controlled environments during provisional branch verification.

perspt-dashboard
~~~~~~~~~~~~~~~~

Browser-based real-time monitoring of agent sessions. Built with Axum 0.8,
Askama 0.15 templates, HTMX 2 for partial updates, and Tailwind v4 / DaisyUI 5
for styling. Opens the session store in read-only mode so it never interferes
with the running agent. Provides Overview, DAG, Energy, LLM, Sandbox, and
Decisions pages plus an SSE stream for live updates.

perspt-sdk
~~~~~~~~~~

The reusable platform layer. ``perspt-sdk`` holds the part of the agent that
does not care whether the work is coding or anything else: it defines a
*residual* (a single piece of evidence that something is still wrong), the
energy that sums those residuals, the rule that decides when a result is good
enough to accept, and the certificate that explains an honest stop when retries
run out. A domain package plugs into this crate by implementing the
``AgentDomainPackage`` trait. The design is the subject of Perspt Specification
Proposal 8 (PSP-8); see :doc:`/developer-guide/extending` for how to build on it.

perspt-coding
~~~~~~~~~~~~~

The first domain package. ``perspt-coding`` tells the platform what "wrong"
means for source code: it lists the coding residual classes (compiler errors,
language-server diagnostics, syntax-tree problems, failing tests), supplies the
weights and acceptance threshold for the energy, and maps the worst residual to
a correction direction the agent can act on. Language adapters for Rust, Python,
and TypeScript live here.

perspt-research
~~~~~~~~~~~~~~~

A second domain, kept deliberately small. ``perspt-research`` exists to prove
that a new field of work can reuse the same platform without forking the engine.
It reframes "wrong" for research writing - unsupported claims, stale evidence,
source mismatch, missing citations - onto the same energy the SDK already
defines. The full verifier suites are out of scope; the point is to show the
contracts hold for more than coding.
