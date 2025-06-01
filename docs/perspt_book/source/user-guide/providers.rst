AI Providers
============

This comprehensive guide covers all supported AI providers in Perspt powered by the modern genai crate (v0.3.5), their latest capabilities, configuration options, and best practices for optimal performance.

Overview
--------

Perspt leverages the unified genai crate to provide seamless access to multiple AI providers with consistent APIs and enhanced features:

.. grid:: 2 2 3 4
    :gutter: 3

    .. grid-item-card:: OpenAI
        :text-align: center
        :class-header: sd-bg-primary sd-text-white

        Latest GPT models including reasoning models (o1-series), GPT-4.1, and optimized variants

    .. grid-item-card:: Anthropic
        :text-align: center
        :class-header: sd-bg-secondary sd-text-white

        Claude 3.5 family with constitutional AI and safety-focused design

    .. grid-item-card:: Google AI
        :text-align: center
        :class-header: sd-bg-success sd-text-white

        Gemini 2.5 Pro and multimodal capabilities with large context windows

    .. grid-item-card:: Groq
        :text-align: center
        :class-header: sd-bg-warning sd-text-white

        Ultra-fast inference with Llama and Mixtral models

    .. grid-item-card:: Cohere
        :text-align: center
        :class-header: sd-bg-info sd-text-white

        Command R+ models optimized for business and RAG applications

    .. grid-item-card:: XAI
        :text-align: center
        :class-header: sd-bg-dark sd-text-white

        Grok models with real-time web access and humor

    .. grid-item-card:: Ollama
        :text-align: center
        :class-header: sd-bg-light sd-text-dark

        Local model hosting with privacy and offline capabilities

    .. grid-item-card:: AWS Bedrock
        :text-align: center
        :class-header: sd-bg-danger sd-text-white

        Enterprise-grade with Nova and Titan models

OpenAI
------

OpenAI provides cutting-edge language models including the latest reasoning capabilities through the genai crate integration.

Supported Models
~~~~~~~~~~~~~~~~

.. list-table::
   :header-rows: 1
   :widths: 30 20 25 25

   * - Model
     - Context Length
     - Best For
     - Notes
   * - ``gpt-4.1``
     - 128K tokens
     - Enhanced reasoning, latest capabilities
     - Most advanced GPT-4 variant (2025)
   * - ``o1-preview``
     - 128K tokens
     - Complex reasoning, problem solving
     - Advanced reasoning with step-by-step thinking
   * - ``o1-mini``
     - 128K tokens
     - Fast reasoning, coding tasks
     - Efficient reasoning model
   * - ``o3-mini``
     - 128K tokens
     - Latest reasoning capabilities
     - Newest reasoning model (2025)
   * - ``gpt-4o``
     - 128K tokens
     - Multimodal, fast performance
     - Optimized for speed and quality
   * - ``gpt-4o-mini``
     - 128K tokens
     - Fast, cost-effective (default)
     - Efficient version of GPT-4o
   * - ``gpt-4-turbo``
     - 128K tokens
     - Complex reasoning, analysis
     - Previous generation flagship
   * - ``gpt-3.5-turbo``
     - 16K tokens
     - Fast, cost-effective
     - Good for simple tasks

Configuration
~~~~~~~~~~~~~

Basic OpenAI configuration with genai crate:

.. code-block:: json

   {
     "provider_type": "openai",
     "api_key": "sk-your-openai-api-key",
     "default_model": "gpt-4o-mini",
     "providers": {
       "openai": "https://api.openai.com/v1"
     }
   }

CLI Usage
~~~~~~~~~

.. code-block:: bash

   # Use latest reasoning model
   perspt --provider-type openai --model o1-mini
   
   # Use fastest model (default)
   perspt --provider-type openai --model gpt-4o-mini
   
   # List all available OpenAI models
   perspt --provider-type openai --list-models

**Reasoning Model Features**

O1-series models provide enhanced reasoning with visual feedback:

.. code-block:: text

   > Solve this logic puzzle: There are 5 houses in a row...
   
   [Reasoning...] Let me work through this step by step:
   1. Setting up the constraints...
   2. Analyzing the color clues...
   3. Cross-referencing with pet information...
   [Streaming...] Based on my analysis, here's the solution...

