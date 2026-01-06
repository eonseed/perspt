.. Perspt documentation master file

Perspt Documentation
====================

**Your Terminal's Window to the AI World** 🤖

Perspt is a high-performance terminal-based LLM interface with autonomous coding capabilities 
powered by the **SRBN (Stabilized Recursive Barrier Network)** engine.

.. only:: html

   .. grid:: 3
      :gutter: 3

      .. grid-item-card:: 🚀 Quick Start
         :link: quickstart
         :link-type: doc

         Install and chat in 5 minutes.

      .. grid-item-card:: 🤖 Agent Mode
         :link: tutorials/agent-mode
         :link-type: doc

         Autonomous code generation with SRBN.

      .. grid-item-card:: 📖 Architecture
         :link: developer-guide/architecture
         :link-type: doc

         7-crate workspace design.

   Key Features
   ------------

   .. list-table::
      :widths: 5 95
      :class: borderless

      * - 🤖
        - **SRBN Agent Mode** — Autonomous coding with Lyapunov stability guarantees (v0.5.0)
      * - 🔌
        - **Multi-Provider** — OpenAI GPT-5.2, Claude Opus 4.5, Gemini 3, Groq, Ollama
      * - 🔬
        - **LSP Integration** — Real-time type checking via ``ty`` server
      * - 🧪
        - **Test Runner** — pytest integration with V_log energy
      * - 💰
        - **Token Budget** — Cost control with usage monitoring
      * - 🎨
        - **Beautiful TUI** — Ratatui-based with diff viewer and task tree
      * - 🔒
        - **Security** — Policy engine with command sanitization

   ----

.. toctree::
   :maxdepth: 2
   :caption: 📚 Getting Started

   introduction
   quickstart
   installation
   getting-started

.. toctree::
   :maxdepth: 2
   :caption: 🎓 Tutorials

   tutorials/index

.. toctree::
   :maxdepth: 2
   :caption: 📖 User Guide

   user-guide/index

.. toctree::
   :maxdepth: 2
   :caption: 💡 Concepts

   concepts/index

.. toctree::
   :maxdepth: 2
   :caption: 🔧 How-To Guides

   howto/index
   configuration

.. toctree::
   :maxdepth: 2
   :caption: 📋 Reference

   reference/index

.. toctree::
   :maxdepth: 2
   :caption: 🔌 API Reference

   api/index

.. toctree::
   :maxdepth: 2
   :caption: 🛠️ Developer Guide

   developer-guide/index

.. toctree::
   :maxdepth: 1
   :caption: 📎 Appendices

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

