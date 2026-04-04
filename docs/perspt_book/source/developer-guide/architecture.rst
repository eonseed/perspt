.. _developer-guide-architecture:

Architecture
============

Perspt is a Rust workspace with eight crates plus a root integration crate.
Version 0.5.6 implements the PSP-5 specification for the experimental SRBN agent.

Workspace Layout
----------------

.. code-block:: text

   perspt/                       # Root: integration crate (perspt)
   +-- crates/
   |   +-- perspt-core/          # Types, config, LLM provider, events, plugins
   |   +-- perspt-agent/         # SRBN orchestrator, agents, ledger, LSP, tools
   |   +-- perspt-tui/           # Ratatui TUI (chat + agent + review modal)
   |   +-- perspt-cli/           # Clap CLI entry point, subcommands
   |   +-- perspt-store/         # DuckDB session store
   |   +-- perspt-policy/        # Starlark policy engine
   |   +-- perspt-sandbox/       # Command sandboxing
   |   +-- perspt-dashboard/     # Axum web dashboard
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
       "perspt-cli" -> "perspt-agent";
       "perspt-cli" -> "perspt-tui";
       "perspt-cli" -> "perspt-store";
       "perspt-agent" -> "perspt-core";
       "perspt-agent" -> "perspt-store";
       "perspt-agent" -> "perspt-policy";
       "perspt-agent" -> "perspt-sandbox";
       "perspt-tui" -> "perspt-core";
       "perspt-tui" -> "perspt-agent";
       "perspt-store" -> "perspt-core";
       "perspt-policy" [label="perspt-policy\n(Starlark)"];
       "perspt-sandbox" [label="perspt-sandbox"];
       "perspt-dashboard" -> "perspt-store";
       "perspt-dashboard" -> "perspt-core";
       "perspt-cli" -> "perspt-dashboard";
   }


Crate: ``perspt-core``
-----------------------

The foundation crate. Re-exports all canonical types.

**Modules:**

- ``types`` — All PSP-5 types (see :ref:`type-inventory` below)
- ``config`` — ``Config { provider, model, api_key }``
- ``events`` — ``AgentEvent`` (~30 variants), ``AgentAction``, ``NodeStatus``, ``ActionType``
- ``llm_provider`` — ``GenAIProvider`` wrapping the ``genai`` crate; ``EOT_SIGNAL``
- ``plugin`` — ``LanguagePlugin`` trait + ``PythonPlugin``, ``RustPlugin``, ``JsPlugin``
- ``memory`` — ``ProjectMemory`` loaded from ``.perspt/memory.toml``
- ``normalize`` — Model and provider name normalization

**Key Plugin Types:**

.. code-block:: rust

   pub trait LanguagePlugin: Send + Sync {
       fn name(&self) -> &str;
       fn detect(&self, path: &Path) -> bool;
       fn get_init_action(&self, opts: &InitOptions) -> ProjectAction;
       fn test_command(&self) -> Option<String>;
       fn syntax_check_command(&self) -> Option<String>;
       fn verifier_profile(&self) -> VerifierProfile;
       fn owns_file(&self, path: &Path) -> bool;
       // ... ~15 methods total
   }

Plugins provide verifier profiles with fallback chains:

.. code-block:: rust

   pub struct VerifierProfile {
       pub plugin_name: String,
       pub capabilities: Vec<VerifierCapability>,
       pub lsp: LspCapability,
   }

   pub struct VerifierCapability {
       pub stage: VerifierStage,       // SyntaxCheck | Build | Test | Lint
       pub command: Option<String>,     // Primary command
       pub available: bool,
       pub fallback_command: Option<String>,
       pub fallback_available: bool,
   }


Crate: ``perspt-agent``
------------------------

The experimental SRBN orchestrator and its subsystems.

**Modules:**

- ``orchestrator`` — ``SRBNOrchestrator`` with ``petgraph::DiGraph<SRBNNode, Dependency>``.  Split into phase-focused sub-modules: ``mod`` (struct, constructors, run, helpers, tests), ``init`` (workspace bootstrap), ``solo`` (single-file mode), ``planning`` (architect interaction), ``verification`` (energy computation), ``convergence`` (stability loop), ``commit`` (ledger promotion), ``repair`` (correction prompts), ``bundle`` (artifact application).
- ``agent`` — ``Agent`` trait + ``ArchitectAgent``, ``ActuatorAgent``, ``VerifierAgent``, ``SpeculatorAgent``
- ``tools`` — ``AgentTools`` (read_file, write_file, apply_diff, delete_file, move_file, run_command, search_code, etc.)
- ``prompts`` — Externalized prompt templates for architect (existing/greenfield) and actuator roles
- ``ledger`` — ``MerkleLedger`` atop ``SessionStore``
- ``lsp`` — ``LspClient`` (JSON-RPC over stdio)
- ``test_runner`` — ``TestRunnerTrait`` + ``PythonTestRunner``, ``RustTestRunner``, ``PluginVerifierRunner``
- ``context_retriever`` — ``ContextRetriever`` for workspace search

