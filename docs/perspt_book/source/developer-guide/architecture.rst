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

   #[tokio::main]
   async fn main() -> Result<()> {
       // Set up panic hook before anything else
       setup_panic_hook();
       
       // Initialize logging
       env_logger::Builder::from_default_env()
           .filter_level(LevelFilter::Error)
           .init();

       // Parse CLI arguments with clap
       let matches = Command::new("Perspt - Performance LLM Chat CLI")
           .version("0.4.0")
           .author("Vikrant Rathore")
           .about("A performant CLI for talking to LLMs using the genai crate")
           // ... argument definitions
           .get_matches();

       // Load configuration and create provider
       let config = config::load_config(config_path).await?;
       let provider = Arc::new(GenAIProvider::new_with_config(
           config.provider_type.as_deref(),
           config.api_key.as_deref()
       )?);

       // Initialize terminal and run UI
       let mut terminal = initialize_terminal()?;
       run_ui(&mut terminal, config, model_name, api_key, provider).await?;
       cleanup_terminal()?;
       
       Ok(())
   }

   fn setup_panic_hook() {
       panic::set_hook(Box::new(move |panic_info| {
           // Force terminal restoration immediately
           let _ = disable_raw_mode();
           let _ = execute!(io::stdout(), LeaveAlternateScreen);
           
           // Provide contextual error messages and recovery tips
           // ...
       }));
   }

config.rs
~~~~~~~~~

Configuration management and validation.

**Responsibilities**:

- Configuration file parsing (JSON)
- Environment variable integration
- Configuration validation and defaults
- Provider inference and API key management

**Key Structures**:

.. code-block:: rust

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Config {
       pub provider: String,
       pub api_key: Option<String>,
       pub model: Option<String>,
       pub temperature: Option<f32>,
       pub max_tokens: Option<u32>,
       pub timeout_seconds: Option<u64>,
   }

   impl Config {
       pub fn load() -> Result<Self, ConfigError> {
           // Load from file, environment, or defaults
       }

       pub fn infer_provider_from_key(api_key: &str) -> String {
           // Smart provider inference from API key format
       }

       pub fn get_effective_model(&self) -> String {
           // Get model with provider-specific defaults
       }
   }

llm_provider.rs
~~~~~~~~~~~~~~~

LLM provider abstraction using the genai crate for unified API access.

**Responsibilities**:

- Multi-provider LLM integration (OpenAI, Anthropic, Gemini, etc.)
- Streaming response handling with real-time updates
- Error handling and retry logic
- Message formatting and conversation management

**Key Functions**:

.. code-block:: rust

   use genai::chat::{ChatMessage, ChatRequest, ChatRequestOptions, ChatResponse};
   use genai::Client;

   pub async fn send_message(
       config: &Config,
       message: &str,
       tx: UnboundedSender<String>,
   ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
       // Create GenAI client with provider configuration
       let client = Client::default();
       
       // Build chat request with streaming enabled
       let chat_req = ChatRequest::new(vec![
           ChatMessage::system("You are a helpful assistant."),
           ChatMessage::user(message),
       ]);

       // Configure request options
       let options = ChatRequestOptions {
           model: Some(config.get_effective_model()),
           temperature: config.temperature,
           max_tokens: config.max_tokens,
           stream: Some(true),
           ..Default::default()
       };

       // Execute streaming request
       let stream = client.exec_stream(&chat_req, &options).await?;
       
       // Process streaming response
       while let Some(chunk) = stream.next().await {
           match chunk {
               Ok(response) => {
                   if let Some(content) = response.content_text_as_str() {
                       tx.send(content.to_string())?;
                   }
               }
               Err(e) => return Err(e.into()),
           }
       }
       
       Ok(())
   }

**Provider Support**:

The GenAI crate provides unified access to:

- **OpenAI**: GPT-3.5, GPT-4, GPT-4-turbo, o1-mini models
- **Anthropic**: Claude-3 models (Haiku, Sonnet, Opus)
- **Google**: Gemini Pro and Gemini 2.5 Pro models
- **Cohere**: Command models
- **Groq**: High-speed inference models

**Streaming Architecture**:

The streaming implementation uses Tokio channels for real-time communication:

.. code-block:: rust

   // Channel for streaming content to UI
   let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
   
   // Spawn streaming task
   let stream_task = tokio::spawn(async move {
       send_message(&config, &message, tx).await
   });
   
   // Handle streaming updates in UI thread
   while let Some(content) = rx.recv().await {
       // Update UI with new content
       update_ui_content(content);
   }

ui.rs
~~~~~

Terminal UI management using Ratatui for responsive user interaction.

**Responsibilities**:

- Real-time terminal UI rendering with Ratatui
- Cross-platform input handling with Crossterm
- Streaming content display with immediate updates
- Markdown rendering with pulldown-cmark
- Conversation history management

**Key Functions**:

.. code-block:: rust

   use ratatui::{
       backend::CrosstermBackend,
       layout::{Constraint, Direction, Layout},
       style::{Color, Modifier, Style},
       text::{Line, Span, Text},
       widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
       Frame, Terminal
   };
   use crossterm::{
       event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
       execute,
       terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
   };

   pub async fn run_ui(
       terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
       config: Config,
       model_name: String,
       api_key: String,
   ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
       let mut app = App::new(config, model_name, api_key);
       
       loop {
           // Render UI frame
           terminal.draw(|f| ui(f, &app))?;
           
           // Handle events with timeout for responsiveness
           if event::poll(Duration::from_millis(50))? {
               if let Event::Key(key) = event::read()? {
                   match app.handle_key_event(key).await {
                       Ok(should_quit) => {
                           if should_quit { break; }
                       }
                       Err(e) => app.set_error(format!("Error: {}", e)),
                   }
               }
           }
           
           // Handle streaming updates
           app.process_streaming_updates();
       }
       
       Ok(())
   }

   fn ui(f: &mut Frame, app: &App) {
       // Create responsive layout
       let chunks = Layout::default()
           .direction(Direction::Vertical)
           .constraints([
               Constraint::Min(3),     // Messages area
               Constraint::Length(3),  // Input area
               Constraint::Length(1),  // Status bar
           ])
           .split(f.size());

       // Render conversation messages
       render_messages(f, app, chunks[0]);
       
       // Render input area with prompt
       render_input_area(f, app, chunks[1]);
       
       // Render status bar with model info
       render_status_bar(f, app, chunks[2]);
   }

**Real-time Streaming**:

The UI handles streaming responses with immediate display updates:

.. code-block:: rust

   impl App {
       pub fn process_streaming_updates(&mut self) {
           // Non-blocking check for new streaming content
           while let Ok(content) = self.stream_receiver.try_recv() {
               if let Some(last_message) = self.messages.last_mut() {
                   last_message.content.push_str(&content);
                   self.scroll_to_bottom = true;
               }
           }
       }
       
       pub fn start_streaming_response(&mut self, user_message: String) {
           // Add user message to conversation
           self.add_message(Message::user(user_message.clone()));
           
           // Add placeholder for assistant response
           self.add_message(Message::assistant(String::new()));
           
           // Start streaming task
           let config = self.config.clone();
           let tx = self.stream_sender.clone();
           
           tokio::spawn(async move {
               if let Err(e) = send_message(&config, &user_message, tx).await {
                   // Handle streaming errors
                   eprintln!("Streaming error: {}", e);
               }
           });
       }
   }

**Markdown Rendering**:

Conversation messages support rich markdown formatting:

.. code-block:: rust

   use pulldown_cmark::{Event, Options, Parser, Tag};
   
   fn render_markdown_to_text(markdown: &str) -> Text {
       let parser = Parser::new_ext(markdown, Options::all());
       let mut spans = Vec::new();
       
       for event in parser {
           match event {
               Event::Text(text) => {
                   spans.push(Span::raw(text.to_string()));
               }
               Event::Code(code) => {
                   spans.push(Span::styled(
                       code.to_string(),
                       Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                   ));
               }
               Event::Start(Tag::Strong) => {
                   // Handle bold text styling
               }
               // ... other markdown elements
               _ => {}
           }
       }
       
       Text::from(Line::from(spans))
   }

