//! # Perspt Coding Domain Package
//!
//! `perspt-coding` is the first domain package built on [`perspt_sdk`] (PSP-8).
//! It exercises the SDK's Phase-0/1 contracts with a real consumer: it declares
//! the coding residual schema, supplies the coding [`EnergyModel`] (weights,
//! `rho_gate`, tolerance, correction budget), and maps dominant residuals into
//! coding correction directions.
//!
//! The coding domain operates on discrete verifier residuals (compiler, LSP,
//! AST, tests) and exposes no continuous embedding-space coordinate, so its
//! analytic constants `alpha, beta, delta, L, eta` remain `NotClaimed`; only the
//! measured discrete gate and the spectral `mu` apply.
//!
//! Language adapters (Rust, Python, TypeScript) will grow into full SDK
//! verifier-suite providers in later phases; this crate currently provides the
//! domain-level residual schema, energy model, and Rust correction mappers for
//! the unresolved-import / missing-module cases (PSP-8 Reference Implementation
//! step 10).

#![forbid(unsafe_code)]

pub mod lang;

use perspt_sdk::{
    AgentDomainPackage, CorrectionDirection, DomainDetection, DomainId, DomainScope, EnergyComponent,
    EnergyModel, ResidualClass, ResidualEvent, ResidualSchema, ResidualWeight, StabilityClaim,
    WorkspaceSnapshot,
};

/// The language an adapter targets, used to specialize correction directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodingLanguage {
    Rust,
    Python,
    TypeScript,
}

/// The coding domain package.
#[derive(Debug, Clone, Default)]
pub struct CodingDomain;

impl CodingDomain {
    pub fn new() -> Self {
        Self
    }

    /// The residual classes the coding domain can emit.
    fn classes() -> Vec<ResidualClass> {
        vec![
            ResidualClass::Syntax,
            ResidualClass::Type,
            ResidualClass::Build,
            ResidualClass::TestFailure,
            ResidualClass::Lint,
            ResidualClass::Format,
            ResidualClass::Runtime,
            ResidualClass::Dependency,
            ResidualClass::Manifest,
            ResidualClass::ImportGraph,
            ResidualClass::SymbolMismatch,
            ResidualClass::InterfaceMismatch,
            ResidualClass::OwnershipViolation,
            ResidualClass::Regression,
            ResidualClass::SensorUnavailable,
            ResidualClass::ToolFailure,
            ResidualClass::SheafInconsistency,
        ]
    }
}

impl AgentDomainPackage for CodingDomain {
    fn domain_id(&self) -> DomainId {
        DomainId::new("coding")
    }

    fn detect(&self, workspace: &WorkspaceSnapshot) -> DomainDetection {
        let mut evidence = Vec::new();
        for marker in ["Cargo.toml", "pyproject.toml", "package.json", "go.mod"] {
            if workspace.has_file_named(marker) {
                evidence.push(format!("found {marker}"));
            }
        }
        let activated = !evidence.is_empty();
        DomainDetection {
            domain: self.domain_id(),
            activated,
            confidence: if activated { 0.95 } else { 0.0 },
            evidence,
        }
    }

    fn residual_schema(&self, _scope: &DomainScope) -> ResidualSchema {
        ResidualSchema::new(Self::classes())
    }

    fn energy_model(&self, scope: &DomainScope) -> EnergyModel {
        use EnergyComponent::*;
        use ResidualClass::*;
        // Compiler/type errors weigh heavily (they block everything downstream);
        // structural and behavioral residuals weigh moderately; degraded
        // sensors are V_boot. Every class carries an explicit weight — no class
        // defaults to an implicit weight of 1.
        let weights = vec![
            ResidualWeight::new(Syntax, Syn, 4.0).with_hard_threshold(0.0),
            ResidualWeight::new(Type, Syn, 3.0).with_hard_threshold(0.0),
            ResidualWeight::new(Build, Syn, 3.0).with_hard_threshold(0.0),
            ResidualWeight::new(ImportGraph, Str, 2.0),
            ResidualWeight::new(SymbolMismatch, Str, 2.0),
            ResidualWeight::new(InterfaceMismatch, Str, 2.5),
            ResidualWeight::new(OwnershipViolation, Str, 2.0),
            ResidualWeight::new(Dependency, Str, 1.5),
            ResidualWeight::new(Manifest, Str, 1.5),
            ResidualWeight::new(Lint, Str, 0.5),
            ResidualWeight::new(Format, Str, 0.25),
            ResidualWeight::new(TestFailure, Log, 2.0),
            ResidualWeight::new(Runtime, Log, 2.0),
            ResidualWeight::new(Regression, Log, 3.0),
            ResidualWeight::new(SensorUnavailable, Boot, 1.0),
            ResidualWeight::new(ToolFailure, Boot, 1.0),
            ResidualWeight::new(SheafInconsistency, Sheaf, 2.0),
        ];

        let mut model = EnergyModel::new("coding", 0.5).with_correction_budget(4);
        model.residual_weights = weights;
        model.energy_tolerance = 0.0;
        // The coding domain is measured-only: analytic constants are NotClaimed.
        model.stability_claim = Some(StabilityClaim::not_claimed(format!(
            "coding scope: {}",
            scope.label
        )));
        model
    }

