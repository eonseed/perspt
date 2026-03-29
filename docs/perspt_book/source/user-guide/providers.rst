.. _user-guide-providers:

Providers
=========

Perspt supports multiple LLM providers through the ``genai`` crate.

Supported Providers
-------------------

.. list-table::
   :header-rows: 1
   :widths: 15 25 25 35

   * - Provider
     - Env Variable
     - Default Model
     - Notes
   * - OpenAI
     - ``OPENAI_API_KEY``
     - ``gpt-4.1``
     - GPT-4, GPT-4o, o-series
   * - Anthropic
     - ``ANTHROPIC_API_KEY``
     - ``claude-sonnet-4-20250514``
     - Claude 4 family
   * - Google Gemini
     - ``GEMINI_API_KEY``
     - ``gemini-3.1-flash-lite-preview``
     - Gemini 2.x family
   * - Groq
     - ``GROQ_API_KEY``
     - ``llama-3.3-70b-versatile``
     - Llama, Mixtral
   * - Cohere
     - ``COHERE_API_KEY``
     - ``command-r-plus``
     - Command R family
   * - xAI
     - ``XAI_API_KEY``
     - ``grok-3-mini-fast``
     - Grok models
   * - DeepSeek
     - ``DEEPSEEK_API_KEY``
     - ``deepseek-chat``
     - DeepSeek models
   * - Ollama
     - *(none)*
     - ``llama3.2``
     - Local models via Ollama

Configuration Methods
---------------------

**1. Environment Variables** (recommended):

.. code-block:: bash

   export GEMINI_API_KEY="your-key"
   perspt

**2. CLI Flags**:

.. code-block:: bash

   perspt chat --api-key "your-key" --provider-type openai --model gpt-4.1

**3. Config File** (``~/.config/perspt/config.json``):

.. code-block:: json

   {
     "default_provider": "anthropic",
     "default_model": "claude-sonnet-4-20250514",
     "api_key": "sk-ant-xxx"
   }

Priority order: CLI flags > environment variables > config file > auto-detection.

Listing Available Models
------------------------

.. code-block:: bash

   perspt --list-models

Provider-Specific Notes
-----------------------

**OpenAI**

.. code-block:: bash

   export OPENAI_API_KEY="sk-xxx"
   perspt chat --model gpt-4.1

**Anthropic**

.. code-block:: bash

   export ANTHROPIC_API_KEY="sk-ant-xxx"
   perspt chat --model claude-sonnet-4-20250514

**Google Gemini**

.. code-block:: bash

   export GEMINI_API_KEY="AIza..."
   perspt chat --model gemini-3.1-flash-lite-preview

**Ollama (Local)**

.. code-block:: bash

   ollama serve
   ollama pull llama3.2
   perspt chat --model llama3.2

No API key required. Perspt auto-detects Ollama as the fallback provider.