Data Flow
---------

Real-time Message Processing Pipeline
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

1. **User Input Capture**:

   .. code-block:: text

      Terminal keypress → Crossterm event → Ratatui input handler → Message validation

2. **Message Processing**:

   .. code-block:: text

      User message → Conversation context → GenAI chat request → Provider routing

3. **LLM Provider Interaction**:

   .. code-block:: text

      GenAI client → HTTP streaming request → Real-time response chunks → Channel transmission

4. **Response Display**:

   .. code-block:: text

      Streaming chunks → UI update → Markdown rendering → Terminal display

Streaming Response Flow
~~~~~~~~~~~~~~~~~~~~~~~

The application uses Tokio channels for real-time streaming:

.. code-block:: rust

   async fn message_flow_example() {
       // 1. User input received
       let user_message = "Explain quantum computing";
       
       // 2. Create streaming channel
       let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
       
       // 3. Start streaming task
       let config = app.config.clone();
       tokio::spawn(async move {
           send_message(&config, &user_message, tx).await
       });
       
       // 4. Process streaming updates in real-time
       while let Some(chunk) = rx.recv().await {
           app.append_to_current_response(chunk);
           app.trigger_ui_refresh();
       }
   }

Error Handling Strategy
-----------------------

Comprehensive Error Management
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Perspt uses Rust's robust error handling with custom error types:

