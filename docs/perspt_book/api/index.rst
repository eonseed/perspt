.. _api-index:

API Reference
=============

Crate-level API documentation for Perspt's Rust workspace.

.. tip::

   For full Rustdoc-generated documentation, run:

   .. code-block:: bash

      cargo doc --open --workspace --no-deps

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Crate
     - Responsibility
   * - :doc:`perspt-core`
     - Types, config, LLM provider, events, plugins.
   * - :doc:`perspt-sdk`
     - Domain-neutral SRBN control plane: energy, gates, ledger.
   * - :doc:`perspt-coding`
     - Coding domain package: residuals, adapters, barriers.
   * - :doc:`perspt-research`
     - Research domain package (second domain proof).
   * - :doc:`perspt-agent`
     - Governed candidate runtime, tool loop, verifier, work-graph dispatch.
   * - :doc:`perspt-tui`
     - Ratatui terminal UI (chat + agent).
   * - :doc:`perspt-cli`
     - Clap CLI entry point and subcommands.
   * - :doc:`perspt-store`
     - DuckDB session persistence.
   * - :doc:`perspt-policy`
     - Starlark policy engine.
   * - :doc:`perspt-sandbox`
     - Command sandboxing and isolation.
   * - :doc:`perspt-dashboard`
     - Axum + Askama + HTMX web dashboard.
   * - :doc:`perspt-prompt-macros`
     - Build-time compiler for typed prompt section libraries.
   * - :doc:`perspt-benchmark`
     - Optional, feature-gated manual evaluation tooling.

.. toctree::
   :maxdepth: 2
   :hidden:

   perspt-core
   perspt-sdk
   perspt-coding
   perspt-research
   perspt-agent
   perspt-tui
   perspt-cli
   perspt-store
   perspt-policy
   perspt-sandbox
   perspt-dashboard
   perspt-prompt-macros
   perspt-benchmark