**Environment Variables**

.. code-block:: bash

   export OPENAI_API_KEY="sk-your-key-here"
   export OPENAI_ORG_ID="org-your-org-id"  # Optional

Anthropic (Claude)
------------------

Anthropic's Claude models excel at safety, reasoning, and nuanced understanding through constitutional AI principles.

Supported Models
~~~~~~~~~~~~~~~~

.. list-table::
   :header-rows: 1
   :widths: 35 20 25 20

   * - Model
     - Context Length
     - Best For
     - Notes
   * - ``claude-3-5-sonnet-20241022``
     - 200K tokens
     - Balanced performance, latest version
     - Recommended default
   * - ``claude-3-5-sonnet-20240620``
     - 200K tokens
     - Previous Sonnet version
     - Stable and reliable
   * - ``claude-3-5-haiku-20241022``
     - 200K tokens
     - Fast responses, cost-effective
     - Good for simple tasks
   * - ``claude-3-opus-20240229``
     - 200K tokens
     - Most capable, complex reasoning
     - Highest quality responses

Configuration
~~~~~~~~~~~~~

.. code-block:: json

   {
     "provider_type": "anthropic",
     "api_key": "sk-ant-your-anthropic-key",
     "default_model": "claude-3-5-sonnet-20241022",
     "providers": {
       "anthropic": "https://api.anthropic.com"
     }
   }

CLI Usage
~~~~~~~~~

.. code-block:: bash

   # Use latest Claude model
   perspt --provider-type anthropic --model claude-3-5-sonnet-20241022
   
   # Use fastest Claude model  
   perspt --provider-type anthropic --model claude-3-5-haiku-20241022
   
   # List available Anthropic models
   perspt --provider-type anthropic --list-models

**Environment Variables**

.. code-block:: bash

   export ANTHROPIC_API_KEY="sk-ant-your-key-here"

Google AI (Gemini)
------------------

Google's Gemini models offer multimodal capabilities and large context windows with competitive performance.

Supported Models
~~~~~~~~~~~~~~~~

.. list-table::
   :header-rows: 1
   :widths: 35 20 25 20

   * - Model
     - Context Length
     - Best For
     - Notes
   * - ``gemini-2.0-flash-exp``
     - 1M tokens
     - Latest experimental model
     - Cutting-edge capabilities (2025)
   * - ``gemini-1.5-pro``
     - 2M tokens
     - Large documents, complex analysis
     - Largest context window
   * - ``gemini-1.5-flash``
     - 1M tokens
     - Fast responses, good balance
     - Recommended default
   * - ``gemini-pro``
     - 32K tokens
     - General purpose tasks
     - Stable and reliable

Configuration
~~~~~~~~~~~~~

.. code-block:: json

   {
     "provider_type": "google",
     "api_key": "your-google-api-key",
     "default_model": "gemini-1.5-flash",
     "providers": {
       "google": "https://generativelanguage.googleapis.com"
     }
   }

CLI Usage
~~~~~~~~~

.. code-block:: bash

   # Use latest Gemini model
   perspt --provider-type google --model gemini-2.0-flash-exp
   
   # Use model with largest context
   perspt --provider-type google --model gemini-1.5-pro
   
   # List available Google models
   perspt --provider-type google --list-models

**Environment Variables**

.. code-block:: bash

   export GOOGLE_API_KEY="your-key-here"
   # or
   export GEMINI_API_KEY="your-key-here"
       "User-Agent": "Perspt/1.0"
     }
   }

Best Practices
~~~~~~~~~~~~~~

1. **Model Selection**:
   - Use ``gpt-4-turbo`` for complex reasoning tasks
   - Use ``gpt-3.5-turbo`` for simple queries to save costs
   - Use ``gpt-4-vision-preview`` when working with images

2. **Token Management**:
   - Monitor usage with longer conversations
   - Use appropriate ``max_tokens`` limits
   - Consider conversation history truncation

