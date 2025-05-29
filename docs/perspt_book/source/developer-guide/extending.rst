Extending Perspt
================

This guide covers how to extend Perspt with custom providers, plugins, and integrations.

Plugin System Overview
-----------------------

Perspt's plugin system allows you to extend functionality without modifying the core application. Plugins can:

- Add new AI providers
- Implement custom commands
- Process specialized content types
- Integrate with external services
- Enhance the user interface

Plugin Architecture
~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   #[async_trait]
   pub trait Plugin: Send + Sync {
       /// Plugin identification
       fn name(&self) -> &str;
       fn version(&self) -> &str;
       fn description(&self) -> &str;
       
       /// Lifecycle methods
       async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError>;
       async fn shutdown(&mut self) -> Result<(), PluginError>;
       
       /// Command handling
       async fn handle_command(
           &self, 
           command: &str, 
           args: &[String]
       ) -> Result<PluginResponse, PluginError>;
       
       fn supported_commands(&self) -> Vec<String>;
       
       /// Event handling (optional)
       async fn on_message_sent(&self, message: &Message) -> Result<(), PluginError> {
           Ok(())
       }
       
       async fn on_response_received(&self, response: &Response) -> Result<(), PluginError> {
           Ok(())
       }
   }

Creating Custom Providers
--------------------------

To create a new AI provider, implement the `LLMProvider` trait:

Basic Provider Implementation
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use async_trait::async_trait;
   use serde::{Deserialize, Serialize};
   use perspt::{LLMProvider, Message, ChatOptions, ChatResponse, LLMError};

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct CustomProviderConfig {
       pub api_key: String,
       pub base_url: String,
       pub model: String,
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

Installation Methods
~~~~~~~~~~~~~~~~~~~

**Package Manager**:

.. code-block:: bash

   # Install from crates.io
   perspt plugin install file-processor

   # Install from Git
   perspt plugin install --git https://github.com/eonseed/perspt-plugin

   # Install local plugin
   perspt plugin install --path ./my-plugin

**Manual Installation**:

.. code-block:: bash

   # Copy plugin to plugins directory
   cp plugin.so ~/.config/perspt/plugins/

Best Practices
--------------

Plugin Development
~~~~~~~~~~~~~~~~~

1. **Error Handling**: Always provide meaningful error messages
2. **Configuration**: Support configuration through the plugin config
3. **Documentation**: Include comprehensive documentation
4. **Testing**: Write thorough unit and integration tests
5. **Performance**: Consider performance implications
6. **Security**: Validate all inputs and handle sensitive data carefully

Provider Development
~~~~~~~~~~~~~~~~~~~

1. **Rate Limiting**: Implement proper rate limiting
2. **Retry Logic**: Handle temporary failures gracefully
3. **Streaming**: Support streaming responses when possible
4. **Configuration**: Provide comprehensive configuration options
5. **Monitoring**: Include metrics and logging

Next Steps
----------

- :doc:`testing` - Testing strategies for extensions
- :doc:`../api/index` - API reference for plugin development
- :doc:`contributing` - How to contribute your extensions
- :doc:`architecture` - Understanding Perspt's internal architecture
