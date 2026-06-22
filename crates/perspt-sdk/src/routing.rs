//! Phase-aware model routing (PSP-8 System 3).
//!
//! Exploration is read-only orientation and should use a low-cost model without
//! weakening acceptance gates. Routing therefore resolves a [`ModelRoute`] per
//! [`AgentPhase`]: an explicit `explorer_model` wins; otherwise exploration
//! defaults to the cheapest tier (`Speculator`). `--model` sets all tiers unless
//! a phase-specific override is provided.

use serde::{Deserialize, Serialize};

/// Model capability tiers, cheapest to most capable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelTier {
    Speculator,
    Verifier,
    Actuator,
    Architect,
}

/// Agent phases that request a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentPhase {
    Explore,
    Plan,
    Implement,
    Verify,
    Repair,
    Review,
    Research,
}

impl AgentPhase {
    /// The default tier for a phase when no override is configured.
    pub fn default_tier(self) -> ModelTier {
        match self {
            AgentPhase::Explore | AgentPhase::Research => ModelTier::Speculator,
            AgentPhase::Verify | AgentPhase::Review => ModelTier::Verifier,
            AgentPhase::Implement | AgentPhase::Repair => ModelTier::Actuator,
            AgentPhase::Plan => ModelTier::Architect,
        }
    }
}

/// Per-route budget.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ModelBudget {
    pub max_tokens: u64,
    pub max_calls: u32,
    pub max_wall_clock_secs: u64,
}

impl Default for ModelBudget {
    fn default() -> Self {
        Self {
            max_tokens: 100_000,
            max_calls: 50,
            max_wall_clock_secs: 600,
        }
    }
}

/// The configured models per tier, plus optional explorer overrides.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelTierConfig {
    pub speculator_model: String,
    pub verifier_model: String,
    pub actuator_model: String,
    pub architect_model: String,
    /// Explicit exploration model override (`explorer_model` / `--explorer-model`).
    pub explorer_model: Option<String>,
    /// Reuse an existing tier for exploration (`--explorer-tier`).
    pub explorer_tier: Option<ModelTier>,
}

impl ModelTierConfig {
    /// Set every tier to a single model (`--model`).
    pub fn uniform(model: impl Into<String>) -> Self {
        let m = model.into();
        Self {
            speculator_model: m.clone(),
            verifier_model: m.clone(),
            actuator_model: m.clone(),
            architect_model: m,
            explorer_model: None,
            explorer_tier: None,
        }
    }

    pub fn model_for_tier(&self, tier: ModelTier) -> &str {
        match tier {
            ModelTier::Speculator => &self.speculator_model,
            ModelTier::Verifier => &self.verifier_model,
            ModelTier::Actuator => &self.actuator_model,
            ModelTier::Architect => &self.architect_model,
        }
    }
}

/// A resolved model route for one phase (PSP-8 `ModelRoute`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelRoute {
    pub phase: AgentPhase,
    pub requested_tier: Option<ModelTier>,
    pub resolved_tier: ModelTier,
    pub model: String,
    pub fallback_model: Option<String>,
    pub budget: ModelBudget,
    pub reason: String,
}

/// Resolve the model route for a phase.
///
/// Resolution order:
/// 1. An explicit `requested_tier` is honored.
/// 2. For `Explore`, an `explorer_model` override wins (tier = `explorer_tier`
///    or `Speculator`).
/// 3. Otherwise the phase's default tier applies.
pub fn resolve_route(
    phase: AgentPhase,
    config: &ModelTierConfig,
    requested_tier: Option<ModelTier>,
    budget: ModelBudget,
) -> ModelRoute {
    // Explicit per-call tier override always wins.
    if let Some(tier) = requested_tier {
        return ModelRoute {
            phase,
            requested_tier,
            resolved_tier: tier,
            model: config.model_for_tier(tier).to_string(),
            fallback_model: Some(config.speculator_model.clone()),
            budget,
            reason: "explicit tier override".into(),
        };
    }

    // Exploration model override.
    if phase == AgentPhase::Explore {
        if let Some(model) = &config.explorer_model {
            let tier = config.explorer_tier.unwrap_or(ModelTier::Speculator);
            return ModelRoute {
                phase,
                requested_tier: None,
                resolved_tier: tier,
                model: model.clone(),
                fallback_model: Some(config.speculator_model.clone()),
                budget,
                reason: "explorer_model override".into(),
            };
        }
        if let Some(tier) = config.explorer_tier {
            return ModelRoute {
                phase,
                requested_tier: None,
                resolved_tier: tier,
                model: config.model_for_tier(tier).to_string(),
                fallback_model: Some(config.speculator_model.clone()),
                budget,
                reason: "explorer_tier override".into(),
            };
        }
    }

    let tier = phase.default_tier();
    ModelRoute {
        phase,
        requested_tier: None,
        resolved_tier: tier,
        model: config.model_for_tier(tier).to_string(),
        fallback_model: Some(config.speculator_model.clone()),
        budget,
        reason: "phase default tier".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> ModelTierConfig {
        ModelTierConfig {
            speculator_model: "spec-1".into(),
            verifier_model: "verif-1".into(),
            actuator_model: "act-1".into(),
            architect_model: "arch-1".into(),
            explorer_model: None,
            explorer_tier: None,
        }
    }

    #[test]
    fn explore_defaults_to_speculator() {
        let route = resolve_route(AgentPhase::Explore, &config(), None, ModelBudget::default());
        assert_eq!(route.resolved_tier, ModelTier::Speculator);
        assert_eq!(route.model, "spec-1");
    }

    #[test]
    fn explorer_model_wins_over_speculator() {
        let mut config = config();
        config.explorer_model = Some("cheap-explorer".into());
        let route = resolve_route(AgentPhase::Explore, &config, None, ModelBudget::default());
        assert_eq!(route.model, "cheap-explorer");
        assert_eq!(route.reason, "explorer_model override");
    }

    #[test]
    fn explorer_tier_reuses_existing_tier_model() {
        let mut config = config();
        config.explorer_tier = Some(ModelTier::Verifier);
        let route = resolve_route(AgentPhase::Explore, &config, None, ModelBudget::default());
        assert_eq!(route.resolved_tier, ModelTier::Verifier);
        assert_eq!(route.model, "verif-1");
    }

    #[test]
    fn plan_routes_to_architect() {
        let route = resolve_route(AgentPhase::Plan, &config(), None, ModelBudget::default());
        assert_eq!(route.resolved_tier, ModelTier::Architect);
        assert_eq!(route.model, "arch-1");
    }

    #[test]
    fn explicit_tier_override_beats_phase_default() {
        let route = resolve_route(
            AgentPhase::Implement,
            &config(),
            Some(ModelTier::Speculator),
            ModelBudget::default(),
        );
        assert_eq!(route.resolved_tier, ModelTier::Speculator);
    }

    #[test]
    fn uniform_sets_all_tiers() {
        let config = ModelTierConfig::uniform("one-model");
        for phase in [
            AgentPhase::Explore,
            AgentPhase::Plan,
            AgentPhase::Implement,
            AgentPhase::Verify,
        ] {
            let route = resolve_route(phase, &config, None, ModelBudget::default());
            assert_eq!(route.model, "one-model");
        }
    }
}
