// src/config.rs
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::fs;

#[derive(Debug, Clone, Deserialize, PartialEq)] // Added PartialEq for easier assertions in tests
pub struct AppConfig {
    pub providers: HashMap<String, String>, // e.g., "openai" -> "https://api.openai.com/v1"
    pub api_key: Option<String>,           // For API-based providers
    pub default_model: Option<String>,     // Model name for API, path for local_llm
    pub default_provider: Option<String>,  // Name of the provider configuration to use, e.g., "openai", "local_main_model"
    pub provider_type: Option<String>,     // Type of provider, e.g., "openai", "gemini", "local_llm"
}

// Helper function for testing: processes a config from a JSON string
// This mirrors the logic within the Some(path) arm of load_config after file reading.
pub fn process_loaded_config(mut config: AppConfig) -> AppConfig {
    if config.provider_type.is_none() {
        if let Some(dp) = &config.default_provider {
            if dp == "openai" || dp == "gemini" {
                config.provider_type = Some(dp.clone());
            } else {
                // If default_provider is something else (e.g., a local model profile name)
                // and provider_type is missing, we might not be able to infer standard API types.
                // Defaulting to "gemini" here might be too presumptive if it's truly a custom/local setup.
                // However, for backward compatibility with current logic:
                config.provider_type = Some("gemini".to_string());
            }
        } else {
            // Default to gemini if nothing else is specified and provider_type is missing
            config.provider_type = Some("gemini".to_string());
        }
    }
    config
}


pub async fn load_config(config_path: Option<&String>) -> Result<AppConfig, Box<dyn Error>> {
    let config: AppConfig = match config_path {
        Some(path) => {
            let config_str = fs::read_to_string(path)?;
            let initial_config: AppConfig = serde_json::from_str(&config_str)?;
            process_loaded_config(initial_config)
        }
        None => {
            // Default configuration
            let mut providers_map = HashMap::new();
            providers_map.insert("gemini".to_string(), "https://generativelanguage.googleapis.com/v1beta/".to_string());
            providers_map.insert("openai".to_string(), "https://api.openai.com/v1".to_string());
            
            AppConfig {
                providers: providers_map,
                api_key: None,
                default_model: Some("gemini-pro".to_string()), 
                default_provider: Some("gemini".to_string()),   
                provider_type: Some("gemini".to_string()),    
            }
        }
    };
    Ok(config)
}


#[cfg(test)]
mod tests {
    use super::*; 

    #[tokio::test]
    async fn test_load_config_defaults() {
        let config = load_config(None).await.unwrap();
        assert_eq!(config.provider_type, Some("gemini".to_string()));
        assert_eq!(config.default_provider, Some("gemini".to_string()));
        assert_eq!(config.default_model, Some("gemini-pro".to_string()));
        assert!(config.providers.contains_key("gemini"));
        assert!(config.providers.contains_key("openai"));
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
    async fn test_load_config_from_json_string_infer_provider_type_gemini() {
        let json_input = r#"
        {
            "providers": {"gemini": "https://gemini.api/v1"},
            "default_provider": "gemini",
            "default_model": "gemini-1.0-pro"
        }
        "#;
        let config = load_config_from_string_for_test(json_input).await.unwrap();
        assert_eq!(config.provider_type, Some("gemini".to_string()));
        assert_eq!(config.default_provider, Some("gemini".to_string()));
        assert_eq!(config.default_model, Some("gemini-1.0-pro".to_string()));
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
        // Falls back to "gemini" as per current process_loaded_config logic
        assert_eq!(config.provider_type, Some("gemini".to_string())); 
        assert_eq!(config.default_model, Some("some-generic-model".to_string()));
    }

    #[tokio::test]
    async fn test_load_config_from_json_string_default_provider_unknown_no_provider_type() {
        let json_input = r#"
        {
            "providers": {"custom_local": "path_irrelevant"},
            "default_provider": "custom_local_profile_name",
            "default_model": "my_custom_model.bin"
            // provider_type is missing
        }
        "#;
        let config = load_config_from_string_for_test(json_input).await.unwrap();
        // According to current logic in process_loaded_config, if default_provider is not "openai" or "gemini",
        // and provider_type is missing, it will default provider_type to "gemini".
        assert_eq!(config.provider_type, Some("gemini".to_string()));
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
        assert_eq!(config.provider_type, Some("gemini".to_string())); // Default
        assert_eq!(config.default_provider, None);
        assert_eq!(config.default_model, None);
        assert_eq!(config.api_key, None);
        assert_eq!(config.providers, HashMap::new());
    }
}
