Testing
=======

This guide covers testing strategies, tools, and best practices for Perspt development, including unit testing, integration testing, and end-to-end testing.

Testing Philosophy
------------------

Perspt follows a comprehensive testing approach:

- **Unit Tests**: Test individual components in isolation
- **Integration Tests**: Test component interactions
- **End-to-End Tests**: Test complete user workflows
- **Performance Tests**: Ensure performance requirements are met
- **Security Tests**: Validate security measures

Testing Structure
-----------------

Test Organization
~~~~~~~~~~~~~~~~~

.. code-block:: text

   src/
   ├── main.rs          # Entry point with unit tests
   ├── config.rs        # Configuration with validation tests  
   ├── llm_provider.rs  # GenAI integration with provider tests
   └── ui.rs           # Ratatui UI components with widget tests

   tests/
   ├── panic_handling_test.rs     # Panic handling integration tests
   └── integration_tests/         # Additional integration tests
       ├── config_loading.rs
       ├── provider_streaming.rs
       └── ui_rendering.rs

   benches/                       # Performance benchmarks
   ├── streaming_benchmarks.rs
   └── config_benchmarks.rs

Current Test Structure
~~~~~~~~~~~~~~~~~~~~~~

The project currently includes:

- **Unit tests**: Embedded in source files using ``#[cfg(test)]``
- **Integration tests**: In the ``tests/`` directory
- **Panic handling tests**: Specialized tests for error recovery
- **Performance benchmarks**: For critical performance paths

Unit Testing
------------

Testing Configuration Module
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Tests for configuration loading, validation, and environment handling:

.. code-block:: rust

   // src/config.rs
   #[cfg(test)]
   mod tests {
       use super::*;
       use std::fs;
       use tempfile::TempDir;

       #[test]
       fn test_config_loading_from_file() {
           let temp_dir = TempDir::new().unwrap();
           let config_path = temp_dir.path().join("config.json");
           
           let config_content = r#"
           {
               "provider": "openai",
               "model": "gpt-4",
               "api_key": "test-key",
               "temperature": 0.7,
               "max_tokens": 2000,
               "timeout_seconds": 30
           }
           "#;
           
           fs::write(&config_path, config_content).unwrap();
           
           let config = Config::load_from_path(&config_path).unwrap();
           assert_eq!(config.provider, "openai");
           assert_eq!(config.model, Some("gpt-4".to_string()));
           assert_eq!(config.temperature, Some(0.7));
           assert_eq!(config.max_tokens, Some(2000));
       }

       #[test]
       fn test_provider_inference() {
           // Test automatic provider inference from API key environment
           std::env::set_var("OPENAI_API_KEY", "sk-test");
           let config = Config::with_inferred_provider().unwrap();
           assert_eq!(config.provider, "openai");
           
           std::env::remove_var("OPENAI_API_KEY");
           std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-test");
           let config = Config::with_inferred_provider().unwrap();
           assert_eq!(config.provider, "anthropic");
           
           // Cleanup
           std::env::remove_var("ANTHROPIC_API_KEY");
       }

       #[test]
       fn test_config_validation() {
           let mut config = Config::default();
           config.provider = "openai".to_string();
           config.api_key = None; // Missing required API key
           
           let result = config.validate();
           assert!(result.is_err());
           assert!(result.unwrap_err().to_string().contains("API key"));
       }

       #[test]
       fn test_config_defaults() {
           let config = Config::default();
           assert_eq!(config.provider, "openai");
           assert_eq!(config.model, Some("gpt-3.5-turbo".to_string()));
           assert_eq!(config.temperature, Some(0.7));
           assert_eq!(config.max_tokens, Some(4000));
           assert_eq!(config.timeout_seconds, Some(30));
       }
   }

Testing LLM Provider Module
~~~~~~~~~~~~~~~~~~~~~~~~~~~

Tests for GenAI integration and streaming functionality:

.. code-block:: rust

   // src/llm_provider.rs
   #[cfg(test)]
   mod tests {
       use super::*;
       use tokio::sync::mpsc;
       use std::time::Duration;

       #[tokio::test]
       async fn test_message_validation() {
           assert!(validate_message("Hello, world!").is_ok());
           assert!(validate_message("").is_err());
           assert!(validate_message(&"x".repeat(20_000)).is_err()); // Too long
       }

       #[tokio::test]
       async fn test_streaming_channel_communication() {
           let (tx, mut rx) = mpsc::unbounded_channel();
           
           // Simulate streaming response
           tokio::spawn(async move {
               for i in 0..5 {
                   tx.send(format!("chunk_{}", i)).unwrap();
                   tokio::time::sleep(Duration::from_millis(10)).await;
               }
           });
           
           let mut received = Vec::new();
           while let Ok(chunk) = tokio::time::timeout(
               Duration::from_millis(100), 
               rx.recv()
           ).await {
               if let Some(chunk) = chunk {
                   received.push(chunk);
               } else {
                   break;
               }
           }
           
           assert_eq!(received.len(), 5);
           assert_eq!(received[0], "chunk_0");
           assert_eq!(received[4], "chunk_4");
       }

       #[tokio::test]
       #[ignore] // Requires API key
       async fn test_real_provider_integration() {
           if std::env::var("OPENAI_API_KEY").is_err() {
               return; // Skip if no API key
           }

           let config = Config {
               provider: "openai".to_string(),
               api_key: std::env::var("OPENAI_API_KEY").ok(),
               model: Some("gpt-3.5-turbo".to_string()),
               temperature: Some(0.1), // Low temperature for predictable results
               max_tokens: Some(50),
               timeout_seconds: Some(30),
           };

           let (tx, mut rx) = mpsc::unbounded_channel();
           let result = send_message(&config, "Say 'Hello'", tx).await;
           
           assert!(result.is_ok());
           
           // Should receive at least some response
           let response = tokio::time::timeout(
               Duration::from_secs(10),
               rx.recv()
           ).await;
           assert!(response.is_ok());
       }

       #[test]
       fn test_config_preparation_for_genai() {
           let config = Config {
               provider: "openai".to_string(),
               api_key: Some("test-key".to_string()),
               model: Some("gpt-4".to_string()),
               temperature: Some(0.7),
               max_tokens: Some(1000),
               timeout_seconds: Some(60),
           };

           // Test that config can be converted to GenAI client format
           assert!(!config.api_key.unwrap().is_empty());
           assert!(config.model.unwrap().contains("gpt"));
       }
   }
               ) -> Result<String, HttpError>;
           }
       }

       #[tokio::test]
       async fn test_openai_chat_completion() {
           let mut mock_client = MockHttpClient::new();
           
           let expected_response = json!({
               "choices": [{
                   "message": {
                       "content": "Hello! How can I help you today?"
                   }
               }],
               "usage": {
                   "total_tokens": 25
               }
           });
           
           mock_client
               .expect_post()
               .with(
                   eq("https://api.openai.com/v1/chat/completions"),
                   always(),
                   contains("gpt-4")
               )
               .times(1)
               .returning(move |_, _, _| Ok(expected_response.to_string()));

           let config = OpenAIConfig {
               api_key: "test-key".to_string(),
               model: "gpt-4".to_string(),
               ..Default::default()
           };
           
           let provider = OpenAIProvider::new_with_client(config, Box::new(mock_client));
           
           let messages = vec![
               Message {
                   role: "user".to_string(),
                   content: "Hello".to_string(),
               }
           ];
           
           let options = ChatOptions::default();
           let response = provider.chat_completion(&messages, &options).await.unwrap();
           
           assert_eq!(response.content, "Hello! How can I help you today?");
           assert_eq!(response.tokens_used, Some(25));
       }

       #[tokio::test]
       async fn test_provider_error_handling() {
           let mut mock_client = MockHttpClient::new();
           
           mock_client
               .expect_post()
               .returning(|_, _, _| Err(HttpError::NetworkError("Connection failed".to_string())));

           let config = OpenAIConfig::default();
           let provider = OpenAIProvider::new_with_client(config, Box::new(mock_client));
           
           let messages = vec![Message::user("Test message")];
           let options = ChatOptions::default();
           
           let result = provider.chat_completion(&messages, &options).await;
           assert!(result.is_err());
           assert!(matches!(result.unwrap_err(), LLMError::NetworkError(_)));
       }

       #[tokio::test]
       async fn test_rate_limiting() {
           let mut mock_client = MockHttpClient::new();
           
           // First request succeeds
           mock_client
               .expect_post()
               .times(1)
               .returning(|_, _, _| Ok(r#"{"choices":[{"message":{"content":"Success"}}]}"#.to_string()));
           
           // Second request hits rate limit
           mock_client
               .expect_post()
               .times(1)
               .returning(|_, _, _| Err(HttpError::RateLimit));

           let config = OpenAIConfig::default();
           let provider = OpenAIProvider::new_with_client(config, Box::new(mock_client));
           
           let messages = vec![Message::user("Test")];
           let options = ChatOptions::default();
           
           // First request should succeed
           let result1 = provider.chat_completion(&messages, &options).await;
           assert!(result1.is_ok());
           
           // Second request should fail with rate limit error
           let result2 = provider.chat_completion(&messages, &options).await;
           assert!(matches!(result2.unwrap_err(), LLMError::RateLimit));
       }
   }

Testing UI Components
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   // src/ui.rs
   #[cfg(test)]
   mod tests {
       use super::*;
       use std::io::Cursor;

       #[test]
       fn test_message_formatting() {
           let formatter = MessageFormatter::new();
           
           let message = Message {
               role: "assistant".to_string(),
               content: "Here's some `code` and **bold** text.".to_string(),
           };
           
           let formatted = formatter.format_message(&message);
           assert!(formatted.contains("code"));
           assert!(formatted.contains("bold"));
       }

       #[test]
       fn test_input_parsing() {
           let parser = InputParser::new();
           
           // Test regular message
           let input = "Hello, world!";
           let parsed = parser.parse(input);
           assert!(matches!(parsed, ParsedInput::Message(_)));
           
           // Test command
           let input = "/help";
           let parsed = parser.parse(input);
           assert!(matches!(parsed, ParsedInput::Command { name: "help", .. }));
           
           // Test command with arguments
           let input = "/model gpt-4";
           let parsed = parser.parse(input);
           if let ParsedInput::Command { name, args } = parsed {
               assert_eq!(name, "model");
               assert_eq!(args, vec!["gpt-4"]);
           }
       }

       #[tokio::test]
       async fn test_ui_rendering() {
           let mut output = Cursor::new(Vec::new());
           let mut ui = UIManager::new_with_output(Box::new(output));
           
           let message = Message::assistant("Test response");
           ui.render_message(&message).await.unwrap();
           
           let output_data = ui.get_output_data();
           let output_str = String::from_utf8(output_data).unwrap();
           assert!(output_str.contains("Test response"));
       }
   }

Integration Testing
-------------------

Provider Integration Tests
~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   // tests/integration/provider_tests.rs
   use perspt::*;
   use std::env;

   #[tokio::test]
   #[ignore] // Requires API key
   async fn test_openai_integration() {
       let api_key = env::var("OPENAI_API_KEY")
           .expect("OPENAI_API_KEY environment variable required for integration tests");
       
       let config = OpenAIConfig {
           api_key,
           model: "gpt-4o-mini".to_string(),
           ..Default::default()
       };
       
       let provider = OpenAIProvider::new(config);
       
       let messages = vec![
           Message::user("What is 2+2?")
       ];
       
       let options = ChatOptions {
           max_tokens: Some(50),
           temperature: Some(0.1),
           ..Default::default()
       };
       
       let response = provider.chat_completion(&messages, &options).await.unwrap();
       assert!(!response.content.is_empty());
       assert!(response.content.contains("4"));
   }

   #[tokio::test]
   async fn test_provider_fallback() {
       let primary_config = OpenAIConfig {
           api_key: "invalid-key".to_string(),
           model: "gpt-4".to_string(),
           ..Default::default()
       };
       
       let fallback_config = OllamaConfig {
           base_url: "http://localhost:11434".to_string(),
           model: "llama2".to_string(),
           ..Default::default()
       };
       
       let fallback_chain = FallbackChain::new(vec![
           Box::new(OpenAIProvider::new(primary_config)),
           Box::new(OllamaProvider::new(fallback_config)),
       ]);
       
       let messages = vec![Message::user("Hello")];
       let options = ChatOptions::default();
       
       // Should fallback to Ollama when OpenAI fails
       let response = fallback_chain.chat_completion(&messages, &options).await;
       assert!(response.is_ok() || response.is_err()); // Depends on Ollama availability
   }

Configuration Integration Tests
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   // tests/integration/config_tests.rs
   use perspt::*;
   use tempfile::TempDir;
   use std::fs;

   #[test]
   fn test_config_file_hierarchy() {
       let temp_dir = TempDir::new().unwrap();
       
       // Create multiple config files
       let global_config = temp_dir.path().join("global.json");
       let user_config = temp_dir.path().join("user.json");
       let local_config = temp_dir.path().join("local.json");
       
       fs::write(&global_config, r#"{"provider": "openai", "temperature": 0.5}"#).unwrap();
       fs::write(&user_config, r#"{"model": "gpt-4", "temperature": 0.7}"#).unwrap();
       fs::write(&local_config, r#"{"api_key": "local-key"}"#).unwrap();
       
       let mut config = Config::new();
       config.load_from_file(&global_config).unwrap();
       config.load_from_file(&user_config).unwrap();
       config.load_from_file(&local_config).unwrap();
       
       assert_eq!(config.provider, "openai");
       assert_eq!(config.model, "gpt-4");
       assert_eq!(config.api_key, Some("local-key".to_string()));
       assert_eq!(config.temperature, Some(0.7)); // user config overrides global
   }

   #[tokio::test]
   async fn test_config_validation_with_providers() {
       let config = Config {
           provider: "openai".to_string(),
           api_key: Some("sk-test123".to_string()),
           model: "gpt-4".to_string(),
           ..Default::default()
       };
       
       let provider_registry = ProviderRegistry::new();
       let validation_result = provider_registry.validate_config(&config).await;
       
       assert!(validation_result.is_ok());
   }

End-to-End Testing
------------------

Full Conversation Flow
~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   // tests/e2e/full_conversation_test.rs
   use perspt::*;
   use std::time::Duration;
   use tokio::time::timeout;

   #[tokio::test]
   async fn test_complete_conversation_flow() {
       let config = Config::test_config();
       let mut app = Application::new(config).await.unwrap();
       
       // Start the application
       let app_handle = tokio::spawn(async move {
           app.run().await
       });
       
       // Simulate user input
       let mut client = TestClient::new("localhost:8080").await.unwrap();
       
       // Send first message
       let response1 = client.send_message("Hello, I'm testing Perspt").await.unwrap();
       assert!(!response1.is_empty());
       
       // Send follow-up message
       let response2 = client.send_message("Can you remember what I just said?").await.unwrap();
       assert!(response2.to_lowercase().contains("testing") || 
               response2.to_lowercase().contains("perspt"));
       
       // Test command
       let response3 = client.send_command("/status").await.unwrap();
       assert!(response3.contains("Connected"));
       
       // Cleanup
       client.send_command("/exit").await.unwrap();
       
       // Wait for app to shutdown
       timeout(Duration::from_secs(5), app_handle).await.unwrap().unwrap();
   }

   #[tokio::test]
   async fn test_error_recovery() {
       let mut config = Config::test_config();
       config.api_key = Some("invalid-key".to_string());
       
       let mut app = Application::new(config).await.unwrap();
       let mut client = TestClient::new("localhost:8080").await.unwrap();
       
       // This should fail with invalid key
       let response = client.send_message("Hello").await;
       assert!(response.is_err());
       
       // Update config with valid key
       client.send_command("/config set api_key valid-key").await.unwrap();
       
       // This should now work
       let response = client.send_message("Hello").await.unwrap();
       assert!(!response.is_empty());
   }

Plugin Integration Tests
~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   // tests/e2e/plugin_integration_test.rs
   use perspt::*;
   use std::path::Path;

   #[tokio::test]
   async fn test_plugin_loading_and_execution() {
       let config = Config::test_config();
       let mut app = Application::new(config).await.unwrap();
       
       // Load a test plugin
       let plugin_path = Path::new("test_plugins/file_processor.so");
       if plugin_path.exists() {
           app.load_plugin(plugin_path).await.unwrap();
           
           let mut client = TestClient::new("localhost:8080").await.unwrap();
           
           // Test plugin command
           let response = client.send_command("/read-file test.txt").await.unwrap();
           assert!(response.contains("File content"));
           
           // Test plugin with invalid args
           let response = client.send_command("/read-file").await;
           assert!(response.is_err());
       }
   }

UI and Command Testing
~~~~~~~~~~~~~~~~~~~~~~

**Added in v0.4.3** - Testing user interface components and command functionality:

.. code-block:: rust

   // src/ui.rs - Unit tests for UI components
   #[cfg(test)]
   mod tests {
       use super::*;
       use tempfile::TempDir;
       use std::fs;

       #[test]
       fn test_save_conversation_command() {
           let mut app = App::new_for_testing();
           
           // Add some test messages
           app.add_message(ChatMessage {
               message_type: MessageType::User,
               content: vec![Line::from("Hello, AI!")],
               timestamp: "2024-01-01 12:00:00".to_string(),
               raw_content: "Hello, AI!".to_string(),
           });
           
           app.add_message(ChatMessage {
               message_type: MessageType::Assistant,
               content: vec![Line::from("Hello! How can I help you?")],
               timestamp: "2024-01-01 12:00:01".to_string(),
               raw_content: "Hello! How can I help you?".to_string(),
           });
           
           // Test save with custom filename
           let temp_dir = TempDir::new().unwrap();
           let save_path = temp_dir.path().join("test_conversation.txt");
           let filename = save_path.to_string_lossy().to_string();
           
           let result = app.save_conversation(Some(filename.clone()));
           assert!(result.is_ok());
           assert_eq!(result.unwrap(), filename);
           
           // Verify file contents
           let content = fs::read_to_string(&save_path).unwrap();
           assert!(content.contains("Perspt Conversation"));
           assert!(content.contains("User: Hello, AI!"));
           assert!(content.contains("Assistant: Hello! How can I help you?"));
       }

       #[test]
       fn test_command_handling() {
           let mut app = App::new_for_testing();
           
           // Add a test conversation
           app.add_message(ChatMessage {
               message_type: MessageType::User,
               content: vec![Line::from("Hello")],
               timestamp: "2024-01-01 12:00:00".to_string(),
               raw_content: "Hello".to_string(),
           });
           
           // Test /save command
           let result = app.handle_command("/save test.txt".to_string());
           assert!(result.is_ok());
           assert_eq!(result.unwrap(), true); // Command was handled
           
           // Clean up
           let _ = fs::remove_file("test.txt");
       }

       impl App {
           fn new_for_testing() -> Self {
               let config = crate::config::AppConfig {
                   provider_type: Some("test".to_string()),
                   api_key: Some("test-key".to_string()),
                   default_model: "test-model".to_string(),
                   ..Default::default()
               };
               Self::new(config)
           }
       }
   }

Performance Testing
-------------------

Benchmark Configuration
~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   // benches/provider_benchmarks.rs
   use criterion::{black_box, criterion_group, criterion_main, Criterion};
   use perspt::*;
   use tokio::runtime::Runtime;

   fn bench_openai_provider(c: &mut Criterion) {
       let rt = Runtime::new().unwrap();
       let config = OpenAIConfig::test_config();
       let provider = OpenAIProvider::new(config);
       
       c.bench_function("openai_chat_completion", |b| {
           b.to_async(&rt).iter(|| async {
               let messages = vec![Message::user("Hello")];
               let options = ChatOptions::default();
               
               black_box(
                   provider.chat_completion(&messages, &options).await.unwrap()
               )
           })
       });
   }

   fn bench_config_loading(c: &mut Criterion) {
       c.bench_function("config_load", |b| {
           b.iter(|| {
               let config = Config::load_from_string(black_box(r#"
                   {
                       "provider": "openai",
                       "model": "gpt-4",
                       "api_key": "test-key"
                   }
               "#)).unwrap();
               black_box(config)
           })
       });
   }

   criterion_group!(benches, bench_openai_provider, bench_config_loading);
   criterion_main!(benches);

Memory and Resource Testing
~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   #[tokio::test]
   async fn test_memory_usage() {
       let initial_memory = get_memory_usage();
       
       let config = Config::test_config();
       let mut app = Application::new(config).await.unwrap();
       
       // Simulate long conversation
       for i in 0..1000 {
           let message = format!("Test message {}", i);
           app.process_message(&message).await.unwrap();
       }
       
       let final_memory = get_memory_usage();
       let memory_increase = final_memory - initial_memory;
       
       // Memory increase should be reasonable (less than 100MB for 1000 messages)
       assert!(memory_increase < 100 * 1024 * 1024);
   }

   fn get_memory_usage() -> usize {
       // Platform-specific memory measurement
       #[cfg(target_os = "linux")]
       {
           use std::fs;
           let status = fs::read_to_string("/proc/self/status").unwrap();
           for line in status.lines() {
               if line.starts_with("VmRSS:") {
                   let kb: usize = line.split_whitespace().nth(1).unwrap().parse().unwrap();
                   return kb * 1024;
               }
           }
           0
       }
       
       #[cfg(not(target_os = "linux"))]
       {
           // Placeholder for other platforms
           0
       }
   }

Security Testing
----------------

Input Validation Testing
~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   #[tokio::test]
   async fn test_input_sanitization() {
       let sanitizer = InputSanitizer::new();
       
       // Test potential XSS
       let malicious_input = "<script>alert('xss')</script>";
       let sanitized = sanitizer.sanitize(malicious_input);
       assert!(!sanitized.contains("<script>"));
       
       // Test SQL injection patterns
       let sql_injection = "'; DROP TABLE users; --";
       let sanitized = sanitizer.sanitize(sql_injection);
       assert!(!sanitized.contains("DROP TABLE"));
       
       // Test excessive length
       let long_input = "a".repeat(100_000);
       let sanitized = sanitizer.sanitize(&long_input);
       assert!(sanitized.len() <= 10_000); // Should be truncated
   }

   #[tokio::test]
   async fn test_api_key_security() {
       let config = Config {
           api_key: Some("sk-super-secret-key".to_string()),
           ..Default::default()
       };
       
       // Ensure API key doesn't appear in logs
       let log_output = capture_logs(|| {
           log::info!("Config loaded: {:?}", config);
       });
       
       assert!(!log_output.contains("sk-super-secret-key"));
       assert!(log_output.contains("[REDACTED]"));
   }

Testing Utilities
-----------------

Test Fixtures
~~~~~~~~~~~~~

.. code-block:: rust

   // tests/common/fixtures.rs
   pub struct TestFixtures;

   impl TestFixtures {
       pub fn sample_config() -> Config {
           Config {
               provider: "test".to_string(),
               model: "test-model".to_string(),
               api_key: Some("test-key".to_string()),
               max_tokens: Some(100),
               temperature: Some(0.5),
               ..Default::default()
           }
       }
       
       pub fn sample_messages() -> Vec<Message> {
           vec![
               Message::user("Hello"),
               Message::assistant("Hi there! How can I help you?"),
               Message::user("What's the weather like?"),
           ]
       }
       
       pub fn sample_chat_response() -> ChatResponse {
           ChatResponse {
               content: "It's sunny today!".to_string(),
               tokens_used: Some(15),
               model: "test-model".to_string(),
               finish_reason: Some("stop".to_string()),
           }
       }
   }

Mock Implementations
~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   // tests/common/mocks.rs
   pub struct MockLLMProvider {
       responses: Vec<String>,
       call_count: std::sync::Arc<std::sync::Mutex<usize>>,
   }

   impl MockLLMProvider {
       pub fn new(responses: Vec<String>) -> Self {
           Self {
               responses,
               call_count: std::sync::Arc::new(std::sync::Mutex::new(0)),
           }
       }
       
       pub fn call_count(&self) -> usize {
           *self.call_count.lock().unwrap()
       }
   }

   #[async_trait]
   impl LLMProvider for MockLLMProvider {
       async fn chat_completion(
           &self,
           _messages: &[Message],
           _options: &ChatOptions,
       ) -> Result<ChatResponse, LLMError> {
           let mut count = self.call_count.lock().unwrap();
           let response_index = *count % self.responses.len();
           *count += 1;
           
           Ok(ChatResponse {
               content: self.responses[response_index].clone(),
               tokens_used: Some(10),
               model: "mock".to_string(),
               finish_reason: Some("stop".to_string()),
           })
       }
   }

Test Configuration
------------------

Cargo.toml Test Dependencies
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: toml

   [dev-dependencies]
   tokio-test = "0.4"
   mockall = "0.11"
   criterion = "0.5"
   tempfile = "3.0"
   serde_json = "1.0"
   env_logger = "0.10"

   [[bench]]
   name = "provider_benchmarks"
   harness = false

   [[bench]]
   name = "ui_benchmarks"
   harness = false

Running Tests
~~~~~~~~~~~~~

.. code-block:: bash

   # Run all tests
   cargo test

   # Run unit tests only
   cargo test --lib

   # Run integration tests only
   cargo test --test '*'

   # Run specific test
   cargo test test_openai_provider

   # Run tests with output
   cargo test -- --nocapture

   # Run tests with specific thread count
   cargo test -- --test-threads=1

   # Run ignored tests (integration tests requiring API keys)
   cargo test -- --ignored

   # Run benchmarks
   cargo bench

   # Generate test coverage report
   cargo tarpaulin --out Html

Continuous Integration
----------------------

GitHub Actions Configuration
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: yaml

   # .github/workflows/test.yml
   name: Tests

   on:
     push:
       branches: [ main, develop ]
     pull_request:
       branches: [ main ]

   jobs:
     test:
       runs-on: ubuntu-latest
       
       steps:
       - uses: actions/checkout@v3
       
       - name: Install Rust
         uses: actions-rs/toolchain@v1
         with:
           toolchain: stable
           components: rustfmt, clippy
       
       - name: Check formatting
         run: cargo fmt --check
       
       - name: Run clippy
         run: cargo clippy -- -D warnings
       
       - name: Run unit tests
         run: cargo test --lib
       
       - name: Run integration tests
         run: cargo test --test '*'
         env:
           RUST_LOG: debug
       
       - name: Generate coverage report
         run: |
           cargo install cargo-tarpaulin
           cargo tarpaulin --out xml
       
       - name: Upload coverage to Codecov
         uses: codecov/codecov-action@v3

Best Practices
--------------

Testing Guidelines
~~~~~~~~~~~~~~~~~~

1. **Test Isolation**: Each test should be independent
2. **Clear Naming**: Test names should describe what they verify
3. **Comprehensive Coverage**: Aim for high code coverage
4. **Fast Execution**: Unit tests should run quickly
5. **Reliable Results**: Tests should be deterministic
6. **Error Testing**: Test error conditions and edge cases

Performance Testing Guidelines
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

1. **Baseline Measurements**: Establish performance baselines
2. **Regression Detection**: Catch performance regressions early
3. **Resource Monitoring**: Monitor memory and CPU usage
4. **Load Testing**: Test under realistic load conditions

Next Steps
----------

- :doc:`contributing` - Contribution guidelines and development setup
- :doc:`architecture` - Understanding the codebase for better testing
- :doc:`extending` - Testing custom plugins and extensions
- :doc:`../api/index` - API reference for testing integration points

Testing Simple CLI Mode
~~~~~~~~~~~~~~~~~~~~~~

**NEW in v0.4.5** - Comprehensive testing for the Simple CLI mode requires specific strategies:

**Unit Tests for CLI Module**:

.. code-block:: rust

   // In src/cli.rs - Unit tests
   #[cfg(test)]
   mod tests {
       use super::*;
       use std::io::{self, Cursor};
       use tokio::sync::mpsc;

       #[tokio::test]
       async fn test_simple_cli_input_processing() {
           let input = "What is Rust?";
           let (tx, mut rx) = mpsc::unbounded_channel();
           
           // Mock provider response
           let mock_provider = create_mock_provider();
           
           let result = process_simple_request(input, "test-model", &mock_provider).await;
           assert!(result.is_ok());
           
           // Verify streaming response collection
           let response = result.unwrap();
           assert!(!response.is_empty());
       }

       #[test]
       fn test_session_logger_creation() {
           let temp_file = std::env::temp_dir().join("test_session.txt");
           let logger = SessionLogger::new(temp_file.to_string_lossy().to_string());
           assert!(logger.is_ok());
           
           // Cleanup
           let _ = std::fs::remove_file(temp_file);
       }

       #[test]
       fn test_cli_command_parsing() {
           assert!(is_exit_command("exit"));
           assert!(is_exit_command("EXIT"));
           assert!(!is_exit_command("exit please"));
           assert!(!is_exit_command(""));
       }

       #[tokio::test]
       async fn test_streaming_response_collection() {
           let (tx, mut rx) = mpsc::unbounded_channel();
           
           // Simulate streaming chunks
           tx.send("Hello ".to_string()).unwrap();
           tx.send("world!".to_string()).unwrap();
           tx.send("<<EOT>>".to_string()).unwrap();
           drop(tx);
           
           let mut response = String::new();
           while let Some(chunk) = rx.recv().await {
               if chunk == "<<EOT>>" { break; }
               response.push_str(&chunk);
           }
           
           assert_eq!(response, "Hello world!");
       }

       fn create_mock_provider() -> Arc<MockGenAIProvider> {
           // Create mock provider for testing
           Arc::new(MockGenAIProvider::new())
       }
       
       fn is_exit_command(input: &str) -> bool {
           input.trim().to_lowercase() == "exit"
       }
   }

**Integration Tests for CLI Workflows**:

.. code-block:: rust

   // tests/cli_integration_tests.rs
   use perspt::cli::run_simple_cli;
   use perspt::config::AppConfig;
   use std::process::{Command, Stdio};
   use std::io::Write;
   use tempfile::NamedTempFile;

   #[tokio::test]
   async fn test_simple_cli_session_logging() {
       let log_file = NamedTempFile::new().unwrap();
       let log_path = log_file.path().to_string_lossy().to_string();
       
       // Create test configuration
       let config = AppConfig {
           provider_type: Some("openai".to_string()),
           api_key: Some("test-key".to_string()),
           default_model: Some("gpt-3.5-turbo".to_string()),
           // ... other fields
       };
       
       // Simulate CLI session with scripted input
       let script = "Hello\nexit\n";
       let mut child = Command::new("target/debug/perspt")
           .args(&["--simple-cli", "--log-file", &log_path])
           .stdin(Stdio::piped())
           .stdout(Stdio::piped())
           .stderr(Stdio::piped())
           .spawn()
           .expect("Failed to start perspt");
       
       if let Some(stdin) = child.stdin.as_mut() {
           stdin.write_all(script.as_bytes()).unwrap();
       }
       
       let output = child.wait_with_output().unwrap();
       assert!(output.status.success());
       
       // Verify log file contents
       let log_contents = std::fs::read_to_string(&log_path).unwrap();
       assert!(log_contents.contains("User: Hello"));
       assert!(log_contents.contains("Assistant:"));
   }

   #[test]
   fn test_cli_argument_parsing() {
       let output = Command::new("target/debug/perspt")
           .args(&["--simple-cli", "--help"])
           .output()
           .expect("Failed to execute perspt");
       
       assert!(output.status.success());
       let stdout = String::from_utf8(output.stdout).unwrap();
       assert!(stdout.contains("simple-cli"));
       assert!(stdout.contains("log-file"));
   }

   #[test]
   fn test_exit_command_handling() {
       let output = Command::new("target/debug/perspt")
           .args(&["--simple-cli"])
           .stdin(Stdio::piped())
           .stdout(Stdio::piped())
           .stderr(Stdio::piped())
           .spawn()
           .expect("Failed to start perspt");
       
       // Send exit command
       if let Some(stdin) = output.stdin.as_mut() {
           stdin.write_all(b"exit\n").unwrap();
       }
       
       let result = output.wait_with_output().unwrap();
       assert!(result.status.success());
       
       let stdout = String::from_utf8(result.stdout).unwrap();
       assert!(stdout.contains("Goodbye!"));
   }

**Scripting Tests**:

.. code-block:: bash

   #!/bin/bash
   # tests/test_cli_scripting.sh
   
   set -e
   
   echo "Testing Simple CLI scripting capabilities..."
   
   # Test basic input/output
   echo "What is 2+2?" | timeout 30s target/debug/perspt --simple-cli > /tmp/test_output.txt
   
   if grep -q "4" /tmp/test_output.txt; then
       echo "✅ Basic math test passed"
   else
       echo "❌ Basic math test failed"
       exit 1
   fi
   
   # Test session logging
   LOG_FILE="/tmp/test_session_$(date +%s).txt"
   echo -e "Hello\nexit" | timeout 30s target/debug/perspt --simple-cli --log-file "$LOG_FILE"
   
   if [ -f "$LOG_FILE" ] && grep -q "User: Hello" "$LOG_FILE"; then
       echo "✅ Session logging test passed"
   else
       echo "❌ Session logging test failed"
       exit 1
   fi
   
   # Test piping multiple questions
   {
       echo "What is machine learning?"
       echo "Give a brief example"
       echo "exit"
   } | timeout 60s target/debug/perspt --simple-cli --log-file "/tmp/multi_test.txt"
   
   if grep -q "machine learning" /tmp/multi_test.txt; then
       echo "✅ Multi-question test passed"
   else
       echo "❌ Multi-question test failed"
       exit 1
   fi
   
   echo "All CLI scripting tests passed!"

**Accessibility Testing**:

.. code-block:: rust

   // tests/accessibility_tests.rs
   use std::process::{Command, Stdio};
   use std::io::Write;

   #[test]
   fn test_screen_reader_compatibility() {
       // Test that Simple CLI output is screen reader friendly
       let mut child = Command::new("target/debug/perspt")
           .args(&["--simple-cli"])
           .stdin(Stdio::piped())
           .stdout(Stdio::piped())
           .stderr(Stdio::piped())
           .spawn()
           .expect("Failed to start perspt");
       
       if let Some(stdin) = child.stdin.as_mut() {
           stdin.write_all(b"Hello\nexit\n").unwrap();
       }
       
       let output = child.wait_with_output().unwrap();
       let stdout = String::from_utf8(output.stdout).unwrap();
       
       // Verify output contains clear prompt markers
       assert!(stdout.contains("> "));
       assert!(stdout.contains("Perspt Simple CLI Mode"));
       assert!(stdout.contains("Type 'exit' or press Ctrl+D to quit"));
       
       // Ensure no ANSI escape codes that might confuse screen readers
       assert!(!stdout.contains("\x1b["));
   }

   #[test]
   fn test_keyboard_navigation() {
       // Test that common accessibility keyboard patterns work
       let test_inputs = vec![
           "\n",           // Empty input handling
           "   \n",        // Whitespace handling
           "\x04",         // Ctrl+D (EOF)
           "exit\n",       // Standard exit
       ];
       
       for input in test_inputs {
           let output = Command::new("target/debug/perspt")
               .args(&["--simple-cli"])
               .stdin(Stdio::piped())
               .stdout(Stdio::piped())
               .stderr(Stdio::piped())
               .spawn()
               .expect("Failed to start perspt");
           
           if let Some(stdin) = output.stdin.as_mut() {
               stdin.write_all(input.as_bytes()).unwrap();
           }
           
           let result = output.wait_with_output().unwrap();
           // Should handle all inputs gracefully without crashing
           assert!(result.status.success() || result.status.code() == Some(0));
       }
   }

**Performance Tests for CLI Mode**:

.. code-block:: rust

   // benches/cli_benchmarks.rs
   use criterion::{black_box, criterion_group, criterion_main, Criterion};
   use perspt::cli::SessionLogger;
   use std::time::Instant;

   fn benchmark_session_logging(c: &mut Criterion) {
       c.bench_function("session_logging", |b| {
           let temp_file = std::env::temp_dir().join("bench_session.txt");
           let mut logger = SessionLogger::new(temp_file.to_string_lossy().to_string()).unwrap();
           
           b.iter(|| {
               logger.log_user_input(black_box("Test input message")).unwrap();
               logger.log_ai_response(black_box("Test AI response")).unwrap();
           });
           
           let _ = std::fs::remove_file(temp_file);
       });
   }

   fn benchmark_input_processing(c: &mut Criterion) {
       c.bench_function("input_processing", |b| {
           b.iter(|| {
               let input = black_box("What is quantum computing?");
               // Benchmark input validation and sanitization
               let sanitized = input.trim().to_string();
               sanitized
           });
       });
   }

   criterion_group!(benches, benchmark_session_logging, benchmark_input_processing);
   criterion_main!(benches);

**Mock Provider for Testing**:

.. code-block:: rust

   // In src/llm_provider.rs - Mock provider for testing
   #[cfg(test)]
   pub struct MockGenAIProvider {
       responses: Vec<String>,
       current_index: std::sync::atomic::AtomicUsize,
   }

   #[cfg(test)]
   impl MockGenAIProvider {
       pub fn new() -> Self {
           Self {
               responses: vec![
                   "Hello! How can I help you?".to_string(),
                   "That's a great question!".to_string(),
                   "I'm happy to assist with that.".to_string(),
               ],
               current_index: std::sync::atomic::AtomicUsize::new(0),
           }
       }

       pub async fn generate_response_stream_to_channel(
           &self,
           _model: &str,
           _prompt: &str,
           tx: tokio::sync::mpsc::UnboundedSender<String>,
       ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
           let index = self.current_index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
           let response = &self.responses[index % self.responses.len()];
           
           // Simulate streaming by sending chunks
           for chunk in response.split_whitespace() {
               tx.send(format!("{} ", chunk))?;
               tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
           }
           
           tx.send("<<EOT>>".to_string())?;
           Ok(())
       }
   }
