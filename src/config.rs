//! # Configuration Management Module
//!
//! This module handles all configuration-related functionality for the Perspt application.
//! It provides a flexible configuration system that supports multiple LLM providers,
//! custom endpoints, and various authentication methods.
//!
//! ## Features
//!
//! - **Multi-provider support**: Configuration for OpenAI, Anthropic, Google, Mistral, and more
//! - **Automatic inference**: Smart provider type detection based on configuration
//! - **Default fallbacks**: Sensible defaults when configuration is missing
//! - **JSON-based**: Human-readable JSON configuration files
//! - **Environment integration**: Support for environment-based API keys
//!
//! ## Configuration Structure
//!
//! The configuration supports both simple and complex setups:
//!
//! ```json
//! {
//!   "api_key": "your-api-key",
//!   "provider_type": "openai",
//!   "default_model": "gpt-4o-mini",
//!   "providers": {
//!     "openai": "https://api.openai.com/v1",
//!     "anthropic": "https://api.anthropic.com"
//!   }
//! }
//! ```

// src/config.rs
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use anyhow::Result;

/// Application configuration structure that defines all configurable aspects of Perspt.
///
/// This structure supports flexible configuration for multiple LLM providers with
/// automatic fallbacks and intelligent defaults. The configuration can be loaded
/// from JSON files or created programmatically for testing.
///
/// # Fields
///
/// * `providers` - Map of provider names to their API endpoints
/// * `api_key` - Universal API key for authentication (provider-specific keys can override)
/// * `default_model` - Model identifier to use when none is specified
/// * `default_provider` - Provider name to use when none is specified
/// * `provider_type` - Provider type classification for API compatibility
///
/// # Provider Types
///
/// Supported provider types:
/// - `openai`: OpenAI GPT models
/// - `anthropic`: Anthropic Claude models  
/// - `google`: Google Gemini models
/// - `mistral`: Mistral AI models
/// - `perplexity`: Perplexity AI models
/// - `deepseek`: DeepSeek models
/// - `aws-bedrock`: AWS Bedrock service
/// - `azure-openai`: Azure OpenAI service
///
/// # Examples
///
/// ```rust
/// use perspt::config::AppConfig;
/// use std::collections::HashMap;
///
/// // Create a basic OpenAI configuration
/// let mut providers = HashMap::new();
/// providers.insert("openai".to_string(), "https://api.openai.com/v1".to_string());
///
/// let config = AppConfig {
///     providers,
///     api_key: Some("sk-...".to_string()),
///     default_model: Some("gpt-4o-mini".to_string()),
///     default_provider: Some("openai".to_string()),
///     provider_type: Some("openai".to_string()),
/// };
/// ```
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AppConfig {
    /// Map of provider names to their API base URLs.
    /// 
    /// Example: "openai" -> "<https://api.openai.com/v1>"
    /// This allows for custom endpoints and local installations.
    pub providers: HashMap<String, String>,
    
    /// Universal API key for provider authentication.
    /// 
    /// Individual providers may override this with provider-specific keys.
    /// Can be None if using environment variables or other auth methods.
    pub api_key: Option<String>,
    
    /// Default model identifier to use for LLM requests.
    /// 
    /// This should be a valid model name for the configured provider.
    /// Examples: "gpt-4o-mini", "claude-3-sonnet-20240229", "gemini-pro"
    pub default_model: Option<String>,
    
    /// Name of the default provider configuration to use.
    /// 
    /// This should match a key in the providers HashMap.
    /// Used for provider selection when multiple are configured.
    pub default_provider: Option<String>,
    
    /// Provider type classification for API compatibility.
    /// 
    /// Determines which API interface and authentication method to use.
    /// Valid values: "openai", "anthropic", "google", "mistral", "perplexity", 
    /// "deepseek", "aws-bedrock", "azure-openai"
    pub provider_type: Option<String>,
}

