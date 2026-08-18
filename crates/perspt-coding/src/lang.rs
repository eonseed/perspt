//! Language adapters (PSP-8 System 5 / Gate D).
//!
//! A coding language adapter evolves from a command selector into a verifier
//! suite: it parses compiler/type-checker/test output into typed
//! [`ResidualEvent`]s and maps each into a [`CorrectionDirection`]. Actually
//! *running* the tools is the runtime's job; the testable core here is the
//! normalization from raw diagnostic text to residual evidence, which is what
//! makes corrections directed rather than undirected retries.

use std::path::Path;

use perspt_sdk::{
    CorrectionDirection, IndependenceRoute, ResidualClass, ResidualEvent, ResidualSeverity,
    SensorRef,
};

use crate::runtime::{default_classify_runtime, SmokeInvocation};
use crate::LanguageId;

/// A coding language adapter: a verifier-suite provider for one language.
pub trait LanguageAdapter: Send + Sync {
    /// The open, stable language identifier this adapter answers to.
    fn id(&self) -> LanguageId;
    /// The primary diagnostic sensor for this language.
    fn diagnostic_sensor(&self) -> SensorRef;
    /// Parse raw diagnostic output into typed residuals.
    fn parse_diagnostics(&self, node_id: &str, generation: u32, raw: &str) -> Vec<ResidualEvent>;
    /// Map a residual to a correction direction, or `None` when there is none.
    fn correction_for(&self, residual: &ResidualEvent) -> Option<CorrectionDirection>;

    /// Runtime smoke invocations to exercise the built artifact's entrypoints
    /// (PSP-8 runtime probe). Default: none — a new adapter opts in by overriding
    /// this. The runtime executes the returned commands from `workspace`.
    fn smoke_invocations(&self, _workspace: &Path) -> Vec<SmokeInvocation> {
        Vec::new()
    }

    /// Classify the output of a smoke invocation into `Runtime` residuals.
    /// Default: shared crash-marker + non-zero-exit detection.
    fn classify_runtime(
        &self,
        node_id: &str,
        generation: u32,
        invocation: &SmokeInvocation,
        exit_success: bool,
        output: &str,
    ) -> Vec<ResidualEvent> {
        default_classify_runtime(node_id, generation, invocation, exit_success, output)
    }
}

/// Read the `[package] name` from a Cargo.toml, for naming `cargo run -p` targets.
fn cargo_package_name(manifest: &Path) -> Option<String> {
    let content = std::fs::read_to_string(manifest).ok()?;
    let mut in_package = false;
    for raw in content.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if in_package {
            if let Some(rest) = line.strip_prefix("name") {
                if let Some(eq) = rest.trim().strip_prefix('=') {
                    return Some(eq.trim().trim_matches('"').trim_matches('\'').to_string());
                }
            }
        }
    }
    None
}

fn residual(
    node_id: &str,
    generation: u32,
    class: ResidualClass,
    sensor: SensorRef,
    summary: &str,
) -> ResidualEvent {
    let mut r = ResidualEvent::new(
        node_id,
        generation,
        class,
        ResidualSeverity::Error,
        1.0,
        sensor,
    )
    .expect("unit score is valid");
    r.evidence.summary = summary.to_string();
    r
}

/// The sensor fingerprint: tool identity, parser version, and the cluster
/// normalization profile (PSP-10 Assumption 2).
fn fingerprint(parser_id: &str) -> String {
    format!("{parser_id}+{}", crate::diag::CLUSTER_PROFILE_V1)
}

/// Fold structured diagnostics into cluster-level residuals with full
/// evidence (PSP-10 system 26). One residual per root-cause cluster; the
/// magnitude is the profile-normalized cluster magnitude, never a raw
/// diagnostic count.
fn structured_residuals(
    node_id: &str,
    generation: u32,
    diagnostics: Vec<crate::diag::StructuredDiagnostic>,
    raw: &str,
    sensor: SensorRef,
) -> Vec<ResidualEvent> {
    crate::diag::cluster(diagnostics)
        .into_iter()
        .filter_map(|cluster| {
            let summary = if cluster.members.len() > 1 {
                format!(
                    "{} ({} diagnostics in cluster {})",
                    cluster.root.message,
                    cluster.members.len(),
                    cluster.code
                )
            } else {
                format!("{} ({})", cluster.root.message, cluster.code)
            };
            crate::diag::cluster_residual(
                node_id,
                generation,
                cluster.class,
                cluster.magnitude(),
                sensor.clone(),
                &summary,
                raw,
                &cluster.members,
            )
            .ok()
        })
        .collect()
}