3. **Rate Limits**:
   - Implement retry logic for rate limit errors
   - Consider upgrading to higher tier plans for increased limits

Anthropic (Claude)
------------------

Anthropic's Claude models are known for their helpfulness, harmlessness, and honesty.

Supported Models
~~~~~~~~~~~~~~~~

.. list-table::
   :header-rows: 1
   :widths: 25 25 25 25

   * - Model
     - Context Length
     - Best For
     - Notes
   * - ``claude-3-opus-20240229``
     - 200K tokens
     - Complex reasoning, creative tasks
     - Most capable Claude model
   * - ``claude-3-sonnet-20240229``
     - 200K tokens
     - Balanced performance/speed
     - Good general-purpose model
   * - ``claude-3-haiku-20240307``
     - 200K tokens
     - Fast responses, simple tasks
     - Most cost-effective
   * - ``claude-2.1``
     - 200K tokens
     - Legacy support
     - Deprecated, use Claude-3

Configuration
~~~~~~~~~~~~~

Basic Anthropic configuration:

.. code-block:: json

   {
     "provider": "anthropic",
     "api_key": "your-anthropic-api-key",
     "model": "claude-3-opus-20240229",
     "base_url": "https://api.anthropic.com",
     "version": "2023-06-01",
     "max_tokens": 4000,
     "temperature": 0.7,
     "top_p": 1.0,
     "top_k": 40,
     "stop_sequences": ["\\n\\nHuman:", "\\n\\nAssistant:"]
   }

Advanced Configuration
~~~~~~~~~~~~~~~~~~~~~~

**System Messages**:

.. code-block:: json

   {
     "provider": "anthropic",
     "model": "claude-3-opus-20240229",
     "system_message": "You are a helpful assistant specialized in software development. Provide detailed, accurate responses with code examples when appropriate."
   }

**Content Filtering**:

.. code-block:: json

   {
     "provider": "anthropic",
     "content_filtering": {
       "enabled": true,
       "strictness": "moderate"
     }
   }

Best Practices
~~~~~~~~~~~~~~

1. **Model Selection**:
   - Use ``claude-3-opus`` for complex analysis and creative work
   - Use ``claude-3-sonnet`` for balanced general-purpose tasks
   - Use ``claude-3-haiku`` for quick questions and simple tasks

2. **Prompt Engineering**:
   - Claude responds well to clear, structured prompts
   - Use explicit instructions and examples
   - Leverage Claude's strong reasoning capabilities

3. **Long Conversations**:
   - Take advantage of the large context window
   - Maintain conversation flow without frequent truncation

Google AI (Gemini)
------------------

Google's Gemini models offer strong reasoning and multimodal capabilities.

Supported Models
~~~~~~~~~~~~~~~~

.. list-table::
   :header-rows: 1
   :widths: 25 25 25 25

   * - Model
     - Context Length
     - Best For
     - Notes
   * - ``gemini-2.5-pro``
     - 2M tokens
     - Advanced reasoning, analysis
     - Latest and most capable
   * - ``gemini-2.0-flash``
     - 1M tokens
     - Fast, efficient performance
     - Optimized for speed
   * - ``gemini-1.5-pro``
     - 2M tokens
     - Complex reasoning, long context
     - High-capability model
   * - ``gemini-1.5-flash``
     - 1M tokens
     - Fast responses, good quality
     - Balanced speed and capability
   * - ``gemini-pro``
     - 32K tokens
     - General reasoning
     - Legacy model
   * - ``gemini-pro-vision``
     - 16K tokens
     - Multimodal tasks
     - Supports images and text

Configuration
~~~~~~~~~~~~~

Basic Google AI configuration:

.. code-block:: json

   {
     "provider": "google",
     "api_key": "your-google-api-key",
     "model": "gemini-pro",
     "base_url": "https://generativelanguage.googleapis.com/v1",
     "safety_settings": {
       "harassment": "BLOCK_MEDIUM_AND_ABOVE",
       "hate_speech": "BLOCK_MEDIUM_AND_ABOVE",
       "sexually_explicit": "BLOCK_MEDIUM_AND_ABOVE",
       "dangerous_content": "BLOCK_MEDIUM_AND_ABOVE"
     },
     "generation_config": {
       "temperature": 0.7,
       "top_p": 1.0,
       "top_k": 40,
       "max_output_tokens": 4000
     }
   }

