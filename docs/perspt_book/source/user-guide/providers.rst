AI Providers
============

This comprehensive guide covers all supported AI providers in Perspt, their capabilities, configuration options, and best practices for optimal performance.

Overview
--------

Perspt supports multiple AI providers, each with unique strengths and capabilities:

.. grid:: 1 2 2 2
    :gutter: 3

    .. grid-item-card:: OpenAI
        :text-align: center
        :class-header: sd-bg-primary sd-text-white

        Industry-leading models including GPT-4, GPT-3.5, and specialized variants

    .. grid-item-card:: Anthropic
        :text-align: center
        :class-header: sd-bg-secondary sd-text-white

        Claude family models known for safety and nuanced understanding

    .. grid-item-card:: Google AI
        :text-align: center
        :class-header: sd-bg-success sd-text-white

        Gemini models with multimodal capabilities and reasoning

    .. grid-item-card:: Local Models
        :text-align: center
        :class-header: sd-bg-info sd-text-white

        Ollama, LM Studio, and other local inference solutions

OpenAI
------

OpenAI provides some of the most capable and widely-used language models.

Supported Models
~~~~~~~~~~~~~~~~

.. list-table::
   :header-rows: 1
   :widths: 25 25 25 25

   * - Model
     - Context Length
     - Best For
     - Notes
   * - ``gpt-4-turbo``
     - 128K tokens
     - Complex reasoning, analysis
     - Latest and most capable
   * - ``gpt-4``
     - 8K tokens
     - High-quality responses
     - Reliable for most tasks
   * - ``gpt-3.5-turbo``
     - 16K tokens
     - Fast, cost-effective
     - Good for simple tasks
   * - ``gpt-4-vision-preview``
     - 128K tokens
     - Image analysis
     - Supports multimodal input

Configuration
~~~~~~~~~~~~~

Basic OpenAI configuration:

.. code-block:: json

   {
     "provider": "openai",
     "api_key": "your-openai-api-key",
     "model": "gpt-4-turbo",
     "base_url": "https://api.openai.com/v1",
     "organization": "optional-org-id",
     "max_tokens": 4000,
     "temperature": 0.7,
     "top_p": 1.0,
     "frequency_penalty": 0.0,
     "presence_penalty": 0.0
   }

Advanced Configuration
~~~~~~~~~~~~~~~~~~~~~~

**Function Calling**:

.. code-block:: json

   {
     "provider": "openai",
     "model": "gpt-4-turbo",
     "functions": {
       "enabled": true,
       "auto_invoke": true,
       "available_functions": [
         "web_search",
         "code_execution",
         "file_operations"
       ]
     }
   }

**Streaming Responses**:

.. code-block:: json

   {
     "provider": "openai",
     "model": "gpt-4-turbo",
     "stream": true,
     "stream_buffer_size": 1024
   }

**Custom Headers**:

.. code-block:: json

   {
     "provider": "openai",
     "headers": {
       "Custom-Header": "value",
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
   * - ``gemini-pro``
     - 32K tokens
     - General reasoning
     - Balanced performance
   * - ``gemini-pro-vision``
     - 16K tokens
     - Multimodal tasks
     - Supports images and text
   * - ``gemini-ultra``
     - 32K tokens
     - Complex reasoning
     - Highest capability tier

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
