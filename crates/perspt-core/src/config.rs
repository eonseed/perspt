//! Configuration types for Perspt.
//!
//! The on-disk configuration is TOML. Every field is optional so that a missing
//! or partial config file never errors; effective values are computed by merging
//! the file with environment-based detection and built-in defaults.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

use perspt_sdk::{EffectKind, FootprintSpec, ProposalBinding, RiskClass};

/// Placeholder shown instead of a real API key in `config --show`.
const MASKED_API_KEY: &str = "***";

/// One entry of the `[providers.<id>]` table (PSP-9 system 1).
///
/// A provider record describes *resolution policy*, not embedded secrets:
/// `api_key_env` names an environment variable; a literal `api_key` is
/// accepted for local development but masked on display. Vertex entries may
/// omit a static key entirely and use the per-request ADC token resolver.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderEntry {
    /// genai adapter kind. Defaults to the table key, so
    /// `[providers.anthropic]` needs no explicit adapter while
    /// `[providers.local]` can say `adapter = "ollama"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    /// Environment variable holding the API key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    /// Literal API key. Prefer `api_key_env`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Base URL override for OpenAI-compatible / local endpoints.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Google Cloud project id (Vertex only; ADC discovery when absent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// Vertex location; defaults to `global` via the existing resolver.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
}

impl ProviderEntry {
    /// The adapter this entry binds, defaulting to the table key.
    pub fn adapter_or<'a>(&'a self, key: &'a str) -> &'a str {
        self.adapter.as_deref().unwrap_or(key)
    }
}

/// The `[models]` table: fully qualified `provider::model` route values per
/// tier (PSP-9 system 1). Values stay fully qualified so identity,
/// calibration, and replay never depend on an ambient default provider.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architect: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actuator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speculator: Option<String>,
    /// Optional adjudication route (PSP-9 system 8).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adjudicator: Option<String>,
    /// Hard wall-clock deadline for one model turn, in seconds (default
    /// 120). A turn that exceeds it is an ordinary transport failure: it
    /// consumes sticky failover instead of hanging the session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_timeout_secs: Option<u64>,
}

/// Product surface allowed to create a lifecycle for an external server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalToolMode {
    Agent,
    Chat,
}

fn default_external_modes() -> Vec<ExternalToolMode> {
    vec![ExternalToolMode::Agent]
}

/// MCP transports supported by Perspt 0.6.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalToolTransport {
    Stdio,
    #[serde(alias = "http")]
    StreamableHttp,
}

fn default_external_timeout_ms() -> u64 {
    30_000
}

fn default_external_result_bytes() -> usize {
    1_048_576
}

fn default_external_stderr_bytes() -> usize {
    65_536
}

/// Locally trusted governance declaration for one discovered remote tool.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ExternalToolPolicy {
    /// Omission fails closed to `RunShell`.
    pub effect: Option<EffectKind>,
    pub risk: Option<RiskClass>,
    /// Omission fails closed to an opaque workspace footprint.
    pub footprint: Option<FootprintSpec>,
    pub proposal_bindings: Vec<ProposalBinding>,
}

/// One `[[external_tools]]` server. Secret values are never stored here:
/// environment maps contain destination/header names and source variable names.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ExternalToolConfig {
    pub id: String,
    pub transport: ExternalToolTransport,
    /// Direct argv. Element zero is the program; no shell parsing occurs.
    pub command: Vec<String>,
    pub url: Option<String>,
    /// Child variable -> source environment variable, for stdio servers.
    pub env_from_env: BTreeMap<String, String>,
    /// HTTP header -> source environment variable.
    pub headers_from_env: BTreeMap<String, String>,
    #[serde(default = "default_external_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_external_result_bytes")]
    pub max_result_bytes: usize,
    #[serde(default = "default_external_stderr_bytes")]
    pub max_stderr_bytes: usize,
    #[serde(default = "default_external_modes")]
    pub modes: Vec<ExternalToolMode>,
    pub tools: BTreeMap<String, ExternalToolPolicy>,
}

