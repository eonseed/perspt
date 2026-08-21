.. _developer-guide-architecture:

Architecture
============

Perspt is a Rust workspace of fourteen crates plus a dev-only ``xtask``
automation crate. Eight crates make up the running program, four crates form
the reusable platform layer (SDK, prompt codegen, and two domain packages),
one crate is the optional feature-gated benchmark harness, and one root crate
ties them together. Version 0.6.6 implements the PSP-9 governed candidate
runtime and the PSP-10 typed prompt, search, and integration layers.

Workspace Layout
----------------

.. code-block:: text

   perspt/                       # Root: integration crate (perspt)
   +-- crates/
   |   +-- perspt-core/          # Types, config, LLM provider, events, plugins, prompts
   |   +-- perspt-agent/         # Governed candidate runtime, tool loop, verifier, search
   |   +-- perspt-tui/           # Ratatui TUI (chat + agent + review modal)
   |   +-- perspt-cli/           # Clap CLI entry point, subcommands
   |   +-- perspt-store/         # DuckDB session store
   |   +-- perspt-policy/        # Starlark policy engine
   |   +-- perspt-sandbox/       # Command sandboxing
   |   +-- perspt-dashboard/     # Axum web dashboard
   |   +-- perspt-sdk/           # Domain-neutral SRBN platform SDK
   |   +-- perspt-prompt-macros/ # Build-time prompt section codegen
   |   +-- perspt-coding/        # Coding domain package (first domain)
   |   +-- perspt-research/      # Research domain package (second domain)
   |   +-- perspt-benchmark/     # Optional credentialed evaluation harness
   +-- xtask/                    # PSP code-rule checker (dev-only)
   +-- tests/                    # Integration tests
   +-- docs/                     # Sphinx documentation


Dependency Graph
----------------

.. graphviz::
   :align: center

   digraph crates {
       rankdir=BT;
       node [shape=box, style=rounded];
       "perspt-cli" -> "perspt-core";
       "perspt-cli" -> "perspt-sdk";
       "perspt-cli" -> "perspt-tui";
       "perspt-cli" -> "perspt-agent";
       "perspt-cli" -> "perspt-coding";
       "perspt-cli" -> "perspt-prompt-macros";
       "perspt-cli" -> "perspt-research";
       "perspt-cli" -> "perspt-store";
       "perspt-cli" -> "perspt-dashboard";
       "perspt-cli" -> "perspt-benchmark" [style=dotted, label="feature: benchmark"];
       "perspt-core" -> "perspt-sdk";
       "perspt-core" -> "perspt-prompt-macros";
       "perspt-agent" -> "perspt-core";
       "perspt-agent" -> "perspt-sdk";
       "perspt-agent" -> "perspt-coding";
       "perspt-agent" -> "perspt-research";
       "perspt-agent" -> "perspt-policy";
       "perspt-agent" -> "perspt-sandbox";
       "perspt-agent" -> "perspt-store";
       "perspt-tui" -> "perspt-core";
       "perspt-tui" -> "perspt-agent";
       "perspt-tui" -> "perspt-store";
       "perspt-store" -> "perspt-core";
       "perspt-policy" -> "perspt-core";
       "perspt-sandbox" [label="perspt-sandbox"];
       "perspt-coding" -> "perspt-sdk";
       "perspt-coding" -> "perspt-prompt-macros";
       "perspt-research" -> "perspt-sdk";
       "perspt-prompt-macros" -> "perspt-sdk";
       "perspt-dashboard" -> "perspt-store";
       "perspt-dashboard" -> "perspt-sdk";
       "perspt-benchmark" -> "perspt-agent";
       "perspt-benchmark" -> "perspt-core";
       "perspt-benchmark" -> "perspt-sdk";
       "perspt-benchmark" -> "perspt-store";
   }


PSP-9 / PSP-10 Overview
-----------------------

PSP-9 replaced the multi-agent orchestrator with a single governed candidate
runtime: every model-issued tool call becomes a typed proposal, a
deterministic admissibility kernel (in ``perspt-sdk``) decides whether it may
affect the reversible candidate workspace, and every gate is evaluated on the
re-measured candidate, never on the model's account of it. Every event lands
in a hash-chained durable ledger (``perspt-store``), so sessions replay and
resume deterministically. PSP-10 adds typed prompt section libraries with
build-time codegen (``perspt-prompt-macros``), the bounded search forest with
exact no-good learning, graph staging behind a global integration gate, and
the optional benchmark harness (``perspt-benchmark``).


Crate: ``perspt-core``
-----------------------

