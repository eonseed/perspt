.. Perspt Documentation master file

👁️ Perspt: Your Terminal's Window to the AI World 🤖
=====================================================

.. raw:: html

   <div align="center">
   <h3>
   <em>"The keyboard hums, the screen aglow,<br>
   AI's wisdom, a steady flow.<br>
   Will robots take over, it's quite the fright,<br>
   Or just provide insights, day and night?<br>
   We ponder and chat, with code as our guide,<br>
   Is AI our helper or our human pride?"</em>
   </h3>
   </div>

**Perspt** (pronounced "perspect," short for **Per**\ sonal **S**\ pectrum **P**\ ertaining **T**\ houghts) is a 
high-performance command-line interface (CLI) application that gives you a peek into the mind of Large Language Models (LLMs). 
Built with Rust for maximum speed and reliability, it allows you to chat with the latest AI models from multiple 
providers directly in your terminal using a modern, unified interface powered by the cutting-edge ``genai`` crate.

.. only:: html

   .. grid:: 2
      :gutter: 3

      .. grid-item-card:: 🚀 Quick Start
         :link: getting-started
         :link-type: doc
         
         Get up and running with Perspt in minutes. Install, configure, and start chatting with AI models.

      .. grid-item-card:: 📚 User Guide
         :link: user-guide/index
         :link-type: doc
         
         Complete guide to using Perspt effectively, from basic chat to advanced features.

      .. grid-item-card:: 🛠 Developer Guide
         :link: developer-guide/index
         :link-type: doc
         
         Deep dive into Perspt's architecture, contribute to the project, and extend functionality.

      .. grid-item-card:: 📖 API Reference
         :link: api/index
         :link-type: doc
         
         Comprehensive API documentation generated from source code comments.

.. only:: latex

   .. rubric:: Documentation Navigation

   * **🚀 Quick Start**: Get up and running with Perspt in minutes. Install, configure, and start chatting with AI models.
     (See chapter: :ref:`getting-started`)
   
   * **📚 User Guide**: Complete guide to using Perspt effectively, from basic chat to advanced features.
     (See chapter: :ref:`user-guide`)
   
   * **🛠 Developer Guide**: Deep dive into Perspt's architecture, contribute to the project, and extend functionality.
     (See chapter: :ref:`developer-guide`)
   
   * **📖 API Reference**: Comprehensive API documentation generated from source code comments.
     (See chapter: :ref:`api-reference`)

✨ Key Features
---------------

.. list-table::
   :widths: 20 80
   :header-rows: 0

   * - 🤖
     - **SRBN Agent Mode:** NEW! Autonomous coding assistant using Stabilized Recursive Barrier Networks - decomposes tasks, generates code, and verifies via LSP
   * - 🔬
     - **LSP Integration:** Real-time type checking via ``ty`` for Python with automatic error correction
   * - 🧪
     - **Test Runner:** Integrated pytest execution with V_log (Logic Energy) calculation from weighted tests
   * - 🎨
     - **Interactive Chat Interface:** A colorful and responsive chat interface powered by Ratatui
   * - 🖥️
     - **Simple CLI Mode:** Minimal command-line mode for direct Q&A, scripting, and accessibility
   * - ⚡
     - **Streaming Responses:** Real-time streaming of LLM responses for an interactive experience
   * - 🔀
     - **Multiple Provider Support:** Seamlessly switch between OpenAI, Anthropic, Google, Groq, Cohere, XAI, DeepSeek, and Ollama
   * - 📊
     - **Token Budget Tracking:** Configurable limits for tokens and cost control with usage monitoring
   * - 🔧
     - **Retry Policy:** PSP-4 compliant retry limits (3 for compilation, 5 for tools) with automatic escalation

🎯 Supported AI Providers
--------------------------

.. tabs::

   .. tab:: OpenAI

      - **GPT-4.1** - Latest and most advanced model
      - **GPT-4o series** - GPT-4o, GPT-4o-mini for fast responses
      - **o1 reasoning models** - o1-preview, o1-mini, o3-mini
      - **GPT-4 series** - GPT-4-turbo, GPT-4 for complex tasks
      - Latest model variants automatically supported

   .. tab:: Anthropic

      - Claude 3.5 (latest Sonnet, Haiku)
      - Claude 3 (Opus, Sonnet, Haiku)
      - Latest Claude models

   .. tab:: Google

      - **Gemini 2.5 Pro** - Latest multimodal model
      - Gemini Pro, Gemini 1.5 Pro/Flash
      - PaLM models

   .. tab:: Ollama (Local)

      - **Llama 3.2** - Latest Meta model
      - **CodeLlama** - Code-specialized models
      - **Mistral** - Fast and capable models
      - **Qwen** - Multilingual models
      - All popular open-source models

   .. tab:: Cloud Providers

      - **Groq**: Ultra-fast Llama 3.x inference
      - **Cohere**: Command R/R+ models
      - **XAI**: Grok models
      - **DeepSeek**: Advanced reasoning models

.. note::
   Perspt leverages the powerful `genai <https://crates.io/crates/genai>`_ crate for unified LLM access, 
   ensuring automatic support for new models and providers with cutting-edge features like reasoning model support.

📋 Perspt
---------

.. toctree::
   :maxdepth: 2
   :caption: Getting Started

   introduction
   getting-started
   installation
   configuration

.. toctree::
   :maxdepth: 2
   :caption: User Guide

   user-guide/index
   user-guide/basic-usage
   user-guide/advanced-features
   user-guide/providers
   user-guide/troubleshooting

.. toctree::
   :maxdepth: 2
   :caption: Developer Guide

   developer-guide/index
   developer-guide/architecture
   developer-guide/contributing
   developer-guide/extending
   developer-guide/testing

.. toctree::
   :maxdepth: 2
   :caption: API Reference

   api/index
   api/modules
   api/config
   api/llm-provider
   api/ui
   api/main

.. toctree::
   :maxdepth: 1
   :caption: Appendices

   changelog
   license
   acknowledgments

`Download as PDF <https://github.com/eonseed/perspt/raw/master/docs/perspt.pdf>`_

🔗 Quick Links
---------------

- **Repository:** `GitHub <https://github.com/eonseed/perspt>`_
- **Crates.io:** `perspt <https://crates.io/crates/perspt>`_
- **Issues:** `Bug Reports & Feature Requests <https://github.com/eonseed/perspt/issues>`_
- **Discussions:** `Community Chat <https://github.com/eonseed/perspt/discussions>`_

Indices and Tables
==================

* :ref:`genindex`
* :ref:`modindex`
* :ref:`search`

