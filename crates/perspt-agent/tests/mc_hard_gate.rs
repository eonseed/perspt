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
#[cfg(any(target_os = "linux", target_os = "macos"))]
use perspt_core::{ExternalOracleConfig, TestPolicy};
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

fn rust_fixture(dir: &std::path::Path, lib_body: &str) {
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname='lint-gate-fixture'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    std::fs::write(dir.join("src/lib.rs"), lib_body).unwrap();
}

async fn apply_lib(workspace: &CandidateWorkspace, content: &str) {
    apply_file(workspace, "src/lib.rs", content).await;
}

async fn apply_file(workspace: &CandidateWorkspace, path: &str, content: &str) {
    let entry = perspt_sdk::base_entries()
        .into_iter()
        .find(|entry| entry.name == "write_file")
        .unwrap();
    let call = perspt_sdk::ProviderToolCall {
        call_id: "w1".into(),
        name: "write_file".into(),
        arguments: serde_json::json!({"path": path, "content": content}),
    };
    use perspt_agent::toolloop::EffectExecutor;
    workspace.apply(&call, &entry).await.unwrap();
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[tokio::test]
async fn evolving_tests_allow_an_intentional_contract_change() {
    let dir = tempfile::tempdir().unwrap();
    rust_fixture(dir.path(), "pub fn answer() -> u32 { 2 }\n");
    std::fs::create_dir_all(dir.path().join("tests")).unwrap();
    std::fs::write(
        dir.path().join("tests/contract.rs"),
        "use lint_gate_fixture::answer; #[test] fn contract() { assert_eq!(answer(), 2); }\n",
    )
    .unwrap();
    let workspace = CandidateWorkspace::create(dir.path(), "n1", 0, "rev-0").unwrap();
    apply_lib(&workspace, "pub fn answer() -> u32 { 3 }\n").await;
    apply_file(
        &workspace,
        "tests/contract.rs",
        "use lint_gate_fixture::answer; #[test] fn contract() { assert_eq!(answer(), 3); }\n",
    )
    .await;

    let evolving = CodingCandidateMeasurer::new(&workspace, "n1", 0)
        .measure()
        .await
        .unwrap();
    assert!(
        evolving.hard_pass,
        "the resulting implementation and resulting tests are authoritative in evolving mode"
    );

    let compatible = CodingCandidateMeasurer::new(&workspace, "n1", 0)
        .with_test_policy(TestPolicy::BackwardCompatible, None)
        .measure()
        .await
        .unwrap();
    assert!(
        !compatible.hard_pass,
        "backward-compatible mode must additionally enforce the old contract"
    );
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[tokio::test]
async fn external_oracle_is_additional_protected_acceptance_evidence() {
    let dir = tempfile::tempdir().unwrap();
    rust_fixture(dir.path(), "pub fn answer() -> u32 { 2 }\n");
    let workspace = CandidateWorkspace::create(dir.path(), "n1", 0, "rev-0").unwrap();
    apply_lib(&workspace, "pub fn answer() -> u32 { 3 }\n").await;
    let protected = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(protected.path().join("tests")).unwrap();
    std::fs::write(
        protected.path().join("tests/acceptance.rs"),
        "use lint_gate_fixture::answer; #[test] fn acceptance() { assert_eq!(answer(), 4); }\n",
    )
    .unwrap();

    let measured = CodingCandidateMeasurer::new(&workspace, "n1", 0)
        .with_test_policy(
            TestPolicy::ExternalOracle,
            Some(ExternalOracleConfig {
                path: protected.path().to_path_buf(),
                command: "cargo test --test acceptance".into(),
            }),
        )
        .measure()
        .await
        .unwrap();
    assert!(
        !measured.hard_pass,
        "the protected acceptance suite must gate the candidate"
    );

    std::fs::write(
        protected.path().join("tests/acceptance.rs"),
        "use lint_gate_fixture::answer; #[test] fn acceptance() { assert_eq!(answer(), 3); }\n",
    )
    .unwrap();
    let accepted = CodingCandidateMeasurer::new(&workspace, "n1", 0)
        .with_test_policy(
            TestPolicy::ExternalOracle,
            Some(ExternalOracleConfig {
                path: protected.path().to_path_buf(),
                command: "cargo test --test acceptance".into(),
            }),
        )
        .measure()
        .await
        .unwrap();
    assert!(
        accepted.hard_pass,
        "a passing protected suite must permit the otherwise valid candidate: {:?}",
        accepted
            .residuals
            .iter()
            .map(|residual| &residual.evidence.summary)
            .collect::<Vec<_>>()
    );
}

/// A clippy warning is advisory: it surfaces as a Lint residual with
/// positive energy but no longer blocks hard pass, which gates only on the
/// `HardGatePolicy` stages (syntax, build, test). Before this fix any
/// pre-existing repository warning made hard pass permanently unreachable.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[tokio::test]
async fn a_lint_only_warning_does_not_block_hard_pass() {
    let dir = tempfile::tempdir().unwrap();
    rust_fixture(dir.path(), "pub fn answer() -> u32 { 2 }\n");
    let workspace = CandidateWorkspace::create(dir.path(), "n1", 0, "rev-0").unwrap();
    // `return 2;` trips clippy::needless_return under `-D warnings`.
    apply_lib(&workspace, "pub fn answer() -> u32 {\n    return 2;\n}\n").await;
    let measured = CodingCandidateMeasurer::new(&workspace, "n1", 0)
        .measure()
        .await
        .unwrap();
    assert!(
        measured.hard_pass,
        "lint must not gate: {:?}",
        measured
            .residuals
            .iter()
            .map(|r| (r.class, r.evidence.summary.clone()))
            .collect::<Vec<_>>()
    );
    assert!(
        measured
            .residuals
            .iter()
            .any(|r| r.class == ResidualClass::Lint || r.evidence.summary.contains("lint")),
        "the lint finding must still be reported as an advisory residual"
    );
    assert!(
        measured.energy > 0.0,
        "advisory residuals still cost energy"
    );
}

#[cfg(target_os = "windows")]
#[tokio::test]
async fn governed_verifiers_fail_closed_without_a_windows_backend() {
    let dir = tempfile::tempdir().unwrap();
    rust_fixture(dir.path(), "pub fn answer() -> u32 { 2 }\n");
    let workspace = CandidateWorkspace::create(dir.path(), "n1", 0, "rev-0").unwrap();
    let measured = CodingCandidateMeasurer::new(&workspace, "n1", 0)
        .measure()
        .await
        .unwrap();
    assert!(!measured.hard_pass);
    assert!(measured.residuals.iter().any(|residual| {
        residual
            .evidence
            .summary
            .contains("no registered governed process sandbox")
    }));
}

#[cfg(target_os = "windows")]
#[tokio::test]
async fn reduced_isolation_runs_the_native_windows_verifiers() {
    let dir = tempfile::tempdir().unwrap();
    rust_fixture(dir.path(), "pub fn answer() -> u32 { 2 }\n");
    let workspace =
        CandidateWorkspace::create_with_policy(dir.path(), "n1", 0, "rev-0", true).unwrap();
    apply_lib(&workspace, "pub fn answer() -> u32 { 3 }\n").await;
    let measured = CodingCandidateMeasurer::new(&workspace, "n1", 0)
        .measure()
        .await
        .unwrap();
    assert!(
        measured.hard_pass,
        "explicit reduced isolation should run the installed native toolchain: {:?}",
        measured
            .residuals
            .iter()
            .map(|residual| &residual.evidence.summary)
            .collect::<Vec<_>>()
    );
}

/// Every gate stage of one candidate reuses one shared target dir — no
/// `check-0`/`build-1` cold siblings — so a measurement pays one cold build
/// plus incrementals.
#[tokio::test]
async fn gate_stages_share_one_warm_target_dir() {
    let dir = tempfile::tempdir().unwrap();
    rust_fixture(dir.path(), "pub fn answer() -> u32 { 2 }\n");
    let workspace = CandidateWorkspace::create(dir.path(), "n1", 0, "rev-0").unwrap();
    apply_lib(&workspace, "pub fn answer() -> u32 { 2 }\n").await;
    CodingCandidateMeasurer::new(&workspace, "n1", 0)
        .measure()
        .await
        .unwrap();
    let target_root = workspace.overlay_root().join(".perspt-target");
    let entries: Vec<String> = std::fs::read_dir(&target_root)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(
        entries,
        vec!["shared".to_string()],
        "gate stages must share one warm target dir"
    );
}

/// A failing test still blocks hard pass — demoting lint never loosens the
/// required stages.
#[tokio::test]
async fn a_failing_test_still_blocks_hard_pass() {
    let dir = tempfile::tempdir().unwrap();
    rust_fixture(dir.path(), "pub fn answer() -> u32 { 2 }\n");
    let workspace = CandidateWorkspace::create(dir.path(), "n1", 0, "rev-0").unwrap();
    let failing = "pub fn answer() -> u32 { 2 }\n#[cfg(test)]\nmod tests {\n    \
         #[test]\n    fn wrong() { assert_eq!(super::answer(), 3); }\n}\n";
    apply_lib(&workspace, failing).await;
    let measured = CodingCandidateMeasurer::new(&workspace, "n1", 0)
        .measure()
        .await
        .unwrap();
    assert!(!measured.hard_pass, "a failing test must block hard pass");
}

/// A plugin's declared no-op stage (Python has no build step) *satisfies*
/// its required-stage obligation. Before this fix `required-stage:build`
/// blocked hard pass on every Python project forever — the model would fix
/// the code and then loop against a residual no edit can address.
#[tokio::test]
async fn a_declared_no_op_stage_satisfies_its_required_stage() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src/t")).unwrap();
    std::fs::write(
        dir.path().join("pyproject.toml"),
        "[project]\nname='t'\nversion='0.1.0'\nrequires-python='>=3.10'\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("src/t/__init__.py"), "").unwrap();
    std::fs::write(
        dir.path().join("src/t/lib.py"),
        "def answer() -> int:\n    return 2\n",
    )
    .unwrap();
    let workspace = CandidateWorkspace::create(dir.path(), "n1", 0, "rev-0").unwrap();
    let measured = CodingCandidateMeasurer::new(&workspace, "n1", 0)
        .measure()
        .await
        .unwrap();
    let evidence = required_stage_residuals(&measured);
    assert!(
        !evidence
            .iter()
            .any(|summary| summary.contains("required-stage:build")),
        "python's declared no-op build stage must satisfy the required \
         stage, never block it: {evidence:?}"
    );
}
