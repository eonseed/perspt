.. _tutorial-local-models:

Local Models with Ollama
========================

Run Perspt with local models for privacy and offline usage.

Prerequisites
-------------

- `Ollama <https://ollama.ai>`_ installed
- Sufficient RAM for your chosen model (7B models: 8GB+, 70B models: 64GB+)

Setup
-----

.. code-block:: bash

   # Install Ollama (macOS)
   brew install ollama

   # Start the Ollama service
   ollama serve

   # Pull a model
   ollama pull llama3.2
   ollama pull codellama  # For coding tasks

Using Ollama with Perspt
-------------------------

Perspt auto-detects Ollama if no cloud API keys are set:

.. code-block:: bash

   # Unset any cloud keys
   unset OPENAI_API_KEY ANTHROPIC_API_KEY GEMINI_API_KEY

   # Launch Perspt - auto-detects Ollama
   perspt

   # Or specify a model explicitly
   perspt chat --model llama3.2

Agent Mode with Local Models
-----------------------------

.. code-block:: bash

   perspt agent --model qwen2.5-coder -w ./my-project "Create a Python utility"

.. admonition:: Performance Note
   :class: note

   Local models are slower and less capable than cloud models for complex agent
   tasks. For best results with agent mode, use a capable model (70B+ parameters)
   or use cloud models for the Architect and Verifier tiers:

   .. code-block:: bash

      export GEMINI_API_KEY="your-key"
      perspt agent \
        --architect-model gemini-3.1-pro \
        --actuator-model qwen2.5-coder \
        -w ./project "Create a utility"

Available Models
----------------

Popular Ollama models for use with Perspt:

.. list-table::
   :header-rows: 1
   :widths: 25 15 60

   * - Model
     - Size
     - Best For
   * - ``llama3.3``
     - 8B/70B
     - General chat
   * - ``qwen2.5-coder``
     - 7B/32B
     - Code generation
   * - ``deepseek-coder-v2``
     - 16B/236B
     - Code generation
   * - ``mistral``
     - 7B
     - General purpose
   * - ``phi4``
     - 14B
     - Reasoning and lightweight tasks
