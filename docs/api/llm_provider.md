# LLM Provider Module API Documentation

## Module: `llm_provider.rs`

### Overview
Unified LLM provider interface that leverages the `allms` crate for seamless integration with multiple AI providers. This module provides automatic model discovery, dynamic provider support, and consistent API behavior across different LLM services.

## Core Philosophy

The module is designed around these principles:

1. **Automatic Updates**: Leverages `allms` crate for automatic support of new models and providers
2. **Dynamic Discovery**: Uses `try_from_str()` for validation and future compatibility
3. **Consistent API**: Unified interface across all providers
4. **Reduced Maintenance**: No manual tracking of model names or API changes

## Enums

### `ProviderType`
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderType {
    OpenAI,
    Anthropic,
    Google,
    Groq,
    Cohere,
    Xai,
    DeepSeek,
    Ollama,
}
```

**Description**: Enumeration of supported LLM provider types.

**Methods**:

#### `from_string()`
```rust
pub fn from_string(s: &str) -> Option<Self>
```
**Description**: Converts string representation to ProviderType enum.

**Parameters**:
- `s: &str` - String representation of provider type

**Returns**:
- `Option<ProviderType>` - Some(provider) if recognized, None otherwise

**Supported Strings**:
| Input | Output |
|-------|--------|
| "openai" | `ProviderType::OpenAI` |
| "anthropic" | `ProviderType::Anthropic` |
| "google", "gemini" | `ProviderType::Google` |
| "groq" | `ProviderType::Groq` |
| "cohere" | `ProviderType::Cohere` |
| "xai" | `ProviderType::Xai` |
| "deepseek" | `ProviderType::DeepSeek` |
| "ollama" | `ProviderType::Ollama` |

**Example**:
```rust
let provider_type = ProviderType::from_string("anthropic");
assert_eq!(provider_type, Some(ProviderType::Anthropic));

let unknown = ProviderType::from_string("unknown");
assert_eq!(unknown, None);
```

#### `to_string()`
```rust
pub fn to_string(&self) -> &'static str
```
**Description**: Converts ProviderType enum to canonical string representation.

**Returns**:
- `&'static str` - String representation of the provider type

**Example**:
```rust
let provider = ProviderType::Anthropic;
assert_eq!(provider.to_string(), "anthropic");
```

## Structs

### `UnifiedLLMProvider`
```rust
#[derive(Debug)]
pub struct UnifiedLLMProvider {
    provider_type: ProviderType,
}
```

**Description**: Main LLM provider implementation using the `allms` crate for unified access to multiple AI providers.

**Methods**:

#### `new()`
```rust
pub fn new(provider_type: ProviderType) -> Self
```
**Description**: Creates a new UnifiedLLMProvider instance.

**Parameters**:
- `provider_type: ProviderType` - The type of provider to create

**Returns**:
- `Self` - New provider instance

**Example**:
```rust
let provider = UnifiedLLMProvider::new(ProviderType::OpenAI);
```

#### `get_available_models()`
```rust
pub fn get_available_models(&self) -> Vec<String>
```
**Description**: Retrieves all available models for the provider type using the `allms` crate enums.

**Returns**:
- `Vec<String>` - List of available model identifiers

**Model Sources by Provider**:
- **OpenAI**: `OpenAIModels` enum from genai
- **Anthropic**: `AnthropicModels` enum from genai
- **Google**: `GoogleModels` enum from genai
- **Groq**: `GroqModels` enum from genai
- **Cohere**: `CohereModels` enum from genai
- **XAI**: `XaiModels` enum from genai
- **DeepSeek**: `DeepSeekModels` enum from genai
- **Ollama**: Dynamic model discovery from local Ollama instance

**Example**:
```rust
let provider = UnifiedLLMProvider::new(ProviderType::OpenAI);
let models = provider.get_available_models();
// Returns: ["gpt-4o-mini", "gpt-4o", "gpt-4-turbo", ...]
```

### `SimpleResponse`
```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct SimpleResponse {
    pub content: String,
}
```

**Description**: Simple response structure for LLM completions.

**Fields**:
- `content: String` - The response content from the LLM

## Traits

