.. _api-perspt-core:

``perspt-core``
===============

The foundation crate providing all canonical types, configuration, LLM provider
integration, event system, and language plugins.

Modules
-------

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Module
     - Description
   * - ``types``
     - All PSP-5 types: SRBNNode, NodeState, NodeClass, ModelTier, EnergyComponents, BehavioralContract, StabilityMonitor, RetryPolicy, TaskPlan, AgentContext, TokenBudget, ArtifactBundle, OwnershipManifest, VerificationResult, EscalationReport, SheafValidationResult, ProvisionalBranch, InterfaceSealRecord, ContextPackage, ContextProvenance, StructuralDigest, SummaryDigest, RestrictionMap, BlockedDependency
   * - ``config``
     - ``Config { provider, model, api_key }``
   * - ``events``
     - ``AgentEvent`` (~30 variants), ``AgentAction``, ``NodeStatus``, ``ActionType``, channel types
   * - ``llm_provider``
     - ``GenAIProvider``, ``LlmResponse``, ``EOT_SIGNAL``, streaming support
   * - ``plugin``
     - ``LanguagePlugin`` trait, ``PythonPlugin``, ``RustPlugin``, ``JsPlugin``, ``PluginRegistry``
   * - ``memory``
     - ``ProjectMemory`` from ``.perspt/memory.toml``
   * - ``normalize``
     - Model and provider name normalization

Key Types
---------

**SRBNNode** — The core DAG node:

.. code-block:: rust

   pub struct SRBNNode {
       pub node_id: String,
       pub goal: String,
       pub context_files: Vec<PathBuf>,
       pub output_targets: Vec<PathBuf>,
       pub contract: BehavioralContract,
       pub tier: ModelTier,
       pub monitor: StabilityMonitor,
       pub state: NodeState,
       pub parent_id: Option<String>,
       pub children: Vec<String>,
       pub node_class: NodeClass,
       pub owner_plugin: Option<String>,
       pub provisional_branch_id: Option<String>,
       pub interface_seal_hash: Option<String>,
   }

**EnergyComponents** — Lyapunov energy decomposition:

.. code-block:: rust

   pub struct EnergyComponents {
       pub v_syn: f32,    // LSP diagnostics
       pub v_str: f32,    // Contract compliance
       pub v_log: f32,    // Test results
       pub v_boot: f32,   // Bootstrap commands
       pub v_sheaf: f32,  // Cross-node validation
   }

**AgentEvent** — 30+ lifecycle events:

``TaskStatusChanged``, ``PlanGenerated``, ``PlanReady``, ``NodeSelected``,
``BundleApplied``, ``VerificationComplete``, ``SheafValidationComplete``,
``BranchCreated``, ``InterfaceSealed``, ``BranchFlushed``, ``BranchMerged``,
``EscalationClassified``, ``GraphRewriteApplied``, ``DegradedVerification``,
``SensorFallback``, ``ContextDegraded``, ``ContextBlocked``, ``ProvenanceDrift``,
``ModelFallback``, ``ToolReadiness``, ``ApprovalRequest``, ``EnergyUpdated``,
``NodeCompleted``, ``Complete``, ``Error``, ``Log``, ``FallbackPlanner``,
``DependentUnblocked``, ``StructuralDependencyMissing``

See :doc:`../developer-guide/architecture` for the complete type inventory.
