Extending Perspt
================

This guide covers how to extend Perspt with custom providers, plugins, and integrations based on the current GenAI-powered architecture.

Extension Overview
------------------

Perspt's architecture allows several extension points:

- **Custom LLM Providers**: Add new providers through the GenAI crate
- **UI Components**: Enhance the Ratatui-based terminal interface
- **Configuration Extensions**: Add custom configuration options
- **Command Extensions**: Implement custom slash commands
- **Streaming Enhancements**: Custom streaming response processing

Working with GenAI Providers
----------------------------

Adding New Providers
~~~~~~~~~~~~~~~~~~~~

Perspt uses the `genai` crate which supports multiple providers out of the box. To add support for a new provider:

.. code-block:: rust

   // In config.rs - Add provider support
   impl Config {
       pub fn infer_provider_from_key(api_key: &str) -> String {
           match api_key {
               key if key.starts_with("sk-") => "openai".to_string(),
               key if key.starts_with("claude-") => "anthropic".to_string(),
               key if key.starts_with("AIza") => "gemini".to_string(),
               key if key.starts_with("gsk_") => "groq".to_string(),
               key if key.starts_with("xai-") => "xai".to_string(),
               key if key.starts_with("ds-") => "deepseek".to_string(),
               key if key.starts_with("hf_") => "huggingface".to_string(), // New provider
               key if key.starts_with("co_") => "cohere".to_string(),       // New provider
               _ => "openai".to_string(),
           }
       }

       pub fn get_effective_model(&self) -> String {
           match self.model {
               Some(ref model) => model.clone(),
               None => match self.provider.as_str() {
                   "openai" => "gpt-4o-mini".to_string(),
                   "anthropic" => "claude-3-5-sonnet-20241022".to_string(),
                   "gemini" => "gemini-1.5-flash".to_string(),
                   "groq" => "llama-3.1-70b-versatile".to_string(),
                   "cohere" => "command-r-plus".to_string(),
                   "xai" => "grok-3-beta".to_string(),
                   "deepseek" => "deepseek-chat".to_string(),
                   "ollama" => "llama3.2".to_string(),
                   "huggingface" => "microsoft/DialoGPT-medium".to_string(), // New default
                   _ => "gpt-4o-mini".to_string(),
               }
           }
       }
   }

Custom Provider Implementation
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

For providers not supported by GenAI, you can extend the message handling:

.. code-block:: rust

   // In llm_provider.rs - Custom provider wrapper
   pub async fn send_message_custom_provider(
       config: &Config,
       message: &str,
       tx: UnboundedSender<String>,
   ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
       match config.provider.as_str() {
           "custom_provider" => {
               send_message_to_custom_api(config, message, tx).await
           }
           _ => {
               // Use standard GenAI implementation
               send_message(config, message, tx).await
           }
       }
   }

   async fn send_message_to_custom_api(
       config: &Config,
       message: &str,
       tx: UnboundedSender<String>,
   ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
       // Custom HTTP client implementation
       let client = reqwest::Client::new();
       
       let payload = serde_json::json!({
           "prompt": message,
           "max_tokens": config.max_tokens.unwrap_or(1000),
           "temperature": config.temperature.unwrap_or(0.7),
           "stream": true
       });

       let response = client
           .post("https://api.custom-provider.com/v1/chat")
           .header("Authorization", format!("Bearer {}", config.api_key.as_ref().unwrap()))
           .json(&payload)
           .send()
           .await?;

       // Handle streaming response
       let mut stream = response.bytes_stream();
       while let Some(chunk) = stream.next().await {
           let chunk = chunk?;
           if let Ok(text) = String::from_utf8(chunk.to_vec()) {
               tx.send(text)?;
           }
       }

       Ok(())
   }

Extending UI Components
-----------------------

Custom Terminal UI Elements
~~~~~~~~~~~~~~~~~~~~~~~~~~~

You can extend the Ratatui-based UI with custom components:

.. code-block:: rust

   // In ui.rs - Custom rendering components
   use ratatui::{
       prelude::*,
       widgets::{Block, Borders, Paragraph, Wrap},
   };

   pub fn render_custom_status_bar(f: &mut Frame, app: &App, area: Rect) {
       let status_text = format!(
           "Provider: {} | Model: {} | Messages: {} | Memory: {:.1}MB",
           app.config.provider,
           app.config.get_effective_model(),
           app.messages.len(),
           app.get_memory_usage_mb()
       );

       let status_paragraph = Paragraph::new(status_text)
           .style(Style::default().fg(Color::Yellow))
           .block(Block::default().borders(Borders::TOP));

       f.render_widget(status_paragraph, area);
   }

   pub fn render_typing_indicator(f: &mut Frame, area: Rect, is_typing: bool) {
       if is_typing {
           let indicator = Paragraph::new("AI is typing...")
               .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::ITALIC))
               .wrap(Wrap { trim: true });
           
           f.render_widget(indicator, area);
       }
   }

   // Custom markdown rendering enhancements
   pub fn render_enhanced_markdown(content: &str) -> Text {
       use pulldown_cmark::{Event, Parser, Tag};
       
       let parser = Parser::new(content);
       let mut spans = Vec::new();
       let mut current_style = Style::default();
       
       for event in parser {
           match event {
               Event::Start(Tag::Emphasis) => {
                   current_style = current_style.add_modifier(Modifier::ITALIC);
               }
               Event::Start(Tag::Strong) => {
                   current_style = current_style.add_modifier(Modifier::BOLD);
               }
               Event::Start(Tag::CodeBlock(_)) => {
                   current_style = Style::default()
                       .fg(Color::Green)
                       .bg(Color::Black);
               }
               Event::Text(text) => {
                   spans.push(Span::styled(text.to_string(), current_style));
               }
               Event::End(_) => {
                   current_style = Style::default();
               }
               _ => {}
           }
       }
       
       Text::from(Line::from(spans))
   }

Enhanced Scroll Handling
~~~~~~~~~~~~~~~~~~~~~~~~~

