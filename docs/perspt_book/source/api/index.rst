.. _api-index:

API Reference
=============

Crate-level API documentation for Perspt's Rust workspace.

.. tip::

   For full Rustdoc-generated documentation, run:

   .. code-block:: bash

      cargo doc --open --no-deps --all-features

.. grid:: 2
   :gutter: 3

   .. grid-item-card:: perspt-core
      :link: perspt-core
      :link-type: doc

      Types, config, LLM provider, events, plugins.

   .. grid-item-card:: perspt-sdk
      :link: perspt-sdk
      :link-type: doc

      Domain-neutral SRBN control plane: energy, gates, ledger.

   .. grid-item-card:: perspt-coding
      :link: perspt-coding
      :link-type: doc

      Coding domain package: residuals, adapters, barriers.

   .. grid-item-card:: perspt-research
      :link: perspt-research
      :link-type: doc

      Research domain package (skeleton).

   .. grid-item-card:: perspt-agent
      :link: perspt-agent
      :link-type: doc

      Governed candidate runtime, tool loop, verifier, work-graph dispatch.

   .. grid-item-card:: perspt-tui
      :link: perspt-tui
      :link-type: doc

      Ratatui terminal UI (chat + agent).

   .. grid-item-card:: perspt-cli
      :link: perspt-cli
      :link-type: doc

      Clap CLI entry point and subcommands.

   .. grid-item-card:: perspt-store
      :link: perspt-store
      :link-type: doc

      DuckDB session persistence.

   .. grid-item-card:: perspt-policy
      :link: perspt-policy
      :link-type: doc

      Starlark policy engine.

   .. grid-item-card:: perspt-sandbox
      :link: perspt-sandbox
      :link-type: doc

      Command sandboxing and isolation.

   .. grid-item-card:: perspt-dashboard
      :link: perspt-dashboard
      :link-type: doc

      Axum + Askama + HTMX web dashboard.

   .. grid-item-card:: perspt-prompt-macros
      :link: perspt-prompt-macros
      :link-type: doc

      Build-time compiler for typed prompt section libraries.

   .. grid-item-card:: perspt-benchmark
      :link: perspt-benchmark
      :link-type: doc

      Optional, feature-gated manual evaluation tooling.

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