// ============================ Rust ============================

/// The Rust verifier-suite adapter (rustc / cargo / rust-analyzer).
#[derive(Debug, Clone, Default)]
pub struct RustAdapter;

/// Classify a rustc error code into a residual class.
pub fn classify_rust_code(code: &str) -> ResidualClass {
    match code {
        // Unresolved imports / missing modules.
        "E0432" | "E0433" | "E0583" | "E0761" => ResidualClass::ImportGraph,
        // Cannot find name / value / type.
        "E0412" | "E0425" | "E0422" | "E0531" => ResidualClass::SymbolMismatch,
        // Type / trait-bound mismatches.
        "E0308" | "E0277" | "E0599" | "E0061" => ResidualClass::Type,
        // Borrow / ownership / lifetimes.
        "E0382" | "E0499" | "E0502" | "E0505" | "E0506" | "E0597" => {
            ResidualClass::OwnershipViolation
        }
        // Visibility / privacy.
        "E0603" | "E0616" => ResidualClass::InterfaceMismatch,
        // Anything else compiler-emitted is a generic type/build residual.
        _ => ResidualClass::Type,
    }
}

impl LanguageAdapter for RustAdapter {
    fn id(&self) -> LanguageId {
        LanguageId::new("rust")
    }

    fn diagnostic_sensor(&self) -> SensorRef {
        SensorRef::new("rustc", IndependenceRoute::Compiler)
    }

    fn parse_diagnostics(&self, node_id: &str, generation: u32, raw: &str) -> Vec<ResidualEvent> {
        // The structured plane first: cargo's native JSON stream preserves
        // codes, spans, and suggestions (PSP-10 system 26).
        if crate::diag::cargo::looks_like_stream(raw) {
            let sensor = self
                .diagnostic_sensor()
                .with_fingerprint(fingerprint(crate::diag::cargo::PARSER_ID));
            let mut residuals = structured_residuals(
                node_id,
                generation,
                crate::diag::cargo::parse(raw),
                raw,
                sensor,
            );
            residuals.extend(rust_test_failures(node_id, generation, raw));
            return residuals;
        }
        // Text fallback under its own parser identity; never pooled with
        // the JSON parser's measurements.
        let sensor = self
            .diagnostic_sensor()
            .with_fingerprint("rustc-text-v1".to_string());
        let mut residuals = Vec::new();
        for line in raw.lines() {
            let line = line.trim();
            // `error[E0432]: unresolved import `foo``
            if let Some(rest) = line.strip_prefix("error[") {
                if let Some(end) = rest.find(']') {
                    let code = &rest[..end];
                    let class = classify_rust_code(code);
                    let summary = rest[end + 1..].trim_start_matches(':').trim();
                    residuals.push(residual(
                        node_id,
                        generation,
                        class,
                        sensor.clone(),
                        summary,
                    ));
                }
            }
        }
        residuals.extend(rust_test_failures(node_id, generation, raw));
        residuals
    }

    fn correction_for(&self, residual: &ResidualEvent) -> Option<CorrectionDirection> {
        let summary = &residual.evidence.summary;
        match residual.class {
            ResidualClass::ImportGraph => Some(
                CorrectionDirection::new(
                    ResidualClass::ImportGraph,
                    format!(
                        "resolve the unresolved import ({summary}): add the missing `use` path or \
                         declare the missing `mod`; do not regenerate unrelated code"
                    ),
                )
                .with_rationale("unresolved imports are structural, not behavioral"),
            ),
            ResidualClass::SymbolMismatch => Some(CorrectionDirection::new(
                ResidualClass::SymbolMismatch,
                format!("define or correct the referenced name ({summary}); check spelling and path"),
            )),
            ResidualClass::Type => Some(CorrectionDirection::new(
                ResidualClass::Type,
                format!("reconcile the type/trait mismatch ({summary}); keep the public signature stable"),
            )),
            ResidualClass::OwnershipViolation => Some(CorrectionDirection::new(
                ResidualClass::OwnershipViolation,
                format!("fix the borrow/ownership error ({summary}); clone, borrow, or restructure \
                    lifetimes"),
            )),
            ResidualClass::InterfaceMismatch => Some(CorrectionDirection::new(
                ResidualClass::InterfaceMismatch,
                format!("adjust visibility ({summary}); make the item `pub` or use an accessible path"),
            )),
            ResidualClass::TestFailure => Some(CorrectionDirection::new(
                ResidualClass::TestFailure,
                "fix the implementation the failing test attributes to; do not weaken the assertion",
            )),
            ResidualClass::Runtime => Some(CorrectionDirection::new(
                ResidualClass::Runtime,
                format!(
                    "the built binary failed when actually run ({summary}); fix the runtime logic \
                     (panics, index/shape mismatches, unwraps) so every entrypoint executes \
                     cleanly, and add a test/example covering that runtime path"
                ),
            )),
            _ => None,
        }
    }

