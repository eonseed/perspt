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
   │   (CLI/Simple)  │    │    Manager      │    │   (OpenAI,etc.) │
   └─────────┬───────┘    └─────────┬───────┘    └─────────┬───────┘
             │                      │                      │
             v                      v                      v
   ┌─────────────────────────────────────────────────────────────────┐
   │                    Core Application                             │
   │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │
   │  │ UI Manager  │  │ LLM Bridge  │  │ Config Mgr  │              │
   │  │   / CLI     │  │             │  │             │              │
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

- Command-line argument parsing with clap derive macros
- Application initialization and mode selection (TUI vs Simple CLI)
- Main event loop coordination for both interface modes
- Graceful shutdown handling with terminal restoration
- Comprehensive panic handling with contextual error messages

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
           .version("0.4.5")
           .author("Vikrant Rathore")
           .about("A performant CLI for talking to LLMs using the genai crate")
           .arg(Arg::new("simple-cli")
               .long("simple-cli")
               .help("Use simple command-line interface instead of TUI")
               .action(ArgAction::SetTrue))
           .arg(Arg::new("log-file")
               .long("log-file")
               .help("Log conversation to file (Simple CLI mode only)")
               .value_name("FILE")
               .action(ArgAction::Set))
           // ... other argument definitions
           .get_matches();

       // Load configuration and create provider
       let config = config::load_config(config_path).await?;
       let provider = Arc::new(GenAIProvider::new_with_config(
           config.provider_type.as_deref(),
           config.api_key.as_deref()
       )?);

       // Route to appropriate interface mode
       if matches.get_flag("simple-cli") {
           // Simple CLI mode - minimal Unix-style interface
           let log_file = matches.get_one::<String>("log-file").cloned();
           cli::run_simple_cli(config, model_name, api_key, provider, log_file).await?;
       } else {
           // TUI mode - rich terminal interface
           let mut terminal = initialize_terminal()?;
           run_ui(&mut terminal, config, model_name, api_key, provider).await?;
           cleanup_terminal()?;
       }
       
       Ok(())
   }

   fn setup_panic_hook() {
       panic::set_hook(Box::new(move |panic_info| {
           // Force terminal restoration immediately
           let _ = disable_raw_mode();
           let _ = execute!(io::stdout(), LeaveAlternateScreen);
           
           // Provide contextual error messages and recovery tips
           let panic_str = format!("{}", panic_info);
           if panic_str.contains("PROJECT_ID") {
               eprintln!("💡 Tip: Set PROJECT_ID environment variable for Google Gemini");
           } else if panic_str.contains("API key") {
               eprintln!("💡 Tip: Set your provider's API key environment variable");
           } else if panic_str.contains("model") {
               eprintln!("💡 Tip: Use --list-models to see available models");
           }
           // ... more context-specific help
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

- Multi-provider LLM integration (OpenAI, Anthropic, Gemini, Groq, Cohere, XAI, DeepSeek, Ollama)
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

- **OpenAI**: GPT-4, GPT-3.5, GPT-4o, o1-mini, o1-preview, o3-mini, o4-mini models
- **Anthropic**: Claude 3 (Opus, Sonnet, Haiku), Claude 3.5 models
- **Google**: Gemini Pro, Gemini 1.5 Pro/Flash, Gemini 2.0 models
- **Groq**: Llama 3.x models with ultra-fast inference
- **Cohere**: Command R/R+ models
- **XAI**: Grok models (grok-3-beta, grok-3-fast-beta, etc.)
- **DeepSeek**: DeepSeek chat and reasoning models
- **Ollama**: Local model hosting (requires local setup)

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

**Enhanced Scroll Handling**:

Recent improvements to the scroll system ensure accurate display of long responses:

.. code-block:: rust

   impl App {
       /// Calculate maximum scroll position with text wrapping awareness
       pub fn max_scroll(&self) -> usize {
           // Calculate visible height for the chat area
           let chat_area_height = self.terminal_height.saturating_sub(11).max(1);
           let visible_height = chat_area_height.saturating_sub(2).max(1);
           
           // Calculate terminal width for text wrapping calculations
           let chat_width = self.input_width.saturating_sub(4).max(20);
           
           // Calculate actual rendered lines accounting for text wrapping
           let total_rendered_lines: usize = self.chat_history
               .iter()
               .map(|msg| {
                   let mut lines = 1; // Header line
                   
                   // Content lines - account for text wrapping
                   for line in &msg.content {
                       let line_text = line.spans.iter()
                           .map(|span| span.content.as_ref())
                           .collect::<String>();
                       
                       if line_text.trim().is_empty() {
                           lines += 1; // Empty lines
                       } else {
                           // Character-based text wrapping calculation
                           let display_width = line_text.chars().count();
                           if display_width <= chat_width {
                               lines += 1;
                           } else {
                               let wrapped_lines = (display_width + chat_width - 1) / chat_width;
                               lines += wrapped_lines.max(1);
                           }
                       }
                   }
                   
                   lines += 1; // Separator line after each message
                   lines
               })
               .sum();
           
           // Conservative scroll calculation to prevent content cutoff
           if total_rendered_lines > visible_height {
               let max_scroll = total_rendered_lines.saturating_sub(visible_height);
               max_scroll.saturating_sub(1) // Buffer to ensure last lines are visible
           } else {
               0
           }
       }
       
       /// Update scroll state with accurate content length calculation
       pub fn update_scroll_state(&mut self) {
           // Uses same logic as max_scroll() for consistency
           let chat_width = self.input_width.saturating_sub(4).max(20);
           let total_rendered_lines = /* same calculation as above */;
           
           self.scroll_state = self.scroll_state
               .content_length(total_rendered_lines.max(1))
               .position(self.scroll_position);
       }
   }

**Key Scroll Improvements**:

* **Text Wrapping Awareness**: Uses character count (`.chars().count()`) instead of byte length for accurate Unicode text measurement
* **Conservative Buffering**: Reduces max scroll by 1 position to prevent content cutoff at bottom
* **Consistent Separator Handling**: Always includes separator lines after each message for uniform spacing
* **Terminal Width Adaptive**: Properly calculates available chat area excluding UI borders and padding
* **Synchronized State**: Both `max_scroll()` and `update_scroll_state()` use identical line counting logic

These improvements ensure that all lines of long LLM responses are visible and properly scrollable, especially when viewing the bottom of the conversation.

Data Flow
---------

Real-time Message Processing Pipeline
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**TUI Mode (Terminal User Interface)**:

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

**Simple CLI Mode (NEW in v0.4.5)**:

1. **Input Processing**:

   .. code-block:: text

      stdin readline → Input validation → Command processing → LLM request

2. **Streaming Response**:

   .. code-block:: text

      LLM response chunks → Real-time stdout display → Session logging (optional)

3. **Session Management**:

   .. code-block:: text

      User input → Log timestamp → AI response → Log timestamp → File flush

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

Command Processing Pipeline
~~~~~~~~~~~~~~~~~~~~~~~~~~~

**Added in v0.4.3** - Built-in command system for productivity features:

1. **Command Detection**:

   .. code-block:: text

      User input → Command prefix check ('/') → Command parsing → Action dispatch

2. **Save Command Flow**:

   .. code-block:: text

      /save [filename] → Conversation validation → File generation → User feedback

3. **Command Implementation**:

   .. code-block:: rust

      impl App {
          pub fn handle_input(&mut self, input: String) -> Result<()> {
              if input.starts_with('/') {
                  // Handle built-in commands
                  self.process_command(input)?;
              } else {
                  // Handle regular chat message
                  self.send_user_message(input).await?;
              }
              Ok(())
          }
          
          fn process_command(&mut self, input: String) -> Result<()> {
              let parts: Vec<&str> = input.splitn(2, ' ').collect();
              match parts[0] {
                  "/save" => {
                      let filename = parts.get(1).map(|s| s.to_string());
                      match self.save_conversation(filename) {
                          Ok(saved_filename) => {
                              self.add_system_message(
                                  format!("💾 Conversation saved to: {}", saved_filename)
                              );
                          }
                          Err(e) => {
                              self.add_system_message(
                                  format!("❌ Error saving conversation: {}", e)
                              );
                          }
                      }
                  }
                  _ => {
                      self.add_system_message(
                          format!("❌ Unknown command: {}", parts[0])
                      );
                  }
              }
              Ok(())
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

       /// Save current conversation to a text file
       /// Added in v0.4.3 for productivity workflows
       pub fn save_conversation(&self, filename: Option<String>) -> Result<String> {
           use std::fs;
           use std::time::{SystemTime, UNIX_EPOCH};
           
           // Filter out system messages for export
           let conversation_messages: Vec<_> = self.messages
               .iter()
               .filter(|msg| matches!(msg.role, MessageRole::User | MessageRole::Assistant))
               .collect();
           
           if conversation_messages.is_empty() {
               return Err(anyhow::anyhow!("No conversation to save"));
           }
           
           // Generate filename with timestamp if not provided
           let filename = filename.unwrap_or_else(|| {
               let timestamp = SystemTime::now()
                   .duration_since(UNIX_EPOCH)
                   .unwrap()
                   .as_secs();
               format!("conversation_{}.txt", timestamp)
           });
           
           // Format conversation content
           let mut content = String::new();
           content.push_str("Perspt Conversation\n");
           content.push_str(&"=".repeat(18));
           content.push('\n');
           
           for message in conversation_messages {
               let role = match message.role {
                   MessageRole::User => "User",
                   MessageRole::Assistant => "Assistant", 
                   MessageRole::System => continue, // Skip system messages
               };
               content.push_str(&format!("[{}] {}: {}\n\n", 
                   message.timestamp.format("%Y-%m-%d %H:%M:%S"),
                   role,
                   message.content
               ));
           }
           
           // Write to file
           fs::write(&filename, content)?;
           Ok(filename)
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
               .or_else(|_| env::var("GROQ_API_KEY"))
               .or_else(|_| env::var("COHERE_API_KEY"))
               .or_else(|_| env::var("XAI_API_KEY"))
               .or_else(|_| env::var("DEEPSEEK_API_KEY"))
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
               key if key.starts_with("gsk_") => "groq".to_string(),
               key if key.starts_with("xai-") => "xai".to_string(),
               key if key.starts_with("ds-") => "deepseek".to_string(),
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
           assert_eq!(Config::infer_provider_from_key("gsk_test"), "groq");
           assert_eq!(Config::infer_provider_from_key("xai-test"), "xai");
           assert_eq!(Config::infer_provider_from_key("ds-test"), "deepseek");
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

cli.rs
~~~~~~

**NEW in v0.4.5** - Minimal command-line interface for Unix-style interactions.

**Responsibilities**:

- Unix-style prompt interface (``>``) for direct Q&A
- Session logging with timestamped conversations
- Scriptable interface for automation and workflows
- Accessibility-friendly text-only output
- Integration with existing provider and configuration systems

**Key Functions**:

.. code-block:: rust

   pub async fn run_simple_cli(
       config: AppConfig,
       model_name: String,
       api_key: String,
       provider: Arc<GenAIProvider>,
       log_file: Option<String>,
   ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
       let mut session_log = if let Some(log_path) = log_file {
           Some(SessionLogger::new(log_path)?)
       } else {
           None
       };

       println!("Perspt Simple CLI Mode");
       println!("Model: {}", model_name);
       println!("Type 'exit' or press Ctrl+D to quit.\n");

       loop {
           // Display prompt
           print!("> ");
           io::stdout().flush()?;

           // Read user input
           let mut input = String::new();
           match io::stdin().read_line(&mut input) {
               Ok(0) => break, // EOF (Ctrl+D)
               Ok(_) => {
                   let input = input.trim();
                   if input.is_empty() { continue; }
                   if input == "exit" { break; }

                   // Log user input
                   if let Some(ref mut logger) = session_log {
                       logger.log_user_input(input)?;
                   }

                   // Process with LLM
                   match process_simple_request(input, &model_name, &provider).await {
                       Ok(response) => {
                           println!("{}", response);
                           if let Some(ref mut logger) = session_log {
                               logger.log_ai_response(&response)?;
                           }
                       }
                       Err(e) => {
                           eprintln!("Error: {}", e);
                       }
                   }
                   println!(); // Add spacing between exchanges
               }
               Err(e) => {
                   eprintln!("Input error: {}", e);
                   break;
               }
           }
       }

       println!("Goodbye!");
       Ok(())
   }

   async fn process_simple_request(
       input: &str,
       model: &str,
       provider: &GenAIProvider,
   ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
       let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
       
       // Start streaming request
       provider.generate_response_stream_to_channel(model, input, tx).await?;
       
       // Collect streaming response
       let mut full_response = String::new();
       while let Some(chunk) = rx.recv().await {
           if chunk == "<<EOT>>" { break; }
           print!("{}", chunk);
           io::stdout().flush()?;
           full_response.push_str(&chunk);
       }
       
       Ok(full_response)
   }

**Session Logging Implementation**:

.. code-block:: rust

   struct SessionLogger {
       file: File,
   }

   impl SessionLogger {
       pub fn new(log_path: String) -> Result<Self, std::io::Error> {
           let file = OpenOptions::new()
               .create(true)
               .append(true)
               .open(log_path)?;
           Ok(Self { file })
       }

       pub fn log_user_input(&mut self, input: &str) -> Result<(), std::io::Error> {
           let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
           writeln!(self.file, "[{}] User: {}", timestamp, input)?;
           self.file.flush()?;
           Ok(())
       }

       pub fn log_ai_response(&mut self, response: &str) -> Result<(), std::io::Error> {
           let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
           writeln!(self.file, "[{}] Assistant: {}", timestamp, response)?;
           writeln!(self.file)?; // Add spacing
           self.file.flush()?;
           Ok(())
       }
   }

**Design Rationale**:

- **Unix Philosophy**: Simple, composable tool that follows Unix conventions
- **Streaming Support**: Real-time response display using the same streaming infrastructure as TUI mode
- **Scriptable**: Perfect for automation, shell integration, and batch processing
- **Accessibility**: Text-only output that works well with screen readers and accessibility tools
- **Session Logging**: Built-in conversation logging for documentation and audit trails

**Usage Patterns**:

.. code-block:: bash

   # Basic simple CLI mode
   perspt --simple-cli

   # With session logging
   perspt --simple-cli --log-file session.txt

   # Scripting integration
   echo "What is quantum computing?" | perspt --simple-cli

   # Chained queries
   {
     echo "What is machine learning?"
     echo "Give me 3 examples"
     echo "exit"
   } | perspt --simple-cli --log-file ml-explanation.txt
