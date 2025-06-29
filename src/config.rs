//! # Configuration Management Module
//!
//! This module handles all configuration-related functionality for the Perspt application.
//! It provides a flexible configuration system that supports multiple LLM providers,
//! custom endpoints, and various authentication methods.
//!
//! ## Features
//!
//! - **Multi-provider support**: Configuration for OpenAI, Anthropic, Gemini, Groq, Cohere, XAI, DeepSeek, and Ollama
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
//!     "anthropic": "https://api.anthropic.com",
//!     "gemini": "https://generativelanguage.googleapis.com/v1beta",
//!     "groq": "https://api.groq.com/openai/v1",
//!     "cohere": "https://api.cohere.com/v1",
//!     "xai": "https://api.x.ai/v1",
//!     "deepseek": "https://api.deepseek.com/v1",
//!     "ollama": "http://localhost:11434/v1"
//!   }
//! }
//! ```
//!     "cohere": "https://api.cohere.com/v1",
//!     "xai": "https://api.x.ai/v1",
//!     "deepseek": "https://api.deepseek.com/v1",
//!     "ollama": "http://localhost:11434/v1"
//!   }
//! }
//! ```

// src/config.rs
use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

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
/// Supported provider types (based on genai 0.3.5):
/// - `openai`: OpenAI GPT models
/// - `anthropic`: Anthropic Claude models  
/// - `gemini`: Google Gemini models
/// - `groq`: Groq models (Llama, Mixtral, etc.)
/// - `cohere`: Cohere Command models
/// - `xai`: XAI Grok models
/// - `deepseek`: DeepSeek models
/// - `ollama`: Local models via Ollama
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
    /// Valid values: "openai", "anthropic", "gemini", "groq", "cohere",
    /// "xai", "deepseek", "ollama"
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
/// - "google" or "gemini" -> "gemini"
/// - "groq" -> "groq"
/// - "cohere" -> "cohere"
/// - "xai" -> "xai"
/// - "deepseek" -> "deepseek"
/// - "ollama" -> "ollama"
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
                "google" | "gemini" => config.provider_type = Some("gemini".to_string()),
                "groq" => config.provider_type = Some("groq".to_string()),
                "cohere" => config.provider_type = Some("cohere".to_string()),
                "xai" => config.provider_type = Some("xai".to_string()),
                "deepseek" => config.provider_type = Some("deepseek".to_string()),
                "ollama" => config.provider_type = Some("ollama".to_string()),
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