.. code-block:: rust

   use anyhow::{Context, Result};
   use thiserror::Error;

   #[derive(Error, Debug)]
   pub enum PersptError {
       #[error("Configuration error: {0}")]
       Config(#[from] ConfigError),
       
       #[error("LLM provider error: {0}")]
       Provider(#[from] genai::Error),
       
       #[error("UI error: {0}")]
       UI(#[from] std::io::Error),
       
       #[error("Network error: {0}")]
       Network(String),
       
       #[error("Streaming error: {0}")]
       Streaming(String),
   }

   // Graceful error recovery in main application loop
   pub async fn handle_error_with_recovery(error: PersptError) -> bool {
       match error {
           PersptError::Network(_) => {
               // Show retry dialog, attempt reconnection
               show_retry_dialog();
               true // Continue running
           }
           PersptError::Provider(_) => {
               // Try fallback provider if available
               attempt_provider_fallback();
               true
           }
           PersptError::UI(_) => {
               // Terminal issues - attempt recovery
               attempt_terminal_recovery();
               false // May need to exit
           }
           _ => {
               // Log error and continue
               log::error!("Application error: {}", error);
               true
           }
       }
   }

Memory Management
-----------------

Efficient Message Storage
~~~~~~~~~~~~~~~~~~~~~~~~~

Perspt manages conversation history efficiently in memory:

.. code-block:: rust

   #[derive(Debug, Clone)]
   pub struct Message {
       pub role: MessageRole,
       pub content: String,
       pub timestamp: std::time::SystemTime,
   }

   #[derive(Debug, Clone)]
   pub enum MessageRole {
       User,
       Assistant,
       System,
   }

   impl Message {
       pub fn user(content: String) -> Self {
           Self {
               role: MessageRole::User,
               content,
               timestamp: std::time::SystemTime::now(),
           }
       }

       pub fn assistant(content: String) -> Self {
           Self {
               role: MessageRole::Assistant,
               content,
               timestamp: std::time::SystemTime::now(),
           }
       }
   }

   // Conversation management with memory optimization
   pub struct App {
       messages: Vec<Message>,
       max_history: usize,
       // ... other fields
   }

   impl App {
       pub fn add_message(&mut self, message: Message) {
           self.messages.push(message);
           
           // Limit memory usage by keeping only recent messages
           if self.messages.len() > self.max_history {
               self.messages.drain(0..self.messages.len() - self.max_history);
           }
       }
   }

Streaming Buffer Management
~~~~~~~~~~~~~~~~~~~~~~~~~~~

For streaming responses, Perspt uses efficient buffering:

.. code-block:: rust

   impl App {
       pub fn append_to_current_response(&mut self, content: String) {
           if let Some(last_message) = self.messages.last_mut() {
               match last_message.role {
                   MessageRole::Assistant => {
                       last_message.content.push_str(&content);
                   }
                   _ => {
                       // Create new assistant message if needed
                       self.add_message(Message::assistant(content));
                   }
               }
           }
       }
   }

Concurrency Model
-----------------

Async Architecture with Tokio
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Perspt uses Tokio for efficient asynchronous operations:

.. code-block:: rust

   #[tokio::main]
   async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
       // Initialize panic handler
       setup_panic_hook();
       
       // Parse CLI arguments
       let args = Args::parse();
       
       // Load configuration
       let config = Config::load()?;
       
       // Setup terminal
       enable_raw_mode()?;
       let mut stdout = io::stdout();
       execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
       let backend = CrosstermBackend::new(stdout);
       let mut terminal = Terminal::new(backend)?;
       
       // Run main UI loop
       let result = run_ui(&mut terminal, config, args.model, args.api_key).await;
       
       // Cleanup
       disable_raw_mode()?;
       execute!(
           terminal.backend_mut(),
           LeaveAlternateScreen,
           DisableMouseCapture
       )?;
       terminal.show_cursor()?;
       
       result
   }

Task Management
~~~~~~~~~~~~~~~

The application manages multiple concurrent tasks:

.. code-block:: rust

   pub struct TaskManager {
       streaming_tasks: Vec<tokio::task::JoinHandle<()>>,
       ui_refresh_task: Option<tokio::task::JoinHandle<()>>,
   }

   impl App {
       pub async fn handle_user_input(&mut self, input: String) {
           // Spawn streaming task for LLM communication
           let config = self.config.clone();
           let tx = self.stream_sender.clone();
           
           let handle = tokio::spawn(async move {
               if let Err(e) = send_message(&config, &input, tx).await {
                   log::error!("Streaming error: {}", e);
               }
           });
           
           self.task_manager.streaming_tasks.push(handle);
           
           // Cleanup completed tasks
           self.cleanup_completed_tasks();
       }
       
       fn cleanup_completed_tasks(&mut self) {
           self.task_manager.streaming_tasks.retain(|handle| !handle.is_finished());
       }
   }

Real-time Event Processing
~~~~~~~~~~~~~~~~~~~~~~~~~~

The UI event loop handles multiple event sources concurrently:

.. code-block:: rust

   pub async fn run_ui(
       terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
       config: Config,
       model_name: String,
       api_key: String,
   ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
       let mut app = App::new(config, model_name, api_key);
       
       loop {
           // Render UI
           terminal.draw(|f| ui(f, &app))?;
           
           // Handle multiple event sources
           tokio::select! {
               // Terminal input events
               event = async {
                   if event::poll(Duration::from_millis(50))? {
                       Some(event::read()?)
                   } else {
                       None
                   }
               } => {
                   if let Some(Event::Key(key)) = event {
                       if app.handle_key_event(key).await? {
                           break;
                       }
                   }
               }
               
               // Streaming content updates
               content = app.stream_receiver.recv() => {
                   if let Some(content) = content {
                       app.append_to_current_response(content);
                   }
               }
               
               // Periodic UI refresh
               _ = tokio::time::sleep(Duration::from_millis(16)) => {
                   // 60 FPS refresh rate for smooth UI
               }
           }
       }
       
       Ok(())
   }
           let id = RequestId::new();
           let handle = tokio::spawn(async move {
               tokio::time::timeout(self.request_timeout, process_request(request)).await
           });
           self.active_requests.insert(id, handle);
           id
       }
   }

Security Considerations
-----------------------

API Key Management
~~~~~~~~~~~~~~~~~~

Perspt handles API keys securely through environment variables and configuration:

