.. _workspace-crates:

Workspace Crates
================

Perspt is organized as a twelve-crate Rust workspace under the ``crates/`` directory. The program separates user interaction, core execution, security policy, persistence, and domain specifications into distinct libraries.

Eight crates form the running executable program; three crates define the reusable platform SDK and its target domains; one meta-crate re-exports the libraries.

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
       agent -> sdk [style=dotted, label="PSP-8 Bridge"];
       coding -> sdk;
       research -> sdk;
       dashboard -> store;
       dashboard -> core;
   }

Crate Summary
-------------

.. list-table::
   :header-rows: 1
   :widths: 20 40 40

   * - Crate
     - Operational Purpose
     - Primary Structural Types
   * - **perspt-cli**
     - Parses command-line inputs and dispatches system commands.
     - ``Cli``, ``Commands``
   * - **perspt-core**
     - Formulates shared data structures, configurations, and provider connections.
     - ``GenAIProvider``, ``Config``, ``SRBNNode``, ``AgentEvent``
   * - **perspt-agent**
     - Executes the closed-loop scheduler using legacy PSP-5/7 types.
     - ``SRBNOrchestrator``, ``Agent``, ``LspClient``, ``MerkleLedger``
   * - **perspt-tui**
     - Implements the terminal user interface for interactive review.
     - ``ChatApp``, ``AgentApp``, ``DiffViewer``, ``ReviewModal``
   * - **perspt-store**
     - Manages the DuckDB persistence layer for session logs.
     - ``SessionStore``, ``EnergyRecord``
   * - **perspt-policy**
     - Executes Starlark policies to validate proposed command structures.
     - ``PolicyEngine``, ``sanitize_command``
   * - **perspt-sandbox**
     - Runs external commands inside isolated subprocess environments.
     - ``SandboxedCommand``, ``CommandResult``
   * - **perspt-dashboard**
     - Implements a read-only browser-based monitoring engine.
     - ``AppState``, Axum router, SSE stream
   * - **perspt-sdk**
     - Defines domain-neutral SRBN stability contracts and gate models.
     - ``ResidualEvent``, ``EnergyModel``, ``AgentDomainPackage``, ``ResidualCertificate``
   * - **perspt-coding**
     - Implements the coding-domain adapters, sensors, and mappers.
     - ``CodingDomain``, ``CodingLanguage``
   * - **perspt-research**
     - Implements the research-domain adapter to demonstrate SDK reuse.
     - ``ResearchDomain``

perspt-cli
~~~~~~~~~~

The CLI crate parses user options via ``clap`` and dispatches tasks to the appropriate backend handlers. It supports commands for interactive chat, autonomous execution, ledger queries, configuration editing, and dashboard monitoring.

perspt-core
~~~~~~~~~~~

The core library defines structural datatypes shared across all crates:

- **Data Models**: Structures representing nodes, task plans, budgets, retry policies, and session outcomes.
- **Event Plane**: The ``AgentEvent`` enumeration, carrying over 40 lifecycle telemetry events.
- **Provider Interface**: A wrapper around the ``genai`` crate to handle token stream requests and error failovers.
- **Plugin Registry**: Outdated extension registry; maintained as a transition helper until all language logic is migrated to the coding domain adapter.

perspt-agent
~~~~~~~~~~~~

The orchestrator implementation crate.

- **The Closed Loop**: ``SRBNOrchestrator`` parses the user's task, generates the initial dependency graph, and drives execution through planning, generation, verification, and commit phases.
- **The SDK Bridge**: Currently, the orchestrator drives its internal queue using legacy ``SRBNNode`` and ``StabilityMonitor`` types. It executes the new ``perspt-sdk`` measured acceptance gate alongside this loop, outputting SDK quadratic energy calculations as telemetry.
- **Subsystems**: Includes the native language-server protocol (LSP) client, test runners, local file utilities, and prompt templates.

perspt-tui
~~~~~~~~~~

Implements the terminal rendering engine using ``ratatui``. It allows users to view streamed dialogue, browse unified code changes, inspect the active DAG execution tree, and interactive review of proposed commits.

perspt-store
~~~~~~~~~~~~

Handles state persistence via DuckDB. It records session configurations, node outcomes, and LLM communication logs.

perspt-policy
~~~~~~~~~~~~~

Executes security rules using the Starlark scripting runtime. It checks proposed shell commands and file mutations to prevent actions outside the workspace footprint.

perspt-sandbox
~~~~~~~~~~~~~~

Isolates command executions. It ensures that compiler and test suite runs do not leak resources or persist changes outside temporary workspace branches.

perspt-dashboard
~~~~~~~~~~~~~~~~

A local Axum-based server that reads the persistence store and streams updates to a web browser. It presents real-time heatmaps and graph status timelines.

perspt-sdk
~~~~~~~~~~

The reusable platform layer. It contains the mathematical definitions of the SRBN stability contract: the quadratic Lyapunov energy model, the measured acceptance gate, the spectral constants, and the final residual certificate. It defines the traits that domain packages must implement to plug into the system.

perspt-coding
~~~~~~~~~~~~~

The primary domain package. It implements the SDK traits for code repositories, defining compiler diagnostics and test failures as formal residuals, and mapping them to target correction directions.

perspt-research
~~~~~~~~~~~~~~~

A validation domain package. It implements the SDK traits to model research manuscript compilation and citation validation as stability objectives. It serves as a structural proof that the SDK handles fields of work outside of coding.