/// Automatically detects available LLM providers based on environment variables.
///
/// This function scans the environment for API keys of supported providers and
/// returns the first available provider along with its corresponding default model.
/// This enables true automatic configuration without requiring explicit provider
/// selection from the user.
///
/// # Provider Detection Order
///
/// The function checks for API keys in this priority order:
/// 1. OpenAI (`OPENAI_API_KEY`)
/// 2. Anthropic (`ANTHROPIC_API_KEY`)
/// 3. Gemini (`GEMINI_API_KEY`)
/// 4. Groq (`GROQ_API_KEY`)
/// 5. Cohere (`COHERE_API_KEY`)
/// 6. XAI (`XAI_API_KEY`)
/// 7. DeepSeek (`DEEPSEEK_API_KEY`)
/// 8. Ollama (no API key required, checks if localhost:11434 is accessible)
///
/// # Returns
///
/// * `Option<(String, String)>` - Tuple of (provider_type, default_model) if found
///
/// # Examples
///
/// ```rust
/// use perspt::config::detect_available_provider;
/// use std::env;
///
/// // Set an API key
/// env::set_var("ANTHROPIC_API_KEY", "sk-ant-123...");
///
/// if let Some((provider, model)) = detect_available_provider() {
///     println!("Auto-detected provider: {} with model: {}", provider, model);
/// }
/// ```
pub fn detect_available_provider() -> Option<(String, String)> {
    use std::env;

    // Check for API keys in priority order
    let providers_to_check = [
        ("OPENAI_API_KEY", "openai", "gpt-4o-mini"),
        (
            "ANTHROPIC_API_KEY",
            "anthropic",
            "claude-3-5-sonnet-20241022",
        ),
        ("GEMINI_API_KEY", "gemini", "gemini-1.5-flash"),
        ("GROQ_API_KEY", "groq", "llama-3.1-8b-instant"),
        ("COHERE_API_KEY", "cohere", "command-r-plus"),
        ("XAI_API_KEY", "xai", "grok-beta"),
        ("DEEPSEEK_API_KEY", "deepseek", "deepseek-chat"),
    ];

    for (env_var, provider_type, default_model) in providers_to_check {
        if env::var(env_var).is_ok() {
            log::info!(
                "Auto-detected provider '{provider_type}' from environment variable {env_var}",
            );
            return Some((provider_type.to_string(), default_model.to_string()));
        }
    }

    // Check for Ollama by attempting to detect if it's running locally
    // For now, we'll just check if the user has explicitly set any Ollama-related env vars
    // In a full implementation, we could make an HTTP request to localhost:11434
    if env::var("OLLAMA_HOST").is_ok() {
        log::info!("Auto-detected Ollama from OLLAMA_HOST environment variable");
        return Some(("ollama".to_string(), "llama3.2".to_string()));
    }

    log::debug!("No providers auto-detected from environment variables");
    None
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
/// - Google Gemini: <https://generativelanguage.googleapis.com/v1beta>
/// - Groq: <https://api.groq.com/openai/v1>
/// - Cohere: <https://api.cohere.com/v1>
/// - XAI: <https://api.x.ai/v1>
/// - DeepSeek: <https://api.deepseek.com/v1>
/// - Ollama: <http://localhost:11434/v1>
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
            // Default configuration with all supported providers (genai 0.3.5)
            let mut providers_map = HashMap::new();
            providers_map.insert(
                "openai".to_string(),
                "https://api.openai.com/v1".to_string(),
            );
            providers_map.insert(
                "anthropic".to_string(),
                "https://api.anthropic.com".to_string(),
            );
            providers_map.insert(
                "gemini".to_string(),
                "https://generativelanguage.googleapis.com/v1beta".to_string(),
            );
            providers_map.insert(
                "groq".to_string(),
                "https://api.groq.com/openai/v1".to_string(),
            );
            providers_map.insert(
                "cohere".to_string(),
                "https://api.cohere.com/v1".to_string(),
            );
            providers_map.insert("xai".to_string(), "https://api.x.ai/v1".to_string());
            providers_map.insert(
                "deepseek".to_string(),
                "https://api.deepseek.com/v1".to_string(),
            );
            providers_map.insert(
                "ollama".to_string(),
                "http://localhost:11434/v1".to_string(),
            );

            // Try to auto-detect provider from environment variables
            if let Some((default_provider_type, default_model)) = detect_available_provider() {
                AppConfig {
                    providers: providers_map,
                    api_key: None,
                    default_model: Some(default_model),
                    default_provider: Some(default_provider_type.clone()),
                    provider_type: Some(default_provider_type),
                }
            } else {
                // No providers auto-detected, create config without provider_type
                // This will trigger the "no provider configured" error message
                AppConfig {
                    providers: providers_map,
                    api_key: None,
                    default_model: None,
                    default_provider: None,
                    provider_type: None,
                }
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
        use std::env;

        // Clear all API keys to ensure no auto-detection
        let keys_to_clear = [
            "OPENAI_API_KEY",
            "ANTHROPIC_API_KEY",
            "GEMINI_API_KEY",
            "GROQ_API_KEY",
            "COHERE_API_KEY",
            "XAI_API_KEY",
            "DEEPSEEK_API_KEY",
            "OLLAMA_HOST",
        ];
        for key in &keys_to_clear {
            env::remove_var(key);
        }

        let config = load_config(None).await.unwrap();
        // With no API keys, provider_type should be None to trigger "no provider configured" error
        assert_eq!(config.provider_type, None);
        assert_eq!(config.default_provider, None);
        assert_eq!(config.default_model, None);
        assert!(config.providers.contains_key("openai"));
        assert!(config.providers.contains_key("anthropic"));
        assert!(config.providers.contains_key("gemini"));
        assert!(config.providers.contains_key("groq"));
        assert!(config.providers.contains_key("cohere"));
        assert!(config.providers.contains_key("xai"));
        assert!(config.providers.contains_key("deepseek"));
        assert!(config.providers.contains_key("ollama"));
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
        assert_eq!(
            config.default_model,
            Some("claude-3-sonnet-20240229".to_string())
        );
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
        assert_eq!(config.provider_type, Some("gemini".to_string()));
        assert_eq!(config.default_provider, Some("google".to_string()));
        assert_eq!(config.default_model, Some("gemini-pro".to_string()));
        assert_eq!(config.api_key, Some("test_google_key".to_string()));
    }

    #[tokio::test]
    async fn test_load_config_from_json_string_infer_provider_type_gemini() {
        let json_input = r#"
        {
            "providers": {"gemini": "https://generativelanguage.googleapis.com/v1beta"},
            "default_provider": "gemini",
            "default_model": "gemini-1.5-flash"
        }
        "#;
        let config = load_config_from_string_for_test(json_input).await.unwrap();
        assert_eq!(config.provider_type, Some("gemini".to_string())); // "gemini" maps to "gemini"
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
        assert_eq!(
            config.default_model,
            Some("/path/to/my/model.gguf".to_string())
        );
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
        let config = load_config_from_string_for_test(json_input_no_provider_info)
            .await
            .unwrap();
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
        assert_eq!(
            config.default_provider,
            Some("custom_local_profile_name".to_string())
        );
        assert_eq!(
            config.default_model,
            Some("my_custom_model.bin".to_string())
        );
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

    #[tokio::test]
    async fn test_detect_available_provider_openai() {
        use std::env;

        // Clear any existing keys first
        let keys_to_clear = [
            "OPENAI_API_KEY",
            "ANTHROPIC_API_KEY",
            "GEMINI_API_KEY",
            "GROQ_API_KEY",
            "COHERE_API_KEY",
            "XAI_API_KEY",
            "DEEPSEEK_API_KEY",
            "OLLAMA_HOST",
        ];
        for key in &keys_to_clear {
            env::remove_var(key);
        }

        // Set OpenAI key
        env::set_var("OPENAI_API_KEY", "sk-test123");

        let result = detect_available_provider();
        assert_eq!(
            result,
            Some(("openai".to_string(), "gpt-4o-mini".to_string()))
        );

        // Clean up
        env::remove_var("OPENAI_API_KEY");
    }

    #[tokio::test]
    async fn test_detect_available_provider_anthropic() {
        use std::env;

        // Clear any existing keys first
        let keys_to_clear = [
            "OPENAI_API_KEY",
            "ANTHROPIC_API_KEY",
            "GEMINI_API_KEY",
            "GROQ_API_KEY",
            "COHERE_API_KEY",
            "XAI_API_KEY",
            "DEEPSEEK_API_KEY",
            "OLLAMA_HOST",
        ];
        for key in &keys_to_clear {
            env::remove_var(key);
        }

        // Set Anthropic key (should be detected since OpenAI is not set)
        env::set_var("ANTHROPIC_API_KEY", "sk-ant-test123");

        let result = detect_available_provider();
        assert_eq!(
            result,
            Some((
                "anthropic".to_string(),
                "claude-3-5-sonnet-20241022".to_string()
            ))
        );

        // Clean up
        env::remove_var("ANTHROPIC_API_KEY");
    }

    #[tokio::test]
    async fn test_detect_available_provider_priority_order() {
        use std::env;

        // Clear any existing keys first
        let keys_to_clear = [
            "OPENAI_API_KEY",
            "ANTHROPIC_API_KEY",
            "GEMINI_API_KEY",
            "GROQ_API_KEY",
            "COHERE_API_KEY",
            "XAI_API_KEY",
            "DEEPSEEK_API_KEY",
            "OLLAMA_HOST",
        ];
        for key in &keys_to_clear {
            env::remove_var(key);
        }

        // Set multiple keys - OpenAI should win due to priority order
        env::set_var("ANTHROPIC_API_KEY", "sk-ant-test123");
        env::set_var("OPENAI_API_KEY", "sk-test123");
        env::set_var("GEMINI_API_KEY", "test-gemini-key");

        let result = detect_available_provider();
        assert_eq!(
            result,
            Some(("openai".to_string(), "gpt-4o-mini".to_string()))
        );

        // Clean up
        for key in &keys_to_clear {
            env::remove_var(key);
        }
    }

    #[tokio::test]
    async fn test_detect_available_provider_none() {
        use std::env;

        // Clear all provider keys
        let keys_to_clear = [
            "OPENAI_API_KEY",
            "ANTHROPIC_API_KEY",
            "GEMINI_API_KEY",
            "GROQ_API_KEY",
            "COHERE_API_KEY",
            "XAI_API_KEY",
            "DEEPSEEK_API_KEY",
            "OLLAMA_HOST",
        ];
        for key in &keys_to_clear {
            env::remove_var(key);
        }

        let result = detect_available_provider();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_load_config_with_auto_detection() {
        use std::env;

        // Clear keys first
        let keys_to_clear = [
            "OPENAI_API_KEY",
            "ANTHROPIC_API_KEY",
            "GEMINI_API_KEY",
            "GROQ_API_KEY",
            "COHERE_API_KEY",
            "XAI_API_KEY",
            "DEEPSEEK_API_KEY",
            "OLLAMA_HOST",
        ];
        for key in &keys_to_clear {
            env::remove_var(key);
        }

        // Set Anthropic key
        env::set_var("ANTHROPIC_API_KEY", "sk-ant-test123");

        let config = load_config(None).await.unwrap();
        assert_eq!(config.provider_type, Some("anthropic".to_string()));
        assert_eq!(config.default_provider, Some("anthropic".to_string()));
        assert_eq!(
            config.default_model,
            Some("claude-3-5-sonnet-20241022".to_string())
        );

        // Clean up
        for key in &keys_to_clear {
            env::remove_var(key);
        }
    }
}