Recent improvements to Perspt's scroll system demonstrate best practices for handling long content in terminal UIs:

.. code-block:: rust

   // Custom scroll handling for terminal applications
   impl App {
       /// Advanced scroll calculation accounting for text wrapping
       pub fn calculate_content_height(&self, content: &[ChatMessage], terminal_width: usize) -> usize {
           let chat_width = terminal_width.saturating_sub(4).max(20); // Account for borders
           
           content.iter().map(|msg| {
               let mut lines = 1; // Header line
               
               // Calculate wrapped content lines
               for line in &msg.content {
                   let line_text = line.spans.iter()
                       .map(|span| span.content.as_ref())
                       .collect::<String>();
                   
                   if line_text.trim().is_empty() {
                       lines += 1;
                   } else {
                       // Character-aware text wrapping (important for Unicode)
                       let display_width = line_text.chars().count();
                       if display_width <= chat_width {
                           lines += 1;
                       } else {
                           let wrapped_lines = (display_width + chat_width - 1) / chat_width;
                           lines += wrapped_lines.max(1);
                       }
                   }
               }
               
               lines += 1; // Separator line
               lines
           }).sum()
       }
       
       /// Conservative scroll bounds to prevent content cutoff
       pub fn calculate_max_scroll(&self, content_height: usize, visible_height: usize) -> usize {
           if content_height > visible_height {
               let max_scroll = content_height.saturating_sub(visible_height);
               // Conservative buffer to ensure bottom content is always visible
               max_scroll.saturating_sub(1)
           } else {
               0
           }
       }
   }

**Key Extension Points for Scroll Handling**:

* **Text Wrapping Logic**: Customize how text wraps based on content type or user preferences
* **Scroll Animation**: Add smooth scrolling animations for better user experience  
* **Auto-scroll Behavior**: Implement smart auto-scrolling that respects user navigation intent
* **Content-aware Scrolling**: Different scroll behavior for code blocks, lists, or other content types
* **Accessibility Features**: Add scroll indicators, position feedback, or keyboard shortcuts

**Best Practices for Terminal UI Scrolling**:

1. **Character-based calculations**: Always use `.chars().count()` for Unicode-safe text measurement
2. **Conservative buffering**: Leave small buffers to prevent content cutoff at boundaries
3. **Consistent state**: Keep scroll calculation logic identical across all scroll methods
4. **Terminal adaptation**: Account for borders, padding, and other UI elements in calculations
5. **User feedback**: Provide visual indicators (scrollbars, position info) for scroll state

Configuration Extensions
------------------------

Adding Custom Configuration Options
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

You can extend the configuration system to support custom options:

.. code-block:: rust

   // Extended configuration structure
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct ExtendedConfig {
       #[serde(flatten)]
       pub base: Config,
       
       // Custom extensions
       pub custom_theme: Option<String>,
       pub auto_save: Option<bool>,
       pub custom_commands: Option<HashMap<String, String>>,
       pub ui_preferences: Option<UiPreferences>,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct UiPreferences {
       pub show_timestamps: bool,
       pub message_limit: usize,
       pub enable_syntax_highlighting: bool,
       pub custom_colors: Option<ColorScheme>,
   }

   impl ExtendedConfig {
       pub fn load_extended() -> Result<Self, ConfigError> {
           // Try to load extended config first
           if let Ok(config_str) = fs::read_to_string("config.extended.json") {
               return serde_json::from_str(&config_str)
                   .map_err(|e| ConfigError::ParseError(e.to_string()));
           }
           
           // Fallback to base config
           let base_config = Config::load()?;
           Ok(ExtendedConfig {
               base: base_config,
               custom_theme: None,
               auto_save: Some(true),
               custom_commands: None,
               ui_preferences: Some(UiPreferences::default()),
           })
       }
   }

Custom Command System
~~~~~~~~~~~~~~~~~~~~~

Implement custom slash commands for enhanced functionality:

.. code-block:: rust

   // In main.rs or ui.rs - Command processing
   pub enum CustomCommand {
       SaveConversation(String),
       LoadConversation(String),
       SetTheme(String),
       ShowStats,
       ClearHistory,
       ExportMarkdown(String),
   }

   impl CustomCommand {
       pub fn parse(input: &str) -> Option<Self> {
           let parts: Vec<&str> = input.trim_start_matches('/').split_whitespace().collect();
           
           match parts.get(0)? {
               "save" => Some(CustomCommand::SaveConversation(
                   parts.get(1).unwrap_or("conversation").to_string()
               )),
               "load" => Some(CustomCommand::LoadConversation(
                   parts.get(1).unwrap_or("conversation").to_string()
               )),
               "theme" => Some(CustomCommand::SetTheme(
                   parts.get(1).unwrap_or("default").to_string()
               )),
               "stats" => Some(CustomCommand::ShowStats),
               "clear" => Some(CustomCommand::ClearHistory),
               "export" => Some(CustomCommand::ExportMarkdown(
                   parts.get(1).unwrap_or("conversation.md").to_string()
               )),
               _ => None,
           }
       }

       pub async fn execute(&self, app: &mut App) -> Result<String, Box<dyn std::error::Error>> {
           match self {
               CustomCommand::SaveConversation(name) => {
                   app.save_conversation(name).await?;
                   Ok(format!("Conversation saved as '{}'", name))
               }
               CustomCommand::LoadConversation(name) => {
                   app.load_conversation(name).await?;
                   Ok(format!("Conversation '{}' loaded", name))
               }
               CustomCommand::SetTheme(theme) => {
                   app.set_theme(theme);
                   Ok(format!("Theme changed to '{}'", theme))
               }
               CustomCommand::ShowStats => {
                   let stats = app.get_conversation_stats();
                   Ok(format!(
                       "Messages: {}, Total characters: {}, Session time: {}min",
                       stats.message_count,
                       stats.total_characters,
                       stats.session_time_minutes
                   ))
               }
               CustomCommand::ClearHistory => {
                   app.clear_conversation_history();
                   Ok("Conversation history cleared".to_string())
               }
               CustomCommand::ExportMarkdown(filename) => {
                   app.export_to_markdown(filename).await?;
                   Ok(format!("Conversation exported to '{}'", filename))
               }
           }
       }
   }
       pub timeout: Option<u64>,
   }

   pub struct CustomProvider {
       client: reqwest::Client,
       config: CustomProviderConfig,
   }

   impl CustomProvider {
       pub fn new(config: CustomProviderConfig) -> Self {
           let client = reqwest::Client::builder()
               .timeout(std::time::Duration::from_secs(config.timeout.unwrap_or(30)))
               .build()
               .expect("Failed to create HTTP client");

           Self { client, config }
       }
   }

   #[async_trait]
   impl LLMProvider for CustomProvider {
       async fn chat_completion(
           &self,
           messages: &[Message],
           options: &ChatOptions,
       ) -> Result<ChatResponse, LLMError> {
           let request_body = self.build_request(messages, options)?;
           
           let response = self.client
               .post(&format!("{}/chat/completions", self.config.base_url))
               .header("Authorization", format!("Bearer {}", self.config.api_key))
               .header("Content-Type", "application/json")
               .json(&request_body)
               .send()
               .await
               .map_err(|e| LLMError::NetworkError(e.to_string()))?;

           let response_body: CustomResponse = response
               .json()
               .await
               .map_err(|e| LLMError::ParseError(e.to_string()))?;

           Ok(self.parse_response(response_body)?)
       }

       async fn stream_completion(
           &self,
           messages: &[Message],
           options: &ChatOptions,
       ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, LLMError>>>>, LLMError> {
           // Implement streaming response handling
           todo!("Implement streaming for your provider")
       }

       fn validate_config(&self, config: &ProviderConfig) -> Result<(), LLMError> {
           // Validate provider-specific configuration
           if self.config.api_key.is_empty() {
               return Err(LLMError::ConfigurationError("API key is required".to_string()));
           }
           Ok(())
       }
   }

Advanced Provider Features
~~~~~~~~~~~~~~~~~~~~~~~~~~

**Function Calling Support**:

.. code-block:: rust

   impl CustomProvider {
       fn build_request_with_functions(
           &self,
           messages: &[Message],
           options: &ChatOptions,
           functions: &[Function],
       ) -> Result<CustomRequest, LLMError> {
           CustomRequest {
               model: self.config.model.clone(),
               messages: self.convert_messages(messages),
               functions: Some(functions.iter().map(|f| f.into()).collect()),
               function_call: options.function_call.clone(),
               // ... other fields
           }
       }
   }

**Multimodal Support**:

.. code-block:: rust

   #[async_trait]
   impl MultimodalProvider for CustomProvider {
       async fn chat_completion_with_images(
           &self,
           messages: &[Message],
           images: &[ImageData],
           options: &ChatOptions,
       ) -> Result<ChatResponse, LLMError> {
           let request = self.build_multimodal_request(messages, images, options)?;
           // Implementation
       }
   }

Creating Custom Commands
------------------------

Built-in Command System
~~~~~~~~~~~~~~~~~~~~~~~

**Added in v0.4.3** - Perspt includes a built-in command system for productivity features. Commands are prefixed with ``/`` and processed before regular chat messages.

**Save Conversation Command Implementation**:

.. code-block:: rust

   impl App {
       /// Handle built-in commands like /save
       pub fn handle_command(&mut self, input: String) -> Result<bool, String> {
           if !input.starts_with('/') {
               return Ok(false); // Not a command
           }
           
           let parts: Vec<&str> = input.splitn(2, ' ').collect();
           let command = parts[0];
           
           match command {
               "/save" => {
                   let filename = parts.get(1).map(|s| s.to_string());
                   self.execute_save_command(filename)
               }
               "/help" => {
                   self.show_help_command();
                   Ok(true)
               }
               _ => {
                   self.add_system_message(format!("❌ Unknown command: {}", command));
                   Ok(true)
               }
           }
       }
       
       fn execute_save_command(&mut self, filename: Option<String>) -> Result<bool, String> {
           match self.save_conversation(filename) {
               Ok(saved_filename) => {
                   self.add_system_message(format!("💾 Conversation saved to: {}", saved_filename));
                   Ok(true)
               }
               Err(e) => {
                   self.add_system_message(format!("❌ Error saving conversation: {}", e));
                   Ok(true) // Command was handled, even with error
               }
           }
       }
       
       /// Save conversation to text file with proper formatting
       pub fn save_conversation(&self, filename: Option<String>) -> Result<String> {
           use std::fs;
           use std::time::{SystemTime, UNIX_EPOCH};
           
           // Filter conversation messages (exclude system messages)
           let conversation_messages: Vec<_> = self.chat_history
               .iter()
               .filter(|msg| matches!(msg.message_type, MessageType::User | MessageType::Assistant))
               .collect();
           
           if conversation_messages.is_empty() {
               return Err(anyhow::anyhow!("No conversation to save"));
           }
           
           // Generate timestamped filename if not provided
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
               let role = match message.message_type {
                   MessageType::User => "User",
                   MessageType::Assistant => "Assistant",
                   _ => continue, // Skip other message types
               };
               
               content.push_str(&format!("[{}] {}: {}\n\n", 
                   message.timestamp,
                   role,
                   message.raw_content
               ));
           }
           
           // Write to file
           fs::write(&filename, content)?;
           Ok(filename)
       }
   }

**Command Registration System**:

.. code-block:: rust

   pub struct CommandRegistry {
       commands: HashMap<String, Box<dyn Command>>,
   }
   
   pub trait Command: Send + Sync {
       fn name(&self) -> &str;
       fn description(&self) -> &str;
       fn execute(&self, app: &mut App, args: Vec<&str>) -> Result<(), String>;
   }
   
   impl CommandRegistry {
       pub fn new() -> Self {
           let mut registry = Self {
               commands: HashMap::new(),
           };
           
           // Register built-in commands
           registry.register(Box::new(SaveCommand));
           registry.register(Box::new(HelpCommand));
           
           registry
       }
       
       pub fn register(&mut self, command: Box<dyn Command>) {
           self.commands.insert(command.name().to_string(), command);
       }
       
       pub fn execute(&self, app: &mut App, input: String) -> Result<bool, String> {
           let parts: Vec<&str> = input.splitn(2, ' ').collect();
           let command_name = parts[0].trim_start_matches('/');
           
           if let Some(command) = self.commands.get(command_name) {
               let args = if parts.len() > 1 {
                   parts[1].split_whitespace().collect()
               } else {
                   vec![]
               };
               
               command.execute(app, args)?;
               Ok(true)
           } else {
               Ok(false) // Command not found
           }
       }
   }

Adding Custom Commands
~~~~~~~~~~~~~~~~~~~~~~

You can extend the command system with your own commands:

.. code-block:: rust

   pub struct ExportMarkdownCommand;
   
   impl Command for ExportMarkdownCommand {
       fn name(&self) -> &str {
           "export-md"
       }
       
       fn description(&self) -> &str {
           "Export conversation as markdown with formatting preserved"
       }
       
       fn execute(&self, app: &mut App, args: Vec<&str>) -> Result<(), String> {
           let filename = args.get(0)
               .map(|s| s.to_string())
               .unwrap_or_else(|| format!("conversation_{}.md", 
                   SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()));
           
           let markdown_content = self.format_as_markdown(&app.chat_history)?;
           std::fs::write(&filename, markdown_content)
               .map_err(|e| format!("Failed to write file: {}", e))?;
           
           app.add_system_message(format!("📄 Conversation exported as markdown to: {}", filename));
           Ok(())
       }
   }
   
   // Usage in main application
   let mut command_registry = CommandRegistry::new();
   command_registry.register(Box::new(ExportMarkdownCommand));

**File Processing Plugin Example**:

Here's a complete example of a plugin that adds file processing capabilities:

.. code-block:: rust

   use async_trait::async_trait;
   use perspt::{Plugin, PluginConfig, PluginResponse, PluginError};
   use std::path::Path;
   use tokio::fs;

   pub struct FileProcessorPlugin {
       max_file_size: usize,
       supported_extensions: Vec<String>,
   }

   impl FileProcessorPlugin {
       pub fn new() -> Self {
           Self {
               max_file_size: 10 * 1024 * 1024, // 10MB
               supported_extensions: vec![
                   "txt".to_string(),
                   "md".to_string(),
                   "rs".to_string(),
                   "py".to_string(),
                   "js".to_string(),
               ],
           }
       }

       async fn process_file(&self, file_path: &str) -> Result<String, PluginError> {
           let path = Path::new(file_path);
           
           // Validate file exists
           if !path.exists() {
               return Err(PluginError::InvalidInput(
                   format!("File not found: {}", file_path)
               ));
           }

           // Check file size
           let metadata = fs::metadata(path).await
               .map_err(|e| PluginError::IOError(e.to_string()))?;
           
           if metadata.len() > self.max_file_size as u64 {
               return Err(PluginError::InvalidInput(
                   "File too large".to_string()
               ));
           }

           // Check file extension
           if let Some(ext) = path.extension() {
               let ext_str = ext.to_str().unwrap_or("");
               if !self.supported_extensions.contains(&ext_str.to_string()) {
                   return Err(PluginError::InvalidInput(
                       format!("Unsupported file type: {}", ext_str)
                   ));
               }
           }

           // Read file content
           let content = fs::read_to_string(path).await
               .map_err(|e| PluginError::IOError(e.to_string()))?;

           Ok(content)
       }
   }

   #[async_trait]
   impl Plugin for FileProcessorPlugin {
       fn name(&self) -> &str {
           "file-processor"
       }

       fn version(&self) -> &str {
           "1.0.0"
       }

       fn description(&self) -> &str {
           "Process and analyze text files"
       }

       async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
           if let Some(max_size) = config.get("max_file_size") {
               self.max_file_size = max_size.parse()
                   .map_err(|_| PluginError::ConfigurationError(
                       "Invalid max_file_size".to_string()
                   ))?;
           }

           if let Some(extensions) = config.get("supported_extensions") {
               self.supported_extensions = extensions
                   .split(',')
                   .map(|s| s.trim().to_string())
                   .collect();
           }

           Ok(())
       }

       async fn shutdown(&mut self) -> Result<(), PluginError> {
           // Cleanup resources if needed
           Ok(())
       }

       async fn handle_command(
           &self,
           command: &str,
           args: &[String],
       ) -> Result<PluginResponse, PluginError> {
           match command {
               "read-file" => {
                   if args.is_empty() {
                       return Err(PluginError::InvalidInput(
                           "File path required".to_string()
                       ));
                   }

                   let content = self.process_file(&args[0]).await?;
                   Ok(PluginResponse::Text(format!(
                       "File content ({}):
                        {}",
                       args[0], content
                   )))
               }
               
               "analyze-file" => {
                   if args.is_empty() {
                       return Err(PluginError::InvalidInput(
                           "File path required".to_string()
                       ));
                   }

                   let content = self.process_file(&args[0]).await?;
                   let analysis = self.analyze_content(&content);
                   
                   Ok(PluginResponse::Structured(serde_json::json!({
                       "file": args[0],
                       "lines": content.lines().count(),
                       "characters": content.len(),
                       "words": content.split_whitespace().count(),
                       "analysis": analysis
                   })))
               }

               _ => Err(PluginError::UnsupportedCommand(command.to_string()))
           }
       }

       fn supported_commands(&self) -> Vec<String> {
           vec!["read-file".to_string(), "analyze-file".to_string()]
       }
   }

   impl FileProcessorPlugin {
       fn analyze_content(&self, content: &str) -> serde_json::Value {
           // Simple content analysis
           let lines = content.lines().count();
           let words = content.split_whitespace().count();
           let chars = content.len();
           
           serde_json::json!({
               "complexity": if lines > 100 { "high" } else if lines > 50 { "medium" } else { "low" },
               "language": self.detect_language(content),
               "metrics": {
                   "lines": lines,
                   "words": words,
                   "characters": chars
               }
           })
       }

       fn detect_language(&self, content: &str) -> &str {
           if content.contains("fn main()") && content.contains("println!") {
               "rust"
           } else if content.contains("def ") && content.contains("import ") {
               "python"
           } else if content.contains("function ") && content.contains("console.log") {
               "javascript"
           } else {
               "unknown"
           }
       }
   }

