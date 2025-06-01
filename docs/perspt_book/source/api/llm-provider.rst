LLM Provider Module
===================

The ``llm_provider`` module provides a unified interface for integrating with multiple AI providers through the ``genai`` crate. This module enables automatic model discovery, dynamic provider support, and consistent API behavior across different LLM services.

.. currentmodule:: llm_provider

Core Philosophy
---------------

The module is designed around these principles:

1. **Automatic Updates**: Leverages ``genai`` crate for automatic support of new models and providers
2. **Dynamic Discovery**: Uses ``try_from_str()`` for validation and future compatibility
3. **Consistent API**: Unified interface across all providers
4. **Reduced Maintenance**: No manual tracking of model names or API changes

Core Types
----------

ProviderType
~~~~~~~~~~~~

.. code-block:: rust

   #[derive(Debug, Clone, PartialEq)]
   pub enum ProviderType {
       OpenAI,
       Anthropic,
       Google,
       Mistral,
       Perplexity,
       DeepSeek,
       AwsBedrock,
   }

Enumeration of supported LLM provider types.

**Methods:**

from_string()
^^^^^^^^^^^^^

.. code-block:: rust

   pub fn from_string(s: &str) -> Option<Self>

Converts string representation to ProviderType enum.

**Parameters:**

* ``s`` - String representation of provider type

**Returns:**

* ``Option<ProviderType>`` - Some(provider) if recognized, None otherwise

**Supported Strings:**

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Input String
     - Output
   * - ``"openai"``
     - ``ProviderType::OpenAI``
   * - ``"anthropic"``
     - ``ProviderType::Anthropic``
   * - ``"google"``, ``"gemini"``
     - ``ProviderType::Google``
   * - ``"mistral"``
     - ``ProviderType::Mistral``
   * - ``"perplexity"``
     - ``ProviderType::Perplexity``
   * - ``"deepseek"``
     - ``ProviderType::DeepSeek``
   * - ``"aws"``, ``"bedrock"``, ``"aws-bedrock"``
     - ``ProviderType::AwsBedrock``

**Example:**

.. code-block:: rust

   let provider_type = ProviderType::from_string("anthropic");
   assert_eq!(provider_type, Some(ProviderType::Anthropic));

   let unknown = ProviderType::from_string("unknown");
   assert_eq!(unknown, None);

to_string()
^^^^^^^^^^^

.. code-block:: rust

   pub fn to_string(&self) -> &'static str

Converts ProviderType enum to canonical string representation.

**Returns:**

* ``&'static str`` - String representation of the provider type

**Example:**

.. code-block:: rust

   let provider = ProviderType::Anthropic;
   assert_eq!(provider.to_string(), "anthropic");

UnifiedLLMProvider
~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   #[derive(Debug)]
   pub struct UnifiedLLMProvider {
       provider_type: ProviderType,
   }

Main LLM provider implementation using the ``genai`` crate for unified access to multiple AI providers.

**Methods:**

new()
^^^^^

.. code-block:: rust

   pub fn new(provider_type: ProviderType) -> Self

Creates a new UnifiedLLMProvider instance.

**Parameters:**

* ``provider_type`` - The type of provider to create

**Returns:**

* ``Self`` - New provider instance

**Example:**

.. code-block:: rust

   let provider = UnifiedLLMProvider::new(ProviderType::OpenAI);

get_available_models()
^^^^^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn get_available_models(&self) -> Vec<String>

Retrieves all available models for the provider type using the ``genai`` crate enums.

**Returns:**

* ``Vec<String>`` - List of available model identifiers

**Model Sources by Provider:**

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Provider
     - Model Source
   * - OpenAI
     - ``OpenAIModels`` enum from genai
   * - Anthropic
     - ``AnthropicModels`` enum from genai
   * - Google
     - ``GoogleModels`` enum from genai
   * - Mistral
     - ``MistralModels`` enum from genai
   * - Perplexity
     - ``PerplexityModels`` enum from genai
   * - DeepSeek
     - ``DeepSeekModels`` enum from genai
   * - AWS Bedrock
     - ``AwsBedrockModels`` enum from genai

**Example:**

.. code-block:: rust

   let provider = UnifiedLLMProvider::new(ProviderType::OpenAI);
   let models = provider.get_available_models();
   // Returns: ["gpt-4o-mini", "gpt-4o", "gpt-4-turbo", ...]

SimpleResponse
~~~~~~~~~~~~~~

.. code-block:: rust

   #[derive(Debug, Deserialize, Serialize)]
   pub struct SimpleResponse {
       pub content: String,
   }

Simple response structure for LLM completions.

**Fields:**

* ``content`` - The response content from the LLM

Traits
------

LLMProvider
~~~~~~~~~~~

.. code-block:: rust

   #[async_trait]
   pub trait LLMProvider {
       async fn list_models(&self) -> LLMResult<Vec<String>>;
       async fn send_chat_request(&self, input: &str, model_name: &str, config: &AppConfig, tx: &mpsc::UnboundedSender<String>) -> LLMResult<()>;
       fn provider_type(&self) -> ProviderType;
       async fn validate_config(&self, config: &AppConfig) -> LLMResult<()>;
   }

