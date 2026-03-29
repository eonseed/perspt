.. _api-perspt-agent:

``perspt-agent``
================

The SRBN orchestrator and all supporting subsystems.

Modules
-------

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Module
     - Description
   * - ``orchestrator``
     - ``SRBNOrchestrator`` — petgraph-based DAG execution with PSP-5 lifecycle
   * - ``agent``
     - ``Agent`` trait + ``ArchitectAgent``, ``ActuatorAgent``, ``VerifierAgent``, ``SpeculatorAgent``
   * - ``tools``
     - ``AgentTools`` — 10+ filesystem and shell tools (read_file, write_file, apply_diff, run_command, search_code, list_files, apply_patch, sed_replace, awk_filter, diff_files)
   * - ``ledger``
     - ``MerkleLedger`` — Content-addressed commit tracking over DuckDB
   * - ``lsp``
     - ``LspClient`` — JSON-RPC stdio client for rust-analyzer, ty, pyright, etc.
   * - ``test_runner``
     - ``TestRunnerTrait`` + ``PythonTestRunner``, ``RustTestRunner``, ``PluginVerifierRunner``
   * - ``context_retriever``
     - ``ContextRetriever`` — Workspace search with byte budget limits

Key Traits
----------

**Agent** — Per-tier LLM interaction:

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

**TestRunnerTrait** — Plugin-driven verification:

.. code-block:: rust

   #[async_trait]
   pub trait TestRunnerTrait: Send + Sync {
       async fn run_syntax_check(&self) -> Result<TestResults>;
       async fn run_tests(&self) -> Result<TestResults>;
       async fn run_build_check(&self) -> Result<TestResults>;
       async fn run_lint(&self) -> Result<TestResults>;
       async fn run_stage(&self, stage: VerifierStage) -> Result<TestResults>;
       fn name(&self) -> &str;
   }

Ledger Types
------------

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Type
     - Description
   * - ``MerkleCommit``
     - commit_id, session_id, node_id, merkle_root, parent_hash, energy, stable
   * - ``NodeCommitPayload``
     - Snapshot of node state for ledger commit
   * - ``LedgerStats``
     - total_sessions, total_commits, db_size_bytes
   * - ``NodeReviewSummary``
     - Energy history, escalations, seals, provenance for a single node
   * - ``SessionReviewSummary``
     - Aggregate stats: total/completed/failed/escalated, branches, review outcomes
   * - ``SessionSnapshot``
     - Full session state for resume: node_details, edges, branches, escalations