Multimodal Configuration
~~~~~~~~~~~~~~~~~~~~~~~

For image analysis with Gemini Vision:

.. code-block:: json

   {
     "provider": "google",
     "model": "gemini-pro-vision",
     "multimodal": {
       "enabled": true,
       "supported_formats": ["png", "jpg", "jpeg", "webp", "gif"],
       "max_image_size": "20MB"
     }
   }

Best Practices
~~~~~~~~~~~~~~

1. **Safety Settings**:
   - Configure appropriate safety levels for your use case
   - Consider more permissive settings for creative tasks

2. **Multimodal Usage**:
   - Use Gemini Vision for image analysis and understanding
   - Combine text and images for richer interactions

Azure OpenAI
-------------

Microsoft's Azure OpenAI service provides enterprise-grade access to OpenAI models.

Configuration
~~~~~~~~~~~~~

.. code-block:: json

   {
     "provider": "azure_openai",
     "api_key": "your-azure-api-key",
     "endpoint": "https://your-resource.openai.azure.com/",
     "api_version": "2023-12-01-preview",
     "deployment_name": "gpt-4-turbo",
     "model": "gpt-4-turbo",
     "max_tokens": 4000,
     "temperature": 0.7
   }

Enterprise Features
~~~~~~~~~~~~~~~~~~~

**Managed Identity**:

.. code-block:: json

   {
     "provider": "azure_openai",
     "authentication": {
       "type": "managed_identity",
       "client_id": "your-client-id"
     }
   }

**Content Filtering**:

.. code-block:: json

   {
     "provider": "azure_openai",
     "content_filter": {
       "enabled": true,
       "categories": ["hate", "sexual", "violence", "self_harm"],
       "severity_threshold": "medium"
     }
   }

Local Models
------------

Perspt supports various local inference solutions for privacy and offline usage.

Ollama
~~~~~~

Configuration for Ollama local models:

.. code-block:: json

   {
     "provider": "ollama",
     "base_url": "http://localhost:11434",
     "model": "llama2:7b",
     "stream": true,
     "options": {
       "temperature": 0.7,
       "top_p": 0.9,
       "top_k": 40,
       "repeat_penalty": 1.1,
       "seed": -1,
       "num_ctx": 4096
     }
   }

Popular Ollama Models:

.. code-block:: bash

   # Install popular models
   ollama pull llama2:7b          # General purpose
   ollama pull codellama:7b       # Code generation
   ollama pull mistral:7b         # Fast and capable
   ollama pull neural-chat:7b     # Conversational

LM Studio
~~~~~~~~~

Configuration for LM Studio:

.. code-block:: json

   {
     "provider": "lm_studio",
     "base_url": "http://localhost:1234/v1",
     "model": "local-model",
     "stream": true,
     "context_length": 4096,
     "gpu_layers": 35
   }

OpenAI-Compatible Servers
~~~~~~~~~~~~~~~~~~~~~~~~~

For other OpenAI-compatible local servers:

.. code-block:: json

   {
     "provider": "openai_compatible",
     "base_url": "http://localhost:8000/v1",
     "api_key": "not-needed",
     "model": "local-model-name",
     "stream": true
   }

Provider Comparison
-------------------

.. list-table::
   :header-rows: 1
   :widths: 15 15 15 15 15 15 10

   * - Provider
     - Speed
     - Quality
     - Cost
     - Privacy
     - Context
     - Multimodal
   * - OpenAI
     - Fast
     - Excellent
     - Medium
     - Cloud
     - 128K
     - Yes
   * - Anthropic
     - Medium
     - Excellent
     - Medium
     - Cloud
     - 200K
     - No
   * - Google AI
     - Fast
     - Very Good
     - Low
     - Cloud
     - 32K
     - Yes
   * - Azure OpenAI
     - Fast
     - Excellent
     - Medium
     - Enterprise
     - 128K
     - Yes
   * - Local (Ollama)
     - Variable
     - Good
     - Free
     - Local
     - Variable
     - Limited

