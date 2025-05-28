// src/config.rs
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use anyhow::Result;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AppConfig {
    pub providers: HashMap<String, String>, // e.g., "openai" -> "https://api.openai.com/v1"
    pub api_key: Option<String>,           // For API-based providers
    pub default_model: Option<String>,     // Model name for API
    pub default_provider: Option<String>,  // Name of the provider configuration to use
    pub provider_type: Option<String>,     // Type of provider: "openai", "anthropic", "google", "mistral", "perplexity", "deepseek", "aws-bedrock", "azure-openai"
}

/// Helper function for testing: processes a config from a JSON string
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
