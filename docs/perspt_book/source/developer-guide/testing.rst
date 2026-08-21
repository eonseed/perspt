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
   * - ``crates/perspt-agent/tests/``
     - Mechanism checks
     - 23 ``mc_*.rs`` suites plus ``psp9_runtime.rs`` covering the
       PSP-9/PSP-10 runtime mechanisms (tool loop, dispatch, search,
       recovery)
   * - ``crates/perspt-sdk/tests/``
     - Mechanism checks
     - ``mechanism_checks.rs`` and ``mc_prompt_activation.rs`` for the
       platform SDK


Test Patterns
-------------

**Temporary Store:**

Use ``SessionStore::open()`` against a temporary directory for tests that
need persistence without touching the user database:

.. code-block:: rust

   #[test]
   fn test_session_roundtrip() {
       let dir = tempdir().unwrap();
       let store = SessionStore::open(&dir.path().join("test.db")).unwrap();
       store
           .create_session(&SessionRecord {
               session_id: "s1".into(),
               task: "demo".into(),
               working_dir: "/tmp/demo".into(),
               merkle_root: None,
               detected_toolchain: None,
               status: "active".into(),
           })
           .unwrap();
       assert!(store.get_session("s1").unwrap().is_some());
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
       assert_eq!(energy.total(), 3.0);
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

   cargo fmt --all -- --check
   cargo clippy --locked --workspace --all-targets -- -D warnings
   cargo test --locked --workspace --all-targets -- --test-threads=1
   cargo check --locked -p perspt-cli --features benchmark
   cargo doc --locked --workspace --no-deps
   ./check-rules.sh check

The PR gate runs once on Ubuntu. It downloads the official DuckDB 1.5.5 shared
library, verifies its published checksum, and compiles the optional
``benchmark`` CLI without running live model evaluation. Release builds, not
CI tests, use the expensive ``bundled`` feature. A merge-queue commit,
protected-branch push, or manual full run adds Windows and macOS tests.
Platform-neutral formatting, Clippy, Rustdoc, and PSP rules are not repeated
on every operating system. macOS exercises the native governed verifier.
Windows covers native chat/TUI and shared code, verifies strict coding mode
fails closed, and separately exercises the explicit reduced-isolation verifier.

GitHub merge queues are the pre-merge path: the ``merge_group`` event checks
the proposed change combined with the latest target branch and any changes
ahead of it in the queue. Without an enabled merge-queue ruleset, maintainers
must use the CI workflow's manual full run when pre-merge cross-platform
evidence is required; the protected-branch push remains a post-merge fallback.

The project currently has over 800 tests across all crates.


Benchmarks Are Not Tests
------------------------

``perspt benchmark`` is not part of ``cargo test`` or CI. The
``perspt-benchmark`` crate is gated behind the non-default ``benchmark``
feature of ``perspt-cli``, requires configured model credentials, and is run
manually to evaluate and compare Perspt against other coding agents. Its own
unit tests are credential-free and run with the ordinary workspace suite.


Panic Safety
------------

``main`` installs a panic hook that:

1. Restores terminal (raw mode off, leave alternate screen)
2. Prints the panic message with guidance
3. Exits cleanly

Test this with ``tests/panic_handling_test.rs``.