    fn smoke_invocations(&self, workspace: &Path) -> Vec<SmokeInvocation> {
        let mut out = Vec::new();
        // Binary crates: `cargo run -p <name> -- --help` exercises startup +
        // arg parsing without needing real arguments.
        for (name, _dir) in rust_binary_crates(workspace) {
            out.push(SmokeInvocation::new(
                format!("cargo run -q -p {name} -- --help"),
                format!("{name} --help"),
            ));
        }
        // Examples are the project's own end-to-end smoke; run each one. A good
        // example exercises the real pipeline (e.g. train→predict), so a runtime
        // bug there is caught here.
        for (pkg, example) in rust_examples(workspace) {
            let cmd = match pkg {
                Some(ref p) => format!("cargo run -q -p {p} --example {example}"),
                None => format!("cargo run -q --example {example}"),
            };
            out.push(SmokeInvocation::new(cmd, format!("example {example}")));
        }
        out
    }
}

/// Test-failure lines from `cargo test` text output (both stream shapes).
fn rust_test_failures(node_id: &str, generation: u32, raw: &str) -> Vec<ResidualEvent> {
    raw.lines()
        .map(str::trim)
        .filter(|line| line.starts_with("test result: FAILED") || line.contains("... FAILED"))
        .map(|line| {
            residual(
                node_id,
                generation,
                ResidualClass::TestFailure,
                SensorRef::new("cargo-test", IndependenceRoute::TestOracle)
                    .with_fingerprint("cargo-test-text-v1".to_string()),
                line,
            )
        })
        .collect()
}

/// Discover binary crates (those with `src/main.rs`) in a Cargo workspace:
/// the root package and any `crates/*` members. Returns `(package_name, dir)`.
fn rust_binary_crates(workspace: &Path) -> Vec<(String, std::path::PathBuf)> {
    let mut out = Vec::new();
    let mut consider = |dir: std::path::PathBuf| {
        if dir.join("src/main.rs").exists() {
            if let Some(name) = cargo_package_name(&dir.join("Cargo.toml")) {
                out.push((name, dir));
            }
        }
    };
    consider(workspace.to_path_buf());
    if let Ok(entries) = std::fs::read_dir(workspace.join("crates")) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                consider(entry.path());
            }
        }
    }
    out
}

/// Discover example targets: `examples/*.rs` at the root (no `-p`) and under
/// each `crates/*` member (with that member's `-p`). Returns `(package, stem)`.
fn rust_examples(workspace: &Path) -> Vec<(Option<String>, String)> {
    let mut out = Vec::new();
    let collect = |dir: &Path, pkg: Option<String>, out: &mut Vec<(Option<String>, String)>| {
        if let Ok(entries) = std::fs::read_dir(dir.join("examples")) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        out.push((pkg.clone(), stem.to_string()));
                    }
                }
            }
        }
    };
    collect(workspace, None, &mut out);
    if let Ok(entries) = std::fs::read_dir(workspace.join("crates")) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let pkg = cargo_package_name(&entry.path().join("Cargo.toml"));
                collect(&entry.path(), pkg, &mut out);
            }
        }
    }
    out
}

// ============================ Python ============================

/// The Python verifier-suite adapter. `ty` is the primary sensor; Pyright
/// is a separately fingerprinted fallback (PSP-10 resolved decision 15).
/// Attribution follows the observed output format — the historic
/// split-brain that stamped every observation "pyright" is gone.
#[derive(Debug, Clone, Default)]
pub struct PythonAdapter;

impl LanguageAdapter for PythonAdapter {
    fn id(&self) -> LanguageId {
        LanguageId::new("python")
    }

    fn diagnostic_sensor(&self) -> SensorRef {
        SensorRef::new("ty", IndependenceRoute::Lsp)
    }

