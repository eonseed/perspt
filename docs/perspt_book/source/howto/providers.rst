.. _howto-providers:

Provider Setup
==============

Configure each LLM provider.

OpenAI
------

.. code-block:: bash

   export OPENAI_API_KEY="sk-..."
   perspt --model gpt-4o-mini

**Models**: ``gpt-4o``, ``gpt-4o-mini``, ``gpt-4``, ``o1-mini``, ``o1-preview``, ``o3-mini``

Anthropic
---------

.. code-block:: bash

   export ANTHROPIC_API_KEY="sk-ant-..."
   perspt --provider-type anthropic --model claude-3-5-sonnet-20241022

**Models**: ``claude-3-5-sonnet-20241022``, ``claude-3-opus-20240229``, ``claude-3-haiku-20240307``

Google Gemini
-------------

.. code-block:: bash

   export GEMINI_API_KEY="..."
   perspt --provider-type gemini --model gemini-2.0-flash

**Models**: ``gemini-2.0-flash``, ``gemini-1.5-pro``, ``gemini-1.5-flash``

Groq (Fast Inference)
---------------------

.. code-block:: bash

   export GROQ_API_KEY="..."
   perspt --provider-type groq --model llama-3.3-70b-versatile

**Models**: ``llama-3.3-70b-versatile``, ``mixtral-8x7b-32768``

Ollama (Local)
--------------

.. code-block:: bash

   ollama serve
   perspt --provider-type ollama --model llama3.2

See :doc:`/tutorials/local-models` for setup.

DeepSeek
--------

.. code-block:: bash

   export DEEPSEEK_API_KEY="..."
   perspt --provider-type deepseek --model deepseek-chat

**Models**: ``deepseek-chat``, ``deepseek-reasoner``

List Available Models
---------------------

.. code-block:: bash

   perspt --provider-type openai --list-models
