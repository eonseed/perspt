.. _howto-providers:

Providers
=========

How to set up each LLM provider with Perspt.

OpenAI
------

**Models**: GPT-5.2, o3-mini, o1-preview, GPT-4

.. code-block:: bash

   # Set API key
   export OPENAI_API_KEY="sk-..."

   # Use with Perspt
   perspt chat --model gpt-5.2

**Get API Key**: `platform.openai.com <https://platform.openai.com/api-keys>`_

Anthropic
---------

**Models**: Claude Opus 4.5, Claude 3.5 Sonnet

.. code-block:: bash

   export ANTHROPIC_API_KEY="sk-ant-..."
   perspt chat --model claude-opus-4.5

**Get API Key**: `console.anthropic.com <https://console.anthropic.com/account/keys>`_

Google Gemini
-------------

**Models**: Gemini 3 Flash, Gemini 3 Pro

.. code-block:: bash

   export GEMINI_API_KEY="..."
   perspt chat --model gemini-3-flash

**Get API Key**: `aistudio.google.com <https://aistudio.google.com/apikey>`_

Groq
----

**Models**: Llama 3.x (ultra-fast inference)

.. code-block:: bash

   export GROQ_API_KEY="..."
   perspt chat --model llama-3.3-70b

**Get API Key**: `console.groq.com <https://console.groq.com/keys>`_

**Best for**: Fast prototyping, testing

Cohere
------

**Models**: Command R, Command R+

.. code-block:: bash

   export COHERE_API_KEY="..."
   perspt chat --model command-r-plus

**Get API Key**: `dashboard.cohere.com <https://dashboard.cohere.com/api-keys>`_

XAI (Grok)
----------

**Models**: Grok

.. code-block:: bash

   export XAI_API_KEY="..."
   perspt chat --model grok-2

**Get API Key**: `console.x.ai <https://console.x.ai/>`_

DeepSeek
--------

**Models**: DeepSeek Coder, DeepSeek Chat

.. code-block:: bash

   export DEEPSEEK_API_KEY="..."
   perspt chat --model deepseek-coder

**Get API Key**: `platform.deepseek.com <https://platform.deepseek.com/>`_

Ollama (Local)
--------------

**Models**: Llama 3.2, CodeLlama, DeepSeek Coder (local)

.. code-block:: bash

   # No API key needed
   ollama serve
   ollama pull llama3.2
   perspt chat --model llama3.2

**Setup**: See :doc:`../tutorials/local-models`

Provider Comparison
-------------------

.. list-table::
   :header-rows: 1
   :widths: 15 20 35 30

   * - Provider
     - Speed
     - Best For
     - Cost
   * - OpenAI
     - Medium
     - Reasoning, complex tasks
     - $$$
   * - Anthropic
     - Medium
     - Code generation, safety
     - $$$
   * - Google
     - Fast
     - Long context, multimodal
     - $$
   * - Groq
     - Ultra-fast
     - Prototyping, testing
     - $
   * - Ollama
     - Variable
     - Privacy, offline use
     - Free

Agent Mode Recommendations
--------------------------

For optimal SRBN performance:

.. code-block:: bash

   perspt agent \
     --architect-model gpt-5.2 \       # Deep reasoning
     --actuator-model claude-opus-4.5 \ # Strong coding
     --verifier-model gemini-3-pro \   # Fast analysis
     --speculator-model gemini-3-flash \ # Ultra-fast
     "Your task"

See Also
--------

- :doc:`configuration` - Config file setup
- :doc:`../tutorials/local-models` - Ollama guide
