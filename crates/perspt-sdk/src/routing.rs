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

// ---------------------------------------------------------------------------
// PSP-9 system 3: portfolio routing over fully qualified ModelIds
// ---------------------------------------------------------------------------

use crate::model::{ModelFamily, ModelId, ProviderCapabilities, ProviderCapabilityMask};

/// What a route resolution optimizes (PSP-9 system 3).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RouteObjective {
    /// Hard capability requirements for the route.
    pub require: ProviderCapabilityMask,
    /// Prefer a family decorrelated from this route's family (system 8's
    /// review preference). Family is a routing prior, never a measurement.
    pub decorrelate_from: Option<ModelId>,
}

/// The per-tier fully qualified routes from the `[models]` table.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PortfolioModels {
    pub architect: Option<ModelId>,
    pub actuator: Option<ModelId>,
    pub verifier: Option<ModelId>,
    pub speculator: Option<ModelId>,
    pub adjudicator: Option<ModelId>,
}

impl PortfolioModels {
    fn for_tier(&self, tier: ModelTier) -> Option<&ModelId> {
        match tier {
            ModelTier::Speculator => self.speculator.as_ref(),
            ModelTier::Verifier => self.verifier.as_ref(),
            ModelTier::Actuator => self.actuator.as_ref(),
            ModelTier::Architect => self.architect.as_ref(),
        }
    }

