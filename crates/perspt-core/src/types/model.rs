use super::*;

/// Model tier for different agent roles
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelTier {
    /// Deep reasoning model for planning and architecture
    Architect,
    /// Fast coding model for implementation
    Actuator,
    /// Sensor for LSP + Contract checking
    Verifier,
    /// Fast lookahead for speculation
    Speculator,
}

impl ModelTier {
    /// Get the recommended default model for this tier.
    ///
    /// Architect and Verifier tiers prefer higher-capability models for
    /// reasoning and evaluation. Actuator and Speculator default to the
    /// faster lower-cost Gemini tier. All defaults can be overridden per-tier
    /// via CLI.
    pub fn default_model(&self) -> &'static str {
        match self {
            ModelTier::Architect => "gemini-3.1-pro-preview",
            ModelTier::Verifier => "gemini-3.1-pro-preview",
            ModelTier::Actuator => "gemini-3.1-flash-lite-preview",
            ModelTier::Speculator => "gemini-3.1-flash-lite-preview",
        }
    }

    /// Get the default model name (static, for use when no instance is available).
    /// Returns the Actuator default as the general-purpose fallback.
    pub fn default_model_name() -> &'static str {
        "gemini-3.1-flash-lite-preview"
    }
}

#[cfg(test)]
mod tests {
    use super::ModelTier;

    #[test]
    fn task_type_accepts_common_model_aliases() {
        use super::TaskType;
        // The bare "test"/"tests" the model naturally emits must deserialize
        // (regression: rejecting them forced valid plans to fall back).
        let t: TaskType = serde_json::from_str("\"test\"").unwrap();
        assert_eq!(t, TaskType::UnitTest);
        assert_eq!(
            serde_json::from_str::<TaskType>("\"tests\"").unwrap(),
            TaskType::UnitTest
        );
        assert_eq!(
            serde_json::from_str::<TaskType>("\"implementation\"").unwrap(),
            TaskType::Code
        );
        assert_eq!(
            serde_json::from_str::<TaskType>("\"docs\"").unwrap(),
            TaskType::Documentation
        );
        // Canonical snake_case still works.
        assert_eq!(
            serde_json::from_str::<TaskType>("\"unit_test\"").unwrap(),
            TaskType::UnitTest
        );
    }

    #[test]
    fn gemini_defaults_use_requested_latest_models() {
        assert_eq!(
            ModelTier::Architect.default_model(),
            "gemini-3.1-pro-preview"
        );
        assert_eq!(
            ModelTier::Verifier.default_model(),
            "gemini-3.1-pro-preview"
        );
        assert_eq!(
            ModelTier::Actuator.default_model(),
            "gemini-3.1-flash-lite-preview"
        );
        assert_eq!(
            ModelTier::Speculator.default_model(),
            "gemini-3.1-flash-lite-preview"
        );
        assert_eq!(
            ModelTier::default_model_name(),
            "gemini-3.1-flash-lite-preview"
        );
    }
}
