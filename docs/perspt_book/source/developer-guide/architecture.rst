Architecture
============

This document provides a comprehensive overview of Perspt's architecture, design principles, and internal structure.

Overview
--------

Perspt is built as a modular, extensible command-line application written in Rust. The architecture emphasizes:

- **Modularity**: Clear separation of concerns with well-defined interfaces
- **Extensibility**: Plugin-based architecture for adding new providers and features
- **Performance**: Efficient resource usage and fast response times
- **Reliability**: Robust error handling and graceful degradation
- **Security**: Safe handling of API keys and user data

High-Level Architecture
-----------------------

.. code-block:: text

   ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
   │   User Input    │    │  Configuration  │    │   AI Providers  │
   │     (CLI)       │    │    Manager      │    │   (OpenAI,etc.) │
   └─────────┬───────┘    └─────────┬───────┘    └─────────┬───────┘
             │                      │                      │
             v                      v                      v
   ┌─────────────────────────────────────────────────────────────────┐
   │                    Core Application                             │
   │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │
   │  │ UI Manager  │  │ LLM Bridge  │  │ Config Mgr  │              │
   │  └─────────────┘  └─────────────┘  └─────────────┘              │
   └─────────────────────────────────────────────────────────────────┘
             │                      │                      │
             v                      v                      v
   ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
   │   Conversation  │    │    Provider     │    │    Storage      │
   │     Manager     │    │    Registry     │    │    Layer        │
   └─────────────────┘    └─────────────────┘    └─────────────────┘

Core Components
---------------

main.rs
~~~~~~~

The application entry point and orchestration layer.

**Responsibilities**:

- Command-line argument parsing
- Application initialization
- Main event loop coordination
- Graceful shutdown handling

**Key Functions**:

.. code-block:: rust

   fn main() -> Result<(), Box<dyn std::error::Error>> {
       // Initialize logging and configuration
       // Setup signal handlers
       // Create and run the application
   }

   async fn run_application(config: Config) -> Result<()> {
       // Initialize core components
       // Start the main interaction loop
       // Handle user input and AI responses
   }

config.rs
~~~~~~~~~

Configuration management and validation.

**Responsibilities**:

- Configuration file parsing (JSON/TOML)
- Environment variable integration
- Configuration validation and defaults
- Provider-specific configuration handling

**Key Structures**:

.. code-block:: rust

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Config {
       pub provider: String,
       pub model: String,
       pub api_key: Option<String>,
       pub max_tokens: Option<u32>,
       pub temperature: Option<f32>,
       pub providers: HashMap<String, ProviderConfig>,
       // ... other fields
   }

   pub trait ConfigProvider {
       fn load() -> Result<Config, ConfigError>;
       fn validate(&self) -> Result<(), ConfigError>;
       fn merge(&mut self, other: Config);
   }

llm_provider.rs
~~~~~~~~~~~~~~~

AI provider abstraction and implementation.

**Responsibilities**:

- Provider abstraction layer
- HTTP client management
- Request/response handling
- Error handling and retry logic
- Streaming response support

**Key Traits**:

