# Config Module API Documentation

## Module: `config.rs`

### Overview
Comprehensive configuration management system supporting multiple LLM providers, flexible authentication, and intelligent defaults.

## Data Structures

### `AppConfig`
```rust
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AppConfig {
    pub providers: HashMap<String, String>,
    pub api_key: Option<String>,
    pub default_model: Option<String>,
    pub default_provider: Option<String>,
    pub provider_type: Option<String>,
}
```

**Description**: Main configuration structure containing all configurable aspects of Perspt.

**Fields**:
- `providers: HashMap<String, String>` - Map of provider names to their API base URLs
- `api_key: Option<String>` - Universal API key for authentication
- `default_model: Option<String>` - Default model identifier for LLM requests
- `default_provider: Option<String>` - Name of default provider configuration
- `provider_type: Option<String>` - Provider type classification for API compatibility

**Supported Provider Types**:
- `"openai"` - OpenAI GPT models
- `"anthropic"` - Anthropic Claude models
- `"google"` - Google Gemini models
- `"groq"` - Groq ultra-fast inference
- `"cohere"` - Cohere Command models
- `"xai"` - XAI Grok models
- `"deepseek"` - DeepSeek models
- `"ollama"` - Local Ollama models

**Example Configuration**:
```json
{
  "api_key": "sk-your-api-key",
  "provider_type": "openai",
  "default_model": "gpt-4o-mini",
  "default_provider": "openai",
  "providers": {
    "openai": "https://api.openai.com/v1",
    "anthropic": "https://api.anthropic.com",
    "local-llm": "http://localhost:8080/v1"
  }
}
```

## Functions

### `process_loaded_config()`
```rust
pub fn process_loaded_config(mut config: AppConfig) -> AppConfig
```

**Description**: Processes and validates loaded configuration, applying intelligent defaults and provider type inference.

**Parameters**:
- `config: AppConfig` - The configuration to process

**Returns**:
- `AppConfig` - Processed configuration with inferred values

**Provider Type Inference Logic**:
If `provider_type` is None, attempts inference from `default_provider`:

| Default Provider | Inferred Type |
|------------------|---------------|
| "openai" | "openai" |
| "anthropic" | "anthropic" |
| "google", "gemini" | "google" |
| "groq" | "groq" |
| "cohere" | "cohere" |
| "xai" | "xai" |
| "deepseek" | "deepseek" |
| "ollama" | "ollama" |
| Unknown | "openai" (default) |

**Example**:
```rust
let mut config = AppConfig {
    providers: HashMap::new(),
    api_key: None,
    default_model: None,
    default_provider: Some("anthropic".to_string()),
    provider_type: None, // Will be inferred as "anthropic"
};

let processed = process_loaded_config(config);
assert_eq!(processed.provider_type, Some("anthropic".to_string()));
```

### `load_config()`
```rust
pub async fn load_config(config_path: Option<&String>) -> Result<AppConfig>
```

**Description**: Loads application configuration from a file or provides comprehensive defaults.

**Parameters**:
- `config_path: Option<&String>` - Optional path to JSON configuration file

**Returns**:
- `Result<AppConfig>` - Loaded configuration or error

**Behavior**:

#### With Configuration File (Some(path))
1. Reads JSON file from filesystem
2. Parses JSON into AppConfig structure  
3. Processes configuration with `process_loaded_config()`
4. Returns processed configuration

#### Without Configuration File (None)
Creates default configuration with:
- All supported provider endpoints pre-configured
- OpenAI as default provider with gpt-4o-mini model
- No API key (must be set via environment or CLI)

**Default Provider Endpoints**:
```rust
{
    "openai": "https://api.openai.com/v1",
    "anthropic": "https://api.anthropic.com", 
    "google": "https://generativelanguage.googleapis.com/v1beta/",
    "groq": "https://api.groq.com/openai/v1",
    "cohere": "https://api.cohere.com/v1",
    "xai": "https://api.x.ai/v1",
    "deepseek": "https://api.deepseek.com/v1",
    "ollama": "http://localhost:11434/v1"
}
```