.. code-block:: rust

   impl Config {
       pub fn load() -> Result<Self, ConfigError> {
           // Try environment variable first (most secure)
           let api_key = env::var("OPENAI_API_KEY")
               .or_else(|_| env::var("ANTHROPIC_API_KEY"))
               .or_else(|_| env::var("GEMINI_API_KEY"))
               .ok();
           
           // Load from config file as fallback
           let mut config = Self::load_from_file().unwrap_or_default();
           
           // Environment variables take precedence
           if let Some(key) = api_key {
               config.api_key = Some(key);
               config.provider = Self::infer_provider_from_key(&key);
           }
           
           Ok(config)
       }

       pub fn infer_provider_from_key(api_key: &str) -> String {
           match api_key {
               key if key.starts_with("sk-") => "openai".to_string(),
               key if key.starts_with("claude-") => "anthropic".to_string(),
               key if key.starts_with("AIza") => "gemini".to_string(),
               _ => "openai".to_string(), // Default fallback
           }
       }
   }

Input Validation and Sanitization
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

User input is validated before processing:

.. code-block:: rust

   impl App {
       pub fn validate_user_input(&self, input: &str) -> Result<String, ValidationError> {
           // Check input length limits
           if input.len() > MAX_MESSAGE_LENGTH {
               return Err(ValidationError::TooLong);
           }
           
           // Remove control characters
           let sanitized = input
               .chars()
               .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
               .collect::<String>();
           
           // Trim whitespace
           let sanitized = sanitized.trim().to_string();
           
           if sanitized.is_empty() {
               return Err(ValidationError::Empty);
           }
           
           Ok(sanitized)
       }
   }

Secure Error Handling
~~~~~~~~~~~~~~~~~~~~~

Error messages are sanitized to prevent information leakage:

.. code-block:: rust

   pub fn sanitize_error_message(error: &dyn std::error::Error) -> String {
       match error.to_string() {
           msg if msg.contains("API key") => "Authentication error".to_string(),
           msg if msg.contains("token") => "Authentication error".to_string(),
           msg => {
               // Remove potentially sensitive information
               msg.lines()
                   .filter(|line| !line.contains("Bearer") && !line.contains("Authorization"))
                   .collect::<Vec<_>>()
                   .join("\n")
           }
       }
   }

Testing Architecture
--------------------

Unit Testing Strategy
~~~~~~~~~~~~~~~~~~~~~

Perspt includes comprehensive unit tests for each module:

.. code-block:: rust

   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn test_config_loading() {
           let config = Config::load().unwrap();
           assert!(!config.provider.is_empty());
       }

       #[test]
       fn test_provider_inference() {
           assert_eq!(Config::infer_provider_from_key("sk-test"), "openai");
           assert_eq!(Config::infer_provider_from_key("claude-test"), "anthropic");
           assert_eq!(Config::infer_provider_from_key("AIza-test"), "gemini");
       }

       #[test]
       fn test_message_creation() {
           let msg = Message::user("Hello".to_string());
           assert!(matches!(msg.role, MessageRole::User));
           assert_eq!(msg.content, "Hello");
       }

       #[test]
       fn test_input_validation() {
           let app = App::default();
           
           // Valid input
           assert!(app.validate_user_input("Hello world").is_ok());
           
           // Empty input
           assert!(app.validate_user_input("").is_err());
           
           // Too long input
           let long_input = "a".repeat(10000);
           assert!(app.validate_user_input(&long_input).is_err());
       }
   }

Integration Testing
~~~~~~~~~~~~~~~~~~~

Integration tests verify the complete application flow:

.. code-block:: rust

   // tests/integration_tests.rs
   use perspt::*;
   use std::env;

   #[tokio::test]
   async fn test_full_conversation_flow() {
       // Skip if no API key available
       if env::var("OPENAI_API_KEY").is_err() {
           return;
       }

       let config = Config {
           provider: "openai".to_string(),
           api_key: env::var("OPENAI_API_KEY").ok(),
           model: Some("gpt-3.5-turbo".to_string()),
           temperature: Some(0.7),
           max_tokens: Some(100),
           timeout_seconds: Some(30),
       };

       let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
       
       // Test streaming response
       let result = send_message(&config, "Hello, how are you?", tx).await;
       assert!(result.is_ok());

       // Verify we receive streaming content
       let mut received_content = String::new();
       while let Ok(content) = rx.try_recv() {
           received_content.push_str(&content);
       }
       assert!(!received_content.is_empty());
   }

   #[test]
   fn test_config_loading_hierarchy() {
       // Test config loading from different sources
       let config = Config::load().unwrap();
       assert!(!config.provider.is_empty());
   }

