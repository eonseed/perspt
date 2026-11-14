//! # Perspt Research Domain Package
//!
//! Executable domain package implementing citation verification (`citation-check`)
//! and typed correction packets on top of [`perspt_sdk`], sharing the SRBN
//! control plane without forking the scheduler, kernel, or ledger.

#![forbid(unsafe_code)]

mod verify;

use perspt_sdk::{
    AgentDomainPackage, CorrectionDirection, CorrectionPacket, DomainDetection, DomainEvaluation,
    DomainId, DomainScope, EnergyComponent, EnergyModel, HardGatePolicy, ResidualClass,
    ResidualEvent, ResidualSchema, ResidualWeight, SafetyBarrierDisposition, StabilityClaim,
    VerificationCadence, VerifierSuiteSpec, WorkspaceSnapshot,
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
            "references.md",
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
        ResidualSchema::new(vec![
            ResidualClass::Syntax,             // malformed BibTeX / bibliography syntax
            ResidualClass::TestFailure,        // unsupported / contradicted claim
            ResidualClass::InterfaceMismatch,  // source mismatch
            ResidualClass::ImportGraph,        // citation gap (missing source link)
            ResidualClass::SymbolMismatch,     // mismatched citation label
            ResidualClass::ContextDrift,       // stale evidence
            ResidualClass::SheafInconsistency, // cross-source contradiction
            ResidualClass::SensorUnavailable,
            ResidualClass::ToolFailure,
        ])
    }

    fn energy_model(&self, scope: &DomainScope) -> EnergyModel {
        use EnergyComponent::*;
        use ResidualClass::*;
        let mut model = EnergyModel::new("research", 0.5).with_correction_budget(4);
        model.residual_weights = vec![
            ResidualWeight::new(Syntax, Syn, 3.0).with_hard_threshold(0.0),
            ResidualWeight::new(TestFailure, Log, 3.0),
            ResidualWeight::new(InterfaceMismatch, Str, 2.0),
            ResidualWeight::new(ImportGraph, Str, 2.0),
            ResidualWeight::new(SymbolMismatch, Str, 2.0),
            ResidualWeight::new(ContextDrift, Sheaf, 1.5),
            ResidualWeight::new(SheafInconsistency, Sheaf, 2.5),
            ResidualWeight::new(SensorUnavailable, Boot, 1.0),
            ResidualWeight::new(ToolFailure, Boot, 1.0),
            // Tool-loop harness progress & adjudication classes.
            ResidualWeight::new(ToolArgumentInvalid, Str, 1.0),
            ResidualWeight::new(WitnessStale, Str, 1.0),
            ResidualWeight::new(ToolThrash, Str, 1.0),
            ResidualWeight::new(ToolBatchConflict, Str, 1.0),
            ResidualWeight::new(UnreadEdit, Str, 1.0),
            ResidualWeight::new(BudgetPressure, Str, 1.0),
            ResidualWeight::new(AdjudicatorObjection, Log, 2.0),
            ResidualWeight::new(ProviderUnavailable, Boot, 1.0),
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
            .filter_map(|r| {
                let mut dir = match r.class {
                    ResidualClass::Syntax => Some(CorrectionDirection::new(
                        ResidualClass::Syntax,
                        "fix the malformed BibTeX entry or unbalanced braces in the bibliography",
                    )),
                    ResidualClass::TestFailure => Some(CorrectionDirection::new(
                        ResidualClass::TestFailure,
                        "support or retract the claim: cite a primary source or weaken the statement",
                    )),
                    ResidualClass::ImportGraph | ResidualClass::SymbolMismatch => {
                        Some(CorrectionDirection::new(
                            r.class,
                            "define the missing citation key in the bibliography \
                             (or cite an existing entry)",
                        ))
                    }
                    ResidualClass::InterfaceMismatch => Some(CorrectionDirection::new(
                        ResidualClass::InterfaceMismatch,
                        "align source metadata and citation attributes with the bibliography entry",
                    )),
                    ResidualClass::ContextDrift => Some(CorrectionDirection::new(
                        ResidualClass::ContextDrift,
                        "refresh the stale evidence: re-fetch the source and re-validate quote bounds",
                    )),
                    _ => None,
                }?;
                dir.target_paths = r.affected_paths.clone();
                dir.target_symbols = r.affected_symbols.clone();
                dir.rationale = r.evidence.summary.clone();
                Some(dir)
            })
            .collect()
    }

    fn correction_packet(&self, residuals: &[ResidualEvent]) -> Option<CorrectionPacket> {
        verify::build_correction_packet(self, residuals)
    }

    fn tool_entries(&self, _scope: &DomainScope) -> Vec<perspt_sdk::ToolEntry> {
        Vec::new()
    }

    fn hard_gate_policy(&self, _scope: &DomainScope) -> HardGatePolicy {
        HardGatePolicy {
            required_stages: vec!["citation-check".into()],
        }
    }

    fn verifier_suite(&self, _scope: &DomainScope) -> VerifierSuiteSpec {
        VerifierSuiteSpec {
            stages: vec!["citation-check".into()],
            cadence: VerificationCadence::default(),
        }
    }

    fn safety_barrier(&self, _scope: &DomainScope) -> SafetyBarrierDisposition {
        SafetyBarrierDisposition::NotClaimed {
            reason: "the research domain declares no operational unsafe set yet; \
                     chance-safety certification is unavailable"
                .into(),
        }
    }

    fn evaluate_workspace(
        &self,
        snapshot: &WorkspaceSnapshot,
        node_id: &str,
        generation: u32,
    ) -> Option<DomainEvaluation> {
        verify::evaluate_research_workspace(snapshot, node_id, generation)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use perspt_sdk::{
        DomainRegistry, IndependenceRoute, ResidualSeverity, SensorRef, score_candidate,
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
        let score = score_candidate(&model, &[claim]).unwrap();
        assert_eq!(score.total, 3.0);
    }

    #[test]
    fn registry_admits_research_alongside_other_domains() {
        let mut registry = DomainRegistry::new();
        registry.register(std::sync::Arc::new(ResearchDomain::new()));
        assert_eq!(registry.len(), 1);
        assert!(registry.by_id(&DomainId::new("research")).is_some());
    }

    #[test]
    fn evaluate_workspace_passes_valid_bib_and_md_citations() {
        let domain = ResearchDomain::new();
        let mut contents = BTreeMap::new();
        contents.insert(
            "refs.bib".to_string(),
            "@article{smith2024,\n  title={Stability}\n}\n".to_string(),
        );
        contents.insert(
            "draft.md".to_string(),
            "Prior work [@smith2024] proves convergence.\n".to_string(),
        );
        let ws = WorkspaceSnapshot::new("/paper", vec!["draft.md".into(), "refs.bib".into()])
            .with_contents(contents);

        let eval = domain.evaluate_workspace(&ws, "n1", 0).unwrap();
        assert_eq!(eval.ran_stages, vec!["citation-check"]);
        assert!(eval.residuals.is_empty(), "{:?}", eval.residuals);
    }

    #[test]
    fn evaluate_workspace_flags_missing_citations_and_builds_packet() {
        let domain = ResearchDomain::new();
        let mut contents = BTreeMap::new();
        contents.insert(
            "refs.bib".to_string(),
            "@article{known2023,\n}\n".to_string(),
        );
        contents.insert(
            "draft.md".to_string(),
            "See [@known2023; @missing2026] and \\cite{also_missing}.\n".to_string(),
        );
        let ws = WorkspaceSnapshot::new("/paper", vec!["draft.md".into(), "refs.bib".into()])
            .with_contents(contents);

        let eval = domain.evaluate_workspace(&ws, "n1", 0).unwrap();
        assert_eq!(eval.residuals.len(), 1);
        assert_eq!(eval.residuals[0].class, ResidualClass::ImportGraph);
        let symbols: Vec<&str> = eval.residuals[0]
            .affected_symbols
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(symbols, vec!["missing2026", "also_missing"]);

        let packet = domain.correction_packet(&eval.residuals).unwrap();
        assert_eq!(packet.affected.paths, vec!["draft.md"]);
        assert!(!packet.is_empty());
    }

    #[test]
    fn evaluate_workspace_flags_malformed_bibtex() {
        let domain = ResearchDomain::new();
        let mut contents = BTreeMap::new();
        contents.insert(
            "refs.bib".to_string(),
            "@article{broken\n  title={Missing comma and brace}\n".to_string(),
        );
        let ws = WorkspaceSnapshot::new("/paper", vec!["refs.bib".into()]).with_contents(contents);

        let eval = domain.evaluate_workspace(&ws, "n1", 0).unwrap();
        assert_eq!(eval.residuals.len(), 1);
        assert_eq!(eval.residuals[0].class, ResidualClass::Syntax);
    }
}