Multi-Provider Setup
--------------------

Configure multiple providers for different use cases:

.. code-block:: json

   {
     "providers": {
       "primary": {
         "provider": "openai",
         "model": "gpt-4-turbo",
         "api_key": "your-openai-key"
       },
       "coding": {
         "provider": "anthropic",
         "model": "claude-3-opus-20240229",
         "api_key": "your-anthropic-key"
       },
       "local": {
         "provider": "ollama",
         "model": "codellama:7b",
         "base_url": "http://localhost:11434"
       }
     },
     "default_provider": "primary"
   }

Switch between providers during conversation:

.. code-block:: text

   > /provider coding
   Switched to coding provider (Claude-3 Opus)
   
   > /provider local
   Switched to local provider (CodeLlama)

Fallback Configuration
~~~~~~~~~~~~~~~~~~~~~~

Set up automatic fallbacks:

.. code-block:: json

   {
     "fallback_chain": [
       {
         "provider": "openai",
         "model": "gpt-4-turbo"
       },
       {
         "provider": "anthropic",
         "model": "claude-3-sonnet-20240229"
       },
       {
         "provider": "ollama",
         "model": "llama2:7b"
       }
     ],
     "fallback_conditions": [
       "rate_limit_exceeded",
       "api_error",
       "timeout"
     ]
   }

Troubleshooting
---------------

Common Issues
~~~~~~~~~~~~~

**API Key Issues**:

.. code-block:: text

   > /validate-key
   Checking API key validity...
   ✓ OpenAI key: Valid
   ✗ Anthropic key: Invalid or expired

**Connection Problems**:

.. code-block:: bash

   # Test connectivity
   curl -H "Authorization: Bearer your-api-key" \\
        https://api.openai.com/v1/models

**Rate Limiting**:

.. code-block:: json

   {
     "rate_limiting": {
       "requests_per_minute": 60,
       "tokens_per_minute": 40000,
       "retry_strategy": "exponential_backoff",
       "max_retries": 3
     }
   }

Performance Optimization
~~~~~~~~~~~~~~~~~~~~~~~~

**Request Optimization**:

.. code-block:: json

   {
     "optimization": {
       "batch_requests": true,
       "compress_requests": true,
       "connection_pooling": true,
       "timeout": 30
     }
   }

**Caching**:

.. code-block:: json

   {
     "cache": {
       "enabled": true,
       "provider_specific": true,
       "ttl": 3600,
       "max_size": "100MB"
     }
   }

Next Steps
----------

- :doc:`troubleshooting` - Detailed troubleshooting for provider-specific issues
- :doc:`advanced-features` - Advanced features that work with different providers
- :doc:`../configuration` - Complete configuration reference
- :doc:`../developer-guide/extending` - Create custom provider integrations

Groq
----

Groq provides ultra-fast inference speeds with popular open-source models, optimized for real-time conversations.

Supported Models
~~~~~~~~~~~~~~~~

.. list-table::
   :header-rows: 1
   :widths: 35 20 25 20

   * - Model
     - Context Length
     - Best For
     - Notes
   * - ``llama-3.1-405b-reasoning``
     - 128K tokens
     - Complex reasoning, analysis
     - Largest Llama model
   * - ``llama-3.1-70b-versatile``
     - 128K tokens
     - Balanced performance
     - Good general purpose model
   * - ``llama-3.1-8b-instant``
     - 128K tokens
     - Ultra-fast responses
     - Best for speed
   * - ``mixtral-8x7b-32768``
     - 32K tokens
     - Mixture of experts
     - Strong coding capabilities

Configuration
~~~~~~~~~~~~~

.. code-block:: json

   {
     "provider_type": "groq",
     "api_key": "your-groq-api-key",
     "default_model": "llama-3.1-70b-versatile",
     "providers": {
       "groq": "https://api.groq.com/openai/v1"
     }
   }

CLI Usage
~~~~~~~~~

