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
               key if key.starts_with("hf_") => "huggingface".to_string(), // New provider
               key if key.starts_with("co_") => "cohere".to_string(),       // New provider
               _ => "openai".to_string(),
           }
       }

       pub fn get_effective_model(&self) -> String {
           match self.model {
               Some(ref model) => model.clone(),
               None => match self.provider.as_str() {
                   "openai" => "gpt-3.5-turbo".to_string(),
                   "anthropic" => "claude-3-haiku-20240307".to_string(),
                   "gemini" => "gemini-pro".to_string(),
                   "huggingface" => "microsoft/DialoGPT-medium".to_string(), // New default
                   "cohere" => "command".to_string(),                        // New default
                   _ => "gpt-3.5-turbo".to_string(),
               }
           }
       }
   }

Custom Provider Implementation
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

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
~~~~~~~~~~~~~~~~~~~~~~~~~~

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

Configuration Extensions
-----------------------

Adding Custom Configuration Options
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

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
~~~~~~~~~~~~~~~~~~~~

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
~~~~~~~~~~~~~~~~~~~~~~~~~

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

Creating Custom Plugins
------------------------

Command Plugin Example
~~~~~~~~~~~~~~~~~~~~~~

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
~~~~~~~~~~~~~~~~~~~~~~~~~

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
~~~~~~~~~~~~~~~~~~~~~

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
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

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
~~~~~~~~~~~~~~~~~~

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
~~~~~~~~~~~~~~~~~~

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
~~~~~~~~~~~~~~~~~~

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
~~~~~~~~~~~~~~~~~~~

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
   export PERSPT_PROVIDER="openai"
   export PERSPT_MODEL="gpt-4"

Best Practices
--------------

Provider Extension Development
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

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
~~~~~~~~~~~~~~~~~~~~~~~

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
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

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
~~~~~~~~~~~~~~~~~~~~~~~~

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
~~~~~~~~~~~~~~~~~~~~~~

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
---------------

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
