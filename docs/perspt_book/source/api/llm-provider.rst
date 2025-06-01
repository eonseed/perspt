LLM Provider Module
===================

The ``llm_provider`` module provides a modern, unified interface for integrating with multiple AI providers using the cutting-edge ``genai`` crate. This module enables real-time streaming responses, automatic model discovery, and consistent API behavior across different LLM services.

.. currentmodule:: llm_provider

Core Philosophy
---------------

The module is designed around these principles:

1. **Modern GenAI Integration**: Built on the latest ``genai`` crate with support for newest models like o1-mini, Gemini 2.0, and Claude 3.5
2. **Real-time Streaming**: Advanced streaming with proper event handling and reasoning chunk support
3. **Zero-Configuration**: Automatic environment variable detection with manual override options
4. **Developer-Friendly**: Comprehensive logging, error handling, and debugging capabilities
5. **Production-Ready**: Thread-safe, async-first design with proper resource management

## Supported Providers

The module supports multiple LLM providers through the genai crate:

* **OpenAI**: GPT-4, GPT-3.5, GPT-4o, o1-mini, o1-preview models
* **Anthropic**: Claude 3 (Opus, Sonnet, Haiku), Claude 3.5 models  
* **Google**: Gemini Pro, Gemini 1.5 Pro/Flash, Gemini 2.0 models
* **Groq**: Llama 3.x models with ultra-fast inference
* **Cohere**: Command R/R+ models
* **XAI**: Grok models
* **Ollama**: Local model hosting (requires local setup)

## Architecture

The provider uses the genai crate's ``Client`` as the underlying interface, which handles:

* Authentication via environment variables
* Provider-specific API endpoints and protocols
* Request/response serialization
* Rate limiting and retry logic

Core Types
----------

GenAIProvider
~~~~~~~~~~~~~

.. code-block:: rust

   pub struct GenAIProvider {
       client: Client,
   }

Main LLM provider implementation using the ``genai`` crate for unified access to multiple AI providers.

**Design Philosophy:**

The provider is designed around the principle of "configure once, use everywhere". It automatically handles provider-specific authentication requirements, API endpoints, and response formats while presenting a consistent interface to the application.

**Configuration Methods:**

1. **Auto-configuration**: Uses environment variables (recommended)
2. **Explicit configuration**: API keys and provider types via constructor  
3. **Runtime configuration**: Dynamic provider switching (future enhancement)

**Thread Safety:** The provider is thread-safe and can be shared across async tasks using ``Arc<GenAIProvider>``. The underlying genai client handles concurrent requests efficiently.

**Methods:**

new()
^^^^^

.. code-block:: rust

   pub fn new() -> Result<Self>

Creates a new GenAI provider with automatic configuration.

This constructor creates a provider instance using the genai client's default configuration, which automatically detects and uses environment variables for authentication. This is the recommended approach for production use.

**Environment Variables:**

The client will automatically detect and use these environment variables:

* ``OPENAI_API_KEY``: For OpenAI models
* ``ANTHROPIC_API_KEY``: For Anthropic Claude models
* ``GEMINI_API_KEY``: For Google Gemini models
* ``GROQ_API_KEY``: For Groq models
* ``COHERE_API_KEY``: For Cohere models
* ``XAI_API_KEY``: For XAI Grok models

**Returns:**

* ``Result<Self>`` - A configured provider instance or configuration error

**Errors:**

This method can fail if:

* The genai client cannot be initialized
* Required system dependencies are missing
* Network configuration prevents client creation

**Example:**

.. code-block:: rust

   // Set environment variable first
   std::env::set_var("OPENAI_API_KEY", "sk-your-key");

   // Create provider with auto-configuration
   let provider = GenAIProvider::new()?;

new_with_config()
^^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn new_with_config(provider_type: Option<&str>, api_key: Option<&str>) -> Result<Self>

Creates a new GenAI provider with explicit configuration.

This constructor allows explicit specification of provider type and API key, which is useful for CLI applications, testing, or when configuration needs to be provided at runtime rather than through environment variables.

**Arguments:**

* ``provider_type`` - Optional provider identifier (e.g., "openai", "anthropic")
* ``api_key`` - Optional API key for authentication

**Provider Type Mapping:**

