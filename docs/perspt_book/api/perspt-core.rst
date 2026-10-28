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
     - Core shared types: NodeClass, ModelTier, EnergyComponents, TaskPlan, PlannedTask, PlannedContract, PlannedTest, TaskType, CommandContract, SensorStatus, StageOutcome, VerifierStrictness, StructuralDigest, SummaryDigest, ArtifactKind, ContextBudget, RestrictionMap, ContextPackage, ContextProvenance
   * - ``config``
     - ``Config`` - provider/model/API-key basics plus the ``[providers]``, ``[models]``, ``[external_tools]``, ``[verification]``, ``[exploration]``, ``[prompts]``, and ``[context]`` tables
   * - ``events``
     - ``AgentEvent`` (33 variants), ``AgentAction``, ``NodeStatus``, ``ActionType``, channel types
   * - ``llm_provider``
     - ``GenAIProvider``, ``LlmResponse``, ``EOT_SIGNAL``, streaming support
   * - ``plugin``
     - ``LanguagePlugin`` trait, ``PythonPlugin``, ``RustPlugin``, ``JsPlugin``, ``PluginRegistry``
   * - ``memory``
     - ``ProjectMemory`` from ``.perspt/memory.toml``
   * - ``normalize``
     - Model and provider name normalization
   * - ``prompts``
     - ``PlatformPromptLibrary`` - the platform prompt section library, compiled at build time by ``perspt-prompt-macros``
   * - ``portfolio``
     - ``ModelPortfolio``, ``ProviderHandle`` - several concurrently live provider handles
   * - ``tools_driver``
     - ``CoreToolCall``, ``CoreToolSpec``, ``CoreTurnOutput`` - the ``genai`` tool-calling driver
   * - ``local_command``
     - ``LocalCommand`` - provider-independent commands handled entirely by frontends
   * - ``path``
     - Canonical path resolution for artifact paths
   * - ``paths``
     - Centralized platform-aware path helpers (config/data/project tiers)

Key Types
---------

**Config** - The main configuration struct (excerpt):

.. code-block:: rust

   pub struct Config {
       pub provider: Option<String>,
       pub model: Option<String>,
       pub api_key: Option<String>,
       pub base_url: Option<String>,
       // ... per-tier model overrides, plus the [providers], [models],
       // [external_tools], [verification], [exploration], [prompts],
       // and [context] tables
   }

**EnergyComponents** - Lyapunov energy decomposition:

.. code-block:: rust

   pub struct EnergyComponents {
       pub v_syn: f32,    // LSP diagnostics
       pub v_str: f32,    // Contract compliance
       pub v_log: f32,    // Test results
       pub v_boot: f32,   // Bootstrap commands
       pub v_sheaf: f32,  // Cross-node validation
   }

**AgentEvent** - 33 lifecycle events:

``TaskStatusChanged``, ``PlanGenerated``, ``EnergyUpdated``, ``Log``,
``NodeCompleted``, ``ApprovalRequest``, ``Complete``, ``Error``,
``PlanReady``, ``NodeSelected``, ``FallbackPlanner``,
``VerificationComplete``, ``BundleApplied``, ``SensorFallback``,
``DegradedVerification``, ``EscalationClassified``,
``SheafValidationComplete``, ``GraphRewriteApplied``, ``BranchCreated``,
``InterfaceSealed``, ``BranchFlushed``, ``DependentUnblocked``,
``BranchMerged``, ``ContextDegraded``, ``ContextBlocked``,
``StructuralDependencyMissing``, ``ModelFallback``, ``ProvenanceDrift``,
``ToolReadiness``, ``BudgetUpdated``, ``PlanRevised``, ``FileDeleted``,
``FileMoved``

See :doc:`../developer-guide/architecture` for the complete type inventory.
