//! `HardGatePolicy` mechanism checks (PSP-10 Phase 1).
//!
//! The declared required stages are authoritative: a stage no plugin ran
//! records `SensorUnavailable` and blocks hard pass. Before this wiring the
//! policy had zero runtime consumers and `hard_pass` was derived purely from
//! the plugin verifier profile.

use std::sync::Arc;

use perspt_agent::toolloop::CandidateMeasurer;
use perspt_agent::{CandidateWorkspace, CodingCandidateMeasurer};
use perspt_core::plugin::VerifierStage;
use perspt_research::ResearchDomain;
use perspt_sdk::{AgentDomainPackage, DomainScope, ResidualClass};

fn required_stage_residuals(measured: &perspt_agent::toolloop::Measured) -> Vec<String> {
    measured
        .residuals
        .iter()
        .filter(|residual| {
            residual.class == ResidualClass::SensorUnavailable
                && residual.evidence.summary.contains("required-stage:")
        })
        .map(|residual| residual.evidence.summary.clone())
        .collect()
}

/// An empty workspace runs no plugin stages, so every one of the coding
/// domain's declared required stages is a missing sensor and hard pass is
/// blocked with explicit evidence — not silently.
#[tokio::test]
async fn missing_required_stages_block_hard_pass_with_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = CandidateWorkspace::create(dir.path(), "n1", 0, "rev-0").unwrap();
    let measured = CodingCandidateMeasurer::new(&workspace, "n1", 0)
        .measure()
        .await
        .unwrap();
    assert!(!measured.hard_pass);
    let evidence = required_stage_residuals(&measured);
    for stage in ["syntax", "build", "test"] {
        assert!(
            evidence
                .iter()
                .any(|summary| summary.contains(&format!("required-stage:{stage}"))),
            "missing evidence for required stage {stage}: {evidence:?}"
        );
    }
}

/// A domain's own declaration is what binds: the research domain requires
/// `citation-check`, which no plugin provides, so the measurement says so.
#[tokio::test]
async fn domain_declared_stage_is_authoritative() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = CandidateWorkspace::create(dir.path(), "n1", 0, "rev-0").unwrap();
    let measured = CodingCandidateMeasurer::new(&workspace, "n1", 0)
        .with_domain(Arc::new(ResearchDomain::new()))
        .measure()
        .await
        .unwrap();
    assert!(!measured.hard_pass);
    let evidence = required_stage_residuals(&measured);
    assert!(
        evidence
            .iter()
            .any(|summary| summary.contains("required-stage:citation-check")),
        "citation-check must be reported missing: {evidence:?}"
    );
}

/// Name reconciliation: every stage the coding domain requires resolves to a
/// `VerifierStage::policy_name`, so the policy names and the runner's stage
/// names cannot drift apart silently.
#[test]
fn required_stage_names_map_onto_verifier_stages() {
    let policy_names: Vec<&'static str> = [
        VerifierStage::SyntaxCheck,
        VerifierStage::Build,
        VerifierStage::Test,
        VerifierStage::Lint,
    ]
    .iter()
    .map(VerifierStage::policy_name)
    .collect();
    let coding = perspt_coding::CodingDomain::new();
    let policy = coding.hard_gate_policy(&DomainScope::default());
    assert!(!policy.required_stages.is_empty());
    for stage in &policy.required_stages {
        assert!(
            policy_names.contains(&stage.as_str()),
            "declared stage {stage} has no VerifierStage::policy_name mapping"
        );
    }
}

/// A workspace whose plugin stages all run adds no required-stage residual:
/// the wiring only tightens the gate, it never fabricates a missing sensor.
#[tokio::test]
async fn covered_required_stages_add_no_residual() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname='hard-gate-fixture'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("src/lib.rs"),
        "pub fn answer() -> u32 { 2 }\n",
    )
    .unwrap();
    let workspace = CandidateWorkspace::create(dir.path(), "n1", 0, "rev-0").unwrap();
    let measured = CodingCandidateMeasurer::new(&workspace, "n1", 0)
        .measure()
        .await
        .unwrap();
    let evidence = required_stage_residuals(&measured);
    assert!(
        evidence.is_empty(),
        "covered stages must add no required-stage residual: {evidence:?}"
    );
}