**Agent Trait:**

.. code-block:: rust

   #[async_trait]
   pub trait Agent: Send + Sync {
       async fn process(&self, node: &SRBNNode, ctx: &AgentContext)
           -> Result<AgentMessage>;
       fn name(&self) -> &str;
       fn can_handle(&self, node: &SRBNNode) -> bool;
       fn model(&self) -> &str;
       fn build_prompt(&self, node: &SRBNNode, ctx: &AgentContext) -> String;
   }

**Orchestrator:**

.. code-block:: rust

   pub struct SRBNOrchestrator {
       pub graph: DiGraph<SRBNNode, Dependency>,
       pub context: AgentContext,
       // + ledger, agents (4 tiers), tools, LSP clients, test runners,
       //   event/action channels, per-tier model names + fallbacks
   }

The orchestrator drives the PSP-5 lifecycle:

1. ``detect_workspace()`` — Identify plugins and workspace state
2. ``plan_task()`` — Architect decomposes task into DAG
3. ``execute_dag()`` — Topological traversal with per-node verification loop
4. ``verify_node()`` — Compute V(x) via plugin verifier profile
5. ``sheaf_validate()`` — Cross-node contract checking
6. ``review_node()`` — Interactive approval (unless ``--yes``)
7. ``commit_node()`` — Record in Merkle ledger


Crate: ``perspt-store``
------------------------

DuckDB-backed persistence. **Not SQLite.**

.. code-block:: rust

   pub struct SessionStore {
       conn: Mutex<Connection>,  // duckdb::Connection
   }

Tables:

- ``sessions`` — Session metadata (id, task, working_dir, merkle_root, status)
- ``node_states`` — Per-node snapshots
- ``energy_records`` — Energy history
- ``llm_requests`` — Full LLM request/response logging
- ``structural_digests`` — Content hashes for interface seals
- ``context_provenance`` — Provenance tracking per node
- ``escalation_reports`` — Classified escalations
- ``rewrite_records`` — Graph rewrite audit trail
- ``sheaf_validations`` — Cross-node validation results
- ``provisional_branches`` — Branch lifecycle
- ``branch_lineage`` — Parent-child branch relationships
- ``interface_seals`` — Sealed interface records
- ``branch_flushes`` — Flush cascade records
- ``task_graph_edges`` — DAG edges
- ``review_outcomes`` — Human review decisions
- ``verification_results`` — Full verification snapshots
- ``artifact_bundles`` — Bundle JSON per node
- ``plan_revisions`` — Plan version history
- ``feature_charters`` — Scope governance records
- ``repair_footprints`` — Correction audit trail
- ``budget_envelopes`` — Session budget caps and usage


Crate: ``perspt-tui``
-----------------------

Ratatui-based terminal UI with two modes:

- **ChatApp** — Interactive chat with markdown rendering and streaming
- **AgentApp** — Agent dashboard with DAG tree, energy display, review modal

Key components:

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Component
     - Purpose
   * - ``Dashboard``
     - Main agent dashboard layout
   * - ``TaskTree``
     - DAG visualization with node states
   * - ``ReviewModal``
     - Grouped diff viewer with approve/reject/correct
   * - ``DiffViewer``
     - Unified diff display
   * - ``LogsViewer``
     - LLM log browser
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

- ``sanitize_command(cmd)`` → ``SanitizeResult`` (split, validate, filter)
- ``validate_workspace_bound(cmd, working_dir)`` — Ensure commands stay in scope
- ``validate_artifact_mutation(path, workspace_root, operation)`` — Protect root project files from delete/move
- ``is_safe_for_auto_exec(cmd)`` — Whitelist check for auto-approval


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

PSP-5 Type Inventory
--------------------

All canonical types live in ``perspt_core::types``:

