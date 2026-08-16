//! Model training lineage (PSP-9 system 1).
//!
//! Two distinctions carry weight and must not be collapsed:
//!
//! * **Provider ≠ family.** `groq`, `vertex`, and `ollama` are *hosts*.
//!   Llama served by Groq and Llama served by Ollama are the same lineage and
//!   will miss the same defects; two endpoints are not two opinions.
//! * **Family is a prior, not a measurement.** Even a different family is not
//!   guaranteed decorrelated — training corpora overlap. Family therefore only
//!   seeds routing defaults; no numeric correlation is ever inferred from it.
//!   Credited independence comes from matched labeled ledger statistics
//!   (system 8), never from a vendor name.

use serde::{Deserialize, Serialize};

/// A model's training lineage, used only as a routing *prior*.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelFamily {
    OpenAiGpt,
    AnthropicClaude,
    GoogleGemini,
    XaiGrok,
    DeepSeek,
    CohereCommand,
    Llama,
    Qwen,
    Mistral,
    Other(String),
}

impl ModelFamily {
    /// Classify a provider-native model name into a lineage.
    ///
    /// This is a heuristic over public naming conventions and is applied to
    /// the *model name*, never the provider key, so a Llama on any host
    /// classifies the same. Unknown names map to [`ModelFamily::Other`]
    /// rather than being guessed.
    pub fn from_model_name(name: &str) -> Self {
        let n = name.to_ascii_lowercase();
        // Order matters where prefixes overlap (e.g. "gpt" inside other ids).
        if n.contains("claude") {
            ModelFamily::AnthropicClaude
        } else if n.contains("gemini") || n.contains("gemma") {
            ModelFamily::GoogleGemini
        } else if n.contains("grok") {
            ModelFamily::XaiGrok
        } else if n.contains("deepseek") {
            ModelFamily::DeepSeek
        } else if n.contains("command") && !n.contains("gpt") {
            ModelFamily::CohereCommand
        } else if n.contains("llama") {
            ModelFamily::Llama
        } else if n.contains("qwen") {
            ModelFamily::Qwen
        } else if n.contains("mistral") || n.contains("mixtral") || n.contains("codestral") {
            ModelFamily::Mistral
        } else if n.contains("gpt") || n.starts_with("o1") || n.starts_with("o3") {
            ModelFamily::OpenAiGpt
        } else {
            ModelFamily::Other(name.to_string())
        }
    }

    /// Whether two lineages are distinct — a *routing precondition* (e.g.
    /// `EnsemblePolicy::require_distinct_family`), never a risk statement.
    /// Two [`ModelFamily::Other`] values compare by name because an unknown
    /// lineage cannot be presumed distinct from itself.
    pub fn is_distinct_from(&self, other: &ModelFamily) -> bool {
        self != other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_by_model_name_not_host() {
        assert_eq!(
            ModelFamily::from_model_name("llama-3.3-70b-versatile"),
            ModelFamily::Llama
        );
        assert_eq!(
            ModelFamily::from_model_name("claude-opus-5"),
            ModelFamily::AnthropicClaude
        );
        assert_eq!(
            ModelFamily::from_model_name("gemini-3.5-flash"),
            ModelFamily::GoogleGemini
        );
        assert_eq!(
            ModelFamily::from_model_name("gpt-5.5"),
            ModelFamily::OpenAiGpt
        );
        assert_eq!(
            ModelFamily::from_model_name("qwen2.5-coder"),
            ModelFamily::Qwen
        );
    }

    #[test]
    fn unknown_names_stay_other_instead_of_being_guessed() {
        assert_eq!(
            ModelFamily::from_model_name("sonnetina-9000"),
            ModelFamily::Other("sonnetina-9000".to_string())
        );
    }

    #[test]
    fn same_lineage_on_two_hosts_is_not_distinct() {
        let groq = ModelFamily::from_model_name("llama-3.3-70b-versatile");
        let ollama = ModelFamily::from_model_name("llama3.3:70b");
        assert!(!groq.is_distinct_from(&ollama));
    }
}
