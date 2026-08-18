//! Prompt routes and per-section override resolution (Definition 4).
//!
//! A route is `(adapter, family, optional exact model)`. Resolution
//! operates per section: an *active* exact-model override wins, else an
//! *active* family override, else the base — which always exists, so
//! resolution is total for built-in stages. A configured endpoint name
//! never selects a prompt; endpoint ids name credentials and locations,
//! not model behavior.

use serde::{Deserialize, Serialize};

use crate::model::ModelFamily;

use super::manifest::ActivationState;
use super::section::{OverrideOrigin, SectionTemplate};

/// The prompt identity of one model call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptRoute {
    /// Transport adapter identity (`ModelTransport::adapter_kind`).
    pub adapter: String,
    pub family: ModelFamily,
    pub exact_model: Option<String>,
}

/// One override variant of a section, carrying its activation state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionOverride {
    /// Which routes this override targets.
    pub origin: OverrideOrigin,
    /// Gate AE state; an override resolves only when [`ActivationState::Active`].
    pub activation: ActivationState,
    pub template: SectionTemplate,
}

/// A section's base template plus its (possibly empty) override set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionVariants {
    pub base: SectionTemplate,
    pub overrides: Vec<SectionOverride>,
}

impl SectionVariants {
    pub fn base_only(base: SectionTemplate) -> Self {
        Self {
            base,
            overrides: Vec::new(),
        }
    }

    /// Definition 4's `R`: first matching case wins — active exact-model
    /// override, then active family override, then base. An inactive
    /// override never resolves; an unknown family gets the base, never an
    /// error.
    pub fn resolve(&self, route: &PromptRoute) -> (&SectionTemplate, OverrideOrigin) {
        if let Some(exact) = route.exact_model.as_deref() {
            if let Some(hit) = self.active_override(
                |origin| matches!(origin, OverrideOrigin::ExactModel(model) if model == exact),
            ) {
                return (&hit.template, hit.origin.clone());
            }
        }
        let family_label = self.family_label(&route.family);
        if let Some(hit) = self.active_override(
            |origin| matches!(origin, OverrideOrigin::Family(family) if *family == family_label),
        ) {
            return (&hit.template, hit.origin.clone());
        }
        (&self.base, OverrideOrigin::Base)
    }

    fn active_override(
        &self,
        matches: impl Fn(&OverrideOrigin) -> bool,
    ) -> Option<&SectionOverride> {
        self.overrides
            .iter()
            .find(|entry| entry.activation == ActivationState::Active && matches(&entry.origin))
    }

    fn family_label(&self, family: &ModelFamily) -> String {
        format!("{family:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::section::{
        PromptMessageRole, PromptSectionId, PromptSectionVersion, SectionSchema,
    };

    fn template(body: &str) -> SectionTemplate {
        SectionTemplate {
            schema: SectionSchema {
                id: PromptSectionId("stage/role".into()),
                version: PromptSectionVersion(1),
                role: PromptMessageRole::System,
                required: true,
                priority: 0,
                max_bytes: 1024,
                vars: vec![],
            },
            content_hash: SectionTemplate::hash_body(body),
            body: body.into(),
        }
    }

    fn route(family: ModelFamily, exact: Option<&str>) -> PromptRoute {
        PromptRoute {
            adapter: "genai".into(),
            family,
            exact_model: exact.map(str::to_string),
        }
    }

    #[test]
    fn resolution_order_is_exact_then_family_then_base() {
        let variants = SectionVariants {
            base: template("base"),
            overrides: vec![
                SectionOverride {
                    origin: OverrideOrigin::Family(format!("{:?}", ModelFamily::Qwen)),
                    activation: ActivationState::Active,
                    template: template("family"),
                },
                SectionOverride {
                    origin: OverrideOrigin::ExactModel("qwen-x".into()),
                    activation: ActivationState::Active,
                    template: template("exact"),
                },
            ],
        };
        let (exact, origin) = variants.resolve(&route(ModelFamily::Qwen, Some("qwen-x")));
        assert_eq!(exact.body, "exact");
        assert!(matches!(origin, OverrideOrigin::ExactModel(_)));
        let (family, _) = variants.resolve(&route(ModelFamily::Qwen, Some("qwen-y")));
        assert_eq!(family.body, "family");
        let (base, origin) = variants.resolve(&route(ModelFamily::Mistral, None));
        assert_eq!(base.body, "base");
        assert_eq!(origin, OverrideOrigin::Base);
    }

    #[test]
    fn inactive_overrides_never_resolve_and_other_families_get_base() {
        let variants = SectionVariants {
            base: template("base"),
            overrides: vec![SectionOverride {
                origin: OverrideOrigin::Family(format!("{:?}", ModelFamily::Qwen)),
                activation: ActivationState::Experimental,
                template: template("experimental"),
            }],
        };
        let (resolved, origin) = variants.resolve(&route(ModelFamily::Qwen, None));
        assert_eq!(resolved.body, "base");
        assert_eq!(origin, OverrideOrigin::Base);
        let other = route(ModelFamily::Other("mystery".into()), None);
        assert_eq!(variants.resolve(&other).0.body, "base");
    }
}