    fn correction_directions(&self, residuals: &[ResidualEvent]) -> Vec<CorrectionDirection> {
        let mut directions = Vec::new();
        for r in residuals {
            if let Some(d) = correction_for(r) {
                directions.push(d);
            }
        }
        directions
    }
}

/// Map a single residual to a coding correction direction, or `None` when there
/// is no honest direction (the runtime then escalates rather than retrying
/// blindly).
fn correction_for(residual: &ResidualEvent) -> Option<CorrectionDirection> {
    match residual.class {
        ResidualClass::ImportGraph => {
            // Rust unresolved-import / missing-module direction (PSP-8 ref step 10).
            let symbol = residual
                .affected_symbols
                .first()
                .map(|s| s.name.clone())
                .unwrap_or_else(|| "the missing item".to_string());
            Some(
                CorrectionDirection::new(
                    ResidualClass::ImportGraph,
                    format!(
                        "resolve the unresolved import for `{symbol}`: add the missing `use` path \
                         or declare the missing `mod`, do not regenerate unrelated code"
                    ),
                )
                .with_paths(residual.affected_paths.clone())
                .with_rationale(
                    "unresolved imports are structural; the fix is an import/module \
                     declaration, not a behavioral rewrite",
                ),
            )
        }
        ResidualClass::Type => Some(
            CorrectionDirection::new(
                ResidualClass::Type,
                "reconcile the type mismatch at the reported span; adjust the expression or the \
                 declared signature, keeping the public interface stable",
            )
            .with_paths(residual.affected_paths.clone()),
        ),
        ResidualClass::Dependency | ResidualClass::Manifest => Some(
            CorrectionDirection::new(
                residual.class,
                "repair the dependency/manifest: add or pin the missing crate/package and sync \
                 the lockfile through an approved dependency-mutation effect",
            )
            .with_paths(residual.affected_paths.clone()),
        ),
        ResidualClass::TestFailure => Some(
            CorrectionDirection::new(
                ResidualClass::TestFailure,
                "address the failing test by fixing the implementation it attributes to; do not \
                 weaken or delete the assertion",
            )
            .with_paths(residual.affected_paths.clone()),
        ),
        // No honest direction for degraded sensors or pure tool failures: these
        // are bootstrap problems the runtime escalates, never a code retry.
        ResidualClass::SensorUnavailable | ResidualClass::ToolFailure => None,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perspt_sdk::{score_candidate, IndependenceRoute, ResidualSeverity, SensorRef, SymbolRef};

    fn lsp_import_residual() -> ResidualEvent {
        let mut r = ResidualEvent::new(
            "n1",
            0,
            ResidualClass::ImportGraph,
            ResidualSeverity::Error,
            1.0,
            SensorRef::new("rust-analyzer", IndependenceRoute::Lsp),
        )
        .unwrap();
        r.affected_symbols = vec![SymbolRef { name: "Bar".into(), container: Some("crate::foo".into()) }];
        r.affected_paths = vec!["src/main.rs".into()];
        r
    }

    #[test]
    fn detects_coding_domain_from_cargo_toml() {
        let domain = CodingDomain::new();
        let ws = WorkspaceSnapshot::new("/repo", vec!["Cargo.toml".into(), "src/main.rs".into()]);
        let detection = domain.detect(&ws);
        assert!(detection.activated);
        assert_eq!(detection.domain, DomainId::new("coding"));
    }

    #[test]
    fn energy_model_validates_and_is_measured_only() {
        let domain = CodingDomain::new();
        let model = domain.energy_model(&DomainScope::default());
        assert!(model.validate().is_ok());
        let claim = model.stability_claim.unwrap();
        assert!(!claim.claims_floor(), "coding domain must remain NotClaimed");
    }

    #[test]
    fn rust_unresolved_import_yields_import_direction_not_retry() {
        let domain = CodingDomain::new();
        let directions = domain.correction_directions(&[lsp_import_residual()]);
        assert_eq!(directions.len(), 1);
        assert_eq!(directions[0].addresses, ResidualClass::ImportGraph);
        assert!(directions[0].instruction.contains("Bar"));
        assert!(directions[0].instruction.contains("use"));
    }

    #[test]
    fn degraded_sensor_has_no_correction_direction() {
        let domain = CodingDomain::new();
        let r = ResidualEvent::new(
            "n1",
            0,
            ResidualClass::SensorUnavailable,
            ResidualSeverity::Blocking,
            1.0,
            SensorRef::new("cargo", IndependenceRoute::DeterministicTool),
        )
        .unwrap();
        assert!(domain.correction_directions(&[r]).is_empty());
    }

    #[test]
    fn coding_energy_model_scores_real_residuals() {
        let domain = CodingDomain::new();
        let model = domain.energy_model(&DomainScope::default());
        // ImportGraph weight 2.0, score 1.0 -> 2.0 * 1^2 = 2.0 into V_str.
        let score = score_candidate(&model, &[lsp_import_residual()]).unwrap();
        assert_eq!(score.total, 2.0);
        assert_eq!(score.components.v_str, 2.0);
    }
}