Performance Considerations
--------------------------

Optimization Strategies
~~~~~~~~~~~~~~~~~~~~~~~

Perspt is optimized for performance through several key strategies:

1. **Streaming Responses**: Immediate display of LLM responses as they arrive
2. **Efficient Memory Management**: Limited conversation history with automatic cleanup
3. **Async/Await Architecture**: Non-blocking operations with Tokio
4. **Minimal Dependencies**: Fast compilation and small binary size
5. **Zero-Copy Operations**: Efficient string handling where possible

**Real-time Performance Metrics**:

.. code-block:: rust

   impl App {
       pub fn get_performance_stats(&self) -> PerformanceStats {
           PerformanceStats {
               messages_per_second: self.calculate_message_rate(),
               memory_usage_mb: self.get_memory_usage(),
               ui_refresh_rate: 60.0, // Target 60 FPS
               streaming_latency_ms: self.get_average_streaming_latency(),
           }
       }
       
       fn calculate_message_rate(&self) -> f64 {
           let recent_messages = self.messages.iter()
               .filter(|m| m.timestamp.elapsed().unwrap().as_secs() < 60)
               .count();
           recent_messages as f64 / 60.0
       }
   }

Memory Optimization
~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   const MAX_HISTORY_MESSAGES: usize = 100;
   const MAX_MESSAGE_LENGTH: usize = 8192;

   impl App {
       pub fn optimize_memory(&mut self) {
           // Remove old messages if exceeding limit
           if self.messages.len() > MAX_HISTORY_MESSAGES {
               let keep_from = self.messages.len() - MAX_HISTORY_MESSAGES;
               self.messages.drain(0..keep_from);
           }
           
           // Compact long messages
           for message in &mut self.messages {
               if message.content.len() > MAX_MESSAGE_LENGTH {
                   message.content.truncate(MAX_MESSAGE_LENGTH);
                   message.content.push_str("... [truncated]");
               }
           }
       }
   }

Future Architecture Considerations
----------------------------------

Planned Enhancements
~~~~~~~~~~~~~~~~~~~~

Based on the current GenAI-powered architecture, future enhancements include:

1. **Multi-Provider Streaming**: Simultaneous requests to multiple providers with fastest response wins
2. **Enhanced Conversation Context**: Intelligent context window management for long conversations  
3. **Plugin Architecture**: Extensible plugin system for custom commands and integrations
4. **Advanced UI Components**: Rich markdown rendering, syntax highlighting, and interactive elements
5. **Offline Mode**: Local model support for privacy-sensitive scenarios

**Implementation Roadmap**:

.. code-block:: rust

   // Future: Multi-provider streaming
   pub async fn stream_from_multiple_providers(
       providers: &[String],
       message: &str,
   ) -> Result<impl Stream<Item = String>, Error> {
       let streams = providers.iter().map(|provider| {
           let config = Config::for_provider(provider);
           send_message_stream(&config, message)
       });
       
       // Return the fastest responding stream
       futures::stream::select_all(streams)
   }

   // Future: Plugin system
   pub trait Plugin: Send + Sync {
       async fn execute(&self, command: &str, args: &[String]) -> PluginResult;
       fn commands(&self) -> Vec<String>;
   }

Migration Strategies
~~~~~~~~~~~~~~~~~~~~

For evolutionary architecture changes:

1. **GenAI Provider Expansion**: Easy addition of new providers through the genai crate
2. **Configuration Evolution**: Backward-compatible config format changes
3. **UI Component Modularity**: Incremental UI improvements without breaking changes
4. **Streaming Protocol Evolution**: Enhanced streaming with metadata and typing indicators

Next Steps
----------

For developers looking to contribute or extend Perspt:

- :doc:`contributing` - Contribution guidelines and development setup
- :doc:`extending` - Creating custom providers and plugins  
- :doc:`testing` - Testing strategies and guidelines
- :doc:`../api/index` - API reference and integration guides

The architecture is designed to be extensible and maintainable, making it easy to add new features while preserving the core performance and reliability characteristics.
