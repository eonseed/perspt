//! # Perspt Research Domain Package (skeleton)
//!
//! This crate is the PSP-8 Phase 10 *second-domain readiness check*: it proves
//! the [`perspt_sdk`] contracts admit a second domain package **without forking**
//! the scheduler, admissibility kernel, residual model, replay ledger, or
//! dashboard event model. A research agent reuses the same SRBN control plane;
//! only the residual classes, energy model, and correction directions differ.
//!
//! Research residuals cover unsupported claims, stale evidence, source
//! mismatch, and citation gaps, mapped onto the same energy components the SDK
//! defines. The verifier suites (source discovery, citation provenance, claim
//! extraction) are out of scope for the skeleton; the point is contract
//! conformance.

#![forbid(unsafe_code)]

use perspt_sdk::{
    AgentDomainPackage, CorrectionDirection, DomainDetection, DomainId, DomainScope,
    EnergyComponent, EnergyModel, ResidualClass, ResidualEvent, ResidualSchema, ResidualWeight,
    StabilityClaim, WorkspaceSnapshot,
};

/// The research domain package.
#[derive(Debug, Clone, Default)]
pub struct ResearchDomain;

impl ResearchDomain {
    pub fn new() -> Self {
        Self
    }
}

impl AgentDomainPackage for ResearchDomain {
    fn domain_id(&self) -> DomainId {
        DomainId::new("research")
    }

    fn detect(&self, workspace: &WorkspaceSnapshot) -> DomainDetection {
        let mut evidence = Vec::new();
        for marker in [
            "references.bib",
            "refs.bib",
            "bibliography.bib",
            "sources.md",
        ] {
            if workspace.has_file_named(marker) {
                evidence.push(format!("found {marker}"));
            }
        }
        let activated = !evidence.is_empty();
        DomainDetection {
            domain: self.domain_id(),
            activated,
            confidence: if activated { 0.85 } else { 0.0 },
            evidence,
        }
    }

    fn residual_schema(&self, _scope: &DomainScope) -> ResidualSchema {
        // Research reuses SDK residual classes; an unsupported claim maps to a
        // logic residual, a citation gap to a structural one, stale evidence to
        // context drift, and a contradiction to a sheaf inconsistency.
        ResidualSchema::new(vec![
            ResidualClass::TestFailure,        // unsupported / contradicted claim
            ResidualClass::InterfaceMismatch,  // source mismatch
            ResidualClass::ImportGraph,        // citation gap (missing source link)
            ResidualClass::ContextDrift,       // stale evidence
            ResidualClass::SheafInconsistency, // cross-source contradiction
            ResidualClass::SensorUnavailable,
        ])
    }

    fn energy_model(&self, scope: &DomainScope) -> EnergyModel {
        use EnergyComponent::*;
        use ResidualClass::*;
        let mut model = EnergyModel::new("research", 0.5).with_correction_budget(4);
        model.residual_weights = vec![
            ResidualWeight::new(TestFailure, Log, 3.0), // unsupported claim is severe
            ResidualWeight::new(InterfaceMismatch, Str, 2.0),
            ResidualWeight::new(ImportGraph, Str, 2.0),
            ResidualWeight::new(ContextDrift, Sheaf, 1.5),
            ResidualWeight::new(SheafInconsistency, Sheaf, 2.5),
            ResidualWeight::new(SensorUnavailable, Boot, 1.0),
        ];
        model.energy_tolerance = 0.0;
        model.stability_claim = Some(StabilityClaim::not_claimed(format!(
            "research scope: {}",
            scope.label
        )));
        model
    }

    fn correction_directions(&self, residuals: &[ResidualEvent]) -> Vec<CorrectionDirection> {
        residuals
            .iter()
            .filter_map(|r| match r.class {
                ResidualClass::TestFailure => Some(CorrectionDirection::new(
                    ResidualClass::TestFailure,
                    "support or retract the claim: cite a primary source or weaken the statement",
                )),
                ResidualClass::ImportGraph => Some(CorrectionDirection::new(
                    ResidualClass::ImportGraph,
                    "add the missing citation linking the claim to its source",
                )),
                ResidualClass::ContextDrift => Some(CorrectionDirection::new(
                    ResidualClass::ContextDrift,
                    "refresh the stale evidence: re-fetch the source and re-validate the quote bounds",
                )),
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perspt_sdk::{
        score_candidate, DomainRegistry, IndependenceRoute, ResidualSeverity, SensorRef,
    };

    #[test]
    fn research_domain_detects_bibliography() {
        let domain = ResearchDomain::new();
        let ws = WorkspaceSnapshot::new("/paper", vec!["refs.bib".into(), "draft.md".into()]);
        assert!(domain.detect(&ws).activated);
    }

    #[test]
    fn research_energy_model_validates_and_is_not_claimed() {
        let model = ResearchDomain::new().energy_model(&DomainScope::default());
        assert!(model.validate().is_ok());
        assert!(!model.stability_claim.unwrap().claims_floor());
    }

    #[test]
    fn research_scores_an_unsupported_claim() {
        let domain = ResearchDomain::new();
        let model = domain.energy_model(&DomainScope::default());
        let claim = ResidualEvent::new(
            "claim-7",
            0,
            ResidualClass::TestFailure,
            ResidualSeverity::Error,
            1.0,
            SensorRef::new("claim-checker", IndependenceRoute::SeparateModel),
        )
        .unwrap();
        // TestFailure weight 3.0 -> 3.0 * 1^2 = 3.0 in V_log.
        let score = score_candidate(&model, &[claim]).unwrap();
        assert_eq!(score.total, 3.0);
    }

    #[test]
    fn registry_admits_research_alongside_other_domains() {
        // The SDK control plane admits a second domain without forking.
        let mut registry = DomainRegistry::new();
        registry.register(Box::new(ResearchDomain::new()));
        assert_eq!(registry.len(), 1);
        assert!(registry.by_id(&DomainId::new("research")).is_some());
    }
}