impl Default for ExternalToolConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            transport: ExternalToolTransport::Stdio,
            command: Vec::new(),
            url: None,
            env_from_env: BTreeMap::new(),
            headers_from_env: BTreeMap::new(),
            timeout_ms: default_external_timeout_ms(),
            max_result_bytes: default_external_result_bytes(),
            max_stderr_bytes: default_external_stderr_bytes(),
            modes: default_external_modes(),
            tools: BTreeMap::new(),
        }
    }
}

impl ExternalToolConfig {
    pub fn supports(&self, mode: ExternalToolMode) -> bool {
        self.modes.contains(&mode)
    }

    fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            !self.id.is_empty()
                && self.id.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
                }),
            "external tool server id {:?} must contain only ASCII letters, digits, '-' or '_'",
            self.id
        );
        anyhow::ensure!(
            self.timeout_ms > 0,
            "external tool {:?}: timeout_ms must be positive",
            self.id
        );
        anyhow::ensure!(
            self.max_result_bytes > 0 && self.max_stderr_bytes > 0,
            "external tool {:?}: result and stderr caps must be positive",
            self.id
        );
        anyhow::ensure!(
            !self.modes.is_empty(),
            "external tool {:?}: modes cannot be empty",
            self.id
        );
        let unique_modes: std::collections::BTreeSet<_> =
            self.modes.iter().map(|mode| format!("{mode:?}")).collect();
        anyhow::ensure!(
            unique_modes.len() == self.modes.len(),
            "external tool {:?}: duplicate mode",
            self.id
        );
        match self.transport {
            ExternalToolTransport::Stdio => anyhow::ensure!(
                !self.command.is_empty() && self.url.is_none(),
                "external tool {:?}: stdio requires command argv and forbids url",
                self.id
            ),
            ExternalToolTransport::StreamableHttp => anyhow::ensure!(
                self.command.is_empty() && self.url.is_some(),
                "external tool {:?}: Streamable HTTP requires url and forbids command",
                self.id
            ),
        }
        Ok(())
    }
}

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
    #[serde(
        skip_serializing_if = "Option::is_none",
        alias = "python_package_manager"
    )]
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

    /// The `[providers.<id>]` table: several credentials coexisting in one
    /// process (PSP-9 system 1). Empty for single-provider configurations,
    /// which continue to use the flat fields above.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub providers: BTreeMap<String, ProviderEntry>,

    /// The `[models]` table: per-tier fully qualified `provider::model`
    /// routes. When present it takes precedence over the flat
    /// `*_model` fields.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<ModelsConfig>,

    /// Shared MCP server configuration. Omitted modes default to agent-only.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub external_tools: Vec<ExternalToolConfig>,

    /// Detector for the removed PSP-9 `[ensemble]` block. Deserialization
    /// only: a present block fails validation with a pointed migration
    /// error naming `[exploration]` (PSP-10 cutover), never a silent
    /// ignore.
    #[serde(default, skip_serializing)]
    pub ensemble: Option<toml::Value>,

    /// Verification acceptance-stage options (`[verification]`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification: Option<VerificationConfig>,

    /// Bounded search configuration (`[exploration]`; PSP-10 system 20).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exploration: Option<ExplorationConfig>,

    /// Prompt bundles and activation bounds (`[prompts]`; PSP-10
    /// system 25).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<PromptsConfig>,

    /// Resident-context reserves (`[context]`; PSP-10 Definition 6).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<ContextConfig>,
}

/// The `[context]` block (PSP-10 Definition 6): reserves and working-set
/// bounds for the paged resident context.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ContextConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_set_turns: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synopsis_frame_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_reserve_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guard_reserve_tokens: Option<u64>,
}

impl ContextConfig {
    /// Startup validation: every configured reserve must be positive.
    pub fn validate(&self) -> Result<()> {
        for (name, value) in [
            ("working_set_turns", self.working_set_turns.map(u64::from)),
            ("synopsis_frame_tokens", self.synopsis_frame_tokens),
            ("output_reserve_tokens", self.output_reserve_tokens),
            ("guard_reserve_tokens", self.guard_reserve_tokens),
        ] {
            if value == Some(0) {
                anyhow::bail!("[context] {name} must be positive");
            }
        }
        Ok(())
    }
}

/// The `[verification]` acceptance-stage options.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VerificationConfig {
    /// Declare the plugin `format` verifier stage (e.g. `cargo fmt --check`)
    /// as an acceptance sensor. Off by default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_format: Option<bool>,
}

