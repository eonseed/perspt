.. _api-perspt-benchmark:

``perspt-benchmark``
====================

Optional PSP-10 evaluation tooling, independent of runtime mechanism tests.
Live runs require configured routes and credentials and never run in CI. The
crate is feature-gated behind ``perspt-cli``'s ``benchmark`` feature and
surfaced as ``perspt benchmark`` (``validate``/``run``/``aggregate``).

Core Types
----------

.. code-block:: rust

   pub enum BenchmarkSuite {
       Smoke,     // One production-topology arm over a small task prefix
       Adaptive,  // The paging/adaptive pair for the default-activation decision
       Full,      // The complete seven-arm diagnostic ladder
   }

   pub struct BenchmarkRunOptions {
       pub config_path: Option<PathBuf>,
       pub suite: BenchmarkSuite,
       pub task_limit: Option<usize>,
       pub output: Option<PathBuf>,
   }

Functions
---------

.. list-table::
   :header-rows: 1
   :widths: 40 60

   * - Function
     - Description
   * - ``validate_corpus_command()``
     - Credential-free corpus validation: structure, coverage floors, and
       fail-before/pass-after hidden oracles
   * - ``run_benchmark(options)``
     - Run a suite using the configured production topology; model names
       are never benchmark arguments
   * - ``aggregate_reports(paths)``
     - Aggregate two or more completed reports (also credential-free)

Corpus and Arms
---------------

Tasks live in ``corpus/<id>/``: a ``task.json`` (goal, hidden check argv,
tags, expectation), a ``fixture/`` tree the agent works in, a ``hidden/``
tree of withheld oracle files, and a ``solution/`` overlay used only by
offline corpus validation. The hidden suite runs in a fresh copy of the
post-run fixture with ``hidden/`` overlaid on top, so the oracle and
solution are genuinely unseen during evaluation.

Arms are a cumulative ladder over matched tasks: ``direct``, ``governed``,
``packets``, ``paging``, ``adaptive``, ``multi-family``, ``integration``.
The report publishes paired differences with a seeded 10,000-resample
percentile bootstrap; the primary outcome is hidden-test hard pass.
