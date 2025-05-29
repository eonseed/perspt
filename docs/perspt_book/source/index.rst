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
blazing-fast command-line interface (CLI) application that gives you a peek into the mind of Large Language Models (LLMs). 
Built with Rust for maximum performance and reliability, it allows you to chat with various AI models from multiple 
providers directly in your terminal using a unified, beautiful interface.

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

   .. grid-item-card:: 🛠️ Developer Guide
      :link: developer-guide/index
      :link-type: doc

      Deep dive into Perspt's architecture, contribute to the project, and extend functionality.

   .. grid-item-card:: 📖 API Reference
      :link: api/index
      :link-type: doc

      Comprehensive API documentation generated from source code comments.

✨ Key Features
---------------

.. list-table::
   :widths: 20 80
   :header-rows: 0

   * - 🎨
     - **Interactive Chat Interface:** A colorful and responsive chat interface powered by Ratatui
   * - ⚡
     - **Streaming Responses:** Real-time streaming of LLM responses for an interactive experience
   * - 🔀
     - **Multiple Provider Support:** Seamlessly switch between OpenAI, AWS Bedrock, Anthropic, Google, and more
   * - 🚀
     - **Dynamic Model Discovery:** Automatically discovers available models without manual updates
   * - ⚙️
     - **Configurable:** Flexible configuration via JSON files or command-line arguments
   * - 🔄
     - **Input Queuing:** Type new questions while AI is responding - inputs are queued and processed sequentially
   * - 📜
     - **Markdown Rendering:** Beautiful markdown support directly in the terminal
   * - 🛡️
     - **Graceful Error Handling:** Robust handling of network issues, API errors, and edge cases

🎯 Supported AI Providers
--------------------------

.. tabs::

   .. tab:: OpenAI

      - GPT-4, GPT-4-turbo, GPT-4o series
      - GPT-3.5-turbo models
      - Latest model variants automatically supported

   .. tab:: AWS Bedrock

      - Amazon Nova models
      - Anthropic Claude on Bedrock
      - Automatic model discovery

   .. tab:: Anthropic

      - Claude 3 Opus, Sonnet, Haiku
      - Latest Claude models

   .. tab:: Google

      - Gemini Pro, Gemini Ultra
      - PaLM models

   .. tab:: Others

      - Mistral AI models
      - Perplexity AI
      - DeepSeek models
      - And more via the allms crate

.. note::
   Perspt leverages the powerful `allms <https://crates.io/crates/allms>`_ crate for unified LLM access, 
   ensuring automatic support for new models and providers without manual updates.

📋 Table of Contents
--------------------

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
   acknowledgments
   license
   acknowledgments

🔗 Quick Links
---------------

- **Repository:** `GitHub <https://github.com/yourusername/perspt>`_
- **Crates.io:** `perspt <https://crates.io/crates/perspt>`_
- **Issues:** `Bug Reports & Feature Requests <https://github.com/yourusername/perspt/issues>`_
- **Discussions:** `Community Chat <https://github.com/yourusername/perspt/discussions>`_

Indices and Tables
==================

* :ref:`genindex`
* :ref:`modindex`
* :ref:`search`

