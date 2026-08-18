//! Correction-packet mechanism checks (PSP-10 system 26, Phase 6).
//!
//! The unified path — residuals → typed packet → `branch_correct`-rendered
//! message — preserves paths, symbols, and rationale; every direction
//! participates; and a domain without packets keeps the legacy behavior
//! unchanged.

use perspt_coding::CodingDomain;
use perspt_sdk::{
    AgentDomainPackage, EvidencePayload, IndependenceRoute, ResidualClass, ResidualEvent,
    ResidualSeverity, SensorRef, StructuredDiagnosticRef, SymbolRef,
};

fn residual_with_evidence(
    class: ResidualClass,
    sensor: &str,
    summary: &str,
    paths: &[&str],
    diagnostics: Vec<StructuredDiagnosticRef>,
) -> ResidualEvent {
    // The import cluster is larger, so its log-damped magnitude dominates.
    let magnitude = if class == ResidualClass::ImportGraph {
        1.7
    } else {
        1.0
    };
    let mut event = ResidualEvent::new(
        "n1",
        0,
        class,
        ResidualSeverity::Error,
        magnitude,
        SensorRef::new(sensor, IndependenceRoute::Compiler)
            .with_fingerprint("cargo-json-v1+perspt-cluster-v1:log-damped"),
    )
    .unwrap();
    event.evidence = EvidencePayload {
        summary: summary.into(),
        raw: Some("raw tool output".into()),
        structured: Some(serde_json::to_value(&diagnostics).unwrap()),
    };
    event.affected_paths = paths.iter().map(|p| p.to_string()).collect();
    event.affected_symbols = vec![SymbolRef {
        name: "parse_entry".into(),
        container: Some("src/parser.rs".into()),
    }];
    event
}

fn import_residual() -> ResidualEvent {
    residual_with_evidence(
        ResidualClass::ImportGraph,
        "rustc",
        "unresolved import `missing_crate` (2 diagnostics in cluster E0432)",
        &["src/lib.rs", "src/parser.rs"],
        vec![StructuredDiagnosticRef {
            code: Some("E0432".into()),
            message: "unresolved import `missing_crate`".into(),
            path: Some("src/lib.rs".into()),
            line: Some(1),
            column: Some(5),
        }],
    )
}

fn type_residual() -> ResidualEvent {
    residual_with_evidence(
        ResidualClass::Type,
        "rustc",
        "mismatched types (E0308)",
        &["src/parser.rs"],
        vec![StructuredDiagnosticRef {
            code: Some("E0308".into()),
            message: "mismatched types".into(),
            path: Some("src/parser.rs".into()),
            line: Some(9),
            column: Some(3),
        }],
    )
}

/// The packet folds ALL directions and preserves the affected sets — the
/// historic first-direction-only flattening is gone.
#[test]
fn the_packet_folds_every_direction_with_affected_sets() {
    let domain = CodingDomain::new();
    let packet = domain
        .correction_packet(&[import_residual(), type_residual()])
        .expect("coding residuals produce a packet");
    assert!(packet.operators.len() >= 2, "both directions participate");
    assert!(packet.affected.paths.contains(&"src/lib.rs".to_string()));
    assert!(packet.affected.paths.contains(&"src/parser.rs".to_string()));
    assert!(packet
        .affected
        .symbols
        .iter()
        .any(|symbol| symbol.name == "parse_entry"));
    assert!(packet
        .affected
        .spans
        .iter()
        .any(|span| span == "src/lib.rs:1:5"));
    assert_eq!(packet.diagnostics.len(), 2);
    assert_eq!(packet.dominant_cluster.class, ResidualClass::ImportGraph);
}

/// The rendered `branch_correct` message carries paths, symbols, spans,
/// and rationale to the model.
#[test]
fn the_rendered_message_preserves_paths_symbols_and_rationale() {
    let domain = CodingDomain::new();
    let packet = domain
        .correction_packet(&[import_residual(), type_residual()])
        .unwrap();
    let rendered = perspt_coding::prompts::render_correction(&packet).unwrap();
    assert!(rendered.contains("src/lib.rs"), "{rendered}");
    assert!(rendered.contains("src/parser.rs"));
    assert!(rendered.contains("parse_entry"));
    assert!(rendered.contains("E0432"));
    assert!(rendered.contains("unresolved import"));
    assert!(
        rendered.contains("structural"),
        "the import direction's rationale survives: {rendered}"
    );
    assert!(rendered.contains("Do not weaken or delete tests"));
}

/// SensorUnavailable residuals become named uncertainty, never operators.
#[test]
fn missing_sensors_become_declared_uncertainty() {
    let domain = CodingDomain::new();
    let mut unavailable = ResidualEvent::new(
        "n1",
        0,
        ResidualClass::SensorUnavailable,
        ResidualSeverity::Error,
        1.0,
        SensorRef::new("required-stage:test", IndependenceRoute::TestOracle),
    )
    .unwrap();
    unavailable.evidence.summary = "required sensor unavailable: test".into();
    let packet = domain
        .correction_packet(&[import_residual(), unavailable])
        .unwrap();
    assert_eq!(packet.uncertainty.len(), 1);
    assert_eq!(packet.uncertainty[0].sensor, "required-stage:test");
}

/// A domain that has not opted in (the default) produces no packet, so the
/// legacy correction path is untouched.
#[test]
fn domains_without_packets_keep_the_legacy_path() {
    let research = perspt_research::ResearchDomain::new();
    assert!(research.correction_packet(&[import_residual()]).is_none());
}

/// Residuals with no operators and no diagnostics yield no packet — an
/// empty packet must cause expansion or escalation, never a blind retry.
#[test]
fn direction_free_residuals_yield_no_packet() {
    let domain = CodingDomain::new();
    let mut opaque = ResidualEvent::new(
        "n1",
        0,
        ResidualClass::SensorUnavailable,
        ResidualSeverity::Error,
        1.0,
        SensorRef::new("verifier", IndependenceRoute::Compiler),
    )
    .unwrap();
    opaque.evidence.summary = "sensor down".into();
    assert!(domain.correction_packet(&[opaque]).is_none());
}
