perspt-store API
================

DuckDB-based persistence layer for SRBN session management and LLM logging.

Overview
--------

``perspt-store`` provides persistent storage for:

- **Session management** — Track agent sessions, status, and metadata
- **LLM logging** — Record all requests/responses with latency and token counts
- **Energy history** — Store Lyapunov energy over time for analysis
- **Node state** — Snapshot SRBN node states for rollback

SessionStore
------------

Main interface for session persistence:

.. code-block:: rust

   pub struct SessionStore {
       conn: Connection,
   }

   impl SessionStore {
       /// Create a new store at the given database path
       pub fn new(db_path: &Path) -> Result<Self>
       
       /// Create a new session
       pub fn create_session(&self, task: &str, workspace: &str) -> Result<String>
       
       /// Update session status
       pub fn update_session_status(&self, session_id: &str, status: &str) -> Result<()>
       
       /// Record an LLM request/response
       pub fn record_llm_request(
           &self,
           session_id: &str,
           model: &str,
           prompt: &str,
           response: &str,
           latency_ms: i32,
           tokens_in: i32,
           tokens_out: i32,
           node_id: Option<&str>,
       ) -> Result<()>
       
       /// Get LLM requests for a session
       pub fn get_llm_requests(&self, session_id: &str) -> Result<Vec<LlmRequestRecord>>
       
       /// List recent sessions
       pub fn list_recent_sessions(&self, limit: usize) -> Result<Vec<SessionRecord>>
   }

Record Types
------------

SessionRecord
~~~~~~~~~~~~~

.. code-block:: rust

   pub struct SessionRecord {
       pub session_id: String,
       pub task: String,
       pub workspace: String,
       pub status: String,
       pub created_at: String,
       pub updated_at: String,
   }

LlmRequestRecord
~~~~~~~~~~~~~~~~

.. code-block:: rust

   pub struct LlmRequestRecord {
       pub id: i32,
       pub session_id: String,
       pub model: String,
       pub prompt: String,
       pub response: String,
       pub latency_ms: i32,
       pub tokens_in: i32,
       pub tokens_out: i32,
       pub node_id: Option<String>,
       pub created_at: String,
   }

EnergyRecord
~~~~~~~~~~~~

.. code-block:: rust

   pub struct EnergyRecord {
       pub session_id: String,
       pub node_id: String,
       pub v_syn: f32,
       pub v_str: f32,
       pub v_log: f32,
       pub total: f32,
       pub created_at: String,
   }

NodeStateRecord
~~~~~~~~~~~~~~~

.. code-block:: rust

   pub struct NodeStateRecord {
       pub session_id: String,
       pub node_id: String,
       pub state: String,  // JSON serialized
       pub created_at: String,
   }

Schema Initialization
---------------------

.. code-block:: rust

   /// Initialize schema in an existing DuckDB connection
   pub fn init_schema(conn: &Connection) -> Result<()>

Database Location
-----------------

By default, ``perspt-store`` uses:

- **macOS**: ``~/Library/Application Support/perspt/sessions.duckdb``
- **Linux**: ``~/.local/share/perspt/sessions.duckdb``
- **Windows**: ``%APPDATA%\perspt\sessions.duckdb``

Usage Example
-------------

.. code-block:: rust

   use perspt_store::SessionStore;
   use std::path::Path;

   fn main() -> Result<()> {
       let store = SessionStore::new(Path::new("sessions.duckdb"))?;
       
       // Create a session
       let session_id = store.create_session(
           "Build REST API",
           "/path/to/project"
       )?;
       
       // Log LLM request
       store.record_llm_request(
           &session_id,
           "gpt-5.2",
           "Generate a REST API handler",
           "```rust\nfn handler()...",
           1523,  // latency_ms
           150,   // tokens_in
           420,   // tokens_out
           Some("node-1"),
       )?;
       
       // List sessions
       let sessions = store.list_recent_sessions(10)?;
       
       Ok(())
   }

Source Code
-----------

- ``crates/perspt-store/src/lib.rs``
- ``crates/perspt-store/src/store.rs``
- ``crates/perspt-store/src/schema.rs``

See Also
--------

- :doc:`perspt-agent` - SRBN orchestrator using perspt-store
- :doc:`../reference/cli-reference` - ``perspt logs`` command