    fn all(&self) -> Vec<&ModelId> {
        [
            self.speculator.as_ref(),
            self.verifier.as_ref(),
            self.actuator.as_ref(),
            self.architect.as_ref(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

/// A resolved portfolio route: fully qualified, with a failover chain.
///
/// Routing is *sticky within a node* by default — cross-vendor moves happen
/// at node and role boundaries, not per turn, because switching vendors
/// mid-node discards the prompt cache. Failover to `fallback` is recovery
/// level 1, not a retry.
#[derive(Debug, Clone, PartialEq)]
pub struct PortfolioRoute {
    pub phase: AgentPhase,
    pub resolved_tier: ModelTier,
    pub model: ModelId,
    /// Failover chain, preferentially on *different providers*.
    pub fallback: Vec<ModelId>,
    pub budget: ModelBudget,
    pub reason: String,
}

/// Resolve a portfolio route for a phase (PSP-9 system 3).
///
/// `caps_of` reports each candidate route's capability record; a route that
/// fails `objective.require` is ineligible. `family_of` classifies by model
/// name. Routing policy: Explore/Research take the cheapest capable route;
/// Verify/Review prefer a family different from `decorrelate_from` whenever
/// the portfolio permits; Implement/Repair take the strongest actuator.
pub fn resolve_portfolio_route(
    phase: AgentPhase,
    models: &PortfolioModels,
    objective: &RouteObjective,
    budget: ModelBudget,
    caps_of: &dyn Fn(&ModelId) -> ProviderCapabilities,
    family_of: &dyn Fn(&ModelId) -> ModelFamily,
) -> Option<PortfolioRoute> {
    let tier = phase.default_tier();
    let capable = |m: &&ModelId| caps_of(m).satisfies(&objective.require);

    let primary: &ModelId = match phase {
        AgentPhase::Verify | AgentPhase::Review => {
            // Prefer a decorrelated family among capable routes.
            let preferred = objective.decorrelate_from.as_ref().map(family_of);
            let candidates: Vec<&ModelId> = models.all().into_iter().filter(capable).collect();
            let decorrelated = preferred.as_ref().and_then(|from| {
                candidates
                    .iter()
                    .find(|m| family_of(m).is_distinct_from(from))
                    .copied()
            });
            decorrelated
                .or_else(|| models.for_tier(tier).filter(|m| capable(&m)))
                .or_else(|| candidates.first().copied())?
        }
        _ => models
            .for_tier(tier)
            .filter(|m| capable(&m))
            .or_else(|| models.all().into_iter().find(capable))?,
    };

    // Failover chain: every other capable route, different providers first.
    let mut fallback: Vec<ModelId> = models
        .all()
        .into_iter()
        .filter(capable)
        .filter(|m| *m != primary)
        .cloned()
        .collect();
    fallback.sort_by_key(|m| (m.provider == primary.provider, m.to_string()));
    fallback.dedup();

    Some(PortfolioRoute {
        phase,
        resolved_tier: tier,
        model: primary.clone(),
        fallback,
        budget,
        reason: format!("{phase:?} default tier {tier:?} under objective"),
    })
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

    fn qualified(models: &[(&str, &str)]) -> PortfolioModels {
        let id = |i: usize| ModelId::new(models[i].0, models[i].1);
        PortfolioModels {
            architect: Some(id(0)),
            actuator: Some(id(1)),
            verifier: Some(id(2)),
            speculator: Some(id(3)),
            adjudicator: None,
        }
    }

    fn full_caps(_m: &ModelId) -> crate::model::ProviderCapabilities {
        crate::model::ProviderCapabilities {
            tool_calling: true,
            strict_schema: false,
            parallel_tool_calls: true,
            streaming_tool_calls: true,
            prompt_caching: false,
            structured_output: true,
            max_context_tokens: 200_000,
        }
    }

    fn by_name(m: &ModelId) -> crate::model::ModelFamily {
        crate::model::ModelFamily::from_model_name(&m.model)
    }

    #[test]
    fn review_prefers_a_decorrelated_family_when_the_portfolio_permits() {
        let models = qualified(&[
            ("anthropic", "claude-opus-5"),
            ("openai", "gpt-5.5"),
            ("anthropic", "claude-sonnet-5"),
            ("local", "qwen2.5-coder"),
        ]);
        let objective = RouteObjective {
            decorrelate_from: Some(ModelId::new("openai", "gpt-5.5")),
            ..RouteObjective::default()
        };
        let route = resolve_portfolio_route(
            AgentPhase::Review,
            &models,
            &objective,
            ModelBudget::default(),
            &full_caps,
            &by_name,
        )
        .unwrap();
        assert!(
            by_name(&route.model).is_distinct_from(&crate::model::ModelFamily::OpenAiGpt),
            "review route {} shares the actuator family",
            route.model
        );
    }

    #[test]
    fn failover_chain_prefers_a_different_provider_first() {
        let models = qualified(&[
            ("anthropic", "claude-opus-5"),
            ("openai", "gpt-5.5"),
            ("anthropic", "claude-sonnet-5"),
            ("local", "qwen2.5-coder"),
        ]);
        let route = resolve_portfolio_route(
            AgentPhase::Implement,
            &models,
            &RouteObjective::default(),
            ModelBudget::default(),
            &full_caps,
            &by_name,
        )
        .unwrap();
        assert_eq!(route.model, ModelId::new("openai", "gpt-5.5"));
        let first = route.fallback.first().expect("has fallback");
        assert_ne!(
            first.provider, route.model.provider,
            "failover crosses providers first"
        );
    }

    #[test]
    fn a_capability_requirement_excludes_incapable_routes() {
        let models = qualified(&[
            ("anthropic", "claude-opus-5"),
            ("openai", "gpt-5.5"),
            ("anthropic", "claude-sonnet-5"),
            ("local", "tiny-text-model"),
        ]);
        let text_only = |m: &ModelId| {
            if m.provider == "local" {
                crate::model::ProviderCapabilities::text_only(8_192)
            } else {
                full_caps(m)
            }
        };
        let objective = RouteObjective {
            require: crate::model::ProviderCapabilityMask::tool_loop(),
            ..RouteObjective::default()
        };
        // Explore defaults to the speculator, but the local route lacks tool
        // calling: routing selects another capable route instead.
        let route = resolve_portfolio_route(
            AgentPhase::Explore,
            &models,
            &objective,
            ModelBudget::default(),
            &text_only,
            &by_name,
        )
        .unwrap();
        assert_ne!(route.model.provider, "local");
    }
}