.. code-block:: rust

   #[async_trait]
   pub trait LLMProvider: Send + Sync {
       async fn chat_completion(
           &self,
           messages: &[Message],
           options: &ChatOptions,
       ) -> Result<ChatResponse, LLMError>;
       
       async fn stream_completion(
           &self,
           messages: &[Message],
           options: &ChatOptions,
       ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, LLMError>>>>, LLMError>;
       
       fn validate_config(&self, config: &ProviderConfig) -> Result<(), LLMError>;
   }

**Provider Implementations**:

.. code-block:: rust

   pub struct OpenAIProvider {
       client: reqwest::Client,
       config: OpenAIConfig,
   }

   pub struct AnthropicProvider {
       client: reqwest::Client,
       config: AnthropicConfig,
   }

   pub struct OllamaProvider {
       client: reqwest::Client,
       config: OllamaConfig,
   }

ui.rs
~~~~~

User interface and interaction management.

**Responsibilities**:

- Terminal UI rendering
- Input handling and command parsing
- Response formatting and display
- Command execution
- History management

**Key Components**:

.. code-block:: rust

   pub struct UIManager {
       terminal: Terminal<CrosstermBackend<io::Stdout>>,
       input_handler: InputHandler,
       display_manager: DisplayManager,
       command_processor: CommandProcessor,
   }

   pub trait InputHandler {
       fn read_input(&mut self) -> Result<UserInput, UIError>;
       fn handle_special_keys(&mut self, key: KeyEvent) -> Option<Action>;
   }

   pub trait DisplayManager {
       fn render_message(&mut self, message: &Message) -> Result<(), UIError>;
       fn render_typing_indicator(&mut self) -> Result<(), UIError>;
       fn clear_screen(&mut self) -> Result<(), UIError>;
   }

Data Flow
---------

Message Processing Pipeline
~~~~~~~~~~~~~~~~~~~~~~~~~~~

1. **User Input Capture**:

   .. code-block:: text

      User types message → Input validation → Command detection

2. **Message Processing**:

   .. code-block:: text

      Raw input → Message parsing → Context preparation → Provider routing

3. **AI Provider Interaction**:

   .. code-block:: text

      API request preparation → HTTP client call → Response parsing → Error handling

4. **Response Display**:

   .. code-block:: text

      Response processing → Formatting → Terminal rendering → History storage

Conversation Flow
~~~~~~~~~~~~~~~~~

.. code-block:: rust

   pub struct ConversationManager {
       messages: Vec<Message>,
       context_limit: usize,
       history_storage: Box<dyn HistoryStorage>,
   }

   impl ConversationManager {
       pub fn add_message(&mut self, message: Message) {
           self.messages.push(message);
           self.trim_context_if_needed();
       }

       pub fn get_context(&self) -> Vec<Message> {
           self.messages.iter()
               .take(self.context_limit)
               .cloned()
               .collect()
       }
   }

Error Handling Strategy
-----------------------

Error Types
~~~~~~~~~~~

.. code-block:: rust

   #[derive(Debug, thiserror::Error)]
   pub enum PersptError {
       #[error("Configuration error: {0}")]
       Config(#[from] ConfigError),
       
       #[error("LLM provider error: {0}")]
       LLM(#[from] LLMError),
       
       #[error("UI error: {0}")]
       UI(#[from] UIError),
       
       #[error("Network error: {0}")]
       Network(#[from] reqwest::Error),
       
       #[error("IO error: {0}")]
       IO(#[from] std::io::Error),
   }

Error Recovery
~~~~~~~~~~~~~~

.. code-block:: rust

   pub struct ErrorRecovery {
       retry_strategy: RetryStrategy,
       fallback_providers: Vec<String>,
       graceful_degradation: bool,
   }

   impl ErrorRecovery {
       pub async fn handle_error(&self, error: PersptError) -> RecoveryAction {
           match error {
               PersptError::Network(_) => self.retry_with_backoff().await,
               PersptError::LLM(LLMError::RateLimit) => self.wait_and_retry().await,
               PersptError::LLM(LLMError::InvalidKey) => RecoveryAction::RequireUserAction,
               _ => self.try_fallback_provider().await,
           }
       }
   }

Memory Management
-----------------

Conversation History
~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   pub struct HistoryManager {
       max_memory_mb: usize,
       compression_threshold: usize,
       storage_backend: StorageBackend,
   }

   impl HistoryManager {
       pub fn optimize_memory(&mut self) {
           if self.memory_usage() > self.max_memory_mb {
               self.compress_old_conversations();
               self.archive_distant_history();
           }
       }
   }

Caching Strategy
~~~~~~~~~~~~~~~

.. code-block:: rust

   pub struct ResponseCache {
       cache: LruCache<MessageHash, CachedResponse>,
       ttl: Duration,
       max_size: usize,
   }

   impl ResponseCache {
       pub fn get(&self, messages: &[Message]) -> Option<&CachedResponse> {
           let hash = self.hash_messages(messages);
           self.cache.get(&hash)
               .filter(|response| !response.is_expired())
       }
   }

Concurrency Model
-----------------

Async Architecture
~~~~~~~~~~~~~~~~~

Perspt uses Tokio for asynchronous operations:

.. code-block:: rust

   #[tokio::main]
   async fn main() -> Result<()> {
       let runtime = tokio::runtime::Builder::new_multi_thread()
           .worker_threads(4)
           .enable_all()
           .build()?;
           
       runtime.spawn(handle_user_input());
       runtime.spawn(handle_ai_responses());
       runtime.spawn(background_tasks());
       
       // Main event loop
   }

Task Management
~~~~~~~~~~~~~~

.. code-block:: rust

   pub struct TaskManager {
       active_requests: HashMap<RequestId, JoinHandle<Result<Response>>>,
       request_timeout: Duration,
   }

   impl TaskManager {
       pub async fn submit_request(&mut self, request: Request) -> RequestId {
           let id = RequestId::new();
           let handle = tokio::spawn(async move {
               tokio::time::timeout(self.request_timeout, process_request(request)).await
           });
           self.active_requests.insert(id, handle);
           id
       }
   }

Plugin Architecture
-------------------

Plugin Interface
~~~~~~~~~~~~~~~

.. code-block:: rust

   #[async_trait]
   pub trait Plugin: Send + Sync {
       fn name(&self) -> &str;
       fn version(&self) -> &str;
       
       async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError>;
       async fn handle_command(&self, command: &str, args: &[String]) -> Result<PluginResponse, PluginError>;
       fn supported_commands(&self) -> Vec<String>;
   }

Plugin Manager
~~~~~~~~~~~~~

.. code-block:: rust

   pub struct PluginManager {
       plugins: HashMap<String, Box<dyn Plugin>>,
       plugin_configs: HashMap<String, PluginConfig>,
   }

   impl PluginManager {
       pub async fn load_plugin(&mut self, path: &Path) -> Result<(), PluginError> {
           // Dynamic loading of plugins
           // Plugin validation and initialization
       }
       
       pub async fn execute_command(&self, plugin_name: &str, command: &str, args: &[String]) -> Result<PluginResponse, PluginError> {
           // Command routing and execution
       }
   }

Security Considerations
-----------------------

API Key Management
~~~~~~~~~~~~~~~~~

.. code-block:: rust

   pub struct SecureStorage {
       keyring: Option<Keyring>,
       fallback_encrypted: bool,
   }

   impl SecureStorage {
       pub fn store_api_key(&self, provider: &str, key: &str) -> Result<(), SecurityError> {
           if let Some(keyring) = &self.keyring {
               keyring.set_password("perspt", provider, key)?;
           } else {
               self.store_encrypted(provider, key)?;
           }
           Ok(())
       }
   }

Request Sanitization
~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   pub struct RequestSanitizer;

   impl RequestSanitizer {
       pub fn sanitize_message(message: &str) -> String {
           // Remove potential sensitive patterns
           // Validate input length and format
           // Apply content filtering if configured
       }
   }

Testing Architecture
--------------------

Unit Testing
~~~~~~~~~~~

.. code-block:: rust

   #[cfg(test)]
   mod tests {
       use super::*;
       use mockall::predicate::*;
       
       #[tokio::test]
       async fn test_openai_provider() {
           let mock_client = MockHttpClient::new();
           mock_client.expect_post()
               .with(eq("https://api.openai.com/v1/chat/completions"))
               .returning(|_| Ok(mock_response()));
               
           let provider = OpenAIProvider::new(mock_client);
           let result = provider.chat_completion(&messages, &options).await;
           assert!(result.is_ok());
       }
   }

Integration Testing
~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   #[cfg(test)]
   mod integration_tests {
       use super::*;
       
       #[tokio::test]
       async fn test_full_conversation_flow() {
           let config = test_config();
           let app = Application::new(config).await?;
           
           let response = app.process_message("Hello, world!").await?;
           assert!(!response.content.is_empty());
       }
   }

Performance Considerations
-------------------------

Optimization Strategies
~~~~~~~~~~~~~~~~~~~~~~

1. **Connection Pooling**: Reuse HTTP connections
2. **Request Batching**: Combine multiple requests when possible
3. **Response Streaming**: Start displaying responses immediately
4. **Intelligent Caching**: Cache frequently requested content
5. **Memory Optimization**: Efficient memory usage patterns

Monitoring and Metrics
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   pub struct Metrics {
       request_count: Counter,
       response_time: Histogram,
       error_rate: Gauge,
       memory_usage: Gauge,
   }

   impl Metrics {
       pub fn record_request(&self, duration: Duration, success: bool) {
           self.request_count.inc();
           self.response_time.observe(duration.as_secs_f64());
           if !success {
               self.error_rate.inc();
           }
       }
   }

Future Architecture Considerations
----------------------------------

Planned Enhancements
~~~~~~~~~~~~~~~~~~~

1. **Multi-Provider Parallel Requests**: Send requests to multiple providers simultaneously
2. **Advanced Caching**: Semantic caching based on intent rather than exact text
3. **Plugin Ecosystem**: Rich plugin marketplace and development tools
4. **Distributed Mode**: Support for distributed deployments
5. **Advanced Security**: Zero-knowledge encryption for conversation storage

Migration Strategies
~~~~~~~~~~~~~~~~~~~

For major architectural changes:

1. **Backward Compatibility**: Maintain API compatibility during transitions
2. **Gradual Migration**: Phased rollout of new components
3. **Feature Flags**: Toggle new functionality during development
4. **Data Migration**: Safe migration of user data and configurations

Next Steps
----------

For developers looking to contribute:

- :doc:`contributing` - Contribution guidelines and development setup
- :doc:`extending` - Creating plugins and extensions
- :doc:`testing` - Testing strategies and guidelines
- :doc:`../api/index` - API reference and integration guides
