//! Model portfolio: several concurrently live provider handles (PSP-9
//! systems 1–2).
//!
//! The orchestrator historically owned one `Arc<GenAIProvider>` bound to one
//! adapter and one credential, so the four model tiers were four *names* on
//! one vendor. The portfolio holds one handle per `[providers.<id>]` entry,
//! addressed by the entry key.
//!
//! The capability record here is the **core-native mirror** of
//! `perspt-sdk::ProviderCapabilities` — `perspt-core` must not depend on the
//! SDK, so the types exist twice by design and `perspt-agent::transport`
//! performs the one translation (PSP-9 *Practical Decisions*).

use std::collections::BTreeMap;

use anyhow::{bail, Context, Result};

use crate::config::{Config, ProviderEntry};
use crate::llm_provider::GenAIProvider;

/// Core-native mirror of the SDK capability record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderCaps {
    pub tool_calling: bool,
    pub strict_schema: bool,
    pub parallel_tool_calls: bool,
    pub streaming_tool_calls: bool,
    pub prompt_caching: bool,
    pub structured_output: bool,
    pub max_context_tokens: u32,
    /// Whether the record was live-probed or is a declared adapter default.
    /// Declared defaults are honest about being declarations (Gate U); the
    /// transport probe (PSP-9 phase 1) upgrades them.
    pub probed: bool,
}

/// One live provider route: the entry key, its adapter, a bound client, and
/// its capability record.
pub struct ProviderHandle {
    /// Configuration key: `"anthropic"`, `"local"`, …
    pub id: String,
    /// genai adapter kind name this entry binds.
    pub adapter: String,
    /// Bound client wrapped in the existing provider type, so streaming,
    /// retries, and token accounting are shared with single-provider mode.
    pub provider: GenAIProvider,
    /// Declared (or probed) capabilities.
    pub caps: ProviderCaps,
}

impl std::fmt::Debug for ProviderHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderHandle")
            .field("id", &self.id)
            .field("adapter", &self.adapter)
            .field("caps", &self.caps)
            .finish()
    }
}

/// Several concurrently live provider handles, keyed by entry id.
#[derive(Debug, Default)]
pub struct ModelPortfolio {
    handles: BTreeMap<String, ProviderHandle>,
}

impl ModelPortfolio {
    /// Build a portfolio from the `[providers]` table.
    ///
    /// A configuration without a `[providers]` table yields a one-entry
    /// portfolio from the flat provider fields, so single-provider setups
    /// keep working unchanged. Credentials resolve per entry:
    /// `api_key_env` > literal `api_key`; Vertex entries fall through to the
    /// existing ADC token resolver rather than snapshotting a short-lived
    /// token at startup.
    pub fn from_config(config: &Config) -> Result<Self> {
        let mut handles = BTreeMap::new();
        if config.providers.is_empty() {
            let (provider, resolved) = GenAIProvider::from_config(config, None)?;
            let adapter = resolved.provider.clone();
            handles.insert(
                adapter.clone(),
                ProviderHandle {
                    id: adapter.clone(),
                    caps: declared_caps(&adapter),
                    adapter,
                    provider,
                },
            );
            return Ok(Self { handles });
        }

        for (id, entry) in &config.providers {
            let handle = Self::build_handle(id, entry, config)
                .with_context(|| format!("building provider handle {id:?}"))?;
            handles.insert(id.clone(), handle);
        }
        Ok(Self { handles })
    }

    /// Build one handle from one `[providers.<id>]` entry.
    fn build_handle(id: &str, entry: &ProviderEntry, config: &Config) -> Result<ProviderHandle> {
        let adapter = entry.adapter_or(id).to_string();

        // Resolve the credential without storing it: the env var the genai
        // adapter reads is set process-wide, exactly as single-provider mode
        // already does.
        let api_key = match &entry.api_key_env {
            Some(var) => std::env::var(var).ok(),
            None => entry.api_key.clone(),
        };

        if let Some(base_url) = entry.base_url.as_deref() {
            if let Some(env_var) = crate::llm_provider::provider_base_url_env_var(&adapter) {
                if std::env::var(env_var).is_err() {
                    std::env::set_var(env_var, base_url);
                }
            }
        }
        if adapter.eq_ignore_ascii_case("vertex") {
            // Reuse the existing ADC resolver; entry fields override.
            let vertex_config = Config {
                vertex_project_id: entry
                    .project_id
                    .clone()
                    .or(config.vertex_project_id.clone()),
                vertex_location: entry.location.clone().or(config.vertex_location.clone()),
                ..Config::default()
            };
            crate::llm_provider::configure_vertex_environment(&vertex_config);
        }

        let provider = GenAIProvider::new_with_config(Some(&adapter), api_key.as_deref())?;
        Ok(ProviderHandle {
            id: id.to_string(),
            caps: declared_caps(&adapter),
            adapter,
            provider,
        })
    }

