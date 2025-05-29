# Perspt Developer Guide

## 🏗️ Architecture Overview

Perspt is built with a modular, extensible architecture that separates concerns and promotes maintainability. The application follows Rust best practices and leverages the powerful `allms` crate for unified LLM access.

```
┌─────────────────────────────────────────────────────┐
│                 📐 Architecture                     │
├─────────────────────────────────────────────────────┤
│                                                     │
│  ┌─────────────┐    ┌─────────────┐                 │
│  │   main.rs   │────│  config.rs  │                 │
│  │ Entry Point │    │Configuration│                 │
│  └─────────────┘    └─────────────┘                 │
│         │                    │                      │
│         └────────┬───────────┘                      │
│                  │                                  │
│         ┌─────────────┐    ┌─────────────┐          │
│         │    ui.rs    │────│llm_provider │          │
│         │Terminal UI  │    │   .rs       │          │
│         └─────────────┘    │ LLM Bridge  │          │
│                            └─────────────┘          │
│                                                     │
└─────────────────────────────────────────────────────┘
```

## 🧩 Module Structure

### Core Modules

#### 1. `main.rs` - Application Entry Point
- **Purpose**: Application bootstrap, CLI parsing, error handling
- **Responsibilities**:
  - Command-line argument parsing
  - Configuration loading and validation
  - Terminal initialization and cleanup
  - Global panic handling
  - Application lifecycle management

#### 2. `config.rs` - Configuration Management
- **Purpose**: Flexible, provider-agnostic configuration system
- **Responsibilities**:
  - JSON configuration parsing
  - Default value management
  - Provider type inference
  - Environment variable integration

#### 3. `llm_provider.rs` - LLM Abstraction Layer
- **Purpose**: Unified interface to multiple LLM providers
- **Responsibilities**:
  - Provider abstraction
  - Model discovery and validation
  - Streaming response handling
  - Error categorization and recovery

#### 4. `ui.rs` - Terminal User Interface
- **Purpose**: Beautiful, responsive terminal interface
- **Responsibilities**:
  - Real-time chat rendering
  - Markdown parsing and display
  - Keyboard event handling
  - Status and error management

## 🎯 Design Principles

### 1. **Modularity**
Each module has a single, well-defined responsibility with clear interfaces.

```rust
// Example: Clean module interfaces
pub trait LLMProvider {
    async fn send_chat_request(&self, input: &str, model: &str, config: &AppConfig, tx: &Sender<String>) -> Result<()>;
    async fn list_models(&self) -> Result<Vec<String>>;
    async fn validate_config(&self, config: &AppConfig) -> Result<()>;
    fn provider_type(&self) -> ProviderType;
}
```

### 2. **Extensibility**
New providers can be added by implementing the `LLMProvider` trait.

```rust
// Example: Adding a new provider
pub struct NewProvider {
    api_key: String,
    base_url: String,
}

#[async_trait]
impl LLMProvider for NewProvider {
    async fn send_chat_request(&self, input: &str, model: &str, config: &AppConfig, tx: &Sender<String>) -> Result<()> {
        // Implementation here
    }
    
    // ... other trait methods
}
```

### 3. **Error Resilience**
Comprehensive error handling at every level with graceful degradation.

```rust
// Example: Robust error handling
fn setup_panic_hook() {
    panic::set_hook(Box::new(move |panic_info| {
        // Force terminal restoration immediately
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        
        // Provide contextual error messages
        let panic_str = format!("{}", panic_info);
        if panic_str.contains("PROJECT_ID") {
            eprintln!("💡 Tip: Set PROJECT_ID environment variable");
        }
        // ... more context-specific help
    }));
}
```

### 4. **Performance**
Asynchronous operations, efficient memory usage, and minimal allocations.

```rust
// Example: Efficient async streaming
pub async fn send_chat_request(&self, input: &str, model: &str, config: &AppConfig, tx: &Sender<String>) -> Result<()> {
    let response = self.get_completion_response(model, &api_key, input).await?;
    
    // Simulate streaming for better UX
    let chunks: Vec<&str> = response.split_whitespace().collect();
    for (i, chunk) in chunks.iter().enumerate() {
        tx.send(format!("{} ", chunk))?;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    
    Ok(())
}
```

## 🔧 Development Setup

### Prerequisites