* ``"openai"`` → Sets ``OPENAI_API_KEY``
* ``"anthropic"`` → Sets ``ANTHROPIC_API_KEY``
* ``"google"`` or ``"gemini"`` → Sets ``GEMINI_API_KEY``
* ``"groq"`` → Sets ``GROQ_API_KEY``
* ``"cohere"`` → Sets ``COHERE_API_KEY``
* ``"xai"`` → Sets ``XAI_API_KEY``

**Example:**

.. code-block:: rust

   // Create provider with explicit configuration
   let provider = GenAIProvider::new_with_config(
       Some("openai"),
       Some("sk-your-api-key")
   )?;

get_available_models()
^^^^^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub async fn get_available_models(&self, provider: &str) -> Result<Vec<String>>

Retrieves all available models for a specific provider.

This method queries the specified provider's API to get a list of all available models that can be used for chat completion. The list includes both current and legacy models, allowing users to choose the most appropriate model for their needs.

**Arguments:**

* ``provider`` - The provider identifier (e.g., "openai", "anthropic", "google")

**Provider Support:**

Model listing is supported for:

* **OpenAI**: GPT-4, GPT-3.5, GPT-4o, o1 series models
* **Anthropic**: Claude 3/3.5 series (Opus, Sonnet, Haiku)
* **Google**: Gemini Pro, Gemini 1.5/2.0 series
* **Groq**: Llama 3.x series with various sizes
* **Cohere**: Command R/R+ models
* **XAI**: Grok models
* **Ollama**: Requires local setup and running instance

**Returns:**

* ``Result<Vec<String>>`` - List of model identifiers or error

**Errors:**

This method can fail if:

* The provider name is not recognized by genai
* Network connectivity issues prevent API access
* Authentication credentials are invalid or missing
* The provider's API is temporarily unavailable
* Rate limits are exceeded

**Example:**

.. code-block:: rust

   let provider = GenAIProvider::new()?;
   
   // Get OpenAI models
   let openai_models = provider.get_available_models("openai").await?;
   for model in openai_models {
       println!("Available: {}", model);
   }

   // Get Anthropic models
   let claude_models = provider.get_available_models("anthropic").await?;

generate_response_simple()
^^^^^^^^^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub async fn generate_response_simple(&self, model: &str, prompt: &str) -> Result<String>

Generates a simple text response without streaming.

This method provides a straightforward way to get a complete response from an LLM without the complexity of streaming. It's ideal for simple Q&A scenarios, testing, or when the entire response is needed before processing.

**Arguments:**

* ``model`` - The model identifier (e.g., "gpt-4o-mini", "claude-3-5-sonnet-20241022")
* ``prompt`` - The user's message or prompt text

**Model Compatibility:**

Supports all models available through the genai crate:

* OpenAI: ``gpt-4o``, ``gpt-4o-mini``, ``gpt-3.5-turbo``, ``o1-mini``, ``o1-preview``
* Anthropic: ``claude-3-5-sonnet-20241022``, ``claude-3-opus-20240229``, etc.
* Google: ``gemini-1.5-pro``, ``gemini-1.5-flash``, ``gemini-2.0-flash``
* Groq: ``llama-3.1-70b-versatile``, ``mixtral-8x7b-32768``, etc.

**Returns:**

* ``Result<String>`` - The complete response text or error

**Example:**

.. code-block:: rust

   let provider = GenAIProvider::new_with_config(
       Some("openai"), 
       Some("sk-your-key")
   )?;

   let response = provider.generate_response_simple(
       "gpt-4o-mini",
       "What is the capital of France?"
   ).await?;

   println!("AI: {}", response);

generate_response_stream_to_channel()
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub async fn generate_response_stream_to_channel(
       &self, 
       model: &str, 
       prompt: &str,
       tx: mpsc::UnboundedSender<String>
   ) -> Result<()>

Generates a streaming response and sends chunks via mpsc channel.

This is the core streaming method that provides real-time response generation, essential for creating responsive chat interfaces. It properly handles the genai crate's streaming events and manages the async communication with the UI layer.

**Streaming Architecture:**

The method uses an async stream from the genai crate and processes different types of events:

* **Start**: Indicates the beginning of response generation
* **Chunk**: Contains incremental text content (main response text)
* **ReasoningChunk**: Contains reasoning steps (for models like o1)
* **End**: Indicates completion of response generation