The foundation crate. Re-exports all canonical types.

**Modules:**

- ``types`` - Core shared types, split into submodules
  ``types/{context,model,plan,policy,verification,workspace}.rs``
  (see :ref:`type-inventory` below)
- ``config`` - ``Config { provider, model, api_key, ... }`` plus per-tier
  model overrides
- ``events`` - ``AgentEvent`` (33 variants), ``AgentAction``, ``NodeStatus``,
  ``ActionType``
- ``llm_provider`` - ``GenAIProvider`` wrapping the ``genai`` crate;
  ``EOT_SIGNAL``
- ``portfolio`` - ``ModelPortfolio`` with provider handles and declared caps
- ``plugin`` - ``LanguagePlugin`` trait + ``PythonPlugin``, ``RustPlugin``,
  ``JsPlugin``
- ``prompts`` - Typed prompt section libraries (``prompts/*/``) compiled at
  build time by ``perspt-prompt-macros``
- ``memory`` - ``ProjectMemory`` loaded from ``.perspt/memory.toml``
- ``normalize`` - Model and provider name normalization

**Key Plugin Types:**

.. code-block:: rust

   pub trait LanguagePlugin: Send + Sync {
       fn name(&self) -> &str;
       fn detect(&self, path: &Path) -> bool;
       fn get_init_action(&self, opts: &InitOptions) -> ProjectAction;
       fn test_command(&self) -> String;
       fn syntax_check_command(&self) -> Option<String>;
       fn verifier_profile(&self) -> VerifierProfile;
       fn owns_file(&self, path: &str) -> bool;
       // ... ~25 methods total
   }

Plugins provide verifier profiles with fallback chains:

.. code-block:: rust

   pub struct VerifierProfile {
       pub plugin_name: String,
       pub capabilities: Vec<VerifierCapability>,
       pub lsp: LspCapability,
   }

   pub struct VerifierCapability {
       pub stage: VerifierStage,       // SyntaxCheck | Build | Test | Lint | Format
       pub command: Option<String>,     // Primary command
       pub available: bool,
       pub fallback_command: Option<String>,
       pub fallback_available: bool,
   }


Crate: ``perspt-agent``
------------------------

The governed PSP-9/PSP-10 agent runtime.

**Modules:**

- ``runtime`` - ``Psp9AgentRuntime``: work-graph planning, bounded dispatch,
  node assembly, adjudication, staging and integration, resume, and the
  bounded search forest (``runtime/search/``)
- ``toolloop`` - The SRBN tool loop: each model-issued tool call becomes a
  typed proposal; the deterministic admissibility kernel decides whether it
  may affect the candidate
- ``candidate`` - ``CandidateWorkspace``: reversible coding-candidate overlay
- ``measure`` - ``CodingCandidateMeasurer``: full verifier suite at gate
  boundaries, cheap syntax-only pass at mutation boundaries
- ``verifier`` - Governed verifier sandbox: compiler/test/lint processes run
  under a deny-network profile with a read allow-list
- ``tools`` - ``AgentTools`` executor and sandboxing, the
  ``CandidateHandlerRegistry`` execution plane (``tools/handlers/``), and
  first-party tool families (``tools/families/{db,system}.rs``)
- ``transport`` - ``GenAiTransport``: the only adapter between the SDK's
  provider-neutral contract and ``perspt-core``'s ``genai`` driver
- ``turn`` - Universal actor turn runner (worker, explorer, architect,
  adjudicator, evidence summarizer, capability probe)
- ``grant`` - Persistent grant signing-key resolution
- ``probe`` - ``probe_route``/``ProbeReport``: behavioral provider probes
- ``promote`` - Descriptor-relative workspace promotion on Unix; native
  write-through replacement plus best-effort reparse-point rejection on Windows
- ``realize`` - ``SnapshotRealizer``: content-addressed workspace states
- ``exploration`` - Deterministic, read-only repository orientation
- ``external_tools`` - Official-SDK MCP 2026-07-28 client (stdio/stateless
  HTTP; tools, resources, prompts, roots, sampling, elicitation,
  subscriptions, MRTR/tasks), with local admission, replay, and separate
  agent/read-only-chat lifecycles
- ``lsp`` - ``LspClient`` (JSON-RPC over stdio)

**Runtime Flow:**

``Psp9AgentRuntime::run()`` drives one governed session:

1. Exploration - deterministic read-only repository map; ``--exploration-only``
   adds an interactive explorer tool loop and stops before any mutation
2. Planning - one forced-tool-choice architect turn, restricted to the
   privileged ``update_graph`` tool, produces the work graph; the host never
   fabricates multi-node graphs
3. Dispatch - bounded multi-node scheduling (``runtime/dispatch.rs``);
   ``--max-parallel-nodes`` sets the concurrency (default 1; above 1
   requires ``--yes``)
4. Tool loop - per node, the governed loop (``toolloop/``) turns each model
   tool call into a typed proposal, admits it through the deterministic
   kernel, and realizes it against the reversible candidate overlay
   (``candidate.rs``)
5. Measurement - ``measure.rs`` re-measures the candidate through the plugin
   verifier suite inside the sandboxed verifier (``verifier.rs``), or through
   explicitly acknowledged host-user execution in native Windows reduced-
   isolation mode; the gate
   is evaluated on the re-measured candidate, never on the model's account
   of it. The default evolving-test policy measures the resulting tests;
   backward-compatible and protected external evidence are explicit additions,
   never implicit assumptions about every task's contract
6. Adjudication - a tool-free validator reviews only the realized diff and
   records an uncalibrated verdict (``runtime/adjudicate.rs``)
7. Staging and integration - node winners stage into a graph workspace and
   must pass the global integration gate before descriptor-relative
   promotion (``runtime/integrate.rs``, ``promote.rs``). Unix promotion holds
   ancestor directory descriptors; Windows uses write-through native replace
   operations and rejects observed reparse points, without claiming the same
   race-resistant boundary.
8. Recording - every event lands in the durable hash-chained ledger
   (``runtime/recorder.rs``); interrupted sessions resume from the newest
   durable checkpoint with exactly the remaining budgets
   (``runtime/resume.rs``)

Failed attempts feed the bounded search forest (``runtime/search/``):
sequential, at most three branch identities, one branch attempt (quantum) at
a time, with exact no-good learning that suppresses only byte-identical
repeated attempts.


Crate: ``perspt-store``
------------------------

DuckDB-backed persistence. **Not SQLite.**

.. code-block:: rust

   pub struct SessionStore {
       conn: Mutex<Connection>,  // duckdb::Connection
   }

DuckDB Schema and Tables
~~~~~~~~~~~~~~~~~~~~~~~~

The schema (``crates/perspt-store/src/schema.rs``) is applied through an
idempotent transactional migration; ``schema_migrations`` records the
applied version. Eleven tables:

.. list-table:: Database Tables
   :header-rows: 1
   :widths: 25 40 35

   * - Table Name
     - Key Columns
     - Purpose
   * - ``sessions``
     - ``session_id`` (PK), ``task``, ``working_dir``, ``merkle_root``,
       ``detected_toolchain``, ``status``
     - Life cycle of agent sessions
   * - ``schema_migrations``
     - ``version`` (PK), ``applied_at``
     - Applied schema versions
   * - ``psp9_ledger_events``
     - ``(session_id, sequence)`` (PK), ``event_json``, ``prev_hash``,
       ``hash``
     - The durable canonical event stream; hash-chained
   * - ``psp9_artifacts``
     - ``content_hash`` (PK), ``content``, ``byte_len``, ``media_type``
     - Content-addressed artifact bytes
   * - ``psp9_authority_epochs``
     - ``session_id`` (PK), ``epoch``
     - Single-writer authority fencing per session
   * - ``psp9_context_checkpoints``
     - ``(session_id, covered_event_root)`` (PK), ``checkpoint_json``
     - Durable conversation checkpoints for resume
   * - ``psp9_external_effects``
     - ``(session_id, idempotency_key)`` (PK), ``intent_hash``, ``status``
     - External (MCP) effect intents and results, idempotent
   * - ``psp9_grant_policies``
     - ``policy_id`` (PK), ``session_id``, ``policy_json``, ``revoked``
     - Capability grant policies
   * - ``psp9_verdicts``
     - ``(session_id, candidate_id, validator_id)`` (PK), ``stratum``,
       ``missed``, ``unsafe_label``
     - Adjudication verdicts per candidate
   * - ``psp9_calibration_epochs``
     - ``epoch_id`` (PK), ``stratum``, ``target_rho``, ``threshold``,
       ``state``, ``sample_count``
     - Conformal calibration epochs
   * - ``psp9_calibration_samples``
     - ``(epoch_id, sample_id)`` (PK), ``score``, ``unsafe_label``,
       ``audit_selected``
     - Calibration samples; ``unsafe_label`` stays NULL until the delayed
       audit label arrives

Row types live in ``store/rows.rs`` (``SessionRecord``) and
``store/psp9_ledger.rs`` (``Psp9LedgerRow``, ``Psp9VerdictRow``,
``Psp9CalibrationEpochRow``, ``Psp9ExternalEffectRow``). ``repair.rs`` backs
``perspt db repair`` for recovering a database with a poisoned WAL.


Crate: ``perspt-tui``
-----------------------

Ratatui-based terminal UI with two modes:

- **ChatApp** - Interactive chat with streaming, LaTeX math transpilation, ASCII table wrapping, and markdown saving
- **AgentApp** - Agent dashboard with work-graph tree, energy display, review modal

The agent TUI is entered through ``run_agent_tui_with_runtime``
(``agent_app.rs``), which owns the ``Psp9AgentRuntime`` for the session and
wires its event and action channels.

Key components:

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Component
     - Purpose
   * - ``Dashboard``
     - Main agent dashboard layout
   * - ``TaskTree``
     - Work-graph visualization with node states
   * - ``ReviewModal``
     - Grouped diff viewer with approve/reject/correct
   * - ``DiffViewer``
     - Unified diff display
   * - ``telemetry``
     - TUI-side energy component display types
   * - ``FrameRateLimiter``
     - 60fps cap, adaptive rendering


Crate: ``perspt-policy``
--------------------------

Starlark policy evaluation:

.. code-block:: rust

   pub struct PolicyEngine {
       policies: Vec<FrozenModule>,
       policy_dir: PathBuf,
   }

   pub enum PolicyDecision {
       Allow,
       Prompt(String),
       Deny(String),
   }

Utility functions:

- ``sanitize_command(cmd)`` -> ``SanitizeResult`` (split, validate, filter)
- ``validate_workspace_bound(cmd, working_dir)`` - Ensure commands stay in scope
- ``validate_artifact_mutation(path, workspace_root, operation)`` - Protect root project files from delete/move


Crate: ``perspt-sandbox``
---------------------------

Process isolation with active timeout enforcement:

.. code-block:: rust

   pub trait SandboxedCommand: Send + Sync {
       fn execute(&self) -> Result<CommandResult>;
       fn display(&self) -> String;
       fn is_read_only(&self) -> bool;
   }

   pub struct BasicSandbox {
       program: String,
       args: Vec<String>,
       working_dir: Option<PathBuf>,
       timeout: Duration,  // Active: spawn + poll + kill on deadline
   }


.. _type-inventory:

Core Type Inventory
-------------------

All canonical shared types live in ``perspt_core::types``, re-exported from
``types/mod.rs``:

**Workspace (``types/workspace.rs``):**

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Type
     - Description
   * - ``NodeClass``
     - Interface, Implementation (default), Integration
   * - ``VerifierStrictness``
     - Default (compile + tests), Strict (adds lint), Minimal (syntax only)

**Model (``types/model.rs``):**

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Type
     - Description
   * - ``ModelTier``
     - Architect, Actuator, Verifier, Speculator; ``default_model()`` maps
       each tier to its recommended default route

**Energy (``types/context.rs``):**

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Type
     - Fields
   * - ``EnergyComponents``
     - v_syn (LSP), v_str (contracts), v_log (tests), v_boot (commands),
       v_sheaf (cross-node); ``total()`` is the plain sum of the rollups

**Task Planning (``types/plan.rs``):**

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Type
     - Description
   * - ``TaskPlan``
     - Container for ``Vec<PlannedTask>`` with ownership-closure,
       acyclicity, and implicit-dependency validation
   * - ``PlannedTask``
     - id, goal, context_files, output_files, dependencies, task_type,
       contract, command_contract, node_class, dependency_expectations
   * - ``TaskType``
     - Code, Command, UnitTest, IntegrationTest, Refactor, Documentation
   * - ``PlannedContract``
     - interface_signature, invariants, forbidden_patterns, tests
   * - ``PlannedTest``
     - name, criticality (informational string label)
   * - ``DependencyExpectation``
     - required_packages, setup_commands, min_toolchain_version
   * - ``CommandContract``
     - command, expected_exit_code, expected_files,
       forbidden_stderr_patterns, working_dir

**Verification and Context (``types/verification.rs``):**

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Type
     - Description
   * - ``SensorStatus``
     - Available | Fallback { actual, reason } | Unavailable { reason }
   * - ``StageOutcome``
     - stage, passed, sensor_status, output
   * - ``ArtifactKind``
     - Signature, Schema, SymbolInventory, InterfaceSeal
   * - ``StructuralDigest``
     - Content hash of a compile-critical structural artifact (signatures,
       schemas, seals)
   * - ``SummaryDigest``
     - Condensed summary with hash; ``SummaryKind`` is IntentSummary,
       VerifierResults, or DesignRationale
   * - ``ContextBudget``
     - byte_limit (100KB default), file_count_limit (20)
   * - ``RestrictionMap``
     - Per-node context boundary: owned_files, sealed_interfaces,
       structural/summary digests, dependency commits
   * - ``ContextPackage``
     - The bounded, reproducible context assembled for a node
   * - ``ContextProvenance``
     - Audit trail of the digests and files a node's context used

**Policy (``types/policy.rs``):**

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Type
     - Description
   * - ``CommandPolicyDecision``
     - Allow, Deny, RequireApproval
   * - ``ManifestMutationPolicy``
     - Allow, Deny


Events System
-------------

The event system uses unbounded tokio channels:

.. code-block:: rust

   // In perspt_core::events::channel
   pub type EventSender = UnboundedSender<AgentEvent>;
   pub type EventReceiver = UnboundedReceiver<AgentEvent>;
   pub type ActionSender = UnboundedSender<AgentAction>;
   pub type ActionReceiver = UnboundedReceiver<AgentAction>;

``AgentEvent`` has 33 variants (``crates/perspt-core/src/events.rs``):

- **Planning**: ``PlanReady``, ``PlanGenerated``, ``PlanRevised``, ``FallbackPlanner``
- **Execution**: ``NodeSelected``, ``BundleApplied``, ``NodeCompleted``
- **Verification**: ``VerificationComplete``, ``DegradedVerification``, ``SensorFallback``
- **Sheaf**: ``SheafValidationComplete``
- **Branches**: ``BranchCreated``, ``InterfaceSealed``, ``BranchFlushed``, ``BranchMerged``, ``DependentUnblocked``
- **Escalation**: ``EscalationClassified``, ``GraphRewriteApplied``
- **Context**: ``ContextDegraded``, ``ContextBlocked``, ``StructuralDependencyMissing``, ``ProvenanceDrift``
- **Budget**: ``BudgetUpdated``
- **File Ops**: ``FileDeleted``, ``FileMoved``
- **UI**: ``ApprovalRequest``, ``TaskStatusChanged``, ``EnergyUpdated``, ``Log``
- **Lifecycle**: ``Complete``, ``Error``, ``ModelFallback``, ``ToolReadiness``

The sheaf and branch variants are vestigial: the TUI still renders them for
older ledger streams, but the PSP-9 runtime no longer emits them.


Data Flow
---------

.. code-block:: text

   User Input
       |
   [perspt-cli]  Parse args (clap)
       |
   [perspt-core]  Config + Provider init
       |
   +---+---+
   |       |
   chat    agent
   |       |
   [tui]   [perspt-agent]
   |       |
   |       +-- Psp9AgentRuntime
   |       |     +-- exploration        -> read-only repository map
   |       |     +-- plan               -> architect turn (update_graph)
   |       |     +-- dispatch           -> bounded multi-node scheduler
   |       |     +-- tool loop          -> proposals -> admissibility kernel
   |       |     +-- candidate overlay  -> reversible workspace mutations
   |       |     +-- verifier/measure   -> sandboxed deterministic verification
   |       |     +-- adjudicate         -> diff-only validator verdict
   |       |     +-- integrate/promote  -> staging, integration gate, promotion
   |       |     +-- recorder           -> hash-chained ledger events
   |       |
   |       +-- GenAiTransport            (provider-neutral model plane)
   |       +-- CandidateHandlerRegistry  (open execution plane + tool families)
   |       +-- ExternalToolRuntime       (governed MCP)
   |       +-- LspClient                 (JSON-RPC stdio)
   |       |
   |       +-- EventSender --> [perspt-tui AgentApp / perspt-dashboard]
   |       +-- ActionReceiver <-- [perspt-tui ReviewModal]
   |
   [perspt-store]  DuckDB persistence (ledger, checkpoints, verdicts)
   [perspt-policy]  Starlark rule evaluation
   [perspt-sandbox]  Process isolation


Streaming Contract
------------------

Both chat and agent mode use the same streaming protocol:

1. LLM requests stream chunks over ``mpsc::UnboundedSender<String>``
2. End-of-response signaled by ``EOT_SIGNAL`` (``<|EOT|>``)
3. Provider sends EOT - UI never adds its own
4. UI batches channel messages, handles first EOT, ignores duplicates
5. Streaming buffer updates the last assistant message live
6. Pending inputs queue until EOT is received

.. warning::

   Never block the UI select loop. Spawn LLM work on tokio tasks and send
   results via the channel.
