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

   .. grid-item-card:: perspt-agent
      :link: perspt-agent
      :link-type: doc

      SRBN orchestrator, agents, ledger, tools.

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

.. toctree::
   :maxdepth: 2
   :hidden:

   perspt-core
   perspt-agent
   perspt-tui
   perspt-cli
   perspt-store
   perspt-policy
   perspt-sandbox
