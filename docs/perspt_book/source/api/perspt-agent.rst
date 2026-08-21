.. _api-perspt-agent:

``perspt-agent``
================

The governed PSP-9 agent runtime: candidate workspaces, the SRBN tool loop,
sandboxed verification, and work-graph dispatch.

Modules
-------

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Module
     - Description
   * - ``candidate``
     - ``CandidateWorkspace`` - reversible coding candidate overlay with
       compiler-backed measurement
   * - ``exploration``
     - Deterministic, read-only repository orientation for the runtime
   * - ``external_tools``
     - ``ExternalToolRuntime`` - shared governed MCP runtime for agent and
       interactive chat lifecycles
   * - ``grant``
     - Persistent grant signing-key resolution
   * - ``lsp``
     - ``LspClient`` - JSON-RPC stdio client for language servers (the
       sensor architecture)
   * - ``measure``
     - ``CodingCandidateMeasurer`` - full verifier suite at gate boundaries,
       cheap syntax-only pass at mutation boundaries, and the explicit
       evolving/backward-compatible/external-oracle test-evidence policies
   * - ``probe``
     - ``probe_route``/``ProbeReport`` - behavioral provider probes (Gate U)
   * - ``promote``
     - Descriptor-relative workspace promotion
   * - ``realize``
     - ``SnapshotRealizer``/``ProjectionMismatch`` - the gate is evaluated on
       the realized candidate workspace, never the model's account of it
   * - ``runtime``
     - ``Psp9AgentRuntime`` - the authoritative PSP-9 agent runtime
   * - ``toolloop``
     - ``ToolLoop`` - the SRBN tool loop; every model-issued tool call is a
       governed proposal through the admissibility kernel
   * - ``tools``
     - ``AgentTools`` - filesystem and search operations plus the open
       ``CandidateHandlerRegistry`` the governed candidate dispatches through
   * - ``transport``
     - ``GenAiTransport`` - the only adapter joining the SDK's
       provider-neutral contract and ``perspt-core``'s ``genai`` driver
   * - ``turn``
     - Universal actor turn runner shared by every stochastic actor (PSP-10)
   * - ``verifier``
     - Governed verifier sandbox: compiler/test/lint processes run behind a
       deny-network profile and a read allow-list

Key Types
---------

**Psp9ModelRoutes** - Explicit model-plane routes for one PSP-9 session:

.. code-block:: rust

   pub struct Psp9ModelRoutes {
       pub primary: Option<String>,
       pub actuator: Option<String>,
       pub explorer: Option<String>,
       pub adjudicator: Option<String>,
       pub fallbacks: Vec<String>,
   }

**Psp9RunSummary** - Terminal outcome of one run:

.. code-block:: rust

   pub struct Psp9RunSummary {
       pub session_id: String,
       pub node_id: String,
       pub outcome: NodeTerminalOutcome,
       pub turns_used: u32,
       pub ledger_head: String,
       pub promoted_paths: Vec<String>,
   }

Usage
-----

.. code-block:: rust

   let runtime = Psp9AgentRuntime::from_config(workdir, &config, routes, run_config)?
       .with_database_path(db_path);

   let summary = runtime.run(task).await?;
