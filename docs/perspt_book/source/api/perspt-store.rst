.. _api-perspt-store:

``perspt-store``
================

DuckDB-backed session and ledger persistence.

.. important::

   Perspt uses **DuckDB**, not SQLite, for its session store.

Core Type
---------

.. code-block:: rust

   pub struct SessionStore {
       conn: Mutex<Connection>,  // duckdb::Connection
   }

   impl SessionStore {
       pub fn new() -> Result<Self>;           // Default path
       pub fn open(path: &Path) -> Result<Self>; // Custom path
       pub fn open_read_only(path: &Path) -> Result<Self>; // Read-only mode
       pub fn default_db_path() -> PathBuf;    // ~/.local/share/perspt/
   }

.. note::

   ``open_read_only`` uses DuckDB's ``AccessMode::ReadOnly`` and does **not**
   call ``init_schema()``. This makes it safe for concurrent dashboard reads
   alongside the agent's write lock. The database file must already exist.

Record Types
------------

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Type
     - Description
   * - ``SessionRecord``
     - session_id, task, working_dir, merkle_root, detected_toolchain, status
   * - ``NodeStateRecord``
     - Per-node snapshot (id, state, energy, class, plugin, goal)
   * - ``EnergyRecord``
     - v_syn, v_str, v_log, v_boot, v_sheaf, v_total per node
   * - ``LlmRequestRecord``
     - model, prompt, response, tokens_in/out, latency_ms
   * - ``StructuralDigestRecord``
     - Content hash for interface seals
   * - ``ContextProvenanceRecord``
     - What context each node received
   * - ``EscalationReportRecord``
     - Classified escalation with energy and evidence
   * - ``RewriteRecordRow``
     - Graph rewrite audit trail
   * - ``SheafValidationRow``
     - Cross-node validation results
   * - ``ProvisionalBranchRow``
     - Branch lifecycle tracking
   * - ``BranchLineageRow``
     - Parent-child branch relationships
   * - ``InterfaceSealRow``
     - Sealed interface records
   * - ``BranchFlushRow``
     - Flush cascade records
   * - ``TaskGraphEdgeRow``
     - DAG edges between nodes
   * - ``ReviewOutcomeRow``
     - Human review decisions
   * - ``VerificationResultRow``
     - Full verification snapshots
   * - ``ArtifactBundleRow``
     - Stored artifact bundles per node

DuckDB Tables
--------------

The schema is initialized by ``init_schema()`` and includes 17+ tables matching
the record types above.
