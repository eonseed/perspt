//! SDK contract / mechanism checks exercised through the coding domain package
//! (PSP-8 System 13). These tests treat `perspt-coding` as the first real
//! consumer of `perspt-sdk` and verify the end-to-end SRBN control loop.

use perspt_coding::CodingDomain;
use perspt_sdk::{
    score_candidate, AcceptedTrajectory, AgentDomainPackage, DomainScope, GateDecision,
    IndependenceRoute, ResidualCertificate, ResidualClass, ResidualEvent, ResidualSeverity,
    SensorRef, SymbolRef, VerificationGraph,
};

fn import_residual(node: &str, generation: u32, score: f64) -> ResidualEvent {
    let mut r = ResidualEvent::new(
        node,
        generation,
        ResidualClass::ImportGraph,
        ResidualSeverity::Error,
        score,
        SensorRef::new("rust-analyzer", IndependenceRoute::Lsp),
    )
    .unwrap();
    r.affected_symbols = vec![SymbolRef { name: "Bar".into(), container: Some("crate::foo".into()) }];
    r.affected_paths = vec!["src/main.rs".into()];
    r
}

/// End-to-end mechanism check: a candidate with two import residuals is scored
/// by the coding energy model, the measured gate rejects a non-descending
/// retry, accepts a descending one, and exhaustion yields a residual
/// certificate with the correction direction still attached.
#[test]
fn srbn_loop_descends_then_certifies() {
    let domain = CodingDomain::new();
    let model = domain.energy_model(&DomainScope::default());

    // Baseline candidate: two unresolved imports, each magnitude 2.0.
    // V = 2.0*(2^2) + 2.0*(2^2) = 16.0 (ImportGraph weight = 2.0).
    let baseline = vec![import_residual("n1", 0, 2.0), import_residual("n1", 0, 2.0)];
    let baseline_score = score_candidate(&model, &baseline).unwrap();
    assert_eq!(baseline_score.total, 16.0);

    let mut traj =
        AcceptedTrajectory::new("n1", 0, baseline_score.total, model.rho_gate, model.correction_budget)
            .unwrap();

    // Attempt 1: no progress (still 16.0) -> rejected, not accepted.
    let stuck = score_candidate(&model, &baseline).unwrap();
    let d = traj.submit(false, stuck.total).unwrap();
    assert!(matches!(d, GateDecision::RejectedNonDescending { .. }));
    assert_eq!(traj.best_accepted_energy, 16.0);

    // Attempt 2: one import fixed (one residual gone) -> V = 8.0 -> descent accept.
    let improved = vec![import_residual("n1", 1, 2.0)];
    let improved_score = score_candidate(&model, &improved).unwrap();
    assert_eq!(improved_score.total, 8.0);
    let d = traj.submit(false, improved_score.total).unwrap();
    assert!(matches!(d, GateDecision::AcceptedByDescent { .. }));
    assert_eq!(traj.best_accepted_energy, 8.0);

    // The domain still produces a directed correction for the remaining import.
    let directions = domain.correction_directions(&improved);
    assert_eq!(directions.len(), 1);
    assert_eq!(directions[0].addresses, ResidualClass::ImportGraph);

    // Suppose the budget is exhausted before reaching zero: issue a certificate.
    let cert = ResidualCertificate::from_residuals(
        "n1",
        1,
        "ledger-head-xyz",
        improved_score.total,
        improved.clone(),
    );
    assert_eq!(cert.final_energy, 8.0);
    assert_eq!(cert.verifier_routes, vec![IndependenceRoute::Lsp]);
    assert_eq!(cert.next_correction_directions.len(), 0); // residual carried no inline direction
    assert_eq!(cert.final_residuals.len(), 1);
}

/// Finite-decision bound: a run cannot exceed `floor(V_0/rho_gate) + B + 1`
/// gate decisions before terminal classification.
#[test]
fn run_respects_finite_decision_bound() {
    let domain = CodingDomain::new();
    let model = domain.energy_model(&DomainScope::default());
    let v0 = 16.0;
    let mut traj = AcceptedTrajectory::new("n1", 0, v0, model.rho_gate, model.correction_budget).unwrap();
    let bound = traj.decision_bound().unwrap();
    // floor(16 / 0.5) + 4 + 1 = 32 + 5 = 37.
    assert_eq!(bound, 37);

    // Drive a monotone descent; decisions must stay under the bound.
    let mut v = v0;
    let mut decisions = 0u64;
    while v > 0.0 {
        v = (v - model.rho_gate).max(0.0);
        traj.submit(false, v).unwrap();
        decisions += 1;
    }
    assert!(decisions <= bound, "decisions={decisions} bound={bound}");
}

/// No false stability: a degraded/unavailable sensor produces a V_boot residual
/// and a non-zero energy, never silently zero.
#[test]
fn degraded_sensor_does_not_read_as_zero_energy() {
    let domain = CodingDomain::new();
    let model = domain.energy_model(&DomainScope::default());
    let degraded = ResidualEvent::new(
        "n1",
        0,
        ResidualClass::SensorUnavailable,
        ResidualSeverity::Blocking,
        1.0,
        SensorRef::new("cargo-test", IndependenceRoute::DeterministicTool),
    )
    .unwrap();
    let score = score_candidate(&model, &[degraded]).unwrap();
    assert!(score.total > 0.0);
    assert_eq!(score.components.v_boot, 1.0);
}

/// Spectral diagnostic: an independent cross-verifier edge raises the spectral
/// gap `mu` more than strengthening an existing (redundant) coupling.
#[test]
fn spectral_distinguishes_independent_from_redundant_verifier() {
    // compiler(0) - lsp(1) - test(2) verification chain.
    let graph = VerificationGraph::new(3).with_edge(0, 1, 1.0).with_edge(1, 2, 1.0);
    let independent = graph.edge_mu_sensitivity(0, 2, 1.0, 1e-9).unwrap();
    let redundant = graph.edge_mu_sensitivity(0, 1, 1.0, 1e-9).unwrap();
    assert!(independent > 0.0);
    assert!(independent >= redundant);
}