**SRBN Core:**

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Type
     - Description
   * - ``SRBNNode``
     - DAG node: goal, output_targets, contract, tier, monitor, state, node_class, owner_plugin, provisional_branch_id, interface_seal_hash
   * - ``NodeState`` (12 variants)
     - TaskQueued → Planning → Coding → Verifying → Retry → SheafCheck → Committing → Completed / Failed / Escalated / Aborted / Superseded
   * - ``NodeClass``
     - Interface, Implementation (default), Integration
   * - ``ModelTier``
     - Architect, Actuator, Verifier, Speculator
   * - ``BehavioralContract``
     - interface_signature, invariants, forbidden_patterns, weighted_tests, energy_weights
   * - ``StabilityMonitor``
     - energy_history, attempt_count, stable, stability_epsilon, max_retries, retry_policy
   * - ``RetryPolicy``
     - Per-error-type counters: compilation, tool, review

**Energy:**

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Type
     - Fields
   * - ``EnergyComponents``
     - v_syn (LSP), v_str (contracts), v_log (tests), v_boot (init), v_sheaf (cross-node)
   * - ``Criticality``
     - Critical (10.0), High (3.0), Low (1.0)
   * - ``WeightedTest``
     - test_name, criticality

**Task Planning:**

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Type
     - Description
   * - ``TaskPlan``
     - Container for ``Vec<PlannedTask>``
   * - ``PlannedTask``
     - id, goal, output_files, dependencies, task_type, contract, command_contract, node_class
   * - ``TaskType``
     - Code, Command, UnitTest, IntegrationTest, Refactor, Documentation
   * - ``CommandContract``
     - command, expected_exit_code, expected_files, forbidden_stderr_patterns

**Artifact Bundle:**

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Type
     - Description
   * - ``ArtifactBundle``
     - artifacts: Vec<ArtifactOperation>, commands: Vec<String>
   * - ``ArtifactOperation``
     - Write { path, content } | Diff { path, patch } | Delete { path } | Move { from, to }
   * - ``OwnershipManifest``
     - entries: HashMap<String, OwnershipEntry>, fanout_limit

**Verification:**

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Type
     - Description
   * - ``VerificationResult``
     - syntax_ok, build_ok, tests_ok, lint_ok, diagnostics_count, tests_passed/failed, degraded, stage_outcomes
   * - ``StageOutcome``
     - stage, passed, sensor_status, output
   * - ``SensorStatus``
     - Available | Fallback { actual, reason } | Unavailable { reason }
   * - ``VerifierStrictness``
     - Default, Strict, Minimal

**Context Management:**

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Type
     - Description
   * - ``AgentContext``
     - working_dir, history, merkle_root, complexity_k, session_id, auto_approve, defer_tests, token_budget, execution_mode, active_plugins, ownership_manifest
   * - ``TokenBudget``
     - max_tokens, max_cost_usd, tokens_used (in/out), cost_usd, per-1k rates
   * - ``ContextBudget``
     - byte_limit (100KB), file_count_limit (20)
   * - ``RestrictionMap``
     - Per-node context scoping: owned_files, sealed_interfaces, structural_digests
   * - ``ContextPackage``
     - Assembled context with budget tracking
   * - ``ContextProvenance``
     - Audit trail of what context each node received

**Escalation and Rewrite:**

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Type
     - Description
   * - ``EscalationCategory``
     - ImplementationError, ContractMismatch, InsufficientModelCapability, DegradedSensors, TopologyMismatch
   * - ``RewriteAction`` (9 variants)
     - GroundedRetry, ContractRepair, CapabilityPromotion, SensorRecovery, DegradedValidationStop, NodeSplit, InterfaceInsertion, SubgraphReplan, UserEscalation
   * - ``EscalationReport``
     - node_id, category, action, energy_snapshot, stage_outcomes, evidence

**Sheaf Validation:**

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Type
     - Description
   * - ``SheafValidatorClass`` (7 variants)
     - ExportImportConsistency, DependencyGraphConsistency, SchemaContractCompatibility, BuildGraphConsistency, TestOwnershipConsistency, CrossLanguageBoundary, PolicyInvariantConsistency
   * - ``SheafValidationResult``
     - validator_class, plugin_source, passed, evidence_summary, v_sheaf_contribution, requeue_targets

**Provisional Branches:**

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Type
     - Description
   * - ``ProvisionalBranch``
     - branch_id, node_id, state, parent_seal_hash, sandbox_dir
   * - ``ProvisionalBranchState``
     - Active → Sealed → Merged / Flushed
   * - ``InterfaceSealRecord``
     - node_id, sealed_path, artifact_kind, seal_hash, version
   * - ``BranchFlushRecord``
     - parent_node_id, flushed_branch_ids, requeue_node_ids, reason
   * - ``BlockedDependency``
     - child_node_id, parent_node_id, required_seal_paths

