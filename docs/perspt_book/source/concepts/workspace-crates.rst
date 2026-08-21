.. _workspace-crates:

Workspace Crates
================

Perspt is organized as a fourteen-crate Rust workspace under the ``crates/`` directory. The program separates user interaction, core execution, security policy, persistence, and domain specifications into distinct libraries.

Eight crates form the running executable program; three crates define the reusable platform SDK and its target domains; ``perspt-prompt-macros`` is the build-time compiler for the prompt section libraries; ``perspt-benchmark`` is an optional, manually run evaluation suite (feature-gated in ``perspt-cli``, never part of normal validation); one meta-crate re-exports the libraries.

.. graphviz::
   :align: center
   :caption: Crate Dependencies

   digraph deps {
       rankdir=TB;
       node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=10];

       cli [label="perspt-cli\n16 commands", fillcolor="#4ECDC4"];
       tui [label="perspt-tui\nRatatui UI", fillcolor="#96CEB4"];
       agent [label="perspt-agent\nPSP-9 Runtime", fillcolor="#FFEAA7"];
       core [label="perspt-core\nLLM + Types", fillcolor="#45B7D1"];
       store [label="perspt-store\nDuckDB", fillcolor="#87CEEB"];
       policy [label="perspt-policy\nStarlark", fillcolor="#DDA0DD"];
       sandbox [label="perspt-sandbox\nIsolation", fillcolor="#F8B739"];
       dashboard [label="perspt-dashboard\nWeb UI", fillcolor="#FFB6C1"];
       bench [label="perspt-benchmark\nEvaluation (optional)", fillcolor="#D3D3D3"];

       subgraph cluster_sdk {
           label="Reusable SDK Platform";
           style=dashed;
           sdk [label="perspt-sdk\nStability Contract", fillcolor="#AED9E0"];
           coding [label="perspt-coding\nCoding Domain", fillcolor="#C7CEEA"];
           research [label="perspt-research\nResearch Domain", fillcolor="#E2C2C6"];
           macros [label="perspt-prompt-macros\nPrompt Codegen", fillcolor="#AED9E0"];
       }

       cli -> tui;
       cli -> agent;
       cli -> core;
       cli -> store;
       cli -> dashboard;
       cli -> bench [style=dotted, label="benchmark feature"];
       agent -> core;
       agent -> store;
       agent -> policy;
       agent -> sandbox;
       agent -> sdk;
       core -> sdk;
       core -> macros [style=dotted, label="build.rs"];
       coding -> sdk;
       coding -> macros [style=dotted, label="build.rs"];
       macros -> sdk;
       research -> sdk;
       bench -> agent;
       dashboard -> store;
       dashboard -> sdk;
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
     - ``GenAIProvider``, ``Config``, ``TaskPlan``, ``NodeClass``, ``AgentEvent``
   * - **perspt-agent**
     - Executes the governed PSP-9 tool loop and work-graph runtime.
     - ``Psp9AgentRuntime``, ``CandidateWorkspace``, ``AgentTools``, ``GenAiTransport``
   * - **perspt-tui**
     - Implements the terminal user interface for interactive review.
     - ``ChatApp``, ``AgentApp``, ``DiffViewer``, ``ReviewModal``
   * - **perspt-store**
     - Manages the DuckDB persistence layer for session logs.
     - ``SessionStore``, ``SessionRecord``, ``Psp9LedgerRow``
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
     - ``CodingDomain``, ``LanguageId``, ``CodingAdapterRegistry``
   * - **perspt-research**
     - Implements the research-domain adapter to demonstrate SDK reuse.
     - ``ResearchDomain``
   * - **perspt-prompt-macros**
     - Compiles prompt section files into typed sections at build time.
     - ``StageDecl``, ``PromptBuildError``
   * - **perspt-benchmark**
     - Runs the optional, manually invoked PSP-10 evaluation ladder.
     - ``BenchmarkSuite``, ``BenchmarkRunOptions``

perspt-cli
~~~~~~~~~~

The entry-point command-line utility. It parses command-line flags and subcommands using the ``clap`` library and dispatches execution control to corresponding subsystems in other workspace crates.

Key structural types include:

* ``Cli``: The root structure declaring command-line configuration arguments.
* ``Commands``: The enum representing the sixteen subcommands (plus a feature-gated seventeenth):

  * ``chat``: Starts the interactive terminal chat mode (the default).
  * ``simple-chat``: Simple CLI chat mode without the TUI.
  * ``agent``: Runs the governed PSP-9 agent on a task.
  * ``init``: Initializes project memory and Starlark policy rules.
  * ``config``: Modifies runtime configuration values in ``config.toml``.
  * ``ledger``: Commands to query, inspect, or rollback Merkle ledger sessions.
  * ``status``: Shows current agent status.
  * ``audit``: Delayed audit labels and conformal activation.
  * ``providers``: Prints the provider capability matrix.
  * ``replay``: Deterministic, credential-free audit replay of a session.
  * ``abort``: Aborts a PSP-9 session by revoking its authority epoch.
  * ``resume``: Resumes a paused or crashed session.
  * ``dashboard``: Launches the monitoring Axum server and opens the browser.
  * ``db``: Inspects and repairs the local DuckDB store.
  * ``prompts``: Inspects and maintains the compiled prompt section libraries.
  * ``context``: Explains a session's recorded resident-context events.
  * ``benchmark`` (``--features benchmark`` only): Runs the optional model-backed evaluation tooling.

perspt-core
~~~~~~~~~~~

The foundation crate. It defines the central types, data models, and configurations shared globally across all crates. Among workspace crates it depends only on ``perspt-sdk`` and, at build time, on the ``perspt-prompt-macros`` compiler.

Key components:

* **Core Types**: Structures representing the execution parameters, such as:

  .. code-block:: rust

     pub enum NodeClass {
         /// Defines exported signatures, schemas, ownership manifests
         Interface,
         /// Operates on node-owned files plus adjacent sealed interfaces
         Implementation,
         /// Reconciles cross-owner or cross-plugin boundaries
         Integration,
     }

* **Event Plane**: Defines the event system carrying over 40 lifecycle events. The ``AgentEvent`` enum is transmitted across channels to update the TUI and log database records.
* **Provider Gate**: Integrates with the ``genai`` crate to handle provider-agnostic LLM requests, supporting token streaming, prompt assemblies, and backoff limits.
* **Prompt Libraries**: Hosts the versioned prompt section files under ``crates/perspt-core/prompts/`` (``session_bootstrap``, ``graph_plan``, ``repository_explore``, ``adjudicate``, ``evidence_summarize``), compiled at build time by ``perspt-prompt-macros`` against the ``perspt-sdk`` prompt module.

perspt-agent
~~~~~~~~~~~~

The authoritative PSP-9 runtime executor. It houses the governed tool loop, the work-graph dispatcher, tool definitions, and system sensors. Prompt text does not live here: the section libraries live under ``crates/perspt-core/prompts/``.

* **Governed Tool Loop**: The loop in ``crates/perspt-agent/src/toolloop/`` composes the ``perspt-sdk`` admissibility kernel, the measured acceptance gate, and the durable ledger. Every model-issued tool call becomes a typed proposal; admissible effects mutate an isolated ``CandidateWorkspace`` overlay; the ``CodingCandidateMeasurer`` re-measures the candidate under the configured test-evidence policy; and the gate is evaluated on the measurement, never on the model's account of it. Ordinary development uses the resulting tests, while historical regression and protected external acceptance suites are explicit opt-ins.
* **Work-Graph Dispatcher**: ``Psp9AgentRuntime`` drives multi-node execution behind ``--max-parallel-nodes``. A governed architect turn proposes graph revisions through the privileged ``update_graph`` tool; the dispatcher fills free slots with ready nodes whose file footprints do not conflict, stages each node's winner into a content-addressed staging root, and promotes the merged result through one global integration gate. ``perspt-agent`` depends on ``perspt-sdk`` directly; there is no bridge layer.

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
* **Schema Definitions**: It creates and maintains eleven tables:

  * ``sessions``: Top-level session tracking.
  * ``schema_migrations``: Applied schema versions.
  * ``psp9_ledger_events``: The durable hash-chained canonical event stream.
  * ``psp9_artifacts``: Content-addressed artifact bytes keyed by hash.
  * ``psp9_authority_epochs``: Per-session authority epoch counters.
  * ``psp9_grant_policies``: Persisted signed grant intent.
  * ``psp9_external_effects``: External effect intents keyed by idempotency key.
  * ``psp9_context_checkpoints``: Durable context checkpoints per covered event root.
  * ``psp9_verdicts``: Per-validator verdicts feeding the independence statistics.
  * ``psp9_calibration_epochs`` and ``psp9_calibration_samples``: Conformal calibration state and delayed-audit sample labels.