/// Processes and validates a loaded configuration, applying intelligent defaults.
///
/// This function performs post-processing on configuration loaded from JSON or
/// created programmatically. It applies intelligent inference to determine the
/// provider type based on the default provider name if not explicitly set.
///
/// # Arguments
///
/// * `config` - The configuration to process
///
/// # Returns
///
/// * `AppConfig` - The processed configuration with inferred values
///
/// # Provider Type Inference
///
/// If `provider_type` is None, the function attempts to infer it from the
/// `default_provider` field using these mappings:
/// - "openai" -> "openai"
/// - "anthropic" -> "anthropic"  
/// - "google" or "gemini" -> "google"
/// - "mistral" -> "mistral"
/// - "perplexity" -> "perplexity"
/// - "deepseek" -> "deepseek"
/// - "aws", "bedrock", or "aws-bedrock" -> "aws-bedrock"
/// - "azure" or "azure-openai" -> "azure-openai"
/// - Unknown providers default to "openai"
///
/// # Examples
///
/// ```rust
/// use perspt::config::{AppConfig, process_loaded_config};
/// use std::collections::HashMap;
///
/// let mut config = AppConfig {
///     providers: HashMap::new(),
///     api_key: None,
///     default_model: None,
///     default_provider: Some("anthropic".to_string()),
///     provider_type: None, // Will be inferred
/// };
///
/// let processed = process_loaded_config(config);
/// assert_eq!(processed.provider_type, Some("anthropic".to_string()));
/// ```
pub fn process_loaded_config(mut config: AppConfig) -> AppConfig {
    if config.provider_type.is_none() {
        if let Some(dp) = &config.default_provider {
            match dp.as_str() {
                "openai" => config.provider_type = Some("openai".to_string()),
                "anthropic" => config.provider_type = Some("anthropic".to_string()),
                "google" | "gemini" => config.provider_type = Some("google".to_string()),
                "mistral" => config.provider_type = Some("mistral".to_string()),
                "perplexity" => config.provider_type = Some("perplexity".to_string()),
                "deepseek" => config.provider_type = Some("deepseek".to_string()),
                "aws" | "bedrock" | "aws-bedrock" => config.provider_type = Some("aws-bedrock".to_string()),
                "azure" | "azure-openai" => config.provider_type = Some("azure-openai".to_string()),
                _ => {
                    // Default to OpenAI if provider not recognized
                    config.provider_type = Some("openai".to_string());
                }
            }
        } else {
            // Default to OpenAI if nothing else is specified
            config.provider_type = Some("openai".to_string());
        }
    }
    config
}

