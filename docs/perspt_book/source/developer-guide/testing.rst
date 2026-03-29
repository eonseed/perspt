.. _developer-guide-testing:

Testing
=======

Test Infrastructure
-------------------

Perspt uses Rust's standard test framework with ``#[tokio::test]`` for async tests.

.. code-block:: bash

   # Run all tests
   cargo test

   # Run tests for a specific crate
   cargo test -p perspt-core
   cargo test -p perspt-agent
   cargo test -p perspt-store

   # Run a specific test
   cargo test test_name

   # Run with output
   cargo test -- --nocapture


Test Organization
-----------------

.. list-table::
   :header-rows: 1
   :widths: 30 30 40

   * - Location
     - Type
     - Description
   * - ``crates/*/src/*.rs``
     - Unit tests
     - ``#[cfg(test)] mod tests`` blocks
   * - ``tests/``
     - Integration tests
     - Cross-crate integration
   * - ``crates/perspt-store/``
     - Store tests
     - DuckDB schema, CRUD, ledger


Test Patterns
-------------

**In-Memory Store:**

Use ``SessionStore::in_memory()`` for tests that need persistence without disk I/O:

.. code-block:: rust

   #[tokio::test]
   async fn test_ledger_commit() {
       let store = SessionStore::in_memory().unwrap();
       let ledger = MerkleLedger::from_store(store);
       // ... test ledger operations
   }

**Plugin Testing:**

Test plugin detection and verifier profiles:

.. code-block:: rust

   #[test]
   fn test_python_plugin_detection() {
       let plugin = PythonPlugin;
       let dir = tempdir().unwrap();
       std::fs::write(dir.path().join("pyproject.toml"), "").unwrap();
       assert!(plugin.detect(dir.path()));
   }

**Energy Computation:**

Test energy calculation with known inputs:

.. code-block:: rust

   #[test]
   fn test_energy_total() {
       let energy = EnergyComponents {
           v_syn: 1.0,
           v_str: 0.0,
           v_log: 2.0,
           v_boot: 0.0,
           v_sheaf: 0.0,
       };
       let contract = BehavioralContract {
           energy_weights: (1.0, 0.5, 1.0),
           ..Default::default()
       };
       assert_eq!(energy.total(&contract), 3.0);
   }

**Event Testing:**

Test event channel communication:

.. code-block:: rust

   #[tokio::test]
   async fn test_event_flow() {
       let (tx, mut rx) = event_channel();
       tx.send(AgentEvent::Log { message: "test".into() }).unwrap();
       let event = rx.recv().await.unwrap();
       assert!(matches!(event, AgentEvent::Log { .. }));
   }


Quality Gates
-------------

All PRs must pass:

.. code-block:: bash

   cargo build                    # Compile
   cargo test                     # All tests
   cargo clippy -- -D warnings    # No warnings
   cargo fmt -- --check           # Formatted

The project currently has 239+ tests across all crates.


Panic Safety
------------

``main`` installs a panic hook that:

1. Restores terminal (raw mode off, leave alternate screen)
2. Prints the panic message with guidance
3. Exits cleanly

Test this with ``tests/panic_handling_test.rs``.