### `LLMProvider`
```rust
#[async_trait]
pub trait LLMProvider {
    async fn list_models(&self) -> LLMResult<Vec<String>>;
    async fn send_chat_request(&self, input: &str, model_name: &str, config: &AppConfig, tx: &mpsc::UnboundedSender<String>) -> LLMResult<()>;
    fn provider_type(&self) -> ProviderType;
    async fn validate_config(&self, config: &AppConfig) -> LLMResult<()>;
}
```

**Description**: Unified trait for all LLM providers, providing consistent interface across different AI services.

**Methods**:

#### `list_models()`
```rust
async fn list_models(&self) -> LLMResult<Vec<String>>
```
**Description**: Lists all available models for the provider.

**Returns**:
- `LLMResult<Vec<String>>` - List of model identifiers or error

**Example**:
```rust
let provider = UnifiedLLMProvider::new(ProviderType::Anthropic);
let models = provider.list_models().await?;
println!("Available models: {:?}", models);
```

#### `send_chat_request()`
```rust
async fn send_chat_request(
    &self,
    input: &str,
    model_name: &str,
    config: &AppConfig,
    tx: &mpsc::UnboundedSender<String>,
) -> LLMResult<()>
```
**Description**: Sends a chat request to the LLM with streaming response.

**Parameters**:
- `input: &str` - The user's message/prompt
- `model_name: &str` - Model identifier to use
- `config: &AppConfig` - Application configuration
- `tx: &mpsc::UnboundedSender<String>` - Channel for streaming responses

**Returns**:
- `LLMResult<()>` - Success or error

**Behavior**:
1. Validates API key from configuration
2. Creates completion request using `allms` crate
3. Simulates streaming by sending response in chunks
4. Sends `EOT_SIGNAL` when complete

**Error Handling**:
- Missing API key
- Invalid model name
- Network connectivity issues
- Provider-specific errors

**Example**:
```rust
let (tx, mut rx) = mpsc::unbounded_channel();
let provider = UnifiedLLMProvider::new(ProviderType::OpenAI);
let config = load_config(None).await?;

provider.send_chat_request(
    "Hello, how are you?",
    "gpt-4o-mini",
    &config,
    &tx
).await?;

// Receive streaming response
while let Some(chunk) = rx.recv().await {
    if chunk == EOT_SIGNAL {
        break;
    }
    print!("{}", chunk);
}
```

#### `provider_type()`
```rust
fn provider_type(&self) -> ProviderType
```
**Description**: Returns the provider type for this instance.

**Returns**:
- `ProviderType` - The provider type enum

#### `validate_config()`
```rust
async fn validate_config(&self, config: &AppConfig) -> LLMResult<()>
```
**Description**: Validates if the provider can be used with the given configuration.

**Parameters**:
- `config: &AppConfig` - Configuration to validate

**Returns**:
- `LLMResult<()>` - Success or validation error

**Validation Checks**:
- API key presence and format
- Required environment variables
- Provider-specific configuration requirements

**Example**:
```rust
let provider = UnifiedLLMProvider::new(ProviderType::Anthropic);
let config = load_config(None).await?;

match provider.validate_config(&config).await {
    Ok(()) => println!("Configuration valid"),
    Err(e) => eprintln!("Configuration error: {}", e),
}
```

## Type Aliases

### `LLMResult<T>`
```rust
pub type LLMResult<T> = Result<T>;
```
**Description**: Standard result type for LLM operations using `anyhow::Result`.

## Implementation Details

### Provider-Specific API Integration

#### OpenAI
```rust
use allms::llm_models::OpenAIModels;

// Model enumeration
let models: Vec<String> = OpenAIModels::iter()
    .map(|model| model.to_string())
    .collect();

// API request
let completions = Completions::new(&model_str, &api_key);
let response = completions.get_answer(&prompt).await?;
```

#### Anthropic
```rust
use allms::llm_models::AnthropicModels;

// Model enumeration  
let models: Vec<String> = AnthropicModels::iter()
    .map(|model| model.to_string())
    .collect();

// API request
let completions = Completions::new(&model_str, &api_key);
let response = completions.get_answer(&prompt).await?;
```

#### Google/Gemini
```rust
use allms::llm_models::GoogleModels;

// Model enumeration
let models: Vec<String> = GoogleModels::iter()
    .map(|model| model.to_string())
    .collect();

// API request with PROJECT_ID
std::env::set_var("PROJECT_ID", project_id);
let completions = Completions::new(&model_str, &api_key);
let response = completions.get_answer(&prompt).await?;
```

### Error Handling and Recovery

The module implements comprehensive error handling:

```rust
// Panic protection for external crate operations
async fn get_completion_response(&self, model: &str, api_key: &str, prompt: &str) -> LLMResult<String> {
    let result = tokio::task::spawn_blocking(move || {
        // External crate operations in blocking task
        self.execute_llm_request(provider_type, model, api_key, prompt)
    }).await;
    
    match result {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(anyhow::anyhow!("LLM provider panicked")),
    }
}
```

### Streaming Simulation

To provide better user experience, responses are simulated as streaming:

```rust
// Simulate streaming by sending response in chunks
let chunks: Vec<&str> = response.split_whitespace().collect();

for (i, chunk) in chunks.iter().enumerate() {
    tx.send(format!("{} ", chunk))?;
    
    // Add small delay for realistic streaming effect
    if i % 3 == 0 {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

// Signal completion
tx.send(crate::EOT_SIGNAL.to_string())?;
```

## Usage Examples

### Basic Provider Setup
```rust
use perspt::llm_provider::{UnifiedLLMProvider, ProviderType, LLMProvider};

// Create provider
let provider = UnifiedLLMProvider::new(ProviderType::OpenAI);

// List models
let models = provider.list_models().await?;
println!("Available models: {:?}", models);

// Validate configuration
let config = load_config(None).await?;
provider.validate_config(&config).await?;
```

### Chat Request Example
```rust
use tokio::sync::mpsc;

let (tx, mut rx) = mpsc::unbounded_channel();
let provider = UnifiedLLMProvider::new(ProviderType::Anthropic);

// Send request
provider.send_chat_request(
    "Explain quantum computing",
    "claude-3-sonnet-20240229",
    &config,
    &tx
).await?;

// Process streaming response
let mut full_response = String::new();
while let Some(chunk) = rx.recv().await {
    if chunk == crate::EOT_SIGNAL {
        break;
    }
    full_response.push_str(&chunk);
    print!("{}", chunk); // Real-time display
}

println!("\nComplete response: {}", full_response);
```

### Multi-Provider Support
```rust
let providers = vec![
    UnifiedLLMProvider::new(ProviderType::OpenAI),
    UnifiedLLMProvider::new(ProviderType::Anthropic),
    UnifiedLLMProvider::new(ProviderType::Google),
];

for provider in providers {
    println!("Provider: {:?}", provider.provider_type());
    match provider.list_models().await {
        Ok(models) => println!("Models: {:?}", models),
        Err(e) => println!("Error: {}", e),
    }
}
```

## Extension Points

### Adding New Providers

To add support for a new provider:

1. **Add to ProviderType enum**:
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderType {
    // Existing providers...
    NewProvider,
}
```

2. **Update string conversion**:
```rust
impl ProviderType {
    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            // Existing mappings...
            "newprovider" => Some(ProviderType::NewProvider),
            _ => None,
        }
    }
}
```

3. **Implement in UnifiedLLMProvider**:
```rust
pub fn get_available_models(&self) -> Vec<String> {
    match self.provider_type {
        // Existing providers...
        ProviderType::NewProvider => {
            // Return model list for new provider
            vec!["model1".to_string(), "model2".to_string()]
        }
    }
}
```

### Custom Provider Implementation

For completely custom providers, implement the `LLMProvider` trait:

```rust
pub struct CustomProvider {
    api_endpoint: String,
    auth_token: String,
}

#[async_trait]
impl LLMProvider for CustomProvider {
    async fn list_models(&self) -> LLMResult<Vec<String>> {
        // Custom model discovery logic
        todo!()
    }

    async fn send_chat_request(/* ... */) -> LLMResult<()> {
        // Custom API integration
        todo!()
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::Custom // Would need to add this variant
    }

    async fn validate_config(&self, config: &AppConfig) -> LLMResult<()> {
        // Custom validation logic
        todo!()
    }
}
```

## Performance Considerations

1. **Async Operations**: All network operations are async to prevent blocking
2. **Memory Efficiency**: Uses streaming for large responses
3. **Error Recovery**: Graceful handling of network and API errors
4. **Caching**: Model lists could be cached to reduce API calls

## Security Features

1. **API Key Protection**: Secure handling of authentication credentials
2. **Input Validation**: Sanitization of user inputs before API calls
3. **Error Sanitization**: Prevents exposure of sensitive information in error messages
4. **Environment Variable Support**: Secure credential management