```bash
# Install Rust (latest stable)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install development tools
cargo install cargo-watch
cargo install cargo-expand
cargo install cargo-audit
```

### Building the Project

```bash
# Clone and build
git clone https://github.com/your-username/perspt
cd perspt

# Development build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run
```

### Development Workflow

```bash
# Watch for changes and rebuild
cargo watch -x check -x test -x run

# Format code
cargo fmt

# Lint code
cargo clippy

# Security audit
cargo audit
```

## 🔍 Code Analysis

### Key Data Structures

#### `AppConfig` - Configuration Container
```rust
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AppConfig {
    pub providers: HashMap<String, String>,    // Provider URLs
    pub api_key: Option<String>,              // Universal API key
    pub default_model: Option<String>,        // Default model name
    pub default_provider: Option<String>,     // Default provider
    pub provider_type: Option<String>,        // Provider type
}
```

**Design Rationale**:
- `HashMap` for providers allows flexible endpoint configuration
- `Option` types enable graceful handling of missing values
- `Clone` trait for efficient state passing
- `Deserialize` for JSON configuration parsing

#### `ChatMessage` - Message Representation
```rust
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub message_type: MessageType,
    pub content: Vec<Line<'static>>,
    pub timestamp: String,
}
```

**Design Rationale**:
- `MessageType` enum for different message categories
- `Vec<Line<'static>>` for efficient terminal rendering
- Static lifetime for performance optimization

#### `LLMProvider` Trait - Provider Abstraction
```rust
#[async_trait]
pub trait LLMProvider {
    async fn list_models(&self) -> LLMResult<Vec<String>>;
    async fn send_chat_request(&self, input: &str, model_name: &str, config: &AppConfig, tx: &mpsc::UnboundedSender<String>) -> LLMResult<()>;
    fn provider_type(&self) -> ProviderType;
    async fn validate_config(&self, config: &AppConfig) -> LLMResult<()>;
}
```

**Design Rationale**:
- `async_trait` for async method support in traits
- `mpsc::UnboundedSender` for efficient streaming
- Separate validation method for early error detection

### Control Flow Analysis

#### Application Startup
```mermaid
graph TD
    A[main()] --> B[setup_panic_hook()]
    B --> C[parse CLI args]
    C --> D[load_config()]
    D --> E[create LLM provider]
    E --> F[validate configuration]
    F --> G[initialize terminal]
    G --> H[run UI loop]
    H --> I[cleanup terminal]
```

#### Message Processing Flow
```mermaid
graph TD
    A[User Input] --> B[handle_events()]
    B --> C{Enter Key?}
    C -->|Yes| D[initiate_llm_request()]
    C -->|No| E[Update UI State]
    D --> F[Spawn Async Task]
    F --> G[send_chat_request()]
    G --> H[Stream Response]
    H --> I[Update Chat History]
```

## 🚀 Extending Perspt

### Adding a New LLM Provider

#### Step 1: Define Provider Type
```rust
// In llm_provider.rs
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderType {
    OpenAI,
    Anthropic,
    Google,
    // Add your new provider
    NewProvider,
}

impl ProviderType {
    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "openai" => Some(ProviderType::OpenAI),
            "anthropic" => Some(ProviderType::Anthropic),
            // Add your provider
            "newprovider" => Some(ProviderType::NewProvider),
            _ => None,
        }
    }
}
```

#### Step 2: Implement Provider Struct
```rust
#[derive(Debug)]
pub struct NewLLMProvider {
    api_key: String,
    base_url: String,
    // Add provider-specific fields
}

impl NewLLMProvider {
    pub fn new(api_key: String, base_url: String) -> Self {
        Self { api_key, base_url }
    }
    
    // Provider-specific helper methods
    async fn make_api_request(&self, payload: &str) -> Result<String> {
        // Implementation here
        todo!()
    }
}
```

