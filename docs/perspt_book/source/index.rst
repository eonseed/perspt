.. Perspt documentation master file

Perspt Documentation
====================

**Your Terminal's Window to the AI World**

Perspt is a high-performance terminal-based LLM interface that serves two purposes:
a **simple CLI for testing and comparing LLM services** across 8 providers, and an
**experimental implementation** of the **SRBN (Stabilized Recursive Barrier Network)**
engine from the paper *"Stability is All You Need: Lyapunov-Guided Hierarchies for
Long-Horizon LLM Reliability"* by **Vikrant R. and Ronak R.** (pre-publication).
The SRBN agent plans multi-file projects as directed acyclic graphs, verifies each
node with real LSP diagnostics and tests, and commits only when Lyapunov energy
converges. The theoretical framework is mature; the implementation is under active
development.

.. only:: html

   .. grid:: 3
      :gutter: 3

      .. grid-item-card:: Quick Start
         :link: quickstart
         :link-type: doc

         Install and chat in 5 minutes.

      .. grid-item-card:: Agent Mode
         :link: tutorials/agent-mode
         :link-type: doc

         Autonomous multi-file coding with the experimental SRBN engine.

      .. grid-item-card:: Dashboard
         :link: user-guide/dashboard
         :link-type: doc

         Monitor agent execution in real-time via browser.

      .. grid-item-card:: Architecture
         :link: developer-guide/architecture
         :link-type: doc

         8-crate workspace design.

   Key Features
   ------------

   .. list-table::
      :widths: 5 95
      :class: borderless

      * - **SRBN Agent**
        - Experimental autonomous multi-file coding guided by Lyapunov energy, ownership closure, and sheaf validation (based on SRBN paper)
      * - **Multi-Provider**
        - OpenAI, Anthropic, Google Gemini, Groq, Cohere, XAI, DeepSeek, Ollama
      * - **LSP Sensors**
        - Real-time type checking via ``rust-analyzer``, ``ty``, ``pyright``, ``typescript-language-server``, ``gopls``
      * - **Test Runner**
        - pytest integration with weighted V_log energy
      * - **Per-Tier Models**
        - Assign different models to Architect, Actuator, Verifier, and Speculator tiers
      * - **Token Budget**
        - Cost control with usage monitoring and per-request limits
      * - **Beautiful TUI**
        - Ratatui-based with diff viewer, task tree, dashboard, and review modal
      * - **Security**
        - Starlark policy engine with command sanitization and workspace-bound enforcement
      * - **Merkle Ledger**
        - Cryptographic change tracking with session resume and rollback
      * - **Headless Mode**
        - Fully autonomous operation with ``--yes`` for CI/CD and batch workflows

   ----

.. toctree::
   :maxdepth: 2
   :caption: Getting Started

   introduction
   quickstart
   installation
   getting-started

.. toctree::
   :maxdepth: 2
   :caption: Tutorials

   tutorials/index

.. toctree::
   :maxdepth: 2
   :caption: User Guide

   user-guide/index

.. toctree::
   :maxdepth: 2
   :caption: Concepts

   concepts/index

.. toctree::
   :maxdepth: 2
   :caption: How-To Guides

   howto/index
   configuration

.. toctree::
   :maxdepth: 2
   :caption: Reference

   reference/index

.. toctree::
   :maxdepth: 2
   :caption: API Reference

   api/index

.. toctree::
   :maxdepth: 2
   :caption: Developer Guide

   developer-guide/index

.. toctree::
   :maxdepth: 1
   :caption: Appendices

   changelog
   license
   acknowledgments

.. only:: html

   Quick Links
   -----------

   - `GitHub Repository <https://github.com/eonseed/perspt>`_
   - `Crates.io <https://crates.io/crates/perspt>`_
   - `PSP Process <https://github.com/eonseed/perspt/tree/master/docs/psps>`_
   - `Issue Tracker <https://github.com/eonseed/perspt/issues>`_

Indices
-------

* :ref:`genindex`
* :ref:`modindex`
* :ref:`search`
