Configuration Guide
===================

Perspt offers flexible configuration options to customize your AI chat experience. This guide covers all configuration methods, from simple environment variables to advanced JSON configurations.

Configuration Methods
---------------------

Perspt supports multiple configuration approaches, with the following priority order (highest to lowest):

1. **Command-line arguments** (highest priority)
2. **Configuration file** (``config.json``)
3. **Environment variables**
4. **Default values** (lowest priority)

This means command-line arguments will override config file settings, which override environment variables, and so on.

Environment Variables
---------------------

The simplest way to configure Perspt is through environment variables:

API Keys
~~~~~~~~

.. code-block:: bash

   # OpenAI
   export OPENAI_API_KEY="sk-your-openai-api-key-here"

   # Anthropic
   export ANTHROPIC_API_KEY="your-anthropic-api-key-here"

   # Google
   export GOOGLE_API_KEY="your-google-api-key-here"

   # AWS (uses standard AWS credentials)
   export AWS_ACCESS_KEY_ID="your-access-key"
   export AWS_SECRET_ACCESS_KEY="your-secret-key"
   export AWS_REGION="us-east-1"

   # Azure OpenAI
   export AZURE_OPENAI_API_KEY="your-azure-key"
   export AZURE_OPENAI_ENDPOINT="https://your-resource.openai.azure.com/"

Provider Settings
~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Default provider
   export PERSPT_PROVIDER="openai"

   # Default model
   export PERSPT_MODEL="gpt-4o-mini"

   # Custom API base URL
   export PERSPT_API_BASE="https://api.openai.com/v1"

Configuration File
------------------

For persistent settings, create a ``config.json`` file:

Basic Configuration
~~~~~~~~~~~~~~~~~~~

.. code-block:: json

   {
     "api_key": "your-api-key-here",
     "default_model": "gpt-4o-mini",
     "default_provider": "openai",
     "provider_type": "openai"
   }

Complete Configuration
~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: json

   {
     "api_key": "sk-your-openai-api-key",
     "default_model": "gpt-4o-mini",
     "default_provider": "openai",
     "provider_type": "openai",
     "providers": {
       "openai": "https://api.openai.com/v1",
       "anthropic": "https://api.anthropic.com",
       "google": "https://generativelanguage.googleapis.com/v1beta",
       "azure": "https://your-resource.openai.azure.com/"
     },
     "ui": {
       "theme": "dark",
       "show_timestamps": true,
       "markdown_rendering": true,
       "auto_scroll": true
     },
     "behavior": {
       "stream_responses": true,
       "input_queuing": true,
       "auto_save_history": false,
       "max_history_length": 1000
     },
     "advanced": {
       "request_timeout": 30,
       "retry_attempts": 3,
       "retry_delay": 1.0,
       "concurrent_requests": 1
     }
   }

Configuration File Locations
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Perspt searches for configuration files in this order:

1. **Specified path**: ``perspt --config /path/to/config.json``
2. **Current directory**: ``./config.json``
3. **User config directory**:
   - Linux: ``~/.config/perspt/config.json``
   - macOS: ``~/Library/Application Support/perspt/config.json``
   - Windows: ``%APPDATA%/perspt/config.json``

Provider Configuration
----------------------

OpenAI
~~~~~~

.. tabs::

   .. tab:: Environment Variables

      .. code-block:: bash

         export OPENAI_API_KEY="sk-your-key-here"
         export PERSPT_PROVIDER="openai"
         export PERSPT_MODEL="gpt-4o-mini"

   .. tab:: Config File

      .. code-block:: json

         {
           "api_key": "sk-your-key-here",
           "provider_type": "openai",
           "default_model": "gpt-4o-mini",
           "providers": {
             "openai": "https://api.openai.com/v1"
           }
         }

   .. tab:: Command Line

      .. code-block:: bash

         perspt --provider-type openai \
                --model-name gpt-4o-mini \
                --api-key "sk-your-key-here"

**Available Models:**
- ``gpt-4.1`` - Latest and most advanced GPT model
- ``gpt-4o`` - Latest GPT-4 Omni model
- ``gpt-4o-mini`` - Faster, cost-effective GPT-4 Omni
- ``o1-preview`` - Advanced reasoning model
- ``o1-mini`` - Efficient reasoning model  
- ``o3-mini`` - Next-generation reasoning model
- ``gpt-4-turbo`` - Latest GPT-4 Turbo
- ``gpt-4`` - Standard GPT-4

Anthropic
~~~~~~~~~

.. tabs::

   .. tab:: Environment Variables

      .. code-block:: bash

         export ANTHROPIC_API_KEY="your-key-here"
         export PERSPT_PROVIDER="anthropic"
         export PERSPT_MODEL="claude-3-sonnet-20240229"

   .. tab:: Config File

      .. code-block:: json

         {
           "api_key": "your-key-here",
           "provider_type": "anthropic",
           "default_model": "claude-3-sonnet-20240229",
           "providers": {
             "anthropic": "https://api.anthropic.com"
           }
         }

   .. tab:: Command Line

      .. code-block:: bash

         perspt --provider-type anthropic \
                --model-name claude-3-sonnet-20240229 \
                --api-key "your-key-here"