#### Step 3: Implement LLMProvider Trait
```rust
#[async_trait]
impl LLMProvider for NewLLMProvider {
    async fn list_models(&self) -> LLMResult<Vec<String>> {
        // Query your provider's model list endpoint
        let models = vec![
            "new-model-1".to_string(),
            "new-model-2".to_string(),
        ];
        Ok(models)
    }

    async fn send_chat_request(
        &self,
        input: &str,
        model_name: &str,
        config: &AppConfig,
        tx: &mpsc::UnboundedSender<String>,
    ) -> LLMResult<()> {
        // Create API request payload
        let payload = serde_json::json!({
            "model": model_name,
            "messages": [{"role": "user", "content": input}],
            "stream": true
        });

        // Make API request
        let response = self.make_api_request(&payload.to_string()).await?;
        
        // Stream response back
        for chunk in response.split_whitespace() {
            tx.send(format!("{} ", chunk))?;
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        
        tx.send(crate::EOT_SIGNAL.to_string())?;
        Ok(())
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::NewProvider
    }

    async fn validate_config(&self, config: &AppConfig) -> LLMResult<()> {
        if config.api_key.is_none() {
            return Err(anyhow::anyhow!("API key required for NewProvider"));
        }
        Ok(())
    }
}
```

#### Step 4: Update Provider Factory
```rust
// In main.rs or llm_provider.rs
pub fn create_provider(provider_type: ProviderType, config: &AppConfig) -> Result<Arc<dyn LLMProvider + Send + Sync>> {
    match provider_type {
        ProviderType::OpenAI => Ok(Arc::new(UnifiedLLMProvider::new(provider_type))),
        ProviderType::Anthropic => Ok(Arc::new(UnifiedLLMProvider::new(provider_type))),
        // Add your new provider
        ProviderType::NewProvider => {
            let api_key = config.api_key.clone().ok_or_else(|| anyhow::anyhow!("API key required"))?;
            let base_url = config.providers.get("newprovider")
                .cloned()
                .unwrap_or_else(|| "https://api.newprovider.com/v1".to_string());
            Ok(Arc::new(NewLLMProvider::new(api_key, base_url)))
        }
        _ => Err(anyhow::anyhow!("Unsupported provider type")),
    }
}
```

### Adding New UI Features

#### Adding a New Chat Message Type
```rust
// In ui.rs
#[derive(Debug, Clone, PartialEq)]
pub enum MessageType {
    User,
    Assistant,
    Error,
    System,
    Warning,
    // Add new type
    Debug,
}

// Update message styling
fn get_message_style(message_type: &MessageType) -> Style {
    match message_type {
        MessageType::User => Style::default().fg(Color::Cyan),
        MessageType::Assistant => Style::default().fg(Color::Green),
        MessageType::Error => Style::default().fg(Color::Red),
        MessageType::System => Style::default().fg(Color::Yellow),
        MessageType::Warning => Style::default().fg(Color::Magenta),
        // Add styling for new type
        MessageType::Debug => Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC),
    }
}
```

#### Adding New Keyboard Shortcuts
```rust
// In main.rs handle_events function
pub async fn handle_events(/* ... */) -> Option<AppEvent> {
    if let Ok(event) = event::read() {
        match event {
            Event::Key(key) => {
                match key.code {
                    // Existing shortcuts...
                    KeyCode::Enter => { /* ... */ }
                    KeyCode::Esc => { /* ... */ }
                    
                    // Add new shortcuts
                    KeyCode::F(2) => {
                        app.toggle_debug_mode();
                        return Some(AppEvent::Tick);
                    }
                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.save_conversation();
                        return Some(AppEvent::Tick);
                    }
                    
                    // ... rest of handling
                }
            }
        }
    }
    None
}
```

### Configuration Extensions

#### Adding New Configuration Fields
```rust
// Extend AppConfig
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AppConfig {
    // Existing fields...
    pub providers: HashMap<String, String>,
    pub api_key: Option<String>,
    
    // New configuration options
    pub max_history_size: Option<usize>,
    pub auto_save: Option<bool>,
    pub theme: Option<String>,
    pub custom_prompts: Option<HashMap<String, String>>,
}

// Update default configuration
pub async fn load_config(config_path: Option<&String>) -> Result<AppConfig> {
    let config: AppConfig = match config_path {
        Some(path) => { /* ... */ }
        None => {
            AppConfig {
                // Existing defaults...
                providers: providers_map,
                api_key: None,
                
                // New defaults
                max_history_size: Some(1000),
                auto_save: Some(false),
                theme: Some("default".to_string()),
                custom_prompts: Some(HashMap::new()),
            }
        }
    };
    Ok(process_loaded_config(config))
}
```

