//! Fully qualified model identity (PSP-9 system 1).
//!
//! Routing, calibration strata, verdict attribution, and replay all need to
//! name a model without knowing what a client is. A bare model-name string
//! cannot do that once several providers are live, so the identifier carries
//! both halves and is rendered `provider::model`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::SdkError;

/// A fully qualified model: which configured provider serves it, and which
/// provider-native model name.
///
/// `provider` is the *configuration key* from the `[providers]` table (e.g.
/// `"anthropic"`, `"local"`), not a vendor family — `family` classification is
/// [`super::ModelFamily`]'s job. Route values stay fully qualified so
/// identity, calibration, and replay never depend on an ambient default
/// provider.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ModelId {
    /// Provider key from configuration: `"anthropic"`, `"openai"`, `"local"`.
    pub provider: String,
    /// Provider-native model name, passed through verbatim.
    pub model: String,
}

impl ModelId {
    /// Build an id from its two halves.
    pub fn new(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
        }
    }
}

impl fmt::Display for ModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}::{}", self.provider, self.model)
    }
}

impl FromStr for ModelId {
    type Err = SdkError;

    /// Parse `provider::model`. Both halves are required and non-empty; a
    /// bare model name is rejected because an ambient default provider is
    /// exactly what fully qualified routes exist to rule out.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (provider, model) = s.split_once("::").ok_or_else(|| {
            SdkError::Domain(format!(
                "model id {s:?} must be fully qualified as provider::model"
            ))
        })?;
        if provider.is_empty() || model.is_empty() {
            return Err(SdkError::Domain(format!(
                "model id {s:?} has an empty provider or model half"
            )));
        }
        Ok(Self::new(provider, model))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_and_parses_fully_qualified_ids() {
        let id = ModelId::new("anthropic", "claude-opus-5");
        assert_eq!(id.to_string(), "anthropic::claude-opus-5");
        assert_eq!("anthropic::claude-opus-5".parse::<ModelId>().unwrap(), id);
    }

    #[test]
    fn model_halves_may_contain_separators() {
        // Vertex-style names carry slashes; only the FIRST `::` splits.
        let id: ModelId = "vertex::publishers/google/models/gemini".parse().unwrap();
        assert_eq!(id.provider, "vertex");
        assert_eq!(id.model, "publishers/google/models/gemini");
    }

    #[test]
    fn rejects_bare_names_and_empty_halves() {
        assert!("claude-opus-5".parse::<ModelId>().is_err());
        assert!("::model".parse::<ModelId>().is_err());
        assert!("provider::".parse::<ModelId>().is_err());
    }
}
