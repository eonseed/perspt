.. _howto-providers:

Set Up Providers
================

OpenAI
------

.. code-block:: bash

   export OPENAI_API_KEY="sk-xxx"
   perspt chat --model gpt-4.1

Supported models: ``gpt-4.1``, ``gpt-4.1-mini``, ``gpt-4.1-nano``, ``o4-mini``,
``o3``, and others listed by ``perspt --list-models``.


Anthropic
---------

.. code-block:: bash

   export ANTHROPIC_API_KEY="sk-ant-xxx"
   perspt chat --model claude-sonnet-4-20250514

Supported models: ``claude-sonnet-4-20250514``, ``claude-opus-4-20250514`` and others.


Google Gemini
-------------

.. code-block:: bash

   export GEMINI_API_KEY="AIza..."
   perspt chat --model gemini-3.1-flash-lite-preview

Supported models: ``gemini-pro-latest``, ``gemini-3.1-flash-lite-preview``, ``gemini-2.0-flash``,
and others.


Groq
----

.. code-block:: bash

   export GROQ_API_KEY="gsk_xxx"
   perspt chat --model llama-3.3-70b-versatile

Groq provides ultra-fast inference for open-source models.


Cohere
------

.. code-block:: bash

   export COHERE_API_KEY="xxx"
   perspt chat --model command-r-plus


xAI
---

.. code-block:: bash

   export XAI_API_KEY="xxx"
   perspt chat --model grok-3-mini-fast


DeepSeek
--------

.. code-block:: bash

   export DEEPSEEK_API_KEY="xxx"
   perspt chat --model deepseek-chat


Ollama (Local)
--------------

No API key needed. Ollama is the fallback when no cloud keys are set.

.. code-block:: bash

   # Start Ollama
   ollama serve

   # Pull a model
   ollama pull llama3.2

   # Use with Perspt
   perspt chat --model llama3.2

Multiple concurrent models are supported. Use ``ollama list`` to see installed models.
