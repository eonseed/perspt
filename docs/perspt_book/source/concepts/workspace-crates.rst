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
       agent -> sdk [style=dotted, label="SDK Bridge"];
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

The entry-point command-line utility. It parses command-line flags and subcommands using the ``clap`` library and dispatches execution control to corresponding subsystems in other workspace crates.

Key structural types include:

* ``Cli``: The root structure declaring command-line configuration arguments.
* ``Commands``: The enum representing subcommands, including:
  
  * ``chat``: Starts the interactive terminal chat mode.
  * ``agent``: Starts the autonomous closed-loop SRBN coding orchestrator.
  * ``ledger``: Commands to query, inspect, or rollback Merkle ledger sessions.
  * ``config``: Modifies runtime configuration values in ``config.toml``.
  * ``dashboard``: Launches the monitoring Axum server and opens the browser.

perspt-core
~~~~~~~~~~~

The foundation crate. It defines the central types, data models, and configurations shared globally across all crates. It has no internal dependencies on other workspace crates, ensuring clean build separation.

Key components:

* **Core Types**: Structures representing the execution parameters, such as:
  
  .. code-block:: rust

     pub struct SRBNNode {
         pub id: String,
         pub goal: String,
         pub output_files: Vec<String>,
         pub dependencies: Vec<String>,
         pub class: NodeClass, // Interface, Implementation, Integration
         pub status: NodeStatus,
     }

* **Event Plane**: Defines the event system carrying over 40 lifecycle events. The ``AgentEvent`` enum is transmitted across channels to update the TUI and log database records.
* **Provider Gate**: Integrates with the ``genai`` crate to handle provider-agnostic LLM requests, supporting token streaming, prompt assemblies, and backoff limits.

perspt-agent
~~~~~~~~~~~~

The orchestrator and runtime executor. It houses the autonomous agent loops, prompt templates, tool definitions, and system sensors.

* **Closed-Loop Scheduler**: The ``SRBNOrchestrator`` drives the closed-loop convergence pipeline using a petgraph-based directed acyclic graph. It queries the graph for ready nodes, executes generation, verifies output energy, handles retry loops, and commits stable nodes.
* **The SDK Bridge**: Coordinates the legacy orchestrator scheduler with the new platform SDK. It runs the ``perspt-sdk`` measured acceptance gate alongside the verification phase. The bridge is defined inside ``crates/perspt-agent/src/orchestrator/sdk_bridge.rs``:
  
  .. code-block:: rust

     pub struct SdkGateState {
         model: EnergyModel,
         best_accepted: HashMap<String, f64>,
         baseline: HashMap<String, f64>,
     }

  On each convergence step, the bridge maps verification outcomes to SDK residuals, evaluates gate descent conditions, and exposes detailed telemetry.

perspt-tui
~~~~~~~~~~

The terminal user interface built using the ``ratatui`` library. It provides high-performance terminal panels optimized for real-time human-in-the-loop review.

* **ChatApp**: An interactive chat client that supports streaming LLM dialogue, rendering tables, and parsing markdown.
* **AgentApp**: Displays active workspace executions. It visualizes the task graph DAG, displays active Lyapunov energy gauges, and opens a ``ReviewModal`` with unified code diffs for manual validation.

perspt-store
~~~~~~~~~~~~

The local database storage library built on DuckDB. It manages all persistent schemas for sessions, LLM requests, energy convergence logs, and ledger commits.

Key interface structures:

* ``SessionStore``: Encapsulates connection pools to the DuckDB file database.
* **Schema Definitions**: It creates and maintains tables tracking:
  
  * ``node_states``: Per-generation code snapshots.
  * ``energy_records``: Step-by-step energy changes.
  * ``provisional_branches``: Tracking active branches prior to parent commits.

perspt-policy
~~~~~~~~~~~~~

The security policy engine running the Starlark scripting runtime. It checks proposed mutations before execution to guarantee safety bounds.

Key API signatures:

.. code-block:: rust

   pub struct PolicyEngine {
       globals: Globals,
   }

   impl PolicyEngine {
       pub fn check_file_write(&self, path: &Path, content: &str) -> PolicyDecision;
       pub fn check_command(&self, command: &str) -> PolicyDecision;
   }

Starlark scripts allow developers to enforce isolation, blacklist hazardous command invocations, or limit write directories.

perspt-sandbox
~~~~~~~~~~~~~~

Command sandboxing library. It spawns verifiers and test runners inside isolated process boundaries to prevent local system resource contamination.

* ``SandboxedCommand``: Wraps standard library commands, enforcing memory ceilings, processor timeouts, environment sanitization, and redirecting stdout/stderr to files for analysis.

perspt-dashboard
~~~~~~~~~~~~~~~~

A read-only local dashboard built using the Axum web framework. It reads execution history directly from the DuckDB store and streams live progress to a local browser page.

* **Server Routing**: Sets up REST routes for historical session analysis.
* **SSE Stream**: Sends live event frames (graph state changes, energy rollups) using Server-Sent Events (SSE).

perspt-sdk
~~~~~~~~~~

The core platform library defining the reusable SRBN stability protocol specifications.

* **Domain-Neutral SDK**: Implements the mathematical formulations for the Measured Acceptance Gate. It has no knowledge of language rules, compilers, or project layouts. It operates purely on mathematical residuals and domain contracts.
* **Domain Integration Trait**: The core trait that custom domain extensions must implement:
  
  .. code-block:: rust

     pub trait AgentDomainPackage: Send + Sync {
         fn domain_id(&self) -> DomainId;
         fn detect(&self, workspace: &WorkspaceSnapshot) -> DomainDetection;
         fn residual_schema(&self, scope: &DomainScope) -> ResidualSchema;
         fn energy_model(&self, scope: &DomainScope) -> EnergyModel;
         fn correction_directions(&self, residuals: &[ResidualEvent]) -> Vec<CorrectionDirection>;
     }

perspt-coding
~~~~~~~~~~~~~

The coding-domain package, implementing the ``AgentDomainPackage`` trait.

* **Coding Domain Model**: Translates code verification outcomes (compiler syntax diagnostics, type warnings, failed tests, linter reports, and missing symbols) into SDK ``ResidualEvent`` representations.
* **Language Adapters**: Implements adapters for Rust, Python, and TypeScript to parse raw toolchain output, associate them with residual classes, and supply directed correction instructions (e.g. telling the Actuator what type error was detected and how to resolve it).

perspt-research
~~~~~~~~~~~~~~~

An experimental verification domain package.

* **Academic Manuscript Domain**: Implements the SDK traits to manage research document production. It treats LaTeX syntax errors, missing citations, and bibliography inconsistencies as residual events, driving convergence towards a fully validated scientific manuscript. It serves as a proof of concept for the SDK's domain-agnostic separation of concerns.
