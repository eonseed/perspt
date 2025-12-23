.. Perspt documentation master file

Perspt Documentation
====================

**Your Terminal's Window to the AI World** 🤖

Perspt is a high-performance terminal-based chat interface for Large Language Models
with autonomous coding capabilities powered by the SRBN (Stabilized Recursive Barrier Network) engine.

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

   .. grid-item-card:: 📖 Full Guide
      :link: getting-started
      :link-type: doc

      Comprehensive getting started guide.

Key Features
------------

- **SRBN Agent Mode** - Autonomous coding with Lyapunov stability (v0.5.0)
- **Multi-Provider** - OpenAI GPT-5.2, Anthropic Claude Opus 4.5, Google Gemini 3, Groq, Ollama, and more
- **LSP Integration** - Real-time type checking for Python
- **Token Budget** - Cost control with usage monitoring
- **Beautiful TUI** - Markdown rendering, streaming, scrollable history
- **Extensible Architecture** - Modular Cargo workspace design

----

.. toctree::
   :maxdepth: 2
   :caption: Get Started
   :hidden:

   introduction
   quickstart
   installation
   getting-started

.. toctree::
   :maxdepth: 2
   :caption: Tutorials (Learning Tasks)
   :hidden:

   tutorials/index
   tutorials/first-chat
   tutorials/agent-mode
   tutorials/local-models

.. toctree::
   :maxdepth: 2
   :caption: User Guide
   :hidden:

   user-guide/index
   user-guide/basic-usage
   user-guide/advanced-features
   user-guide/agent-mode
   user-guide/providers
   user-guide/troubleshooting

.. toctree::
   :maxdepth: 2
   :caption: Concepts (Supportive Info)
   :hidden:

   concepts/index
   concepts/srbn-architecture
   concepts/workspace-crates
   developer-guide/architecture

.. toctree::
   :maxdepth: 2
   :caption: How-To Guides (Procedural)
   :hidden:

   howto/index
   howto/configuration
   howto/providers
   howto/agent-options
   configuration
   developer-guide/contributing
   developer-guide/extending

.. toctree::
   :maxdepth: 2
   :caption: Reference (Part-task Practice)
   :hidden:

   reference/index
   reference/cli-reference
   reference/troubleshooting
   developer-guide/testing
   api/index

.. toctree::
   :maxdepth: 1
   :caption: Developer Guide
   :hidden:

   developer-guide/index
   developer-guide/architecture
   developer-guide/contributing
   developer-guide/extending
   developer-guide/testing

.. toctree::
   :maxdepth: 1
   :caption: API Reference
   :hidden:

   api/index
   api/config
   api/llm-provider
   api/main
   api/ui
   api/modules

.. toctree::
   :maxdepth: 1
   :caption: Appendices
   :hidden:

   changelog
   license
   acknowledgments

Quick Links
-----------

- `GitHub <https://github.com/eonseed/perspt>`_
- `Crates.io <https://crates.io/crates/perspt>`_
- `PSP Process <https://github.com/eonseed/perspt/tree/master/docs/psps>`_
- `Issues <https://github.com/eonseed/perspt/issues>`_

Indices
-------

* :ref:`genindex`
* :ref:`modindex`
* :ref:`search`