**Arguments:**

* ``model`` - The model identifier to use for generation
* ``prompt`` - The user's input prompt or message
* ``tx`` - Unbounded mpsc sender for streaming response chunks to the UI

**Channel Communication:**

The method sends content chunks through the provided channel as they arrive. The receiving end (typically the UI) should listen for messages and handle:

* Regular text chunks for incremental display
* End-of-transmission signal (``EOT_SIGNAL``) indicating completion
* Error messages prefixed with "Error: " for failure cases

**Event Processing:**

1. **ChatStreamEvent::Start** - Logs stream initiation, no content sent
2. **ChatStreamEvent::Chunk** - Sends content immediately to channel
3. **ChatStreamEvent::ReasoningChunk** - Logs reasoning (future: may send to channel)
4. **ChatStreamEvent::End** - Logs completion, caller should send EOT signal

**Error Handling:**

Stream errors are handled gracefully:

* Errors are logged with full context
* Error messages are sent through the channel
* The method returns the error for caller handling
* Channel send failures are logged but don't halt processing

**Returns:**

* ``Result<()>`` - Success (content sent via channel) or error details

**Example:**

.. code-block:: rust

   use tokio::sync::mpsc;
   use perspt::EOT_SIGNAL;

   let provider = GenAIProvider::new()?;
   let (tx, mut rx) = mpsc::unbounded_channel();

   // Start streaming in background task
   let provider_clone = provider.clone();
   tokio::spawn(async move {
       match provider_clone.generate_response_stream_to_channel(
           "gpt-4o-mini",
           "Tell me about Rust programming",
           tx.clone()
       ).await {
           Ok(()) => {
               let _ = tx.send(EOT_SIGNAL.to_string());
           }
           Err(e) => {
               let _ = tx.send(format!("Error: {}", e));
               let _ = tx.send(EOT_SIGNAL.to_string());
           }
       }
   });

   // Receive and process chunks
   while let Some(chunk) = rx.recv().await {
       if chunk == EOT_SIGNAL {
           break;
       } else if chunk.starts_with("Error: ") {
           eprintln!("Stream error: {}", chunk);
           break;
       } else {
           print!("{}", chunk); // Display incremental content
       }
   }

generate_response_with_history()
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub async fn generate_response_with_history(&self, model: &str, messages: Vec<ChatMessage>) -> Result<String>

Generate response with conversation history.

**Arguments:**

* ``model`` - The model identifier
* ``messages`` - Vector of ChatMessage objects representing conversation history

**Returns:**

* ``Result<String>`` - Complete response text or error

get_supported_providers()
^^^^^^^^^^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub fn get_supported_providers() -> Vec<&'static str>

Get a list of supported providers.

**Returns:**

* ``Vec<&'static str>`` - List of supported provider identifiers

**Supported Providers:**

.. code-block:: rust

   [
       "openai",
       "anthropic", 
       "gemini",
       "groq",
       "cohere",
       "ollama",
       "xai"
   ]

test_model()
^^^^^^^^^^^^

.. code-block:: rust

   pub async fn test_model(&self, model: &str) -> Result<bool>

Test if a model is available and working.

**Arguments:**

* ``model`` - The model identifier to test

**Returns:**

* ``Result<bool>`` - True if model is working, false otherwise

validate_model()
^^^^^^^^^^^^^^^^

.. code-block:: rust

   pub async fn validate_model(&self, model: &str, provider_type: Option<&str>) -> Result<String>

Validate and get the best available model for a provider.

**Arguments:**

* ``model`` - The model identifier to validate
* ``provider_type`` - Optional provider type for fallback model selection

**Returns:**

* ``Result<String>`` - Validated model identifier or fallback model

Utility Functions
-----------------

str_to_adapter_kind()
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   fn str_to_adapter_kind(provider: &str) -> Result<AdapterKind>

Convert a provider string to genai AdapterKind.

**Arguments:**

* ``provider`` - Provider string identifier

**Returns:**

* ``Result<AdapterKind>`` - Corresponding AdapterKind enum variant