## 🧪 Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_config_loading() {
        let config = load_config(None).await.unwrap();
        assert_eq!(config.provider_type, Some("openai".to_string()));
        assert!(config.providers.contains_key("openai"));
    }
    
    #[test]
    fn test_provider_type_inference() {
        let mut config = AppConfig {
            providers: HashMap::new(),
            api_key: None,
            default_model: None,
            default_provider: Some("anthropic".to_string()),
            provider_type: None,
        };
        
        let processed = process_loaded_config(config);
        assert_eq!(processed.provider_type, Some("anthropic".to_string()));
    }
}
```

### Integration Tests
```rust
// tests/integration_test.rs
use perspt::config::load_config;
use perspt::llm_provider::{UnifiedLLMProvider, ProviderType};

#[tokio::test]
async fn test_provider_creation() {
    let config = load_config(None).await.unwrap();
    let provider = UnifiedLLMProvider::new(ProviderType::OpenAI);
    
    // Test model listing (may require API key)
    if std::env::var("OPENAI_API_KEY").is_ok() {
        let models = provider.list_models().await.unwrap();
        assert!(!models.is_empty());
    }
}
```

### Mock Testing
```rust
use mockito::{mock, server_url};

#[tokio::test]
async fn test_api_request_handling() {
    let _m = mock("POST", "/v1/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"choices":[{"message":{"content":"Hello!"}}]}"#)
        .create();

    // Test with mock server
    let provider = create_test_provider(&server_url());
    let result = provider.send_chat_request("test", "gpt-3.5-turbo", &config, &tx).await;
    assert!(result.is_ok());
}
```

## 📊 Performance Optimization

### Memory Management
```rust
// Use static lifetimes for UI strings
pub struct ChatMessage {
    pub message_type: MessageType,
    pub content: Vec<Line<'static>>,  // Static lifetime for efficiency
    pub timestamp: String,
}

// Efficient string handling
fn markdown_to_lines(markdown: &str) -> Vec<Line<'static>> {
    // Convert borrowed strings to owned for static lifetime
    markdown.lines()
        .map(|line| Line::from(line.to_string()))
        .collect()
}
```

### Async Optimization
```rust
// Use tokio::spawn for CPU-intensive tasks
async fn process_response(response: String) -> Vec<Line<'static>> {
    tokio::task::spawn_blocking(move || {
        markdown_to_lines(&response)
    }).await.unwrap()
}

// Batch UI updates to reduce redraws
async fn batch_ui_updates(updates: Vec<UIUpdate>) {
    let batched = updates.into_iter()
        .fold(UIState::new(), |mut state, update| {
            state.apply(update);
            state
        });
    
    render_ui(batched).await;
}
```

### Error Handling Patterns

#### Result-Based Error Handling
```rust
// Custom error types for better error handling
#[derive(Debug)]
pub enum PersptError {
    Config(String),
    Network(String),
    Authentication(String),
    Provider(String),
}

impl std::error::Error for PersptError {}

impl std::fmt::Display for PersptError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PersptError::Config(msg) => write!(f, "Configuration error: {}", msg),
            PersptError::Network(msg) => write!(f, "Network error: {}", msg),
            PersptError::Authentication(msg) => write!(f, "Authentication error: {}", msg),
            PersptError::Provider(msg) => write!(f, "Provider error: {}", msg),
        }
    }
}
```

#### Graceful Degradation
```rust
async fn load_config_with_fallback(config_path: Option<&String>) -> AppConfig {
    match load_config(config_path).await {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Warning: Failed to load config ({}), using defaults", e);
            load_config(None).await.unwrap_or_else(|_| {
                panic!("Failed to create default configuration");
            })
        }
    }
}
```

## 🔒 Security Considerations

### API Key Management
```rust
// Secure API key handling
pub fn get_api_key(config: &AppConfig, provider_type: &ProviderType) -> Result<String> {
    // Priority: environment variable > config file
    let env_var = match provider_type {
        ProviderType::OpenAI => "OPENAI_API_KEY",
        ProviderType::Anthropic => "ANTHROPIC_API_KEY",
        ProviderType::Google => "GOOGLE_API_KEY",
        // ...
    };
    
    std::env::var(env_var)
        .or_else(|_| config.api_key.clone().ok_or_else(|| anyhow::anyhow!("No API key found")))
}