/// The `[exploration]` search block (PSP-10 system 20). Sequential eager
/// branches only; the hard branch cap is 3 for this release.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExplorationConfig {
    /// Branches opened before any expansion trigger (default 1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_branches: Option<u8>,
    /// Branch identities per forest, children included (hard cap 3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_branches: Option<u8>,
    /// Prefer a distinct model family on expansion (a prior, never a
    /// certificate).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distinct_family: Option<bool>,
    /// Cumulative eager-copy file reservation cap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_workspace_files: Option<u64>,
    /// Cumulative eager-copy byte reservation cap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_workspace_bytes: Option<u64>,
}

impl ExplorationConfig {
    /// Validate the branch bounds (PSP-10: at most three branches, at
    /// least one).
    pub fn validate(&self) -> Result<()> {
        let initial = self.initial_branches.unwrap_or(1);
        let max = self.max_branches.unwrap_or(3);
        anyhow::ensure!(
            (1..=3).contains(&max),
            "[exploration] max_branches must be between 1 and 3 (got {max})"
        );
        anyhow::ensure!(
            initial >= 1 && initial <= max,
            "[exploration] initial_branches must be between 1 and max_branches"
        );
        Ok(())
    }
}

/// The `[prompts]` block (PSP-10 system 25, Gate AE). The activation floor
/// may only be raised and the margin narrowed; invalid values fail at
/// startup.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PromptsConfig {
    /// External replacement bundle directories, pinned at session start.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bundles: Vec<String>,
    /// Minimum paired activation tasks (floor 30; raise-only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activation_min_tasks: Option<u32>,
    /// Noninferiority margin epsilon in [0, 0.05].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub noninferiority_margin: Option<f64>,
}

impl PromptsConfig {
    /// The activation bounds after startup validation.
    pub fn activation_bounds(&self) -> Result<perspt_sdk::prompt::ActivationBounds> {
        let bounds = perspt_sdk::prompt::ActivationBounds {
            min_tasks: self.activation_min_tasks.unwrap_or(30),
            noninferiority_margin: self.noninferiority_margin.unwrap_or(0.05),
        };
        bounds
            .validate()
            .map_err(|e| anyhow::anyhow!("[prompts]: {e}"))?;
        Ok(bounds)
    }
}