**Provider Mapping:**

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Input String
     - AdapterKind
   * - ``"openai"``
     - ``AdapterKind::OpenAI``
   * - ``"anthropic"``
     - ``AdapterKind::Anthropic``
   * - ``"gemini"``, ``"google"``
     - ``AdapterKind::Gemini``
   * - ``"groq"``
     - ``AdapterKind::Groq``
   * - ``"cohere"``
     - ``AdapterKind::Cohere``
   * - ``"ollama"``
     - ``AdapterKind::Ollama``
   * - ``"xai"``
     - ``AdapterKind::Xai``

Usage Examples
--------------

Basic Chat Interaction
~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use perspt::llm_provider::GenAIProvider;

   #[tokio::main]
   async fn main() -> Result<(), Box<dyn std::error::Error>> {
       // Initialize provider with environment variables
       let provider = GenAIProvider::new()?;
       
       // Simple question-answer
       let response = provider.generate_response_simple(
           "gpt-4o-mini",
           "Explain async programming in Rust"
       ).await?;
       
       println!("AI: {}", response);
       Ok(())
   }

Streaming Chat Interface
~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use perspt::llm_provider::GenAIProvider;
   use tokio::sync::mpsc;
   use perspt::EOT_SIGNAL;

   #[tokio::main]
   async fn main() -> Result<(), Box<dyn std::error::Error>> {
       let provider = GenAIProvider::new()?;
       let (tx, mut rx) = mpsc::unbounded_channel();
       
       // Start streaming
       tokio::spawn(async move {
           let _ = provider.generate_response_stream_to_channel(
               "claude-3-5-sonnet-20241022",
               "Write a haiku about programming",
               tx
           ).await;
       });
       
       // Display results in real-time
       while let Some(chunk) = rx.recv().await {
           if chunk == EOT_SIGNAL {
               println!("\n[Stream Complete]");
               break;
           }
           print!("{}", chunk);
       }
       
       Ok(())
   }

Error Handling Best Practices
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use perspt::llm_provider::GenAIProvider;
   use anyhow::{Context, Result};

   async fn robust_llm_call() -> Result<String> {
       let provider = GenAIProvider::new()
           .context("Failed to initialize LLM provider")?;
       
       // Test model availability first
       let model = "gpt-4o-mini";
       if !provider.test_model(model).await? {
           return Err(anyhow::anyhow!("Model {} is not available", model));
       }
       
       // Make the actual request with proper error context
       let response = provider.generate_response_simple(
           model,
           "Hello, world!"
       )
       .await
       .context(format!("Failed to generate response using model {}", model))?;
       
       Ok(response)
   }

Provider Selection
~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use perspt::llm_provider::GenAIProvider;

   async fn choose_best_provider() -> Result<(), Box<dyn std::error::Error>> {
       let provider = GenAIProvider::new()?;
       
       // Get all supported providers
       let providers = GenAIProvider::get_supported_providers();
       
       for provider_name in providers {
           println!("Checking provider: {}", provider_name);
           
           // Get available models for each provider
           if let Ok(models) = provider.get_available_models(provider_name).await {
               println!("  Available models: {:?}", models);
               
               // Test the first model
               if !models.is_empty() {
                   let works = provider.test_model(&models[0]).await.unwrap_or(false);
                   println!("  Model {} works: {}", models[0], works);
               }
           }
       }
       
       Ok(())
   }

Implementation Details
----------------------

GenAI Crate Integration
~~~~~~~~~~~~~~~~~~~~~~~

The module is built on the modern ``genai`` crate which provides:

**Unified Client Interface:**

.. code-block:: rust

   use genai::Client;
   
   // Single client handles all providers
   let client = Client::default();
   let models = client.all_model_names(AdapterKind::OpenAI).await?;

**Automatic Authentication:**

.. code-block:: rust

   // Environment variables are automatically detected:
   // OPENAI_API_KEY, ANTHROPIC_API_KEY, GEMINI_API_KEY, etc.
   let client = Client::default();

**Streaming Support:**

.. code-block:: rust

   use genai::chat::{ChatRequest, ChatMessage};
   
   let chat_req = ChatRequest::default()
       .append_message(ChatMessage::user("Hello"));
   
   let stream = client.exec_chat_stream("gpt-4o-mini", chat_req, None).await?;

**Event Processing:**