    fn parse_diagnostics(&self, node_id: &str, generation: u32, raw: &str) -> Vec<ResidualEvent> {
        // `ty check --output-format concise` (primary text form).
        let ty_diagnostics = crate::diag::ty::parse_check_text(raw);
        if !ty_diagnostics.is_empty() {
            let sensor = SensorRef::new("ty", IndependenceRoute::Lsp)
                .with_fingerprint(fingerprint(crate::diag::ty::PARSER_ID));
            let mut residuals =
                structured_residuals(node_id, generation, ty_diagnostics, raw, sensor);
            residuals.extend(python_test_failures(node_id, generation, raw));
            return residuals;
        }
        // Pyright JSON fallback, under its own fingerprint — never pooled
        // with `ty` measurements.
        let pyright_diagnostics = crate::diag::pyright::parse(raw);
        if !pyright_diagnostics.is_empty() {
            let sensor = SensorRef::new("pyright", IndependenceRoute::Lsp)
                .with_fingerprint(fingerprint(crate::diag::pyright::PARSER_ID));
            let mut residuals =
                structured_residuals(node_id, generation, pyright_diagnostics, raw, sensor);
            residuals.extend(python_test_failures(node_id, generation, raw));
            return residuals;
        }
        // Pytest JUnit XML.
        let junit = crate::diag::pytest::parse(raw);
        if !junit.is_empty() {
            let sensor = SensorRef::new("pytest", IndependenceRoute::TestOracle)
                .with_fingerprint(fingerprint(crate::diag::pytest::PARSER_ID));
            return structured_residuals(node_id, generation, junit, raw, sensor);
        }
        // Legacy text scrape (stdlib checks, bare tracebacks) under the
        // honest sensor identity of the interpreter, not a type checker.
        let sensor = SensorRef::new("python-check", IndependenceRoute::Compiler)
            .with_fingerprint("python-text-v1".to_string());
        let mut residuals = Vec::new();
        for line in raw.lines() {
            let lower = line.to_lowercase();
            let class = if lower.contains("could not be resolved")
                || lower.contains("no module named")
            {
                Some(ResidualClass::ImportGraph)
            } else if lower.contains("is not defined") || lower.contains("is possibly unbound") {
                Some(ResidualClass::SymbolMismatch)
            } else if lower.contains("incompatible")
                || lower.contains("expected type")
                || lower.contains("has type")
            {
                Some(ResidualClass::Type)
            } else if lower.contains("failed") && lower.contains("test") {
                Some(ResidualClass::TestFailure)
            } else {
                None
            };
            if let Some(class) = class {
                let sensor = if class == ResidualClass::TestFailure {
                    SensorRef::new("pytest", IndependenceRoute::TestOracle)
                        .with_fingerprint("pytest-text-v1".to_string())
                } else {
                    sensor.clone()
                };
                residuals.push(residual(node_id, generation, class, sensor, line.trim()));
            }
        }
        residuals
    }

    fn correction_for(&self, residual: &ResidualEvent) -> Option<CorrectionDirection> {
        let summary = &residual.evidence.summary;
        match residual.class {
            ResidualClass::ImportGraph => Some(CorrectionDirection::new(
                ResidualClass::ImportGraph,
                format!(
                    "add the missing import or install/declare the package ({summary}); sync the \
                    environment"
                ),
            )),
            ResidualClass::SymbolMismatch => Some(CorrectionDirection::new(
                ResidualClass::SymbolMismatch,
                format!("define the referenced name or fix its binding ({summary})"),
            )),
            ResidualClass::Type => Some(CorrectionDirection::new(
                ResidualClass::Type,
                format!(
                    "reconcile the type mismatch ({summary}); adjust the value or the annotation"
                ),
            )),
            ResidualClass::TestFailure => Some(CorrectionDirection::new(
                ResidualClass::TestFailure,
                "fix the code under the failing pytest case; preserve the assertion",
            )),
            ResidualClass::Runtime => Some(CorrectionDirection::new(
                ResidualClass::Runtime,
                format!(
                    "the package failed when actually run/imported ({summary}); fix the runtime \
                     error (import-time exceptions, shape/type mismatches) and add a test/example \
                     covering that path"
                ),
            )),
            _ => None,
        }
    }

    fn smoke_invocations(&self, workspace: &Path) -> Vec<SmokeInvocation> {
        // Import smoke: importing the package executes all module top-level code,
        // catching import-time errors that unit tests on submodules can miss.
        python_packages(workspace)
            .into_iter()
            .map(|pkg| {
                SmokeInvocation::new(
                    format!("uv run python -c \"import {pkg}\""),
                    format!("import {pkg}"),
                )
            })
            .collect()
    }
}

