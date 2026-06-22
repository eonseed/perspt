.. _howto-providers:

Set Up Providers
================

OpenAI
------

.. code-block:: bash

   export OPENAI_API_KEY="sk-xxx"
   perspt chat --model gpt-5.5

Supported models: ``gpt-5.5``, ``gpt-5-mini``, ``o1``, and any other model your provider account exposes.


Anthropic
---------

.. code-block:: bash

   export ANTHROPIC_API_KEY="sk-ant-xxx"
   perspt chat --model claude-fable

Supported models: ``claude-fable``, ``opus-4.8`` and others.


Google Gemini
-------------

.. code-block:: bash

   export GEMINI_API_KEY="AIza..."
   perspt chat --model gemini-3.5-flash

Supported models: ``gemini-3.1-pro``, ``gemini-3.5-flash``, ``gemini-2.5-pro``,
and others.


Google Vertex AI
----------------

Vertex AI is supported through Google Cloud authentication. You must configure your Google Cloud project ID and optionally the region.

.. code-block:: bash

   export VERTEX_PROJECT_ID="my-gcp-project-id"
   export VERTEX_REGION="us-central1"
   # Run using the Vertex model prefix
   perspt chat --model vertex::gemini-3.5-flash

Supported models: ``vertex::gemini-3.1-pro``, ``vertex::gemini-3.5-flash``, and other models enabled in your Vertex project.


Groq
----

.. code-block:: bash

   export GROQ_API_KEY="gsk_xxx"
   perspt chat --model llama-4-70b

Groq provides ultra-fast inference for open-source models.


Cohere
------

.. code-block:: bash

   export COHERE_API_KEY="xxx"
   perspt chat --model command-r7


xAI
---

.. code-block:: bash

   export XAI_API_KEY="xxx"
   perspt chat --model grok-4


DeepSeek
--------

.. code-block:: bash

   export DEEPSEEK_API_KEY="xxx"
   perspt chat --model deepseek-v4


Ollama (Local)
--------------

No API key needed. Ollama is the fallback when no cloud keys are set.

.. code-block:: bash

   # Start Ollama
   ollama serve

   # Pull a model
   ollama pull llama4

   # Use with Perspt
   perspt chat --model llama4

Multiple concurrent models are supported. Use ``ollama list`` to see installed models.