Integration Plugin Example
~~~~~~~~~~~~~~~~~~~~~~~~~~

Here's a plugin that integrates with external APIs:

.. code-block:: rust

   pub struct WebSearchPlugin {
       api_key: String,
       client: reqwest::Client,
   }

   #[async_trait]
   impl Plugin for WebSearchPlugin {
       fn name(&self) -> &str {
           "web-search"
       }

       fn version(&self) -> &str {
           "1.0.0"
       }

       fn description(&self) -> &str {
           "Search the web and return relevant results"
       }

       async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
           self.api_key = config.get("api_key")
               .ok_or_else(|| PluginError::ConfigurationError(
                   "API key required for web search".to_string()
               ))?
               .to_string();

           Ok(())
       }

       async fn handle_command(
           &self,
           command: &str,
           args: &[String],
       ) -> Result<PluginResponse, PluginError> {
           match command {
               "search" => {
                   if args.is_empty() {
                       return Err(PluginError::InvalidInput(
                           "Search query required".to_string()
                       ));
                   }

                   let query = args.join(" ");
                   let results = self.search_web(&query).await?;
                   
                   Ok(PluginResponse::Structured(serde_json::json!({
                       "query": query,
                       "results": results
                   })))
               }
               _ => Err(PluginError::UnsupportedCommand(command.to_string()))
           }
       }

       fn supported_commands(&self) -> Vec<String> {
           vec!["search".to_string()]
       }
   }

   impl WebSearchPlugin {
       async fn search_web(&self, query: &str) -> Result<Vec<SearchResult>, PluginError> {
           let url = format!("https://api.searchengine.com/search?q={}&key={}", 
                            urlencoding::encode(query), 
                            self.api_key);

           let response: SearchResponse = self.client
               .get(&url)
               .send()
               .await
               .map_err(|e| PluginError::NetworkError(e.to_string()))?
               .json()
               .await
               .map_err(|e| PluginError::ParseError(e.to_string()))?;

           Ok(response.results)
       }
   }

Plugin Configuration
--------------------

Plugin Configuration Schema
~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: json

   {
     "plugins": {
       "file-processor": {
         "enabled": true,
         "config": {
           "max_file_size": 10485760,
           "supported_extensions": "txt,md,rs,py,js,ts"
         }
       },
       "web-search": {
         "enabled": true,
         "config": {
           "api_key": "your-search-api-key",
           "max_results": 10
         }
       }
     }
   }

Dynamic Plugin Loading
~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   pub struct PluginManager {
       plugins: HashMap<String, Box<dyn Plugin>>,
       config: PluginManagerConfig,
   }

   impl PluginManager {
       pub async fn load_plugin_from_path(&mut self, path: &Path) -> Result<(), PluginError> {
           // Dynamic loading implementation
           let plugin = unsafe {
               self.load_dynamic_library(path)?
           };
           
           let plugin_name = plugin.name().to_string();
           self.plugins.insert(plugin_name, plugin);
           
           Ok(())
       }

       pub async fn execute_plugin_command(
           &self,
           plugin_name: &str,
           command: &str,
           args: &[String],
       ) -> Result<PluginResponse, PluginError> {
           let plugin = self.plugins.get(plugin_name)
               .ok_or_else(|| PluginError::PluginNotFound(plugin_name.to_string()))?;

           plugin.handle_command(command, args).await
       }
   }

Custom UI Components
--------------------

Creating Custom Display Components
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use perspt::ui::{DisplayComponent, RenderContext, UIError};

   pub struct CustomProgressBar {
       progress: f32,
       width: usize,
       style: ProgressStyle,
   }

   impl DisplayComponent for CustomProgressBar {
       fn render(&self, context: &mut RenderContext) -> Result<(), UIError> {
           let filled = (self.progress * self.width as f32) as usize;
           let empty = self.width - filled;
           
           let bar = format!(
               "[{}{}] {:.1}%",
               "█".repeat(filled),
               "░".repeat(empty),
               self.progress * 100.0
           );
           
           context.write_line(&bar, &self.style.into())?;
           Ok(())
       }
   }

   pub struct CustomTable {
       headers: Vec<String>,
       rows: Vec<Vec<String>>,
       column_widths: Vec<usize>,
   }

   impl DisplayComponent for CustomTable {
       fn render(&self, context: &mut RenderContext) -> Result<(), UIError> {
           // Render table headers
           self.render_headers(context)?;
           
           // Render table rows
           for row in &self.rows {
               self.render_row(context, row)?;
           }
           
           Ok(())
       }
   }

Custom Command Processors
~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   pub struct CustomCommandProcessor;

   impl CommandProcessor for CustomCommandProcessor {
       fn process_command(
           &self,
           command: &str,
           args: &[String],
           context: &mut CommandContext,
       ) -> Result<CommandResult, CommandError> {
           match command {
               "custom-help" => {
                   let help_text = self.generate_custom_help();
                   Ok(CommandResult::Display(help_text))
               }
               
               "batch-process" => {
                   if args.is_empty() {
                       return Err(CommandError::MissingArguments);
                   }
                   
                   let results = self.process_batch(&args[0])?;
                   Ok(CommandResult::Structured(results))
               }
               
               _ => Err(CommandError::UnknownCommand(command.to_string()))
           }
       }
   }

Testing Plugins and Extensions
------------------------------