**Available Models:**
- ``claude-3-opus-20240229`` - Most capable Claude model
- ``claude-3-sonnet-20240229`` - Balanced performance and speed
- ``claude-3-haiku-20240307`` - Fastest Claude model

Google (Gemini)
~~~~~~~~~~~~~~~

.. tabs::

   .. tab:: Environment Variables

      .. code-block:: bash

         export GOOGLE_API_KEY="your-key-here"
         export PERSPT_PROVIDER="google"
         export PERSPT_MODEL="gemini-pro"

   .. tab:: Config File

      .. code-block:: json

         {
           "api_key": "your-key-here",
           "provider_type": "google",
           "default_model": "gemini-pro",
           "providers": {
             "google": "https://generativelanguage.googleapis.com/v1beta"
           }
         }

   .. tab:: Command Line

      .. code-block:: bash

         perspt --provider-type google \
                --model-name gemini-pro \
                --api-key "your-key-here"

**Available Models:**
- ``gemini-pro`` - Google's most capable model
- ``gemini-pro-vision`` - Multimodal capabilities

Command-Line Options
--------------------

Perspt supports extensive command-line configuration:

Basic Options
~~~~~~~~~~~~~

.. code-block:: bash

   perspt [OPTIONS]

.. list-table::
   :widths: 30 70
   :header-rows: 1

   * - Option
     - Description
   * - ``--config <PATH>``
     - Path to configuration file
   * - ``--provider-type <TYPE>``
     - AI provider (openai, anthropic, google, groq, cohere, xai, deepseek, ollama)
   * - ``--model-name <MODEL>``
     - Specific model to use
   * - ``--api-key <KEY>``
     - API key for authentication
   * - ``--list-models``
     - List available models for provider
   * - ``--help``
     - Show help information
   * - ``--version``
     - Show version information

Advanced Options
~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Custom API endpoint
   perspt --api-base "https://your-custom-endpoint.com/v1"

   # Increase request timeout
   perspt --timeout 60

   # Disable streaming responses
   perspt --no-stream

   # Set maximum retries
   perspt --max-retries 5

   # Custom user agent
   perspt --user-agent "MyApp/1.0"

Examples
~~~~~~~~

.. code-block:: bash

   # Use specific OpenAI model
   perspt --provider-type openai --model-name gpt-4

   # Use Anthropic with custom timeout
   perspt --provider-type anthropic \
          --model-name claude-3-sonnet-20240229 \
          --timeout 45

   # Use custom configuration file
   perspt --config ~/.perspt/work-config.json

   # List available models
   perspt --provider-type openai --list-models

UI Customization
----------------

Interface Settings
~~~~~~~~~~~~~~~~~~

Configure the terminal interface appearance:

.. code-block:: json

   {
     "ui": {
       "theme": "dark",
       "show_timestamps": true,
       "timestamp_format": "%H:%M",
       "markdown_rendering": true,
       "syntax_highlighting": true,
       "auto_scroll": true,
       "scroll_buffer": 1000,
       "word_wrap": true,
       "show_token_count": false
     }
   }

Color Themes
~~~~~~~~~~~~

Customize colors for different message types:

.. code-block:: json

   {
     "ui": {
       "colors": {
         "user_message": "#60a5fa",
         "assistant_message": "#10b981",
         "error_message": "#ef4444",
         "warning_message": "#f59e0b",
         "info_message": "#8b5cf6",
         "timestamp": "#6b7280",
         "border": "#374151",
         "background": "#111827"
       }
     }
   }

Behavior Settings
-----------------

Streaming and Responses
~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: json

   {
     "behavior": {
       "stream_responses": true,
       "input_queuing": true,
       "auto_retry_on_error": true,
       "show_thinking_indicator": true,
       "preserve_context": true
     }
   }

History Management
~~~~~~~~~~~~~~~~~~

.. code-block:: json

   {
     "behavior": {
       "auto_save_history": true,
       "history_file": "~/.perspt/chat_history.json",
       "max_history_length": 1000,
       "history_compression": true,
       "clear_history_on_exit": false
     }
   }

Advanced Configuration
----------------------

Network Settings
~~~~~~~~~~~~~~~~

.. code-block:: json

   {
     "advanced": {
       "request_timeout": 30,
       "connect_timeout": 10,
       "retry_attempts": 3,
       "retry_delay": 1.0,
       "retry_exponential_backoff": true,
       "max_concurrent_requests": 1,
       "user_agent": "Perspt/0.4.0",
       "proxy": {
         "http": "http://proxy:8080",
         "https": "https://proxy:8080"
       }
     }
   }

Security Settings
~~~~~~~~~~~~~~~~~

.. code-block:: json

   {
     "security": {
       "verify_ssl": true,
       "api_key_masking": true,
       "log_requests": false,
       "log_responses": false,
       "encrypt_history": false
     }
   }

Performance Tuning
~~~~~~~~~~~~~~~~~~

.. code-block:: json

   {
     "performance": {
       "buffer_size": 8192,
       "chunk_size": 1024,
       "memory_limit": "100MB",
       "cache_responses": false,
       "preload_models": false
     }
   }

