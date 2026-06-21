//! Configuration types for Perspt.
//!
//! The on-disk configuration is TOML. Every field is optional so that a missing
//! or partial config file never errors; effective values are computed by merging
//! the file with environment-based detection and built-in defaults.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Placeholder shown instead of a real API key in `config --show`.
const MASKED_API_KEY: &str = "***";

/// Main configuration struct.
///
/// All fields are optional. Documented aliases are accepted on load so that
/// older field names keep working (`provider_type`, `default_provider`,
/// `default_model`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Provider id, e.g. `openai`, `anthropic`, `gemini`, `vertex`, `ollama`.
    #[serde(
        alias = "provider_type",
        alias = "default_provider",
        skip_serializing_if = "Option::is_none"
    )]
    pub provider: Option<String>,

    /// Default chat/simple-chat model.
    #[serde(alias = "default_model", skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// API key for the configured provider. Optional; may also come from env.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Optional base URL override for OpenAI-compatible / local endpoints.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,

    /// Google Cloud project id for Vertex AI. Optional; may also come from
    /// `VERTEX_PROJECT_ID` or Google Cloud project environment variables.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertex_project_id: Option<String>,

    /// Vertex AI location. Optional; may also come from `VERTEX_LOCATION`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertex_location: Option<String>,

    /// Preferred package manager for greenfield project init. Optional and
    /// fully plugin-driven: the active language plugin maps it to its own init
    /// command and default (e.g. Python → `uv`, JS → `npm`). Unknown values fall
    /// back to each plugin's default. Examples: `uv`, `poetry`, `pdm`, `pipenv`
    /// (Python); `pnpm`, `yarn` (JS).
    #[serde(skip_serializing_if = "Option::is_none", alias = "python_package_manager")]
    pub package_manager: Option<String>,

    /// Agent Architect-tier model override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architect_model: Option<String>,

    /// Agent Actuator-tier model override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actuator_model: Option<String>,

    /// Agent Verifier-tier model override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verifier_model: Option<String>,

    /// Agent Speculator-tier model override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speculator_model: Option<String>,
}

impl Config {
    /// Parse a `Config` from a TOML string. A partial document is valid.
    pub fn from_toml_str(content: &str) -> Result<Self> {
        toml::from_str(content).context("Failed to parse TOML configuration")
    }

    /// Load a `Config` from a file path. Returns `Config::default()` when the
    /// file does not exist, so callers can always work with effective values.
    pub fn load_from_path(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        Self::from_toml_str(&content)
    }

    /// Serialize this config to a TOML string.
    pub fn to_toml_string(&self) -> Result<String> {
        toml::to_string_pretty(self).context("Failed to serialize configuration to TOML")
    }

    /// Return a clone with the API key masked, for display purposes.
    pub fn masked(&self) -> Self {
        let mut clone = self.clone();
        if clone.api_key.is_some() {
            clone.api_key = Some(MASKED_API_KEY.to_string());
        }
        clone
    }

    /// Set a single key to a string value, used by `config --set`.
    ///
    /// Returns an error for unknown keys so typos surface immediately.
    pub fn set_value(&mut self, key: &str, value: &str) -> Result<()> {
        let value = value.to_string();
        match key {
            "provider" | "provider_type" | "default_provider" => self.provider = Some(value),
            "model" | "default_model" => self.model = Some(value),
            "api_key" => self.api_key = Some(value),
            "base_url" => self.base_url = Some(value),
            "vertex_project_id" => self.vertex_project_id = Some(value),
            "vertex_location" => self.vertex_location = Some(value),
            "architect_model" => self.architect_model = Some(value),
            "actuator_model" => self.actuator_model = Some(value),
            "verifier_model" => self.verifier_model = Some(value),
            "speculator_model" => self.speculator_model = Some(value),
            "package_manager" | "python_package_manager" => self.package_manager = Some(value),
            other => anyhow::bail!(
                "Unknown configuration key: {other}. Valid keys: provider, model, api_key, \
                 base_url, vertex_project_id, vertex_location, architect_model, actuator_model, \
                 verifier_model, speculator_model, package_manager"
            ),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string_parses_to_defaults() {
        let cfg = Config::from_toml_str("").unwrap();
        assert!(cfg.provider.is_none());
        assert!(cfg.model.is_none());
        assert!(cfg.api_key.is_none());
    }

    #[test]
    fn package_manager_set_value_and_alias() {
        let mut cfg = Config::default();
        cfg.set_value("package_manager", "poetry").unwrap();
        assert_eq!(cfg.package_manager.as_deref(), Some("poetry"));
        // The python-specific key is accepted as an alias for clarity.
        let mut cfg2 = Config::default();
        cfg2.set_value("python_package_manager", "pdm").unwrap();
        assert_eq!(cfg2.package_manager.as_deref(), Some("pdm"));
    }

    #[test]
    fn aliases_are_accepted() {
        let cfg = Config::from_toml_str(
            r#"
            provider_type = "openai"
            default_model = "phi-4-npu-ov"
            "#,
        )
        .unwrap();
        assert_eq!(cfg.provider.as_deref(), Some("openai"));
        assert_eq!(cfg.model.as_deref(), Some("phi-4-npu-ov"));
    }

    #[test]
    fn missing_file_returns_default() {
        let path = Path::new("/nonexistent/perspt/config.toml");
        let cfg = Config::load_from_path(path).unwrap();
        assert!(cfg.provider.is_none());
    }

    #[test]
    fn masked_hides_api_key() {
        let cfg = Config {
            api_key: Some("super-secret".to_string()),
            ..Default::default()
        };
        assert_eq!(cfg.masked().api_key.as_deref(), Some("***"));
    }

    #[test]
    fn masked_leaves_absent_key_absent() {
        let cfg = Config::default();
        assert!(cfg.masked().api_key.is_none());
    }

    #[test]
    fn set_value_updates_known_keys() {
        let mut cfg = Config::default();
        cfg.set_value("default_model", "phi-4-npu-ov").unwrap();
        assert_eq!(cfg.model.as_deref(), Some("phi-4-npu-ov"));
        cfg.set_value("provider", "openai").unwrap();
        assert_eq!(cfg.provider.as_deref(), Some("openai"));
        cfg.set_value("vertex_project_id", "test-project").unwrap();
        cfg.set_value("vertex_location", "test-location").unwrap();
        assert_eq!(cfg.vertex_project_id.as_deref(), Some("test-project"));
        assert_eq!(cfg.vertex_location.as_deref(), Some("test-location"));
    }

    #[test]
    fn set_value_rejects_unknown_key() {
        let mut cfg = Config::default();
        assert!(cfg.set_value("nope", "x").is_err());
    }

    #[test]
    fn round_trip_set_does_not_duplicate() {
        let mut cfg = Config::default();
        cfg.set_value("default_model", "a").unwrap();
        cfg.set_value("default_model", "b").unwrap();
        let serialized = cfg.to_toml_string().unwrap();
        // Exactly one model line after two sets.
        assert_eq!(serialized.matches("model").count(), 1);
        let reparsed = Config::from_toml_str(&serialized).unwrap();
        assert_eq!(reparsed.model.as_deref(), Some("b"));
    }
}