Unit Testing Plugins
~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   #[cfg(test)]
   mod tests {
       use super::*;
       use perspt::testing::{MockPluginConfig, MockContext};

       #[tokio::test]
       async fn test_file_processor_plugin() {
           let mut plugin = FileProcessorPlugin::new();
           let config = MockPluginConfig::new();
           
           plugin.initialize(&config).await.unwrap();
           
           // Test file reading
           let response = plugin
               .handle_command("read-file", &["test.txt".to_string()])
               .await;
               
           assert!(response.is_ok());
       }

       #[tokio::test]
       async fn test_plugin_error_handling() {
           let plugin = FileProcessorPlugin::new();
           
           // Test error case
           let response = plugin
               .handle_command("read-file", &[])
               .await;
               
           assert!(matches!(response, Err(PluginError::InvalidInput(_))));
       }
   }

Integration Testing
~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   #[tokio::test]
   async fn test_plugin_integration() {
       let mut app = TestApplication::new().await;
       
       // Load plugin
       app.load_plugin("file-processor", FileProcessorPlugin::new()).await.unwrap();
       
       // Test plugin command execution
       let response = app.execute_command("/read-file test.txt").await.unwrap();
       assert!(!response.is_empty());
   }

Performance Testing
~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   #[tokio::test]
   async fn test_plugin_performance() {
       let plugin = WebSearchPlugin::new();
       let start = std::time::Instant::now();
       
       let _response = plugin
           .handle_command("search", &["rust programming".to_string()])
           .await
           .unwrap();
           
       let duration = start.elapsed();
       assert!(duration.as_secs() < 5); // Should complete within 5 seconds
   }

Distribution and Packaging
--------------------------

Plugin Distribution
~~~~~~~~~~~~~~~~~~~

**Cargo Package**:

.. code-block:: toml

   # Cargo.toml for your plugin
   [package]
   name = "perspt-file-processor"
   version = "1.0.0"
   edition = "2021"

   [dependencies]
   perspt = "1.0"
   async-trait = "0.1"
   tokio = { version = "1.0", features = ["full"] }
   serde = { version = "1.0", features = ["derive"] }

**Plugin Manifest**:

.. code-block:: json

   {
     "name": "file-processor",
     "version": "1.0.0",
     "description": "Process and analyze text files",
     "author": "Your Name",
     "license": "MIT",
     "min_perspt_version": "1.0.0",
     "dependencies": [],
     "commands": ["read-file", "analyze-file"],
     "configuration_schema": {
       "max_file_size": "integer",
       "supported_extensions": "string"
     }
   }

Extension Deployment
~~~~~~~~~~~~~~~~~~~~

**Configuration-Based Extensions**:

.. code-block:: bash

   # Add custom provider configuration
   echo '{
     "provider": "custom_openai",
     "api_key": "your-key",
     "model": "gpt-4",
     "base_url": "https://api.custom-provider.com/v1",
     "timeout_seconds": 60
   }' > ~/.config/perspt/config.json

**Code-Based Extensions**:

.. code-block:: bash

   # Fork and modify the main repository
   git clone https://github.com/eonseed/perspt.git
   cd perspt
   
   # Add your custom provider logic
   # Build and install
   cargo build --release
   cargo install --path .

**Environment-Based Configuration**:

.. code-block:: bash

   # Set provider-specific environment variables
   export OPENAI_API_KEY="your-openai-key"
   export ANTHROPIC_API_KEY="your-anthropic-key"
   export GEMINI_API_KEY="your-gemini-key"
   export GROQ_API_KEY="your-groq-key"
   export COHERE_API_KEY="your-cohere-key"
   export XAI_API_KEY="your-xai-key"
   export DEEPSEEK_API_KEY="your-deepseek-key"
   export OLLAMA_API_BASE="http://localhost:11434"
   export PERSPT_PROVIDER="openai"
   export PERSPT_MODEL="gpt-4o-mini"

Best Practices
--------------

Provider Extension Development
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

1. **Error Handling**: Use comprehensive error types and meaningful messages

   .. code-block:: rust

      use anyhow::{Context, Result};
      use thiserror::Error;

      #[derive(Error, Debug)]
      pub enum ProviderError {
          #[error("API key not provided for {provider}")]
          MissingApiKey { provider: String },
          #[error("Invalid model {model} for provider {provider}")]
          InvalidModel { model: String, provider: String },
          #[error("Request timeout after {seconds}s")]
          Timeout { seconds: u64 },
      }

2. **Configuration Validation**: Implement robust config validation

   .. code-block:: rust

      impl Config {
          pub fn validate(&self) -> Result<()> {
              match self.provider.as_str() {
                  "openai" => {
                      if self.api_key.is_none() {
                          return Err(ProviderError::MissingApiKey {
                              provider: self.provider.clone()
                          }.into());
                      }
                  }
                  provider => {
                      return Err(ProviderError::UnsupportedProvider {
                          provider: provider.to_string()
                      }.into());
                  }
              }
              Ok(())
          }
      }

3. **Async/Await Patterns**: Follow proper async patterns with error handling

   .. code-block:: rust

      pub async fn send_custom_message(
          config: &Config,
          message: &str,
          tx: UnboundedSender<String>,
      ) -> Result<()> {
          let client = build_client(config).await
              .context("Failed to build HTTP client")?;
          
          let mut stream = create_stream(client, message).await
              .context("Failed to create response stream")?;

          while let Some(chunk) = stream.try_next().await
              .context("Error reading from stream")? {
              tx.send(chunk).context("Failed to send chunk")?;
          }
          
          Ok(())
      }

4. **Testing**: Write comprehensive tests for all extension points

   .. code-block:: rust

      #[cfg(test)]
      mod tests {
          use super::*;
          use tokio_test;

          #[tokio::test]
          async fn test_custom_provider_integration() {
              let config = Config {
                  provider: "custom".to_string(),
                  api_key: Some("test-key".to_string()),
                  model: Some("test-model".to_string()),
                  ..Default::default()
              };

              let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
              
              // Test your custom provider logic
              let result = send_custom_message(&config, "test", tx).await;
              assert!(result.is_ok());
          }
      }

