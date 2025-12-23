.. _api-reference:

API Reference
=============

Complete API documentation for Perspt's 6-crate workspace architecture.

.. graphviz::
   :align: center
   :caption: Crate Overview

   digraph crates {
       rankdir=LR;
       node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=10];
       
       cli [label="perspt-cli", fillcolor="#4ECDC4", href="perspt-cli.html"];
       core [label="perspt-core", fillcolor="#45B7D1", href="perspt-core.html"];
       tui [label="perspt-tui", fillcolor="#96CEB4", href="perspt-tui.html"];
       agent [label="perspt-agent", fillcolor="#FFEAA7", href="perspt-agent.html"];
       policy [label="perspt-policy", fillcolor="#DDA0DD", href="perspt-policy.html"];
       sandbox [label="perspt-sandbox", fillcolor="#F8B739", href="perspt-sandbox.html"];
   }

.. toctree::
   :maxdepth: 2
   :caption: Crate APIs

   perspt-cli
   perspt-core
   perspt-agent
   perspt-tui
   perspt-policy
   perspt-sandbox

Crate Summary
-------------

.. list-table::
   :header-rows: 1
   :widths: 20 50 30

   * - Crate
     - Description
     - Key Types
   * - :doc:`perspt-cli`
     - CLI entry point with 8 subcommands
     - ``Commands``, ``Cli``
   * - :doc:`perspt-core`
     - LLM provider, config, memory
     - ``GenAIProvider``, ``Config``
   * - :doc:`perspt-agent`
     - SRBN engine for autonomous coding
     - ``SRBNOrchestrator``, ``TaskPlan``, ``Energy``
   * - :doc:`perspt-tui`
     - Terminal UI components
     - ``AgentApp``, ``Dashboard``, ``DiffViewer``
   * - :doc:`perspt-policy`
     - Security policy engine
     - ``PolicyEngine``, ``Sanitizer``
   * - :doc:`perspt-sandbox`
     - Process isolation
     - ``SandboxedCommand``

Architecture Quick Reference
----------------------------

.. grid:: 2
   :gutter: 3

   .. grid-item-card:: 🖥️ perspt-cli
      :link: perspt-cli
      :link-type: doc

      **8 Subcommands**: chat, agent, init, config, ledger, status, abort, resume

   .. grid-item-card:: 🔌 perspt-core
      :link: perspt-core
      :link-type: doc

      **Thread-safe LLM**: GenAIProvider with Arc<RwLock>

   .. grid-item-card:: 🤖 perspt-agent
      :link: perspt-agent
      :link-type: doc

      **SRBN Engine**: Orchestrator, LSP, Tools, Ledger

   .. grid-item-card:: 🎨 perspt-tui
      :link: perspt-tui
      :link-type: doc

      **Ratatui UI**: Dashboard, DiffViewer, ReviewModal

   .. grid-item-card:: 🛡️ perspt-policy
      :link: perspt-policy
      :link-type: doc

      **Security**: Starlark rules, command sanitization

   .. grid-item-card:: 📦 perspt-sandbox
      :link: perspt-sandbox
      :link-type: doc

      **Isolation**: Resource limits, process control

Common Patterns
---------------

Error Handling
~~~~~~~~~~~~~~

All crates use ``anyhow::Result`` for error propagation:

.. code-block:: rust

   use anyhow::{Context, Result};

   fn example() -> Result<()> {
       do_something()
           .context("Failed to do something")?;
       Ok(())
   }

Async Operations
~~~~~~~~~~~~~~~~

Built on Tokio async runtime:

.. code-block:: rust

   use tokio::sync::mpsc;

   async fn stream_response(sender: mpsc::Sender<String>) -> Result<()> {
       // Stream tokens as they arrive
   }

Thread-Safe Sharing
~~~~~~~~~~~~~~~~~~~

Use ``Arc`` for sharing across tasks:

.. code-block:: rust

   use std::sync::Arc;

   let provider = Arc::new(GenAIProvider::new()?);
   let provider_clone = Arc::clone(&provider);

   tokio::spawn(async move {
       provider_clone.generate_response(...).await
   });

See Also
--------

- :doc:`../developer-guide/architecture` - Workspace architecture overview
- :doc:`../developer-guide/contributing` - How to contribute
- :doc:`../developer-guide/testing` - Testing guide