.. code-block:: bash

   # Ultra-fast responses
   perspt --provider-type groq --model llama-3.1-8b-instant
   
   # Balanced performance
   perspt --provider-type groq --model llama-3.1-70b-versatile

**Environment Variables**

.. code-block:: bash

   export GROQ_API_KEY="your-key-here"

Cohere
------

Cohere specializes in enterprise-focused models with strong RAG (Retrieval-Augmented Generation) capabilities.

Supported Models
~~~~~~~~~~~~~~~~

.. list-table::
   :header-rows: 1
   :widths: 35 20 25 20

   * - Model
     - Context Length
     - Best For
     - Notes
   * - ``command-r-plus``
     - 128K tokens
     - RAG, business applications
     - Most capable Cohere model
   * - ``command-r``
     - 128K tokens
     - General purpose, fast
     - Good balance of speed and quality
   * - ``command``
     - 4K tokens
     - Simple tasks, cost-effective
     - Basic model

Configuration
~~~~~~~~~~~~~

.. code-block:: json

   {
     "provider_type": "cohere",
     "api_key": "your-cohere-api-key", 
     "default_model": "command-r-plus",
     "providers": {
       "cohere": "https://api.cohere.ai"
     }
   }

**Environment Variables**

.. code-block:: bash

   export COHERE_API_KEY="your-key-here"

XAI (Grok)
----------

XAI's Grok models provide real-time web access and are known for their humor and current knowledge.

Supported Models
~~~~~~~~~~~~~~~~

.. list-table::
   :header-rows: 1
   :widths: 35 20 25 20

   * - Model
     - Context Length
     - Best For
     - Notes
   * - ``grok-beta``
     - 128K tokens
     - Current events, humor
     - Latest Grok model
   * - ``grok-vision-beta``
     - 128K tokens
     - Multimodal analysis
     - Image understanding

Configuration
~~~~~~~~~~~~~

.. code-block:: json

   {
     "provider_type": "xai",
     "api_key": "your-xai-api-key",
     "default_model": "grok-beta",
     "providers": {
       "xai": "https://api.x.ai/v1"
     }
   }

**Environment Variables**

.. code-block:: bash

   export XAI_API_KEY="your-key-here"

Ollama (Local Models)
---------------------

Ollama provides local model hosting for privacy, offline usage, and cost control with the genai crate integration.

Supported Models
~~~~~~~~~~~~~~~~

Popular models available through Ollama:

.. code-block:: bash

   # Large models (requires significant RAM)
   llama3.2:90b     # Latest Llama model
   qwen2.5:72b      # Alibaba's capable model
   
   # Medium models (good balance)
   llama3.2:8b      # Recommended default
   mistral-nemo:12b # Mistral's latest
   
   # Small models (fast, low resource)
   llama3.2:3b      # Efficient Llama variant
   qwen2.5:7b       # Compact but capable

Setup and Configuration
~~~~~~~~~~~~~~~~~~~~~~~

1. **Install Ollama**:

.. code-block:: bash

   # macOS
   brew install ollama
   
   # Linux
   curl -fsSL https://ollama.com/install.sh | sh

2. **Download Models**:

.. code-block:: bash

   # Download recommended model
   ollama pull llama3.2:8b
   
   # Download smaller model for testing
   ollama pull llama3.2:3b

3. **Configure Perspt**:

.. code-block:: json

   {
     "provider_type": "ollama",
     "default_model": "llama3.2:8b",
     "providers": {
       "ollama": "http://localhost:11434"
     }
   }

CLI Usage
~~~~~~~~~

.. code-block:: bash

   # Use local Ollama model
   perspt --provider-type ollama --model llama3.2:8b
   
   # List installed Ollama models
   perspt --provider-type ollama --list-models
   
   # Use custom Ollama endpoint
   perspt --provider-type ollama --model llama3.2:8b

**Benefits of Local Models**

- **Privacy**: Data stays on your machine
- **Offline Usage**: No internet required after setup
- **Cost Control**: No per-token charges
- **Customization**: Fine-tune models for specific tasks

**Environment Variables**

.. code-block:: bash

   export OLLAMA_HOST="http://localhost:11434"
