//! Recorded-output parser checks (PSP-10 system 26, Phase 6).
//!
//! Each fixture is a real captured tool output; the assertions prove
//! codes, spans, suggestions, and test identities survive normalization,
//! and that clustering collapses cascades before scoring.

use perspt_coding::diag::{cargo, cluster, pyright, pytest, tsc, ty};
use perspt_sdk::ResidualClass;

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/diag")
        .join(name);
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn cargo_json_preserves_codes_and_spans() {
    let raw = fixture("cargo_check.jsonl");
    assert!(cargo::looks_like_stream(&raw));
    let diagnostics = cargo::parse(&raw);
    assert_eq!(diagnostics.len(), 3);
    assert_eq!(diagnostics[0].code.as_deref(), Some("E0432"));
    assert_eq!(diagnostics[0].class, ResidualClass::ImportGraph);
    assert_eq!(diagnostics[0].path.as_deref(), Some("src/lib.rs"));
    assert_eq!(diagnostics[0].line, Some(1));
    assert_eq!(diagnostics[0].column, Some(5));
    assert_eq!(diagnostics[1].code.as_deref(), Some("E0308"));
    assert_eq!(diagnostics[1].class, ResidualClass::Type);
    // Primary span wins over secondary spans.
    assert_eq!(diagnostics[1].column, Some(21));
}

#[test]
fn ty_concise_text_parses_rules_and_positions() {
    let diagnostics = ty::parse_check_text(&fixture("ty_check.txt"));
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].code.as_deref(), Some("unresolved-import"));
    assert_eq!(diagnostics[0].class, ResidualClass::ImportGraph);
    assert_eq!(diagnostics[0].path.as_deref(), Some("bad.py"));
    assert_eq!(diagnostics[0].line, Some(1));
    assert_eq!(diagnostics[0].column, Some(8));
    assert_eq!(diagnostics[1].class, ResidualClass::Type);
    assert!(diagnostics[1].message.contains("not assignable"));
}

#[test]
fn pyright_json_is_a_separately_fingerprinted_fallback() {
    let diagnostics = pyright::parse(&fixture("pyright.json"));
    // Warnings are excluded; only the error survives.
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code.as_deref(), Some("reportMissingImports"));
    assert_eq!(diagnostics[0].class, ResidualClass::ImportGraph);
    assert_eq!(diagnostics[0].line, Some(1), "LSP 0-based becomes 1-based");
    // The two Python sensors carry distinct parser identities and are
    // never pooled as one calibration stratum.
    assert_ne!(ty::PARSER_ID, pyright::PARSER_ID);
}

#[test]
fn pytest_junit_preserves_test_identity() {
    let diagnostics = pytest::parse(&fixture("pytest_junit.xml"));
    assert_eq!(diagnostics.len(), 1, "passing cases are not failures");
    assert_eq!(
        diagnostics[0].code.as_deref(),
        Some("tests.test_math::test_addition")
    );
    assert_eq!(diagnostics[0].class, ResidualClass::TestFailure);
    assert_eq!(diagnostics[0].message, "assert 2 == 3");
    assert_eq!(diagnostics[0].path.as_deref(), Some("tests/test_math.py"));
    assert_eq!(diagnostics[0].line, Some(7));
}

#[test]
fn tsc_text_normalizes_codes_and_positions() {
    let diagnostics = tsc::parse(&fixture("tsc.txt"));
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].code.as_deref(), Some("TS2322"));
    assert_eq!(diagnostics[0].class, ResidualClass::Type);
    assert_eq!(diagnostics[0].path.as_deref(), Some("src/index.ts"));
    assert_eq!(diagnostics[0].line, Some(4));
    assert_eq!(diagnostics[1].class, ResidualClass::ImportGraph);
}

/// Normalization calibration: cluster magnitudes are log-damped and
/// bounded, replacing the historic unconditional `score = 1.0` per line.
#[test]
fn clustering_normalizes_cascades_before_scoring() {
    let raw = fixture("cargo_check.jsonl");
    let clusters = cluster(cargo::parse(&raw));
    // Two E0432 diagnostics fold into one ImportGraph cluster; E0308 is
    // its own cluster.
    assert_eq!(clusters.len(), 2);
    let import = &clusters[0];
    assert_eq!(import.members.len(), 2);
    let magnitude = import.magnitude();
    assert!(magnitude > 1.0 && magnitude < 2.0, "{magnitude}");
    assert!((clusters[1].magnitude() - 1.0).abs() < 1e-9);
}