// Secure configuration file permissions
pub fn ensure_config_security(config_path: &Path) -> Result<()> {
    let metadata = std::fs::metadata(config_path)?;
    let permissions = metadata.permissions();
    
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = permissions.mode();
        if mode & 0o077 != 0 {
            return Err(anyhow::anyhow!(
                "Configuration file has insecure permissions. Run: chmod 600 {}",
                config_path.display()
            ));
        }
    }
    
    Ok(())
}
```

### Input Sanitization
```rust
pub fn sanitize_input(input: &str) -> String {
    // Remove potentially dangerous characters
    input.chars()
        .filter(|c| c.is_ascii() && !c.is_control() || *c == '\n' || *c == '\t')
        .collect::<String>()
        .trim()
        .to_string()
}
```

## 📈 Monitoring and Debugging

### Logging Setup
```rust
// Configure structured logging
pub fn setup_logging() -> Result<()> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .format(|buf, record| {
            writeln!(buf,
                "{} [{}] - {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .init();
    
    Ok(())
}

// Usage throughout the application
log::info!("Starting Perspt with provider: {}", provider_type);
log::debug!("Configuration loaded: {:?}", config);
log::warn!("Retrying connection after error: {}", error);
log::error!("Fatal error occurred: {}", error);
```

### Metrics Collection
```rust
// Performance metrics
pub struct Metrics {
    pub request_count: AtomicU64,
    pub total_response_time: AtomicU64,
    pub error_count: AtomicU64,
}

impl Metrics {
    pub fn record_request(&self, duration: Duration) {
        self.request_count.fetch_add(1, Ordering::Relaxed);
        self.total_response_time.fetch_add(duration.as_millis() as u64, Ordering::Relaxed);
    }
    
    pub fn average_response_time(&self) -> f64 {
        let count = self.request_count.load(Ordering::Relaxed);
        let total = self.total_response_time.load(Ordering::Relaxed);
        
        if count > 0 {
            total as f64 / count as f64
        } else {
            0.0
        }
    }
}
```

## 🎯 Best Practices

### Code Organization
1. **Single Responsibility**: Each module has one clear purpose
2. **Interface Segregation**: Small, focused traits and interfaces
3. **Dependency Injection**: Use traits for testability
4. **Error Propagation**: Use `Result` types consistently

### Performance Guidelines
1. **Avoid Cloning**: Use references where possible
2. **Batch Operations**: Group related operations together
3. **Async/Await**: Use for I/O operations, not CPU-bound tasks
4. **Memory Pools**: Reuse allocations where possible

### Security Guidelines
1. **Input Validation**: Sanitize all user inputs
2. **Secret Management**: Use environment variables for secrets
3. **Secure Defaults**: Fail securely by default
4. **Audit Dependencies**: Regularly check for vulnerabilities

## 🚀 Deployment and Distribution

### Building Release Binaries
```bash
# Build optimized release
cargo build --release

# Build for specific target
cargo build --release --target x86_64-unknown-linux-gnu

# Build with minimal binary size
cargo build --release --target x86_64-unknown-linux-gnu \
  -Z build-std=std,panic_abort \
  -Z build-std-features=panic_immediate_abort
```

### Cross-Platform Compilation
```bash
# Install cross-compilation targets
rustup target add x86_64-pc-windows-gnu
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin

# Build for multiple platforms
cargo build --release --target x86_64-pc-windows-gnu
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
```

### Packaging
```toml
# Cargo.toml additions for packaging
[package.metadata.deb]
maintainer = "Your Name <your.email@example.com>"
copyright = "2024, Your Name <your.email@example.com>"
license-file = ["LICENSE", "4"]
extended-description = """\
A high-performance terminal-based chat application for \
interacting with various Large Language Models."""
depends = "$auto"
section = "utility"
priority = "optional"
assets = [
    ["target/release/perspt", "usr/bin/", "755"],
    ["README.md", "usr/share/doc/perspt/README", "644"],
]
```

---

## 📚 Resources

- **Rust Documentation**: https://doc.rust-lang.org/
- **Tokio Guide**: https://tokio.rs/tokio/tutorial
- **Ratatui Documentation**: https://ratatui.rs/
- **allms Crate**: https://crates.io/crates/allms
- **Serde Documentation**: https://serde.rs/

---

*Built with ❤️ for the developer community. Happy coding!*
