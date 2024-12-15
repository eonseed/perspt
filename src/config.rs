// src/config.rs
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::fs;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub providers: HashMap<String, String>,
    pub api_key: Option<String>,
    pub default_model: Option<String>,
    pub default_provider: Option<String>,
}

pub async fn load_config(config_path: Option<&String>) -> Result<AppConfig, Box<dyn Error>> {
    let config: AppConfig = match config_path {
        Some(path) => {
            let config_str = fs::read_to_string(path)?;
            let config: AppConfig = serde_json::from_str(&config_str)?;
            config
        }
        None => {
            AppConfig {
                providers: {
                    let mut map = HashMap::new();
                    map.insert("gemini".to_string(), "https://generativelanguage.googleapis.com/v1beta/".to_string());
                    map.insert("openai".to_string(), "https://api.openai.com/v1".to_string());
                    map
                },
                api_key: None,
                default_model: Some("gemini-pro".to_string()),
                default_provider: Some("gemini".to_string()),
            }
        }
    };
    Ok(config)
}

