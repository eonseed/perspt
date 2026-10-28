.. _user-guide-providers:

Providers
=========

Perspt supports multiple LLM providers through the ``genai`` Rust client crate, which provides unified access to all major commercial, open-source, and cloud-provider model APIs.

Supported Providers and Adapters
--------------------------------

Perspt accepts nine provider identifiers, each routed to the matching ``genai`` adapter (any other value is rejected as unsupported):

.. list-table::
   :header-rows: 1
   :widths: 20 20 25 35

   * - Provider / Adapter
     - Adapter Kind ID
     - Reference Model
     - Notes
   * - **OpenAI**
     - ``openai``
     - ``gpt-5.5``
     - SOTA GPT-5.5, GPT-5-mini
   * - **Anthropic**
     - ``anthropic``
     - ``claude-fable``
     - Claude Fable, Opus 4.8, Sonnet 4.6, Haiku 4.6
   * - **Google Gemini**
     - ``gemini``, ``google``
     - ``gemini-3.5-flash``
     - Gemini 3.5 Flash, 3.1 Pro, 3.1 Flash-Lite
   * - **Google Vertex AI**
     - ``vertex``
     - ``vertex::gemini-3.5-flash``
     - Google Cloud Vertex platform
   * - **Groq**
     - ``groq``
     - ``llama-3.3-70b-specdec``
     - Ultra-low latency Llama/Gemma on LPU
   * - **Cohere**
     - ``cohere``
     - ``command-a-plus``
     - Command A+, North Mini Code
   * - **xAI**
     - ``xai``
     - ``grok-4``
     - Grok 4 family
   * - **DeepSeek**
     - ``deepseek``
     - ``deepseek-v4``
     - DeepSeek v4 models (Chat, Coder)
   * - **Ollama**
     - ``ollama``
     - ``llama3.3``
     - Local offline models

Configuration Methods
---------------------

**1. Environment Variables** (recommended):

.. code-block:: bash

   export GEMINI_API_KEY="your-key"
   perspt

**2. CLI Flags**:

.. code-block:: bash

   perspt chat --model gpt-5.5

**3. Config File** (``config.toml``):

.. code-block:: toml

   provider = "anthropic"
   model = "claude-fable"

Agent mode can additionally bind several credentials and routes at once
through the ``[providers.<id>]`` and ``[models]`` tables, which hold
fully qualified ``provider::model`` routes for multi-route portfolios.
See :doc:`agent-mode` for details.

Provider-Specific Notes
-----------------------

**OpenAI**

.. code-block:: bash

   export OPENAI_API_KEY="sk-xxx"
   perspt chat --model gpt-5.5

**Azure OpenAI (via OpenAI Compatible)**

Azure OpenAI requires configuring the base URL override and the API key:

.. code-block:: bash

   export OPENAI_API_KEY="your-azure-key"
   export OPENAI_BASE_URL="https://your-resource.openai.azure.com/openai/deployments/your-deployment"
   perspt chat --model gpt-5.5

**Anthropic**

.. code-block:: bash

   export ANTHROPIC_API_KEY="sk-ant-xxx"
   perspt chat --model claude-fable

**Google Gemini**

.. code-block:: bash

   export GEMINI_API_KEY="AIza..."
   perspt chat --model gemini-3.5-flash

**Google Vertex AI**

Vertex AI requires your Google Cloud project ID and a location (optional, defaults to ``global``). Authentication is typically handled via Google Application Default Credentials (ADC); setting ``VERTEX_API_KEY`` to a bearer token overrides the ADC token.

.. code-block:: bash

   export VERTEX_PROJECT_ID="my-gcp-project-123"
   export VERTEX_LOCATION="us-central1"
   # Run using Vertex model prefix
   perspt chat --model vertex::gemini-3.5-flash

**Ollama (Local)**

.. code-block:: bash

   ollama serve
   ollama pull llama3.3
   perspt chat --model llama3.3

No API key required. Perspt auto-detects Ollama as the fallback provider.