/// Pytest text-output failures (when no JUnit XML was produced).
fn python_test_failures(node_id: &str, generation: u32, raw: &str) -> Vec<ResidualEvent> {
    raw.lines()
        .map(str::trim)
        .filter(|line| {
            let lower = line.to_lowercase();
            lower.contains("failed") && lower.contains("test")
        })
        .map(|line| {
            residual(
                node_id,
                generation,
                ResidualClass::TestFailure,
                SensorRef::new("pytest", IndependenceRoute::TestOracle)
                    .with_fingerprint("pytest-text-v1".to_string()),
                line,
            )
        })
        .collect()
}

/// Discover importable top-level packages: directories containing `__init__.py`
/// under `src/` (src-layout) or the workspace root (flat layout).
fn python_packages(workspace: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for base in [workspace.join("src"), workspace.to_path_buf()] {
        if let Ok(entries) = std::fs::read_dir(&base) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join("__init__.py").exists() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if !out.iter().any(|p| p == name) {
                            out.push(name.to_string());
                        }
                    }
                }
            }
        }
    }
    out
}

// ============================ TypeScript ============================

/// The JavaScript/TypeScript verifier-suite adapter (tsc / eslint).
#[derive(Debug, Clone, Default)]
pub struct TypeScriptAdapter;

/// Classify a TypeScript diagnostic code (e.g. `TS2307`).
pub fn classify_ts_code(code: &str) -> ResidualClass {
    match code {
        "TS2307" => ResidualClass::ImportGraph, // cannot find module
        "TS2304" => ResidualClass::SymbolMismatch, // cannot find name
        "TS2305" | "TS2614" => ResidualClass::InterfaceMismatch, // no exported member
        "TS2322" | "TS2345" | "TS2769" => ResidualClass::Type, // type mismatches
        "TS6133" | "TS6192" => ResidualClass::Lint, // unused
        _ => ResidualClass::Type,
    }
}

impl LanguageAdapter for TypeScriptAdapter {
    fn id(&self) -> LanguageId {
        LanguageId::new("typescript")
    }

    fn diagnostic_sensor(&self) -> SensorRef {
        SensorRef::new("tsc", IndependenceRoute::Compiler)
    }

    fn parse_diagnostics(&self, node_id: &str, generation: u32, raw: &str) -> Vec<ResidualEvent> {
        // The versioned `tsc --pretty false` normalizer preserves paths and
        // positions (PSP-10 system 26).
        let structured = crate::diag::tsc::parse(raw);
        if !structured.is_empty() {
            let sensor = self
                .diagnostic_sensor()
                .with_fingerprint(fingerprint(crate::diag::tsc::PARSER_ID));
            return structured_residuals(node_id, generation, structured, raw, sensor);
        }
        // Fallback for lines without the position prefix.
        let sensor = self
            .diagnostic_sensor()
            .with_fingerprint("tsc-loose-text-v1".to_string());
        let mut residuals = Vec::new();
        for line in raw.lines() {
            if let Some(idx) = line.find("error TS") {
                let rest = &line[idx + "error ".len()..];
                let code: String = rest
                    .chars()
                    .take_while(|c| !c.is_whitespace() && *c != ':')
                    .collect();
                let class = classify_ts_code(&code);
                let summary = rest.split_once(':').map(|(_, s)| s.trim()).unwrap_or(rest);
                residuals.push(residual(
                    node_id,
                    generation,
                    class,
                    sensor.clone(),
                    summary,
                ));
            }
        }
        residuals
    }