Multiple Configurations
-----------------------

Work vs Personal
~~~~~~~~~~~~~~~~

Create separate configurations for different contexts:

**work-config.json:**

.. code-block:: json

   {
     "api_key": "sk-work-key-here",
     "provider_type": "openai",
     "default_model": "gpt-4",
     "ui": {
       "theme": "professional",
       "show_timestamps": true
     },
     "behavior": {
       "auto_save_history": true,
       "history_file": "~/.perspt/work_history.json"
     }
   }

**personal-config.json:**

.. code-block:: json

   {
     "api_key": "sk-personal-key-here",
     "provider_type": "anthropic",
     "default_model": "claude-3-sonnet-20240229",
     "ui": {
       "theme": "vibrant",
       "show_timestamps": false
     },
     "behavior": {
       "auto_save_history": false
     }
   }

Usage:

.. code-block:: bash

   # Work configuration
   perspt --config work-config.json

   # Personal configuration
   perspt --config personal-config.json

   # Create aliases for convenience
   alias work-ai="perspt --config ~/.perspt/work-config.json"
   alias personal-ai="perspt --config ~/.perspt/personal-config.json"

Configuration Validation
-------------------------

Perspt validates your configuration and provides helpful error messages:

.. code-block:: bash

   # Validate configuration without starting
   perspt --config config.json --validate

   # Check configuration and list available models
   perspt --config config.json --list-models

Common validation errors:

- **Invalid API key format**: Ensure your API key follows the correct format
- **Missing required fields**: Some providers require specific configuration
- **Invalid model names**: Use ``--list-models`` to see available options
- **Network connectivity**: Check internet connection and proxy settings

Configuration Templates
-----------------------

Generate template configurations for different use cases:

.. code-block:: bash

   # Generate basic template
   perspt --generate-config basic > config.json

   # Generate advanced template
   perspt --generate-config advanced > advanced-config.json

   # Generate provider-specific template
   perspt --generate-config openai > openai-config.json

Migration and Import
--------------------

From Other Tools
~~~~~~~~~~~~~~~~

Import configurations from similar tools:

.. code-block:: bash

   # Import from environment variables
   perspt --import-env > config.json

   # Import from ChatGPT CLI config
   perspt --import chatgpt-cli ~/.chatgpt-cli/config.yaml

   # Import from OpenAI CLI config
   perspt --import openai-cli ~/.openai/config.json

Backup and Restore
~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Backup current configuration
   cp ~/.config/perspt/config.json ~/.config/perspt/config.backup.json

   # Restore from backup
   cp ~/.config/perspt/config.backup.json ~/.config/perspt/config.json

   # Export configuration with history
   perspt --export-config --include-history > full-backup.json

Best Practices
--------------

Security
~~~~~~~~

1. **Never commit API keys** to version control
2. **Use environment variables** for sensitive data
3. **Rotate API keys** regularly
4. **Use separate keys** for different projects
5. **Enable API key masking** in logs

Organization
~~~~~~~~~~~~

1. **Use descriptive config names** (``work-config.json``, ``research-config.json``)
2. **Create aliases** for frequently used configurations
3. **Document your configurations** with comments (where supported)
4. **Use version control** for non-sensitive configuration parts
5. **Regular backups** of important configurations

Performance
~~~~~~~~~~~

1. **Set appropriate timeouts** based on your network
2. **Configure retry settings** for reliability
3. **Use streaming** for better user experience
4. **Limit history length** to prevent memory issues
5. **Enable compression** for large chat histories

Troubleshooting
---------------

Common Issues
~~~~~~~~~~~~~

**Configuration not found:**

.. code-block:: bash

   # Check current working directory
   ls -la config.json

   # Check user config directory
   ls -la ~/.config/perspt/

   # Use absolute path
   perspt --config /full/path/to/config.json

**Invalid JSON format:**

.. code-block:: bash

   # Validate JSON syntax
   cat config.json | python -m json.tool

   # Or use jq
   jq . config.json

**API key not working:**

.. code-block:: bash

   # Test API key directly
   curl -H "Authorization: Bearer $OPENAI_API_KEY" \
        "https://api.openai.com/v1/models"

   # Check environment variable
   echo $OPENAI_API_KEY

**Provider connection issues:**

.. code-block:: bash

   # Test network connectivity
   ping api.openai.com

   # Check proxy settings
   echo $HTTP_PROXY $HTTPS_PROXY

   # Test with verbose output
   perspt --config config.json --verbose

Getting Help
~~~~~~~~~~~~

If you need assistance with configuration:

1. **Check the examples** in this guide
2. **Use the validation commands** to check your config
3. **Review the error messages** - they often contain helpful hints
4. **Ask the community** on `GitHub Discussions <https://github.com/eonseed/perspt/discussions>`_
5. **File an issue** if you find a bug in configuration handling

.. seealso::

   - :doc:`getting-started` - Basic setup and first run
   - :doc:`user-guide/providers` - Provider-specific guides
   - :doc:`user-guide/troubleshooting` - Common issues and solutions
   - :doc:`user-guide/advanced-features` - Advanced usage patterns
