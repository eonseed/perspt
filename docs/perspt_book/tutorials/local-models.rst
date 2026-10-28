.. _tutorial-local-models:

Local Models with Ollama
========================

Run Perspt with local models for privacy and offline usage.

Prerequisites
-------------

- `Ollama <https://ollama.ai>`_ installed
- Enough memory for the installed model and its context. Check the model's own requirements.

Setup
-----

.. code-block:: bash

   # Install Ollama (macOS)
   brew install ollama

   # Start the Ollama service
   ollama serve

   # Pull a model
   ollama pull llama3.2

Using Ollama with Perspt
-------------------------

Perspt auto-detects Ollama if no cloud API keys are set:

.. code-block:: bash

   # Unset any cloud keys
   unset OPENAI_API_KEY ANTHROPIC_API_KEY GEMINI_API_KEY

   # Launch Perspt - auto-detects Ollama
   perspt

   # Or specify a model explicitly
   perspt chat --model ollama::llama3.2

Agent Mode with Local Models
-----------------------------

.. code-block:: bash

   perspt agent --model ollama::qwen2.5-coder -w ./my-project "Create a Python utility"

**Agent mode needs tool-calling support.** A model that works in chat may not support the agent's structured tool calls.
Pull the selected model first, check that its route supports the required operations, and start with a small task.
An explicit ``ollama::`` prefix keeps the local route selected even when cloud credentials are present.

.. code-block:: bash

   ollama pull qwen2.5-coder
   perspt providers --probe

Provider probes make live requests to configured routes.
The example model IDs illustrate route syntax; installed models are listed by ``ollama list``.
