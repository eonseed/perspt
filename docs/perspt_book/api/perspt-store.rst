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
   * - ``Psp9LedgerRow``
     - session_id, sequence, event_json, prev_hash, hash - one durable
       PSP-9 ledger record
   * - ``Psp9VerdictRow``
     - session_id, candidate_id, validator_id, stratum, missed,
       unsafe_label, evidence_hash
   * - ``Psp9CalibrationEpochRow``
     - epoch_id, stratum, target_rho, threshold, state, sample_count
   * - ``Psp9ExternalEffectRow``
     - idempotency_key, intent_hash, intent_json, result_json, status

The ledger accessors on ``SessionStore`` (append, read, and replay over the
``psp9_*`` tables) live in ``store/psp9_ledger.rs``; ``repair.rs`` provides
``repair_database``/``RepairReport``, the recoverable WAL quarantine behind
``perspt db repair``.

DuckDB Tables
--------------

The schema is initialized by ``init_schema()`` through an idempotent
transactional migration and comprises 11 tables: ``sessions``,
``schema_migrations``, ``psp9_ledger_events``, ``psp9_artifacts``,
``psp9_authority_epochs``, ``psp9_calibration_epochs``,
``psp9_calibration_samples``, ``psp9_context_checkpoints``,
``psp9_external_effects``, ``psp9_grant_policies``, ``psp9_verdicts``.