    /// Resolve a provider handle by entry id.
    pub fn resolve(&self, provider: &str) -> Result<&ProviderHandle> {
        match self.handles.get(provider) {
            Some(handle) => Ok(handle),
            None => bail!(
                "unknown provider {provider:?}; configured providers: [{}]",
                self.provider_ids().join(", ")
            ),
        }
    }

    /// All configured provider ids, sorted.
    pub fn provider_ids(&self) -> Vec<String> {
        self.handles.keys().cloned().collect()
    }

    /// Iterate over the handles, sorted by id.
    pub fn handles(&self) -> impl Iterator<Item = &ProviderHandle> {
        self.handles.values()
    }

    /// Number of live handles.
    pub fn len(&self) -> usize {
        self.handles.len()
    }

    /// Whether the portfolio holds no handles.
    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }
}

/// Declared capability defaults per adapter kind.
///
/// These are *declarations*, marked `probed: false`; the tool-loop transport
/// verifies them per route before relying on them (Gate U). Context windows
/// are conservative lower bounds, not marketing numbers.
pub fn declared_caps(adapter: &str) -> ProviderCaps {
    let full = ProviderCaps {
        tool_calling: true,
        strict_schema: false,
        parallel_tool_calls: true,
        streaming_tool_calls: true,
        prompt_caching: false,
        structured_output: true,
        max_context_tokens: 128_000,
        probed: false,
    };
    match adapter.to_ascii_lowercase().as_str() {
        "openai" => ProviderCaps {
            strict_schema: true,
            prompt_caching: true,
            ..full
        },
        "anthropic" => ProviderCaps {
            prompt_caching: true,
            max_context_tokens: 200_000,
            ..full
        },
        "gemini" | "vertex" => ProviderCaps {
            prompt_caching: true,
            max_context_tokens: 1_000_000,
            ..full
        },
        "groq" | "xai" | "deepseek" | "cohere" => full,
        "ollama" => ProviderCaps {
            // Local models vary; declare the floor and let probing raise it.
            parallel_tool_calls: false,
            streaming_tool_calls: false,
            max_context_tokens: 32_000,
            ..full
        },
        _ => ProviderCaps {
            // Unknown adapters declare text-only; the tool loop then takes
            // the recorded Bundle degradation instead of guessing (Gate U).
            tool_calling: false,
            parallel_tool_calls: false,
            streaming_tool_calls: false,
            structured_output: false,
            max_context_tokens: 32_000,
            ..full
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_config(toml: &str) -> Config {
        Config::from_toml_str(toml).unwrap()
    }

    #[test]
    fn builds_one_handle_per_provider_entry() {
        let config = table_config(
            r#"
            [providers.local]
            adapter  = "ollama"
            base_url = "http://localhost:11434"

            [providers.openai]
            api_key = "sk-test"

            [providers.groq]
            api_key = "gsk-test"
            "#,
        );
        let portfolio = ModelPortfolio::from_config(&config).unwrap();
        assert_eq!(portfolio.len(), 3);
        assert_eq!(portfolio.provider_ids(), ["groq", "local", "openai"]);
        assert_eq!(portfolio.resolve("local").unwrap().adapter, "ollama");
        assert!(portfolio.resolve("nope").is_err());
    }

    #[test]
    fn one_entry_portfolio_from_flat_config_still_works() {
        let config = table_config("provider = \"ollama\"\nmodel = \"qwen2.5-coder\"\n");
        let portfolio = ModelPortfolio::from_config(&config).unwrap();
        assert_eq!(portfolio.len(), 1);
        assert_eq!(portfolio.provider_ids(), ["ollama"]);
    }

    #[test]
    fn unknown_adapter_declares_text_only_not_a_guess() {
        let caps = declared_caps("mystery-gateway");
        assert!(!caps.tool_calling);
        assert!(!caps.probed);
    }

    #[test]
    fn declared_caps_are_marked_unprobed() {
        for adapter in ["openai", "anthropic", "gemini", "ollama", "groq"] {
            assert!(!declared_caps(adapter).probed, "{adapter}");
            assert!(declared_caps(adapter).tool_calling, "{adapter}");
        }
    }
}