Unified trait for all LLM providers, providing consistent interface across different AI services.

**Methods:**

list_models()
^^^^^^^^^^^^^

.. code-block:: rust

   async fn list_models(&self) -> LLMResult<Vec<String>>

Lists all available models for the provider.

**Returns:**

* ``LLMResult<Vec<String>>`` - List of model identifiers or error

**Example:**

.. code-block:: rust

   let provider = UnifiedLLMProvider::new(ProviderType::Anthropic);
   let models = provider.list_models().await?;
   println!("Available models: {:?}", models);

send_chat_request()
^^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   async fn send_chat_request(
       &self,
       input: &str,
       model_name: &str,
       config: &AppConfig,
       tx: &mpsc::UnboundedSender<String>,
   ) -> LLMResult<()>

Sends a chat request to the LLM with streaming response.

**Parameters:**

* ``input`` - The user's message/prompt
* ``model_name`` - Model identifier to use
* ``config`` - Application configuration
* ``tx`` - Channel for streaming responses

**Returns:**

* ``LLMResult<()>`` - Success or error

**Behavior:**

1. Validates API key from configuration
2. Creates completion request using ``genai`` crate
3. Simulates streaming by sending response in chunks
4. Sends ``EOT_SIGNAL`` when complete

**Error Handling:**

* Missing API key
* Invalid model name
* Network connectivity issues
* Provider-specific errors

**Example:**

.. code-block:: rust

   let (tx, mut rx) = mpsc::unbounded_channel();
   let provider = UnifiedLLMProvider::new(ProviderType::OpenAI);
   let config = load_config(None).await?;

   provider.send_chat_request(
       "Hello, how are you?",
       "gpt-4o-mini",
       &config,
       &tx
   ).await?;

   // Receive streaming response
   while let Some(chunk) = rx.recv().await {
       if chunk == EOT_SIGNAL {
           break;
       }
       print!("{}", chunk);
   }

provider_type()
^^^^^^^^^^^^^^^

.. code-block:: rust

   fn provider_type(&self) -> ProviderType

Returns the provider type for this instance.

**Returns:**

* ``ProviderType`` - The provider type enum

validate_config()
^^^^^^^^^^^^^^^^^

.. code-block:: rust

   async fn validate_config(&self, config: &AppConfig) -> LLMResult<()>

Validates if the provider can be used with the given configuration.

**Parameters:**

* ``config`` - Configuration to validate

**Returns:**

* ``LLMResult<()>`` - Success or validation error

**Validation Checks:**

* API key presence and format
* Required environment variables
* Provider-specific configuration requirements

**Example:**

.. code-block:: rust

   let provider = UnifiedLLMProvider::new(ProviderType::Anthropic);
   let config = load_config(None).await?;

   match provider.validate_config(&config).await {
       Ok(()) => println!("Configuration valid"),
       Err(e) => eprintln!("Configuration error: {}", e),
   }

Type Aliases
------------

LLMResult<T>
~~~~~~~~~~~~

.. code-block:: rust

   pub type LLMResult<T> = Result<T>;

Standard result type for LLM operations using ``anyhow::Result``.

Implementation Details
----------------------

Provider-Specific API Integration
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**OpenAI:**

.. code-block:: rust

   use genai::llm_models::OpenAIModels;

   // Model enumeration
   let models: Vec<String> = OpenAIModels::iter()
       .map(|model| model.to_string())
       .collect();

   // API request
   let completions = Completions::new(&model_str, &api_key);

**Anthropic:**

.. code-block:: rust

   use genai::llm_models::AnthropicModels;

   // Model enumeration
   let models: Vec<String> = AnthropicModels::iter()
       .map(|model| model.to_string())
       .collect();

**Google:**

.. code-block:: rust

   use genai::llm_models::GoogleModels;

   // Model enumeration with Gemini support
   let models: Vec<String> = GoogleModels::iter()
       .map(|model| model.to_string())
       .collect();

Constants
---------

EOT_SIGNAL
~~~~~~~~~~

.. code-block:: rust

   pub const EOT_SIGNAL: &str = "<<EOT>>";

End-of-transmission signal used to indicate completion of streaming responses.

Error Handling
--------------

The module uses ``anyhow::Result`` for comprehensive error handling:

* **Configuration Errors**: Missing API keys, invalid provider types
* **Network Errors**: Connection timeouts, API rate limits
* **Validation Errors**: Invalid model names, malformed requests
* **Provider Errors**: Service-specific error responses

**Example Error Handling:**

.. code-block:: rust

   match provider.send_chat_request(input, model, &config, &tx).await {
       Ok(()) => println!("Request successful"),
       Err(e) => {
           match e.downcast_ref::<std::io::Error>() {
               Some(io_err) => eprintln!("Network error: {}", io_err),
               None => eprintln!("Other error: {}", e),
           }
       }
   }

See Also
--------

* :doc:`config` - Configuration module for provider setup
* :doc:`../user-guide/providers` - User guide for provider configuration
* :doc:`../developer-guide/extending` - Guide for adding new providers