/// Loads application configuration from a file or provides sensible defaults.
///
/// This asynchronous function handles configuration loading with robust fallback behavior.
/// It can load configuration from a JSON file or provide comprehensive defaults when
/// no configuration file is specified.
///
/// # Arguments
///
/// * `config_path` - Optional path to a JSON configuration file
///
/// # Returns
///
/// * `Result<AppConfig>` - The loaded configuration or an error
///
/// # Behavior
///
/// ## With Configuration File
/// When a path is provided:
/// 1. Reads the JSON file from the filesystem
/// 2. Parses the JSON into an AppConfig structure
/// 3. Processes the configuration to apply intelligent defaults
/// 4. Returns the processed configuration
///
/// ## Without Configuration File
/// When no path is provided:
/// 1. Creates a comprehensive default configuration
/// 2. Includes all supported provider endpoints
/// 3. Sets OpenAI as the default provider with GPT-4o-mini model
/// 4. Returns the default configuration
///
/// # Default Configuration
///
/// The default configuration includes endpoints for:
/// - OpenAI: <https://api.openai.com/v1>
/// - Anthropic: <https://api.anthropic.com>
/// - Google: <https://generativelanguage.googleapis.com/v1beta/>
/// - Mistral: <https://api.mistral.ai/v1>
/// - Perplexity: <https://api.perplexity.ai>
/// - DeepSeek: <https://api.deepseek.com/v1>
/// - AWS Bedrock: <https://bedrock.amazonaws.com>
/// - Azure OpenAI: <https://api.openai.azure.com>
///
/// # Errors
///
/// This function can return errors for:
/// - File system errors (file not found, permission denied)
/// - JSON parsing errors (invalid syntax, missing fields)
/// - I/O errors during file reading
///
/// # Examples
///
/// ```rust
/// use perspt::config::load_config;
///
/// // Load from file
/// let config = load_config(Some(&"config.json".to_string())).await?;
///
/// // Use defaults
/// let default_config = load_config(None).await?;
/// ```
pub async fn load_config(config_path: Option<&String>) -> Result<AppConfig> {
    let config: AppConfig = match config_path {
        Some(path) => {
            let config_str = fs::read_to_string(path)?;
            let initial_config: AppConfig = serde_json::from_str(&config_str)?;
            process_loaded_config(initial_config)
        }
        None => {
            // Default configuration with all supported providers
            let mut providers_map = HashMap::new();
            providers_map.insert("openai".to_string(), "https://api.openai.com/v1".to_string());
            providers_map.insert("anthropic".to_string(), "https://api.anthropic.com".to_string());
            providers_map.insert("google".to_string(), "https://generativelanguage.googleapis.com/v1beta/".to_string());
            providers_map.insert("mistral".to_string(), "https://api.mistral.ai/v1".to_string());
            providers_map.insert("perplexity".to_string(), "https://api.perplexity.ai".to_string());
            providers_map.insert("deepseek".to_string(), "https://api.deepseek.com/v1".to_string());
            providers_map.insert("aws-bedrock".to_string(), "https://bedrock.amazonaws.com".to_string());
            providers_map.insert("azure-openai".to_string(), "https://api.openai.azure.com".to_string());
            
            AppConfig {
                providers: providers_map,
                api_key: None,
                default_model: Some("gpt-4o-mini".to_string()), 
                default_provider: Some("openai".to_string()),   
                provider_type: Some("openai".to_string()),    
            }
        }
    };
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[tokio::test]
    async fn test_load_config_defaults() {
        let config = load_config(None).await.unwrap();
        assert_eq!(config.provider_type, Some("openai".to_string()));
        assert_eq!(config.default_provider, Some("openai".to_string()));
        assert_eq!(config.default_model, Some("gpt-4o-mini".to_string()));
        assert!(config.providers.contains_key("openai"));
        assert!(config.providers.contains_key("anthropic"));
        assert!(config.providers.contains_key("google"));
        assert_eq!(config.api_key, None);
    }

    // Test helper function that simulates loading from a string, then processes
    async fn load_config_from_string_for_test(json_str: &str) -> Result<AppConfig, Box<dyn Error>> {
        let initial_config: AppConfig = serde_json::from_str(json_str)?;
        Ok(process_loaded_config(initial_config))
    }

    #[tokio::test]
    async fn test_load_config_from_json_string_infer_provider_type_openai() {
        let json_input = r#"
        {
            "providers": {"openai": "https://api.openai.com/v1"},
            "default_provider": "openai",
            "default_model": "gpt-3.5-turbo",
            "api_key": "test_openai_key"
        }
        "#;
        let config = load_config_from_string_for_test(json_input).await.unwrap();
        assert_eq!(config.provider_type, Some("openai".to_string()));
        assert_eq!(config.default_provider, Some("openai".to_string()));
        assert_eq!(config.default_model, Some("gpt-3.5-turbo".to_string()));
        assert_eq!(config.api_key, Some("test_openai_key".to_string()));
    }

    #[tokio::test]
    async fn test_load_config_from_json_string_infer_provider_type_anthropic() {
        let json_input = r#"
        {
            "providers": {"anthropic": "https://api.anthropic.com"},
            "default_provider": "anthropic",
            "default_model": "claude-3-sonnet-20240229",
            "api_key": "test_anthropic_key"
        }
        "#;
        let config = load_config_from_string_for_test(json_input).await.unwrap();
        assert_eq!(config.provider_type, Some("anthropic".to_string()));
        assert_eq!(config.default_provider, Some("anthropic".to_string()));
        assert_eq!(config.default_model, Some("claude-3-sonnet-20240229".to_string()));
        assert_eq!(config.api_key, Some("test_anthropic_key".to_string()));
    }

    #[tokio::test]
    async fn test_load_config_from_json_string_infer_provider_type_google() {
        let json_input = r#"
        {
            "providers": {"google": "https://generativelanguage.googleapis.com/v1beta/"},
            "default_provider": "google",
            "default_model": "gemini-pro",
            "api_key": "test_google_key"
        }
        "#;
        let config = load_config_from_string_for_test(json_input).await.unwrap();
        assert_eq!(config.provider_type, Some("google".to_string()));
        assert_eq!(config.default_provider, Some("google".to_string()));
        assert_eq!(config.default_model, Some("gemini-pro".to_string()));
        assert_eq!(config.api_key, Some("test_google_key".to_string()));
    }

    #[tokio::test]
    async fn test_load_config_from_json_string_infer_provider_type_gemini() {
        let json_input = r#"
        {
            "providers": {"gemini": "https://gemini.api/v1"},
            "default_provider": "gemini",
            "default_model": "gemini-1.5-flash"
        }
        "#;
        let config = load_config_from_string_for_test(json_input).await.unwrap();
        assert_eq!(config.provider_type, Some("google".to_string())); // "gemini" maps to "google"
        assert_eq!(config.default_provider, Some("gemini".to_string()));
        assert_eq!(config.default_model, Some("gemini-1.5-flash".to_string()));
    }
    
    #[tokio::test]
    async fn test_load_config_from_json_string_provider_type_explicitly_set() {
        let json_input = r#"
        {
            "providers": {"local_model_service": "http://localhost:1234"},
            "default_provider": "my_local_setup", 
            "provider_type": "local_llm",
            "default_model": "/path/to/my/model.gguf"
        }
        "#;
        let config = load_config_from_string_for_test(json_input).await.unwrap();
        assert_eq!(config.provider_type, Some("local_llm".to_string()));
        assert_eq!(config.default_provider, Some("my_local_setup".to_string()));
        assert_eq!(config.default_model, Some("/path/to/my/model.gguf".to_string()));
    }

    #[tokio::test]
    async fn test_load_config_from_json_string_missing_provider_type_and_default_provider() {
        let json_input_no_provider_info = r#"
        {
            "providers": {},
            "default_model": "some-generic-model",
            "api_key": null
        }
        "#;
        let config = load_config_from_string_for_test(json_input_no_provider_info).await.unwrap();
        // Falls back to "openai" as per current process_loaded_config logic
        assert_eq!(config.provider_type, Some("openai".to_string())); 
        assert_eq!(config.default_model, Some("some-generic-model".to_string()));
    }

    #[tokio::test]
    async fn test_load_config_from_json_string_default_provider_unknown_no_provider_type() {
        let json_input = r#"
        {
            "providers": {"custom_local": "path_irrelevant"},
            "default_provider": "custom_local_profile_name",
            "default_model": "my_custom_model.bin"
        }
        "#;
        let config = load_config_from_string_for_test(json_input).await.unwrap();
        // According to current logic in process_loaded_config, if default_provider is not recognized,
        // and provider_type is missing, it will default provider_type to "openai".
        assert_eq!(config.provider_type, Some("openai".to_string()));
        assert_eq!(config.default_provider, Some("custom_local_profile_name".to_string()));
        assert_eq!(config.default_model, Some("my_custom_model.bin".to_string()));
    }

    #[tokio::test]
    async fn test_load_config_minimal_config() {
        let json_input = r#"
        {
            "providers": {}
        }
        "#;
        let config = load_config_from_string_for_test(json_input).await.unwrap();
        assert_eq!(config.provider_type, Some("openai".to_string())); // Default
        assert_eq!(config.default_provider, None);
        assert_eq!(config.default_model, None);
        assert_eq!(config.api_key, None);
        assert_eq!(config.providers, HashMap::new());
    }
}