**Errors**:
- File system errors (file not found, permission denied)
- JSON parsing errors (invalid syntax, missing fields)
- I/O errors during file reading

**Examples**:
```rust
// Load from specific file
let config = load_config(Some(&"config.json".to_string())).await?;

// Use defaults
let default_config = load_config(None).await?;

// Error handling
match load_config(Some(&"missing.json".to_string())).await {
    Ok(config) => println!("Loaded: {:?}", config),
    Err(e) => eprintln!("Failed to load config: {}", e),
}
```

## Configuration Examples

### Basic OpenAI Configuration
```json
{
  "api_key": "sk-your-openai-key",
  "provider_type": "openai",
  "default_model": "gpt-4o-mini"
}
```

### Multi-Provider Configuration
```json
{
  "api_key": "your-default-key",
  "provider_type": "anthropic",
  "default_model": "claude-3-sonnet-20240229",
  "default_provider": "anthropic",
  "providers": {
    "openai": "https://api.openai.com/v1",
    "anthropic": "https://api.anthropic.com",
    "local-openai": "http://localhost:8080/v1",
    "proxy-claude": "https://your-proxy.com/anthropic"
  }
}
```

### Minimal Configuration (Provider Inference)
```json
{
  "default_provider": "google",
  "default_model": "gemini-pro"
}
```
*Provider type will be inferred as "google"*

### Local Development Configuration
```json
{
  "provider_type": "openai",
  "default_model": "gpt-3.5-turbo",
  "providers": {
    "openai": "http://localhost:8080/v1"
  }
}
```

## Environment Variable Integration

The configuration system integrates with environment variables for API keys:

```bash
# Provider-specific API keys
export OPENAI_API_KEY="sk-your-openai-key"
export ANTHROPIC_API_KEY="sk-ant-your-anthropic-key"
export GOOGLE_API_KEY="your-google-api-key"

# AWS credentials for Bedrock
export AWS_PROFILE="your-aws-profile"
export AWS_REGION="us-east-1"
export AWS_ACCESS_KEY_ID="your-access-key"
export AWS_SECRET_ACCESS_KEY="your-secret-key"

# Google Cloud for Vertex AI
export PROJECT_ID="your-gcp-project-id"
export GOOGLE_APPLICATION_CREDENTIALS="/path/to/service-account.json"
```

## Configuration Validation

The module provides automatic validation:

1. **Type Inference**: Automatically determines provider type from default provider
2. **Default Fallbacks**: Provides sensible defaults when values are missing
3. **Structure Validation**: Ensures required fields are present
4. **Format Validation**: Validates JSON structure and field types

## Testing Support

The module includes comprehensive test coverage:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_load_config_defaults() {
        let config = load_config(None).await.unwrap();
        assert_eq!(config.provider_type, Some("openai".to_string()));
        assert!(config.providers.contains_key("openai"));
    }

    #[tokio::test]
    async fn test_provider_type_inference() {
        let json_input = r#"
        {
            "default_provider": "anthropic",
            "default_model": "claude-3-sonnet-20240229",
            "api_key": "test_key"
        }
        "#;
        
        let initial_config: AppConfig = serde_json::from_str(json_input).unwrap();
        let config = process_loaded_config(initial_config);
        
        assert_eq!(config.provider_type, Some("anthropic".to_string()));
    }
}
```

## Security Considerations

1. **File Permissions**: Configuration files should have restricted permissions (600)
2. **API Key Handling**: Prefer environment variables over config files for API keys
3. **Validation**: All configuration values are validated before use
4. **Error Messages**: Avoid exposing sensitive information in error messages

## Best Practices

1. **Use Environment Variables**: For API keys and sensitive configuration
2. **Version Control**: Never commit API keys to version control
3. **Separate Environments**: Use different configurations for development/production
4. **Validate Early**: Load and validate configuration at application startup
5. **Graceful Defaults**: Provide sensible defaults for optional configuration

## Migration Guide

### From v0.3.x to v0.4.x
- `provider_type` field added for explicit provider classification
- Automatic provider type inference from `default_provider`
- New provider endpoints for Google, Groq, Cohere, XAI, DeepSeek, and Ollama
- Backward compatibility maintained for existing configurations
