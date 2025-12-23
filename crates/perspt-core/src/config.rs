//! Configuration types for Perspt

use serde::{Deserialize, Serialize};

/// Main configuration struct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            model: "gpt-4".to_string(),
            api_key: None,
        }
    }
}
