.. _user-guide-providers:

Providers
=========

Perspt supports multiple LLM providers through the ``genai`` Rust client crate, which provides unified access to all major commercial, open-source, and cloud-provider model APIs.

Supported Providers and Adapters
--------------------------------

The underlying ``genai`` client supports 29 adapters representing different APIs and providers:

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
   * - **OpenAI Compatible**
     - ``openai_resp``
     - ``custom-model``
     - Targets custom endpoints (e.g., Azure OpenAI)
   * - **Anthropic**
     - ``anthropic``
     - ``claude-fable``
     - Claude Fable, Opus 4.8, Sonnet 4.6, Haiku 4.6
   * - **Google Gemini**
     - ``gemini``
     - ``gemini-3.5-flash``
     - Gemini 3.5 Flash, 3.1 Pro, 3.1 Flash-Lite
   * - **Google Vertex AI**
     - ``vertex``
     - ``vertex::gemini-3.5-flash``
     - Google Cloud Vertex platform
   * - **AWS Bedrock**
     - ``bedrock_api``, ``bedrock_sigv4``
     - ``us.amazon.nova-pro-v2:0``
     - AWS Bedrock cloud execution (Titan, Nova, Claude)
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
     - ``ollama``, ``ollama_cloud``
     - ``llama3.3``
     - Local offline models
   * - **GitHub Copilot**
     - ``github_copilot``
     - ``copilot-model``
     - Copilot developer services
   * - **OpenRouter**
     - ``open_router``
     - ``router-model``
     - Multi-model routing gateway
   * - **Together AI**
     - ``together``
     - ``together-model``
     - Low-latency open-source models
   * - **Fireworks AI**
     - ``fireworks``
     - ``fireworks-model``
     - Fast serverless models
   * - **Nebius AI**
     - ``nebius``
     - ``nebius-model``
     - Nebius cloud inference
   * - **Mimo**
     - ``mimo``
     - ``mimo-model``
     - Mimo execution platform
   * - **Zhipu AI**
     - ``zai``, ``zai_coding``
     - ``glm-4``
     - ChatGLM and coding assistants
   * - **BigModel**
     - ``bigmodel``
     - ``bigmodel-model``
     - BigModel cloud models
   * - **Aliyun**
     - ``aliyun``
     - ``qwen-turbo``
     - Alibaba DashScope API
   * - **Baidu**
     - ``baidu``
     - ``qianfan-model``
     - Baidu Qianfan models
   * - **Moonshot**
     - ``moonshot``
     - ``kimi-model``
     - Moonshot Kimi API
   * - **AIHubMix**
     - ``aihubmix``
     - ``hubmix-model``
     - AIHubMix model services
   * - **OpenCode Go**
     - ``opencode_go``
     - ``opencode-model``
     - Specialized code generation
   * - **Custom**
     - ``custom``
     - ``custom-model``
     - User-defined adapter routing

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

Vertex AI requires your Google Cloud project ID and region (optional, defaults to ``us-central1``). Authentication is typically handled via Google Application Default Credentials (ADC).

.. code-block:: bash

   export VERTEX_PROJECT_ID="my-gcp-project-123"
   export VERTEX_REGION="us-central1"
   # Run using Vertex model prefix
   perspt chat --model vertex::gemini-3.5-flash

**AWS Bedrock**

Bedrock uses your local AWS credentials (e.g. AWS profile or environment keys) and processes calls via Bedrock API adapters.

.. code-block:: bash

   export AWS_ACCESS_KEY_ID="AKIA..."
   export AWS_SECRET_ACCESS_KEY="xxx"
   export AWS_DEFAULT_REGION="us-east-1"
   perspt chat --model us.amazon.nova-pro-v2:0

**Ollama (Local)**

.. code-block:: bash

   ollama serve
   ollama pull llama3.3
   perspt chat --model llama3.3

No API key required. Perspt auto-detects Ollama as the fallback provider.
