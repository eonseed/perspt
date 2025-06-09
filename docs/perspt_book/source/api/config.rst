Configuration Module
====================

The ``config`` module provides comprehensive configuration management for Perspt, supporting multiple LLM providers, flexible authentication, and intelligent defaults.

.. currentmodule:: config

Core Structures
---------------

AppConfig
~~~~~~~~~

.. code-block:: rust

   #[derive(Debug, Clone, Deserialize, PartialEq)]
   pub struct AppConfig {
       pub providers: HashMap<String, String>,
       pub api_key: Option<String>,
       pub default_model: Option<String>,
       pub default_provider: Option<String>,
       pub provider_type: Option<String>,
   }

The main configuration structure containing all configurable aspects of Perspt.

**Fields:**

* ``providers`` - Map of provider names to their API base URLs
* ``api_key`` - Universal API key for authentication
* ``default_model`` - Default model identifier for LLM requests
* ``default_provider`` - Name of default provider configuration
* ``provider_type`` - Provider type classification for API compatibility

**Supported Provider Types:**

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Provider Type
     - Description
   * - ``openai``
     - OpenAI GPT models (GPT-4o, GPT-4o-mini, o1-preview, o1-mini, o3-mini, o4-mini)
   * - ``anthropic``
     - Anthropic Claude models (Claude 3.5 Sonnet, Claude 3 Opus/Sonnet/Haiku)
   * - ``gemini``
     - Google Gemini models (Gemini 1.5 Pro/Flash, Gemini 2.0 Flash)
   * - ``groq``
     - Groq ultra-fast inference (Llama 3.x models)
   * - ``cohere``
     - Cohere Command models (Command R, Command R+)
   * - ``xai``
     - XAI Grok models (grok-3-beta, grok-3-fast-beta)
   * - ``deepseek``
     - DeepSeek models (deepseek-chat, deepseek-reasoner)
   * - ``ollama``
     - Local model hosting via Ollama (requires local setup)

**Example Configuration:**

.. code-block:: json

   {
     "api_key": "sk-your-api-key",
     "provider_type": "openai",
     "default_model": "gpt-4o-mini",
     "default_provider": "openai",
     "providers": {
       "openai": "https://api.openai.com/v1",
       "anthropic": "https://api.anthropic.com",
       "gemini": "https://generativelanguage.googleapis.com/v1beta",
       "groq": "https://api.groq.com/openai/v1",
       "cohere": "https://api.cohere.com/v1",
       "xai": "https://api.x.ai/v1",
       "deepseek": "https://api.deepseek.com/v1",
       "ollama": "http://localhost:11434/v1"
     }
   }

Core Functions
--------------

process_loaded_config
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   pub fn process_loaded_config(mut config: AppConfig) -> AppConfig

Processes and validates loaded configuration, applying intelligent defaults and provider type inference.

**Parameters:**

* ``config`` - The configuration to process

**Returns:**

* ``AppConfig`` - Processed configuration with inferred values

**Provider Type Inference Logic:**

If ``provider_type`` is None, attempts inference from ``default_provider``:

.. list-table::
   :header-rows: 1
   :widths: 30 30 40

   * - Default Provider
     - Inferred Type
     - Notes
   * - ``openai``
     - ``openai``
     - Direct mapping
   * - ``anthropic``
     - ``anthropic``
     - Direct mapping
   * - ``google``, ``gemini``
     - ``gemini``
     - Multiple aliases supported
   * - ``groq``
     - ``groq``
     - Direct mapping
   * - ``cohere``
     - ``cohere``
     - Direct mapping
   * - ``xai``
     - ``xai``
     - Direct mapping
   * - ``deepseek``
     - ``deepseek``
     - Direct mapping
   * - ``ollama``
     - ``ollama``
     - Direct mapping
   * - Unknown
     - ``openai``
     - Fallback default

**Example:**

.. code-block:: rust

   let mut config = AppConfig {
       providers: HashMap::new(),
       api_key: None,
       default_model: None,
       default_provider: Some("anthropic".to_string()),
       provider_type: None, // Will be inferred as "anthropic"
   };

   let processed = process_loaded_config(config);
   assert_eq!(processed.provider_type, Some("anthropic".to_string()));

load_config
~~~~~~~~~~~

.. code-block:: rust

   pub async fn load_config(config_path: Option<&String>) -> Result<AppConfig>

Loads application configuration from a file or provides comprehensive defaults.

**Parameters:**

* ``config_path`` - Optional path to JSON configuration file

**Returns:**

* ``Result<AppConfig>`` - Loaded configuration or error

**Behavior:**

With Configuration File (Some(path))
  1. Reads JSON file from filesystem
  2. Parses JSON into AppConfig structure
  3. Processes configuration with ``process_loaded_config()``
  4. Returns processed configuration

Without Configuration File (None)
  Creates default configuration with all supported provider endpoints pre-configured and OpenAI as default provider.

**Default Provider Endpoints:**

.. code-block:: json

   {
       "openai": "https://api.openai.com/v1",
       "anthropic": "https://api.anthropic.com", 
       "gemini": "https://generativelanguage.googleapis.com/v1beta",
       "groq": "https://api.groq.com/openai/v1",
       "cohere": "https://api.cohere.com/v1",
       "xai": "https://api.x.ai/v1",
       "deepseek": "https://api.deepseek.com/v1",
       "ollama": "http://localhost:11434/v1"
   }

**Possible Errors:**

* File system errors (file not found, permission denied)
* JSON parsing errors (invalid syntax, missing fields)
* I/O errors during file reading

**Examples:**

.. code-block:: rust

   // Load from specific file
   let config = load_config(Some(&"config.json".to_string())).await?;

   // Use defaults
   let default_config = load_config(None).await?;

   // Error handling
   match load_config(Some(&"missing.json".to_string())).await {
       Ok(config) => println!("Loaded: {:?}", config),
       Err(e) => eprintln!("Failed to load config: {}", e),
   }

Configuration Examples
----------------------

Basic OpenAI Configuration
~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: json

   {
     "api_key": "sk-your-openai-key",
     "provider_type": "openai",
     "default_model": "gpt-4o-mini"
   }

Multi-Provider Configuration
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: json

   {
     "api_key": "your-default-key",
     "provider_type": "anthropic",
     "default_model": "claude-3-5-sonnet-20241022",
     "default_provider": "anthropic",
     "providers": {
       "openai": "https://api.openai.com/v1",
       "anthropic": "https://api.anthropic.com",
       "gemini": "https://generativelanguage.googleapis.com/v1beta",
       "groq": "https://api.groq.com/openai/v1",
       "cohere": "https://api.cohere.com/v1",
       "xai": "https://api.x.ai/v1",
       "deepseek": "https://api.deepseek.com/v1",
       "ollama": "http://localhost:11434/v1"
     }
   }

Minimal Configuration with Provider Inference
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: json

   {
     "default_provider": "groq",
     "default_model": "llama-3.1-70b-versatile"
   }

*Provider type will be automatically inferred as "groq"*

Development with Multiple Models
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: json

   {
     "provider_type": "openai",
     "default_model": "gpt-4o-mini",
     "default_provider": "openai",
     "providers": {
       "openai": "https://api.openai.com/v1",
       "groq": "https://api.groq.com/openai/v1",
       "ollama": "http://localhost:11434/v1"
     }
   }

See Also
--------

* :doc:`../configuration` - User configuration guide
* :doc:`llm-provider` - LLM provider implementation
* :doc:`../user-guide/providers` - Provider setup guide