impl Config {
    /// Parse a `Config` from a TOML string. A partial document is valid.
    pub fn from_toml_str(content: &str) -> Result<Self> {
        let config: Self = toml::from_str(content).context("Failed to parse TOML configuration")?;
        config.validate()?;
        Ok(config)
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
        for entry in clone.providers.values_mut() {
            if entry.api_key.is_some() {
                entry.api_key = Some(MASKED_API_KEY.to_string());
            }
        }
        clone
    }

    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            self.ensemble.is_none(),
            "the [ensemble] section was removed by PSP-10: the ensemble is replaced \
             by the bounded search forest. Configure [exploration] instead \
             (initial_branches, max_branches, distinct_family) and delete [ensemble]."
        );
        if let Some(exploration) = &self.exploration {
            exploration.validate()?;
        }
        if let Some(prompts) = &self.prompts {
            prompts.activation_bounds()?;
        }
        if self.models.as_ref().and_then(|m| m.turn_timeout_secs) == Some(0) {
            anyhow::bail!("[models] turn_timeout_secs must be positive");
        }
        if let Some(context) = &self.context {
            context.validate()?;
        }
        let mut server_ids = std::collections::BTreeSet::new();
        for server in &self.external_tools {
            server.validate()?;
            anyhow::ensure!(
                server_ids.insert(server.id.as_str()),
                "duplicate external tool server id {:?}",
                server.id
            );
        }
        Ok(())
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
    fn providers_table_parses_with_defaulted_adapter() {
        let cfg = Config::from_toml_str(
            r#"
            [providers.anthropic]
            api_key_env = "ANTHROPIC_API_KEY"

            [providers.local]
            adapter  = "ollama"
            base_url = "http://localhost:11434"

            [models]
            architect = "anthropic::claude-opus-5"
            speculator = "local::qwen2.5-coder"
            "#,
        )
        .unwrap();
        assert_eq!(cfg.providers.len(), 2);
        let anthropic = &cfg.providers["anthropic"];
        assert_eq!(anthropic.adapter_or("anthropic"), "anthropic");
        assert_eq!(anthropic.api_key_env.as_deref(), Some("ANTHROPIC_API_KEY"));
        let local = &cfg.providers["local"];
        assert_eq!(local.adapter_or("local"), "ollama");
        let models = cfg.models.unwrap();
        assert_eq!(
            models.architect.as_deref(),
            Some("anthropic::claude-opus-5")
        );
        assert_eq!(models.adjudicator, None);
    }

    #[test]
    fn masked_hides_provider_table_keys_too() {
        let cfg = Config::from_toml_str(
            r#"
            [providers.openai]
            api_key = "sk-super-secret"
            "#,
        )
        .unwrap();
        assert_eq!(
            cfg.masked().providers["openai"].api_key.as_deref(),
            Some("***")
        );
    }

    #[test]
    fn flat_config_still_parses_without_tables() {
        let cfg = Config::from_toml_str("provider = \"openai\"\n").unwrap();
        assert!(cfg.providers.is_empty());
        assert!(cfg.models.is_none());
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

    #[test]
    fn external_tools_default_to_agent_only() {
        let config = Config::from_toml_str(
            r#"
            [[external_tools]]
            id = "local-search"
            transport = "stdio"
            command = ["search-server", "--stdio"]
            "#,
        )
        .unwrap();
        let server = &config.external_tools[0];
        assert!(server.supports(ExternalToolMode::Agent));
        assert!(!server.supports(ExternalToolMode::Chat));
    }

    #[test]
    fn external_tool_modes_and_local_policy_parse() {
        let config = Config::from_toml_str(
            r#"
            [[external_tools]]
            id = "records"
            transport = "streamable_http"
            url = "https://tools.example.test/mcp"
            modes = ["agent", "chat"]
            headers_from_env = { Authorization = "RECORDS_AUTH" }

            [external_tools.tools.lookup]
            effect = "data_read"
            risk = "low"
            "#,
        )
        .unwrap();
        let server = &config.external_tools[0];
        assert!(server.supports(ExternalToolMode::Agent));
        assert!(server.supports(ExternalToolMode::Chat));
        assert_eq!(server.tools["lookup"].effect, Some(EffectKind::DataRead));
    }

    #[test]
    fn duplicate_external_server_ids_are_rejected() {
        let invalid = r#"
            [[external_tools]]
            id = "same"
            transport = "stdio"
            command = ["one"]

            [[external_tools]]
            id = "same"
            transport = "stdio"
            command = ["two"]
        "#;
        assert!(Config::from_toml_str(invalid).is_err());
    }
}

#[cfg(test)]
mod psp10_config_tests {
    use super::*;

    #[test]
    fn a_present_ensemble_block_fails_with_a_pointed_migration_error() {
        let error = Config::from_toml_str("[ensemble]\nwidth = 2\n").unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("[exploration]"), "{message}");
        assert!(message.contains("PSP-10"), "{message}");
    }

    #[test]
    fn exploration_bounds_are_enforced_at_startup() {
        assert!(Config::from_toml_str("[exploration]\nmax_branches = 4\n").is_err());
        assert!(Config::from_toml_str("[exploration]\ninitial_branches = 0\n").is_err());
        let config =
            Config::from_toml_str("[exploration]\ninitial_branches = 1\nmax_branches = 3\n")
                .unwrap();
        config.exploration.unwrap().validate().unwrap();
    }

    #[test]
    fn prompt_activation_bounds_are_floor_and_range_checked() {
        assert!(Config::from_toml_str("[prompts]\nactivation_min_tasks = 29\n").is_err());
        assert!(Config::from_toml_str("[prompts]\nnoninferiority_margin = 0.06\n").is_err());
        let config = Config::from_toml_str(
            "[prompts]\nactivation_min_tasks = 40\nnoninferiority_margin = 0.01\n",
        )
        .unwrap();
        let bounds = config.prompts.unwrap().activation_bounds().unwrap();
        assert_eq!(bounds.min_tasks, 40);
    }
}