    fn correction_for(&self, residual: &ResidualEvent) -> Option<CorrectionDirection> {
        let summary = &residual.evidence.summary;
        match residual.class {
            ResidualClass::ImportGraph => Some(CorrectionDirection::new(
                ResidualClass::ImportGraph,
                format!(
                    "fix the module path or add the dependency ({summary}); check tsconfig path \
                    aliases"
                ),
            )),
            ResidualClass::SymbolMismatch => Some(CorrectionDirection::new(
                ResidualClass::SymbolMismatch,
                format!("import or declare the missing name ({summary})"),
            )),
            ResidualClass::InterfaceMismatch => Some(CorrectionDirection::new(
                ResidualClass::InterfaceMismatch,
                format!("export the missing member or fix the import binding ({summary})"),
            )),
            ResidualClass::Type => Some(CorrectionDirection::new(
                ResidualClass::Type,
                format!("reconcile the type mismatch ({summary})"),
            )),
            ResidualClass::Lint => Some(CorrectionDirection::new(
                ResidualClass::Lint,
                format!("remove the unused symbol ({summary})"),
            )),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_unresolved_import_classified_and_directed() {
        let adapter = RustAdapter;
        let raw = "error[E0432]: unresolved import `crate::foo::Bar`";
        let residuals = adapter.parse_diagnostics("n1", 0, raw);
        assert_eq!(residuals.len(), 1);
        assert_eq!(residuals[0].class, ResidualClass::ImportGraph);
        let dir = adapter.correction_for(&residuals[0]).unwrap();
        assert_eq!(dir.addresses, ResidualClass::ImportGraph);
        assert!(dir.instruction.contains("use"));
    }

    #[test]
    fn rust_classifies_a_spread_of_codes() {
        assert_eq!(classify_rust_code("E0308"), ResidualClass::Type);
        assert_eq!(
            classify_rust_code("E0382"),
            ResidualClass::OwnershipViolation
        );
        assert_eq!(
            classify_rust_code("E0603"),
            ResidualClass::InterfaceMismatch
        );
        assert_eq!(classify_rust_code("E0425"), ResidualClass::SymbolMismatch);
    }

    #[test]
    fn rust_test_failure_parsed() {
        let adapter = RustAdapter;
        let raw = "test tests::it_works ... FAILED";
        let residuals = adapter.parse_diagnostics("n1", 0, raw);
        assert_eq!(residuals[0].class, ResidualClass::TestFailure);
        assert_eq!(residuals[0].sensor.route, IndependenceRoute::TestOracle);
    }

    #[test]
    fn python_import_and_type_classified() {
        let adapter = PythonAdapter;
        let raw = "x.py:1: error: Import \"requests\" could not be resolved\nx.py:2: error: Argument 1 has \
            incompatible type \"str\"";
        let residuals = adapter.parse_diagnostics("n1", 0, raw);
        assert_eq!(residuals.len(), 2);
        assert_eq!(residuals[0].class, ResidualClass::ImportGraph);
        assert_eq!(residuals[1].class, ResidualClass::Type);
    }

    #[test]
    fn typescript_codes_classified_and_directed() {
        let adapter = TypeScriptAdapter;
        let raw = "src/a.ts(3,10): error TS2307: Cannot find module 'foo'.\nsrc/b.ts(4,2): error TS2322: \
            Type 'string' is not assignable to type 'number'.";
        let residuals = adapter.parse_diagnostics("n1", 0, raw);
        assert_eq!(residuals.len(), 2);
        assert_eq!(residuals[0].class, ResidualClass::ImportGraph);
        assert_eq!(residuals[1].class, ResidualClass::Type);
        assert!(adapter.correction_for(&residuals[0]).is_some());
    }

    #[test]
    fn rust_smoke_discovers_workspace_binaries_and_examples() {
        let dir = std::env::temp_dir().join(format!(
            "perspt-smoke-rust-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("crates/cli/src")).unwrap();
        std::fs::create_dir_all(dir.join("crates/cli/examples")).unwrap();
        std::fs::write(
            dir.join("crates/cli/Cargo.toml"),
            "[package]\nname = \"weather-cli\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("crates/cli/src/main.rs"), "fn main() {}\n").unwrap();
        std::fs::write(dir.join("crates/cli/examples/demo.rs"), "fn main() {}\n").unwrap();

        let inv = RustAdapter.smoke_invocations(&dir);
        assert!(
            inv.iter()
                .any(|i| i.command == "cargo run -q -p weather-cli -- --help"),
            "got {inv:?}"
        );
        assert!(
            inv.iter().any(|i| i.command.contains("--example demo")),
            "got {inv:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn python_smoke_discovers_src_layout_package() {
        let dir = std::env::temp_dir().join(format!(
            "perspt-smoke-py-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("src/rpncalc")).unwrap();
        std::fs::write(dir.join("src/rpncalc/__init__.py"), "").unwrap();

        let inv = PythonAdapter.smoke_invocations(&dir);
        assert!(
            inv.iter().any(|i| i.command.contains("import rpncalc")),
            "got {inv:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn adapter_for_dispatches_by_language() {
        assert_eq!(RustAdapter.id(), LanguageId::new("rust"));
        assert_eq!(PythonAdapter.id(), LanguageId::new("python"));
        assert_eq!(TypeScriptAdapter.id(), LanguageId::new("typescript"));
    }
}