UI Extension Development
~~~~~~~~~~~~~~~~~~~~~~~~

1. **Component Modularity**: Keep UI components small and focused

   .. code-block:: rust

      pub struct CustomWidget {
          content: String,
          scroll_offset: u16,
      }

      impl CustomWidget {
          pub fn render(&self, area: Rect, buf: &mut Buffer) {
              let block = Block::default()
                  .borders(Borders::ALL)
                  .title("Custom Widget");
              
              let inner = block.inner(area);
              block.render(area, buf);
              
              // Custom rendering logic
              self.render_content(inner, buf);
          }
      }

2. **Event Handling**: Implement responsive event handling

   .. code-block:: rust

      pub fn handle_custom_event(&mut self, event: Event) -> Result<bool> {
          match event {
              Event::Key(key) => {
                  match key.code {
                      KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                          // Custom control handling
                          return Ok(true); // Event consumed
                      }
                      _ => return Ok(false), // Event not handled
                  }
              }
              _ => return Ok(false),
          }
      }

Configuration Extension Development
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

1. **Schema Validation**: Define clear configuration schemas

   .. code-block:: rust

      use serde::{Deserialize, Serialize};

      #[derive(Debug, Deserialize, Serialize)]
      pub struct ExtendedConfig {
          #[serde(flatten)]
          pub base: Config,
          pub custom_timeout: Option<u64>,
          pub retry_attempts: Option<u32>,
          pub custom_headers: Option<std::collections::HashMap<String, String>>,
      }

2. **Environment Integration**: Support environment variable overrides

   .. code-block:: rust

      impl ExtendedConfig {
          pub fn from_env() -> Result<Self> {
              let mut config = Config::load()?;
              
              if let Ok(timeout) = std::env::var("PERSPT_CUSTOM_TIMEOUT") {
                  config.custom_timeout = Some(timeout.parse()?);
              }
              
              if let Ok(retries) = std::env::var("PERSPT_RETRY_ATTEMPTS") {
                  config.retry_attempts = Some(retries.parse()?);
              }
              
              Ok(config)
          }
      }

Performance Considerations
~~~~~~~~~~~~~~~~~~~~~~~~~~

1. **Async Efficiency**: Use proper async patterns to avoid blocking

   .. code-block:: rust

      // Good: Non-blocking async operations
      pub async fn efficient_processing(data: &[String]) -> Result<Vec<String>> {
          let tasks: Vec<_> = data.iter()
              .map(|item| process_item_async(item))
              .collect();
          
          let results = futures::future::try_join_all(tasks).await?;
          Ok(results)
      }

      // Avoid: Blocking operations in async context
      pub async fn inefficient_processing(data: &[String]) -> Result<Vec<String>> {
          let mut results = Vec::new();
          for item in data {
              results.push(process_item_blocking(item)?); // Bad!
          }
          Ok(results)
      }

2. **Memory Management**: Handle large responses efficiently

   .. code-block:: rust

      pub async fn stream_large_response(
          config: &Config,
          message: &str,
          tx: UnboundedSender<String>,
      ) -> Result<()> {
          const CHUNK_SIZE: usize = 1024;
          let mut buffer = String::with_capacity(CHUNK_SIZE);
          
          // Process in chunks to avoid memory spikes
          let mut stream = create_response_stream(config, message).await?;
          
          while let Some(chunk) = stream.try_next().await? {
              buffer.push_str(&chunk);
              
              if buffer.len() >= CHUNK_SIZE {
                  tx.send(buffer.clone())?;
                  buffer.clear();
              }
          }
          
          if !buffer.is_empty() {
              tx.send(buffer)?;
          }
          
          Ok(())
      }

Security Considerations
~~~~~~~~~~~~~~~~~~~~~~~

1. **API Key Management**: Secure handling of sensitive data

   .. code-block:: rust

      use secrecy::{ExposeSecret, Secret};

      pub struct SecureConfig {
          pub provider: String,
          pub api_key: Option<Secret<String>>,
          pub model: Option<String>,
      }

      impl SecureConfig {
          pub fn load_secure() -> Result<Self> {
              let api_key = std::env::var("API_KEY")
                  .map(Secret::new)
                  .ok();
              
              Ok(SecureConfig {
                  provider: "openai".to_string(),
                  api_key,
                  model: Some("gpt-4".to_string()),
              })
          }
          
          pub fn get_api_key(&self) -> Option<&str> {
              self.api_key.as_ref().map(|key| key.expose_secret())
          }
      }

2. **Input Validation**: Sanitize and validate all inputs

   .. code-block:: rust

      pub fn validate_message(message: &str) -> Result<()> {
          if message.is_empty() {
              return Err(anyhow::anyhow!("Message cannot be empty"));
          }
          
          if message.len() > 10_000 {
              return Err(anyhow::anyhow!("Message too long (max 10,000 characters)"));
          }
          
          // Check for potentially harmful content
          if message.contains("<script") || message.contains("javascript:") {
              return Err(anyhow::anyhow!("Message contains potentially harmful content"));
          }
          
          Ok(())
      }

Next Steps
----------

- :doc:`testing` - Testing strategies for extensions
- :doc:`../api/index` - API reference for development
- :doc:`contributing` - How to contribute your extensions
- :doc:`architecture` - Understanding Perspt's internal architecture

Example Projects
----------------

For complete examples of extending Perspt, see:

- **Custom Provider Implementation**: Examples in the main repository showing how to add new LLM providers
- **UI Component Extensions**: Ratatui-based widgets for enhanced functionality  
- **Configuration Extensions**: Advanced configuration patterns and validation
- **Testing Extensions**: Comprehensive test suites for extension development

To get started with your own extensions, we recommend:

1. Fork the main Perspt repository
2. Study the existing provider implementations in ``src/llm_provider.rs``
3. Review the UI components in ``src/ui.rs``
4. Examine the configuration system in ``src/config.rs``
5. Run the test suite to understand the expected behavior
6. Start with small modifications and gradually build up complexity

