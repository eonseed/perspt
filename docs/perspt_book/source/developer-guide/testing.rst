.. _developer-guide-testing:

Testing
=======

Testing strategies for Perspt development.

Running Tests
-------------

.. code-block:: bash

   # All tests
   cargo test --all

   # Specific crate
   cargo test -p perspt-agent

   # Specific test
   cargo test test_name

   # With output
   cargo test -- --nocapture

Test Organization
-----------------

Each crate has its own tests:

.. code-block:: text

   crates/
   └── perspt-agent/
       └── src/
           ├── orchestrator.rs
           └── orchestrator_test.rs  # Unit tests
       └── tests/
           └── integration_test.rs   # Integration tests

Unit Tests
----------

Test individual functions in the same file:

.. code-block:: rust

   // orchestrator.rs

   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn test_energy_calculation() {
           let energy = Energy::compute(
               &[Diagnostic::error("test")],
               &TestResults::default(),
               1.0, 0.5, 2.0,
           );
           assert!(energy.v_syn > 0.0);
       }

       #[tokio::test]
       async fn test_sheafification() {
           let plan = TaskPlan::from_prompt("Create file").await.unwrap();
           assert!(!plan.nodes.is_empty());
       }
   }

Integration Tests
-----------------

Test crate interactions in ``tests/``:

.. code-block:: rust

   // tests/integration_test.rs

   use perspt_agent::SRBNOrchestrator;
   use perspt_core::GenAIProvider;
   use std::sync::Arc;

   #[tokio::test]
   async fn test_full_workflow() {
       let provider = Arc::new(GenAIProvider::new().unwrap());
       let mut orchestrator = SRBNOrchestrator::new(
           provider,
           "./test_workspace".into(),
           Default::default(),
       ).await.unwrap();

       let result = orchestrator.execute("Create hello.py").await;
       assert!(result.is_ok());
   }

Mocking
-------

Use mockall for mocking:

.. code-block:: rust

   use mockall::mock;

   mock! {
       pub LspClient {
           async fn get_diagnostics(&self, path: &Path) -> Result<Vec<Diagnostic>>;
       }
   }

   #[tokio::test]
   async fn test_with_mock_lsp() {
       let mut mock_lsp = MockLspClient::new();
       mock_lsp
           .expect_get_diagnostics()
           .returning(|_| Ok(vec![]));
       
       // Use mock in test
   }

Coverage
--------

.. code-block:: bash

   # Install tarpaulin
   cargo install cargo-tarpaulin

   # Run with coverage
   cargo tarpaulin --out Html

   # Open report
   open tarpaulin-report.html

Documentation Tests
-------------------

Test code examples in docs:

.. code-block:: bash

   cargo test --doc

Benchmarks
----------

Performance benchmarks in ``benches/``:

.. code-block:: rust

   // benches/energy_bench.rs

   use criterion::{criterion_group, criterion_main, Criterion};
   use perspt_agent::Energy;

   fn energy_benchmark(c: &mut Criterion) {
       c.bench_function("energy_compute", |b| {
           b.iter(|| Energy::compute(&[], &Default::default(), 1.0, 0.5, 2.0))
       });
   }

   criterion_group!(benches, energy_benchmark);
   criterion_main!(benches);

.. code-block:: bash

   cargo bench

CI Testing
----------

Tests run in GitHub Actions on every PR:

.. code-block:: yaml

   # .github/workflows/test.yml
   name: Tests
   on: [push, pull_request]
   jobs:
     test:
       runs-on: ubuntu-latest
       steps:
         - uses: actions/checkout@v4
         - uses: dtolnay/rust-toolchain@stable
         - run: cargo test --all

See Also
--------

- :doc:`contributing` - How to contribute
- :doc:`architecture` - Crate design