perspt-policy
~~~~~~~~~~~~~

The security policy engine running the Starlark scripting runtime. It checks proposed mutations before execution to guarantee safety bounds.

Key API signatures:

.. code-block:: rust

   pub enum PolicyDecision {
       Allow,
       Prompt(String),
       Deny(String),
   }

   impl PolicyEngine {
       pub fn evaluate(&self, command: &str) -> PolicyDecision;
       pub fn is_safe(&self, command: &str) -> bool;
   }

Each Starlark policy is a single function that receives the command string and returns ``"allow"``, ``"prompt"``, ``"deny"``, or a boolean; the engine folds multiple policies by taking the strictest decision. The crate also exports the deterministic guards ``sanitize_command``, ``validate_workspace_bound``, and ``validate_artifact_mutation``, which run before any Starlark evaluation.

perspt-sandbox
~~~~~~~~~~~~~~

Bounded command-execution and process-isolation library. It uses Bubblewrap on
Linux and ``sandbox-exec`` on macOS, fails closed when required isolation is
unavailable, and exposes an explicit best-effort mode for externally isolated
embedders and acknowledged native Windows operation.

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
         // PSP-9 agent contract (required):
         fn tool_entries(&self, scope: &DomainScope) -> Vec<ToolEntry>;
         fn hard_gate_policy(&self, scope: &DomainScope) -> HardGatePolicy;
         fn verifier_suite(&self, scope: &DomainScope) -> VerifierSuiteSpec;
         fn safety_barrier(&self, scope: &DomainScope) -> SafetyBarrierDisposition;
     }

perspt-coding
~~~~~~~~~~~~~

The coding-domain package, implementing the ``AgentDomainPackage`` trait.

* **Coding Domain Model**: Translates code verification outcomes (compiler syntax diagnostics, type warnings, failed tests, linter reports, and missing symbols) into SDK ``ResidualEvent`` representations.
* **Language Adapters**: Implements adapters for Rust, Python, and TypeScript to parse raw toolchain output, associate them with residual classes, and supply directed correction instructions (e.g. telling the Actuator what type error was detected and how to resolve it). Adapters are registered in the ``LanguageId``-keyed ``CodingAdapterRegistry``, so adding a language means registering an adapter, never editing the runtime.

perspt-research
~~~~~~~~~~~~~~~

An experimental verification domain package.

* **Academic Manuscript Domain**: Implements the SDK traits to manage research document production. It treats LaTeX syntax errors, missing citations, and bibliography inconsistencies as residual events, driving convergence towards a fully validated scientific manuscript. It serves as a proof of concept for the SDK's domain-agnostic separation of concerns.

perspt-prompt-macros
~~~~~~~~~~~~~~~~~~~~

The build-time compiler for prompt section files (PSP-10). Called from an owning crate's ``build.rs``, it parses every ``prompts/<stage>/NN_name.md`` file, runs the codegen validation list, and emits typed section structs into ``OUT_DIR``. A malformed section fails ``cargo build`` with an error naming the offending file, never a session. The same validation functions serve the runtime bundle scanner and ``perspt prompts lint``.

perspt-benchmark
~~~~~~~~~~~~~~~~

Optional PSP-10 evaluation tooling, independent of the runtime mechanism tests. It exposes the ``perspt benchmark`` subcommand only when ``perspt-cli`` is built with ``--features benchmark``; live runs require configured routes and credentials and never run in CI or normal validation. Suites (``smoke``, ``adaptive``, ``full``) run a cumulative ladder of arms over matched tasks and publish paired differences with a seeded bootstrap.
