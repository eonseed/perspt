perspt-core API
===============

The core crate providing LLM abstraction, configuration, and memory management.

Overview
--------

``perspt-core`` contains the fundamental abstractions used by all other crates:

- **GenAIProvider** - Thread-safe LLM provider with streaming support
- **Config** - Simple configuration struct
- **Memory** - Conversation memory management

GenAIProvider
-------------

Thread-safe LLM provider built on the ``genai`` crate with ``Arc<RwLock>`` for safe sharing across async tasks.

Struct Definition
~~~~~~~~~~~~~~~~~

.. code-block:: rust

   /// Thread-safe LLM provider implementation using Arc<RwLock<>>.
   ///
   /// This provider can be cheaply cloned and shared across multiple agents.
   /// Each clone shares the same underlying client and rate limiting state.
   #[derive(Clone)]
   pub struct GenAIProvider {
       /// The underlying genai client
       client: Arc<Client>,
       /// Shared state for rate limiting and metrics
       shared: Arc<RwLock<SharedState>>,
   }

   struct SharedState {
       total_tokens_used: usize,
       request_count: usize,
   }

Constructor Methods
~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   impl GenAIProvider {
       /// Creates a new GenAI provider with automatic configuration.
       /// 
       /// Uses genai's default client which auto-detects API keys
       /// from environment variables.
       pub fn new() -> Result<Self>

       /// Creates a new GenAI provider with explicit configuration.
       /// 
       /// # Arguments
       /// * `provider_type` - Provider name: "openai", "anthropic", "gemini", etc.
       /// * `api_key` - API key for the provider
       /// 
       /// Sets the appropriate environment variable before creating the client.
       pub fn new_with_config(
           provider_type: Option<&str>,
           api_key: Option<&str>
       ) -> Result<Self>
   }

Streaming Response
~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   impl GenAIProvider {
       /// Generates a streaming response to a channel.
       ///
       /// Sends tokens as they arrive via the provided mpsc sender.
       /// Sends EOT_SIGNAL ("<|EOT|>") when complete.
       ///
       /// # Arguments
       /// * `model` - Model identifier (e.g., "gpt-5.2", "claude-opus-4.5")
       /// * `messages` - Conversation history as ChatMessage vec
       /// * `sender` - Channel to send streaming tokens
       pub async fn generate_response_stream_to_channel(
           &self,
           model: &str,
           messages: Vec<ChatMessage>,
           sender: mpsc::Sender<String>,
       ) -> Result<()>
   }

Metrics Methods
~~~~~~~~~~~~~~~

.. code-block:: rust

   impl GenAIProvider {
       /// Get total tokens used across all requests
       pub async fn get_total_tokens_used(&self) -> usize

       /// Get total request count
       pub async fn get_request_count(&self) -> usize
   }

Supported Providers
~~~~~~~~~~~~~~~~~~~

The provider type maps to environment variables:

.. list-table::
   :header-rows: 1

   * - Provider
     - Environment Variable
   * - ``openai``
     - ``OPENAI_API_KEY``
   * - ``anthropic``
     - ``ANTHROPIC_API_KEY``
   * - ``gemini``
     - ``GEMINI_API_KEY``
   * - ``groq``
     - ``GROQ_API_KEY``
   * - ``cohere``
     - ``COHERE_API_KEY``
   * - ``xai``
     - ``XAI_API_KEY``
   * - ``deepseek``
     - ``DEEPSEEK_API_KEY``
   * - ``ollama``
     - (none - local)

Config
------

Simple configuration struct:

.. code-block:: rust

   /// Main configuration struct
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Config {
       pub provider: String,
       pub model: String,
       pub api_key: Option<String>,
   }

   impl Default for Config {
       fn default() -> Self {
           Self {
               provider: "openai".to_string(),
               model: "gpt-4".to_string(),
               api_key: None,
           }
       }
   }

Memory
------

Conversation memory management for context handling.

Usage Example
-------------

.. code-block:: rust

   use perspt_core::llm_provider::GenAIProvider;
   use tokio::sync::mpsc;

   #[tokio::main]
   async fn main() -> Result<()> {
       // Create provider with auto-detection
       let provider = GenAIProvider::new()?;

       // Create channel for streaming
       let (tx, mut rx) = mpsc::channel(100);

       // Start streaming
       tokio::spawn(async move {
           while let Some(token) = rx.recv().await {
               if token == "<|EOT|>" { break; }
               print!("{}", token);
           }
       });

       // Generate response
       provider.generate_response_stream_to_channel(
           "gpt-5.2",
           vec![ChatMessage::user("Hello!")],
           tx,
       ).await?;

       Ok(())
   }

Source Code
-----------

- ``crates/perspt-core/src/lib.rs``
- ``crates/perspt-core/src/llm_provider.rs``
- ``crates/perspt-core/src/config.rs``
- ``crates/perspt-core/src/memory.rs``