.. code-block:: rust

   use genai::chat::ChatStreamEvent;
   
   while let Some(event) = stream.stream.next().await {
       match event? {
           ChatStreamEvent::Start => println!("Stream started"),
           ChatStreamEvent::Chunk(chunk) => print!("{}", chunk.content),
           ChatStreamEvent::ReasoningChunk(chunk) => println!("Reasoning: {}", chunk.content),
           ChatStreamEvent::End(_) => println!("Stream ended"),
       }
   }

Error Handling
--------------

The module uses ``anyhow::Result`` for comprehensive error handling:

* **Configuration Errors**: Missing API keys, invalid provider types
* **Network Errors**: Connection timeouts, API rate limits  
* **Model Errors**: Invalid model names, unavailable models
* **Stream Errors**: Interrupted streams, malformed responses
* **Authentication Errors**: Invalid API keys, expired tokens

**Example Error Handling:**

.. code-block:: rust

   use anyhow::{Context, Result};
   
   async fn safe_llm_call() -> Result<String> {
       let provider = GenAIProvider::new()
           .context("Failed to create provider")?;
           
       let response = provider.generate_response_simple(
           "gpt-4o-mini",
           "Hello"
       )
       .await
       .context("Failed to generate response")?;
       
       Ok(response)
   }

**Advanced Error Recovery:**

.. code-block:: rust

   // Graceful fallback to alternative models
   async fn robust_generation(provider: &GenAIProvider, prompt: &str) -> Result<String> {
       let preferred_models = ["gpt-4o", "gpt-4o-mini", "gpt-3.5-turbo"];
       
       for model in preferred_models {
           match provider.generate_response_simple(model, prompt).await {
               Ok(response) => return Ok(response),
               Err(e) => {
                   log::warn!("Model {} failed: {}, trying next", model, e);
                   continue;
               }
           }
       }
       
       Err(anyhow::anyhow!("All models failed"))
   }

Performance Considerations
--------------------------

**Async Streaming:**

The streaming implementation is designed for optimal performance:

* Non-blocking async operations
* Immediate chunk forwarding (no batching delays)
* Minimal memory footprint
* Proper backpressure handling

**Memory Management:**

.. code-block:: rust

   // Unbounded channels for streaming (careful with memory)
   let (tx, rx) = mpsc::unbounded_channel();
   
   // Alternative: bounded channels with backpressure
   let (tx, rx) = mpsc::channel(1000);

**Logging and Debugging:**

Comprehensive logging is built-in for performance monitoring:

.. code-block:: rust

   // Enable debug logging to track stream performance
   RUST_LOG=debug ./perspt
   
   // Logs include:
   // - Chunk counts and timing
   // - Content length tracking  
   // - Stream start/end events
   // - Error conditions and recovery

Testing
-------

**Unit Tests:**

.. code-block:: rust

   #[cfg(test)]
   mod tests {
       use super::*;
       
       #[test]
       fn test_str_to_adapter_kind() {
           assert!(str_to_adapter_kind("openai").is_ok());
           assert!(str_to_adapter_kind("invalid").is_err());
       }
       
       #[tokio::test]
       async fn test_provider_creation() {
           let provider = GenAIProvider::new();
           assert!(provider.is_ok());
       }
   }

**Integration Tests:**

.. code-block:: rust

   // Test with real API keys (requires environment setup)
   #[tokio::test]
   #[ignore] // Only run with --ignored
   async fn test_live_openai() -> Result<()> {
       let provider = GenAIProvider::new()?;
       let response = provider.generate_response_simple(
           "gpt-3.5-turbo",
           "Say hello"
       ).await?;
       assert!(!response.is_empty());
       Ok(())
   }

See Also
--------

* :doc:`config` - Configuration module for provider setup and authentication
* :doc:`main` - Main module for application orchestration and LLM provider integration
* :doc:`ui` - UI module for displaying streaming responses
* `GenAI Crate Documentation <https://docs.rs/genai>`_ - Underlying LLM integration library
* `Tokio Documentation <https://docs.rs/tokio>`_ - Async runtime used for streaming

**Related Files:**

* ``src/llm_provider.rs`` - Source implementation
* ``src/config.rs`` - Configuration and provider setup
* ``src/main.rs`` - Provider initialization and usage
* ``tests/`` - Integration and unit tests