Extending Simple CLI Mode
~~~~~~~~~~~~~~~~~~~~~~~~~

**NEW in v0.4.5** - The Simple CLI mode can be extended with custom commands and enhanced functionality:

**Adding Custom CLI Commands**:

.. code-block:: rust

   // In cli.rs - Extend command processing
   pub async fn run_simple_cli_with_commands(
       config: AppConfig,
       model_name: String,
       api_key: String,
       provider: Arc<GenAIProvider>,
       log_file: Option<String>,
   ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
       // ... existing setup code ...

       loop {
           print!("> ");
           io::stdout().flush()?;

           let mut input = String::new();
           match io::stdin().read_line(&mut input) {
               Ok(0) => break,
               Ok(_) => {
                   let input = input.trim();
                   if input.is_empty() { continue; }
                   if input == "exit" { break; }

                   // Handle custom commands
                   if input.starts_with('/') {
                       match process_cli_command(input, &mut session_log).await {
                           Ok(should_continue) => {
                               if !should_continue { break; }
                               continue;
                           }
                           Err(e) => {
                               eprintln!("Command error: {}", e);
                               continue;
                           }
                       }
                   }

                   // Handle regular conversation
                   // ... existing processing code ...
               }
               Err(e) => break,
           }
       }

       Ok(())
   }

   async fn process_cli_command(
       command: &str,
       session_log: &mut Option<SessionLogger>,
   ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
       let parts: Vec<&str> = command.splitn(2, ' ').collect();
       
       match parts[0] {
           "/help" => {
               println!("Available commands:");
               println!("  /help     - Show this help");
               println!("  /clear    - Clear conversation history");
               println!("  /save     - Save current session to file");
               println!("  /model    - Show current model info");
               println!("  /exit     - Exit the application");
               Ok(true)
           }
           "/clear" => {
               // Clear screen using ANSI escape codes
               print!("\x1B[2J\x1B[1;1H");
               io::stdout().flush()?;
               println!("Conversation cleared.");
               Ok(true)
           }
           "/save" => {
               let filename = parts.get(1)
                   .map(|s| s.to_string())
                   .unwrap_or_else(|| {
                       format!("session_{}.txt", 
                           SystemTime::now()
                               .duration_since(UNIX_EPOCH)
                               .unwrap()
                               .as_secs())
                   });
               
               if let Some(ref logger) = session_log {
                   println!("Session saved to: {}", filename);
               } else {
                   println!("No session log active. Use --log-file to enable logging.");
               }
               Ok(true)
           }
           "/model" => {
               println!("Current model: {}", /* current model info */);
               println!("Provider: {}", /* current provider */);
               Ok(true)
           }
           "/exit" => {
               println!("Goodbye!");
               Ok(false)
           }
           _ => {
               println!("Unknown command: {}. Type /help for available commands.", parts[0]);
               Ok(true)
           }
       }
   }

**Enhanced Session Logging**:

.. code-block:: rust

   // Enhanced session logger with metadata
   pub struct EnhancedSessionLogger {
       file: File,
       session_start: SystemTime,
       command_count: u32,
   }

   impl EnhancedSessionLogger {
       pub fn new(log_path: String) -> Result<Self, std::io::Error> {
           let mut file = OpenOptions::new()
               .create(true)
               .append(true)
               .open(&log_path)?;
           
           // Write session header
           let start_time = SystemTime::now();
           let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
           writeln!(file, "=== Perspt Simple CLI Session Started: {} ===", timestamp)?;
           writeln!(file, "Log file: {}", log_path)?;
           writeln!(file)?;
           file.flush()?;
           
           Ok(Self {
               file,
               session_start: start_time,
               command_count: 0,
           })
       }

       pub fn log_command(&mut self, command: &str) -> Result<(), std::io::Error> {
           self.command_count += 1;
           let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
           writeln!(self.file, "[{}] Command {}: {}", timestamp, self.command_count, command)?;
           self.file.flush()?;
           Ok(())
       }

       pub fn log_session_stats(&mut self) -> Result<(), std::io::Error> {
           let duration = self.session_start.elapsed().unwrap_or_default();
           let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
           
           writeln!(self.file)?;
           writeln!(self.file, "=== Session Ended: {} ===", timestamp)?;
           writeln!(self.file, "Duration: {:?}", duration)?;
           writeln!(self.file, "Total commands: {}", self.command_count)?;
           self.file.flush()?;
           Ok(())
       }
   }

**Scriptable Integration Examples**:

.. code-block:: bash

   # Custom script for batch AI queries
   #!/bin/bash
   
   QUESTIONS=(
       "What is machine learning?"
       "Explain deep learning in simple terms"
       "What are neural networks?"
   )
   
   LOG_FILE="ai_learning_session_$(date +%Y%m%d_%H%M%S).txt"
   
   for question in "${QUESTIONS[@]}"; do
       echo "Processing: $question"
       echo "$question" | perspt --simple-cli --log-file "$LOG_FILE"
       echo "---" >> "$LOG_FILE"
   done
   
   echo "Batch processing complete. Results in: $LOG_FILE"

**Integration with External Tools**:

.. code-block:: rust

   // External tool integration example
   pub async fn process_with_external_tool(
       input: &str,
       tool_name: &str,
   ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
       match tool_name {
           "json_format" => {
               // Use jq or similar tool to format JSON responses
               let output = Command::new("jq")
                   .arg(".")
                   .stdin(Stdio::piped())
                   .stdout(Stdio::piped())
                   .spawn()?;
               
               // Process with external tool
               // ... implementation ...
               Ok(formatted_output)
           }
           "markdown_render" => {
               // Use pandoc or similar for markdown conversion
               let output = Command::new("pandoc")
                   .arg("-f").arg("markdown")
                   .arg("-t").arg("plain")
                   .stdin(Stdio::piped())
                   .stdout(Stdio::piped())
                   .spawn()?;
               
               // ... implementation ...
               Ok(rendered_output)
           }
           _ => Err("Unknown tool".into())
       }
   }