**Structural Digests:**

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Type
     - Description
   * - ``StructuralDigest``
     - Content hash of Interface artifacts (signatures, schemas, seals)
   * - ``SummaryDigest``
     - Compressed summaries (IntentSummary, VerifierResults, DesignRationale)
   * - ``ArtifactKind``
     - Signature, Schema, SymbolInventory, InterfaceSeal

**Planning and Budget:**

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Type
     - Description
   * - ``BudgetEnvelope``
     - Session-level budget caps: max_steps, max_revisions, max_cost_usd, usage counters
   * - ``FeatureCharter``
     - Scope governance: max_modules, max_files, max_revisions, language_constraint
   * - ``PlanRevision``
     - Versioned plan: revision_id, sequence, plan, reason, supersedes, status (Active/Superseded/Cancelled)
   * - ``RepairFootprint``
     - Correction audit: affected_files, applied_bundle, diagnosis, resolved flag
   * - ``PlanningPolicy`` (5 variants)
     - Adaptive agent gating: LocalEdit (skip architect), FeatureIncrement (default), LargeFeature, GreenfieldBuild, ArchitecturalRevision. Methods: ``needs_architect()``, ``needs_speculator()``


Events System
-------------

The event system uses unbounded tokio channels:

.. code-block:: rust

   // In perspt_core::events::channel
   pub type EventSender = UnboundedSender<AgentEvent>;
   pub type EventReceiver = UnboundedReceiver<AgentEvent>;
   pub type ActionSender = UnboundedSender<AgentAction>;
   pub type ActionReceiver = UnboundedReceiver<AgentAction>;

``AgentEvent`` has ~35 variants covering the full PSP-5 lifecycle:

- **Planning**: ``PlanReady``, ``PlanGenerated``, ``PlanRevised``
- **Execution**: ``NodeSelected``, ``BundleApplied``, ``NodeCompleted``
- **Verification**: ``VerificationComplete``, ``DegradedVerification``, ``SensorFallback``
- **Sheaf**: ``SheafValidationComplete``
- **Branches**: ``BranchCreated``, ``InterfaceSealed``, ``BranchFlushed``, ``BranchMerged``
- **Escalation**: ``EscalationClassified``, ``GraphRewriteApplied``
- **Context**: ``ContextDegraded``, ``ContextBlocked``, ``ProvenanceDrift``
- **Budget**: ``BudgetUpdated``
- **File Ops**: ``FileDeleted``, ``FileMoved``
- **UI**: ``ApprovalRequest``, ``TaskStatusChanged``, ``EnergyUpdated``, ``Log``
- **Lifecycle**: ``Complete``, ``Error``, ``ModelFallback``, ``ToolReadiness``


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
   |       +-- SRBNOrchestrator
   |       |     +-- detect_workspace()  -> [plugins]
   |       |     +-- plan_task()         -> [Architect Agent]
   |       |     +-- execute_dag()       -> [Actuator Agent]
   |       |     +-- verify_node()       -> [LSP, TestRunner, Verifier Agent]
   |       |     +-- sheaf_validate()    -> [Sheaf Validators]
   |       |     +-- commit_node()       -> [MerkleLedger]
   |       |
   |       +-- AgentTools  (filesystem, search, commands)
   |       +-- LspClient   (JSON-RPC stdio)
   |       +-- TestRunner   (plugin-driven)
   |       +-- ContextRetriever (workspace search)
   |       |
   |       +-- EventSender --> [perspt-tui AgentApp]
   |       +-- ActionReceiver <-- [perspt-tui ReviewModal]
   |
   [perspt-store]  DuckDB persistence
   [perspt-policy]  Starlark rule evaluation
   [perspt-sandbox]  Process isolation


Streaming Contract
------------------

Both chat and agent mode use the same streaming protocol:

1. LLM requests stream chunks over ``mpsc::UnboundedSender<String>``
2. End-of-response signaled by ``EOT_SIGNAL`` (``<|EOT|>``)
3. Provider sends EOT — UI never adds its own
4. UI batches channel messages, handles first EOT, ignores duplicates
5. Streaming buffer updates the last assistant message live
6. Pending inputs queue until EOT is received

.. warning::

   Never block the UI select loop. Spawn LLM work on tokio tasks and send
   results via the channel.
