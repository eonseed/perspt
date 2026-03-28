//! Verification Runners
//!
//! Provides test, syntax-check, build, and lint execution for language plugins.
//!
//! - `PythonTestRunner`: pytest-specific runner with detailed output parsing.
//! - `RustTestRunner`: cargo-based runner with test output parsing.
//! - `PluginVerifierRunner` (PSP-5 Phase 4): generic runner driven entirely by
//!   a plugin's `VerifierProfile`. It executes whatever commands the profile
//!   declares, including fallback commands, without hardcoding language details.
//!
//! The `TestRunnerTrait` is the unified async interface consumed by the orchestrator.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

use crate::types::{BehavioralContract, Criticality};
use perspt_core::plugin::{VerifierProfile, VerifierStage};

/// Result of a test run
#[derive(Debug, Clone, Default)]
pub struct TestResults {
    /// Number of passed tests
    pub passed: usize,
    /// Number of failed tests
    pub failed: usize,
    /// Number of skipped tests
    pub skipped: usize,
    /// Total tests run
    pub total: usize,
    /// Detailed failure information
    pub failures: Vec<TestFailure>,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Raw output
    pub output: String,
    /// Whether the test run was successful (no infrastructure errors)
    pub run_succeeded: bool,
}

impl TestResults {
    /// Check if all tests passed
    pub fn all_passed(&self) -> bool {
        self.run_succeeded && self.failed == 0
    }

    /// Get pass rate as percentage
    pub fn pass_rate(&self) -> f32 {
        if self.total == 0 {
            1.0
        } else {
            (self.passed as f32) / (self.total as f32)
        }
    }
}

/// Information about a single test failure
#[derive(Debug, Clone)]
pub struct TestFailure {
    /// Test name (e.g., "test_divide_by_zero")
    pub name: String,
    /// Test file path
    pub file: Option<String>,
    /// Line number where failure occurred
    pub line: Option<u32>,
    /// Error message
    pub message: String,
    /// Criticality (from weighted tests if matched)
    pub criticality: Criticality,
}

fn force_failure_on_nonzero_exit(
    results: &mut TestResults,
    command_name: &str,
    exit_code: Option<i32>,
    output: &str,
) {
    if results.failed == 0 {
        results.failed = 1;
    }
    if results.total == 0 {
        results.total = results.passed + results.failed + results.skipped;
    }
    if results.failures.is_empty() {
        results.failures.push(TestFailure {
            name: command_name.to_string(),
            file: None,
            line: None,
            message: format!(
                "{} exited with code {:?} without a parseable success summary. Output:\n{}",
                command_name, exit_code, output
            ),
            criticality: Criticality::High,
        });
    }
}

/// Python test runner using uv and pytest
///
/// Handles:
/// 1. Checking for pyproject.toml
/// 2. Setting up Python environment via uv
/// 3. Running pytest
/// 4. Parsing results for V_log calculation
pub struct PythonTestRunner {
    /// Working directory (workspace root)
    working_dir: PathBuf,
    /// Timeout in seconds
    timeout_secs: u64,
    /// Whether to auto-setup if no pyproject.toml
    auto_setup: bool,
}

impl PythonTestRunner {
    /// Create a new Python test runner
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            working_dir,
            timeout_secs: 300, // 5 minute default timeout
            auto_setup: true,
        }
    }

    /// Set timeout
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Disable auto-setup (don't create pyproject.toml if missing)
    pub fn without_auto_setup(mut self) -> Self {
        self.auto_setup = false;
        self
    }

    /// Check if workspace has a Python project setup
    pub fn has_pyproject(&self) -> bool {
        self.working_dir.join("pyproject.toml").exists()
    }

    /// Check if workspace has pytest configured
    pub async fn has_pytest(&self) -> bool {
        // Check if pytest is in pyproject.toml or can be run
        let result = Command::new("uv")
            .args(["run", "pytest", "--version"])
            .current_dir(&self.working_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;

        result.map(|s| s.success()).unwrap_or(false)
    }

    /// Initialize the Python environment with uv
    /// NOTE: This assumes pyproject.toml already exists (created by orchestrator's step_init_project)
    pub async fn setup_environment(&self) -> Result<()> {
        log::info!("Setting up Python environment with uv");

        // Check if pyproject.toml exists; if not, warn and try to proceed
        if !self.has_pyproject() {
            if self.auto_setup {
                log::warn!(
                    "No pyproject.toml found. Project should be initialized via 'uv init' first."
                );
                log::info!("Attempting to run 'uv init' as fallback...");
                let init_output = Command::new("uv")
                    .args(["init"])
                    .current_dir(&self.working_dir)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .await
                    .context("Failed to run uv init")?;

                if !init_output.status.success() {
                    let stderr = String::from_utf8_lossy(&init_output.stderr);
                    log::warn!("uv init failed: {}", stderr);
                    return self.install_pytest_directly().await;
                }
            } else {
                anyhow::bail!(
                    "No pyproject.toml found and auto_setup is disabled. Run 'uv init' first."
                );
            }
        }

        // Sync dependencies (this creates venv and installs deps)
        let output = Command::new("uv")
            .args(["sync", "--dev"])
            .current_dir(&self.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to run uv sync")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            log::warn!("uv sync failed: {}", stderr);
            // Try just installing pytest directly
            return self.install_pytest_directly().await;
        }

        // Ensure pytest is available as a dev dependency.
        // `uv sync --dev` only installs what's already in pyproject.toml;
        // for freshly-generated projects pytest may not be declared yet.
        if !self.has_pytest().await {
            log::info!("pytest not available after sync — adding as dev dependency");
            let add_output = Command::new("uv")
                .args(["add", "--dev", "pytest"])
                .current_dir(&self.working_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await;
            match add_output {
                Ok(o) if o.status.success() => {
                    log::info!("Added pytest as dev dependency");
                }
                Ok(o) => {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    log::warn!("uv add --dev pytest failed: {}", stderr);
                    // Last resort: install directly
                    return self.install_pytest_directly().await;
                }
                Err(e) => {
                    log::warn!("Failed to run uv add --dev pytest: {}", e);
                    return self.install_pytest_directly().await;
                }
            }
        }

        log::info!("Python environment ready");
        Ok(())
    }

    /// Install pytest directly without a full project setup
    async fn install_pytest_directly(&self) -> Result<()> {
        log::info!("Installing pytest via uv pip");

        let output = Command::new("uv")
            .args(["pip", "install", "pytest"])
            .current_dir(&self.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to install pytest")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to install pytest: {}", stderr);
        }

        Ok(())
    }

    /// Run pytest and parse results
    ///
    /// If environment is not set up, will attempt to set it up first.
    pub async fn run_pytest(&self, test_args: &[&str]) -> Result<TestResults> {
        log::info!("Running pytest in {}", self.working_dir.display());

        // Ensure environment is set up
        if !self.has_pytest().await {
            self.setup_environment().await?;
        }

        // Build pytest command
        let mut args = vec!["run", "pytest", "-v", "--tb=short"];
        args.extend(test_args);

        let start = std::time::Instant::now();

        let output = Command::new("uv")
            .args(&args)
            .current_dir(&self.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to run pytest")?;

        let duration_ms = start.elapsed().as_millis() as u64;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let combined = format!("{}\n{}", stdout, stderr);

        log::debug!("pytest exit code: {:?}", output.status.code());
        if !stdout.is_empty() {
            log::debug!("pytest stdout:\n{}", stdout);
        }

        let mut results = self.parse_pytest_output(&combined, duration_ms);
        results.run_succeeded = true; // We got output, run worked
        if !output.status.success() {
            force_failure_on_nonzero_exit(&mut results, "pytest", output.status.code(), &combined);
        }

        // Log summary
        if results.all_passed() {
            log::info!("✅ Tests passed: {}/{}", results.passed, results.total);
        } else {
            log::info!(
                "❌ Tests failed: {} passed, {} failed",
                results.passed,
                results.failed
            );
        }

        Ok(results)
    }

    /// Run pytest on specific test files
    pub async fn run_test_files(&self, test_files: &[&Path]) -> Result<TestResults> {
        let file_args: Vec<&str> = test_files.iter().filter_map(|p| p.to_str()).collect();

        self.run_pytest(&file_args).await
    }

    /// Parse pytest output into TestResults
    fn parse_pytest_output(&self, output: &str, duration_ms: u64) -> TestResults {
        let mut results = TestResults {
            duration_ms,
            output: output.to_string(),
            ..Default::default()
        };

        // Parse summary line: "X passed, Y failed, Z skipped in 0.12s"
        for line in output.lines() {
            let line = line.trim();

            // Look for summary patterns (usually starts with = signs)
            if (line.contains("passed") || line.contains("failed") || line.contains("error"))
                && (line.contains(" in ") || line.starts_with('='))
            {
                let parts: Vec<&str> = line.split_whitespace().collect();
                for i in 0..parts.len() {
                    if parts[i] == "passed" || parts[i] == "passed," {
                        if i > 0 {
                            if let Ok(n) = parts[i - 1].trim_matches(',').parse::<usize>() {
                                results.passed = n;
                            }
                        }
                    } else if parts[i] == "failed" || parts[i] == "failed," {
                        if i > 0 {
                            if let Ok(n) = parts[i - 1].trim_matches(',').parse::<usize>() {
                                results.failed = n;
                            }
                        }
                    } else if parts[i] == "skipped" || parts[i] == "skipped," {
                        if i > 0 {
                            if let Ok(n) = parts[i - 1].trim_matches(',').parse::<usize>() {
                                results.skipped = n;
                            }
                        }
                    } else if (parts[i] == "error" || parts[i] == "errors") && i > 0 {
                        if let Ok(n) = parts[i - 1].trim_matches(',').parse::<usize>() {
                            results.failed += n;
                        }
                    }
                }
            }

            // Parse individual test failures
            // "FAILED test_file.py::TestClass::test_method - AssertionError"
            if line.starts_with("FAILED ") {
                let failure = self.parse_failure_line(line);
                results.failures.push(failure);
            }
        }

        results.total = results.passed + results.failed + results.skipped;
        results
    }

    /// Parse a pytest FAILED line
    fn parse_failure_line(&self, line: &str) -> TestFailure {
        // Format: "FAILED test_file.py::TestClass::test_method - Error message"
        let rest = line.strip_prefix("FAILED ").unwrap_or(line);

        let (test_path, message) = if let Some(idx) = rest.find(" - ") {
            (&rest[..idx], rest[idx + 3..].to_string())
        } else {
            (rest, String::new())
        };

        // Parse test path (file::class::method or file::method)
        let parts: Vec<&str> = test_path.split("::").collect();
        let (file, name) = if parts.len() >= 2 {
            (
                Some(parts[0].to_string()),
                parts.last().unwrap_or(&"").to_string(),
            )
        } else {
            (None, test_path.to_string())
        };

        TestFailure {
            name,
            file,
            line: None,
            message,
            criticality: Criticality::High, // Default, will be updated by match_weighted_tests
        }
    }

    /// Calculate V_log (Logic Energy) from test results and behavioral contract
    /// Uses weighted tests from the contract to determine criticality
    pub fn calculate_v_log(&self, results: &TestResults, contract: &BehavioralContract) -> f32 {
        let gamma = contract.gamma(); // Default 2.0
        let mut v_log = 0.0;

        for failure in &results.failures {
            // Find matching weighted test from contract
            let weight = contract
                .weighted_tests
                .iter()
                .find(|wt| {
                    failure.name.contains(&wt.test_name) || wt.test_name.contains(&failure.name)
                })
                .map(|wt| wt.criticality.weight())
                .unwrap_or(Criticality::High.weight()); // Default to High if no match

            v_log += gamma * weight;
        }

        v_log
    }

    /// Match test failures with weighted tests from contract to set criticality
    pub fn match_weighted_tests(&self, results: &mut TestResults, contract: &BehavioralContract) {
        for failure in &mut results.failures {
            if let Some(wt) = contract.weighted_tests.iter().find(|wt| {
                failure.name.contains(&wt.test_name) || wt.test_name.contains(&failure.name)
            }) {
                failure.criticality = wt.criticality;
            }
        }
    }
}

// =============================================================================
// PSP-5: Generic Test Runner Trait
// =============================================================================

/// PSP-5: Language-agnostic test runner trait
///
/// Allows the orchestrator to run verification steps through any language's
/// toolchain without hardcoding Python paths.
#[async_trait::async_trait]
pub trait TestRunnerTrait: Send + Sync {
    /// Run syntax/type check (e.g., `cargo check`, `uv run ty check .`)
    async fn run_syntax_check(&self) -> Result<TestResults>;

    /// Run the test suite (e.g., `cargo test`, `uv run pytest`)
    async fn run_tests(&self) -> Result<TestResults>;

    /// Run build check (e.g., `cargo build`)
    async fn run_build_check(&self) -> Result<TestResults>;

    /// Run lint check (e.g., `cargo clippy`, `uv run ruff check .`)
    ///
    /// Default: returns a no-op pass for plugins without a lint stage.
    async fn run_lint(&self) -> Result<TestResults> {
        Ok(TestResults {
            passed: 1,
            total: 1,
            run_succeeded: true,
            output: "No lint stage configured".to_string(),
            ..Default::default()
        })
    }

    /// Run a specific verifier stage by enum variant.
    ///
    /// Dispatches to the appropriate method. Convenience for generic callers.
    async fn run_stage(&self, stage: VerifierStage) -> Result<TestResults> {
        match stage {
            VerifierStage::SyntaxCheck => self.run_syntax_check().await,
            VerifierStage::Build => self.run_build_check().await,
            VerifierStage::Test => self.run_tests().await,
            VerifierStage::Lint => self.run_lint().await,
        }
    }

    /// Name of the runner (for logging)
    fn name(&self) -> &str;
}

#[async_trait::async_trait]
impl TestRunnerTrait for PythonTestRunner {
    async fn run_syntax_check(&self) -> Result<TestResults> {
        // Use ty (via uv) for type checking
        let output = Command::new("uv")
            .args(["run", "ty", "check", "."])
            .current_dir(&self.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to run ty check")?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(TestResults {
            passed: if output.status.success() { 1 } else { 0 },
            failed: if output.status.success() { 0 } else { 1 },
            total: 1,
            run_succeeded: true,
            output: format!("{}\n{}", stdout, stderr),
            ..Default::default()
        })
    }

    async fn run_tests(&self) -> Result<TestResults> {
        self.run_pytest(&[]).await
    }

    async fn run_build_check(&self) -> Result<TestResults> {
        // Python doesn't have a separate build step
        Ok(TestResults {
            passed: 1,
            total: 1,
            run_succeeded: true,
            output: "No build step for Python".to_string(),
            ..Default::default()
        })
    }

    async fn run_lint(&self) -> Result<TestResults> {
        let output = Command::new("uv")
            .args(["run", "ruff", "check", "."])
            .current_dir(&self.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to run ruff check")?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(TestResults {
            passed: if output.status.success() { 1 } else { 0 },
            failed: if output.status.success() { 0 } else { 1 },
            total: 1,
            run_succeeded: true,
            output: format!("{}\n{}", stdout, stderr),
            ..Default::default()
        })
    }

    fn name(&self) -> &str {
        "python"
    }
}

/// PSP-5: Rust test runner using cargo
pub struct RustTestRunner {
    /// Working directory (workspace root)
    working_dir: PathBuf,
}

impl RustTestRunner {
    /// Create a new Rust test runner
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }

    /// Parse `cargo test` output for pass/fail counts
    fn parse_cargo_test_output(&self, output: &str) -> TestResults {
        let mut results = TestResults {
            output: output.to_string(),
            run_succeeded: true,
            ..Default::default()
        };

        for line in output.lines() {
            let line = line.trim();

            // Parse "test result: ok. X passed; Y failed; Z ignored"
            if line.starts_with("test result:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                for i in 0..parts.len() {
                    if (parts[i] == "passed;" || parts[i] == "passed") && i > 0 {
                        if let Ok(n) = parts[i - 1].parse::<usize>() {
                            results.passed = n;
                        }
                    } else if (parts[i] == "failed;" || parts[i] == "failed") && i > 0 {
                        if let Ok(n) = parts[i - 1].parse::<usize>() {
                            results.failed = n;
                        }
                    } else if (parts[i] == "ignored;" || parts[i] == "ignored") && i > 0 {
                        if let Ok(n) = parts[i - 1].parse::<usize>() {
                            results.skipped = n;
                        }
                    }
                }
            }
        }

        results.total = results.passed + results.failed + results.skipped;
        results
    }
}

#[async_trait::async_trait]
impl TestRunnerTrait for RustTestRunner {
    async fn run_syntax_check(&self) -> Result<TestResults> {
        let output = Command::new("cargo")
            .args(["check"])
            .current_dir(&self.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to run cargo check")?;

        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(TestResults {
            passed: if output.status.success() { 1 } else { 0 },
            failed: if output.status.success() { 0 } else { 1 },
            total: 1,
            run_succeeded: true,
            output: stderr,
            ..Default::default()
        })
    }

    async fn run_tests(&self) -> Result<TestResults> {
        let output = Command::new("cargo")
            .args(["test"])
            .current_dir(&self.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to run cargo test")?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let combined = format!("{}\n{}", stdout, stderr);

        let mut results = self.parse_cargo_test_output(&combined);
        results.run_succeeded = true;
        if !output.status.success() {
            force_failure_on_nonzero_exit(&mut results, "cargo test", output.status.code(), &combined);
        }
        Ok(results)
    }

    async fn run_build_check(&self) -> Result<TestResults> {
        let output = Command::new("cargo")
            .args(["build"])
            .current_dir(&self.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to run cargo build")?;

        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(TestResults {
            passed: if output.status.success() { 1 } else { 0 },
            failed: if output.status.success() { 0 } else { 1 },
            total: 1,
            run_succeeded: true,
            output: stderr,
            ..Default::default()
        })
    }

    async fn run_lint(&self) -> Result<TestResults> {
        let output = Command::new("cargo")
            .args(["clippy", "--", "-D", "warnings"])
            .current_dir(&self.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to run cargo clippy")?;

        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(TestResults {
            passed: if output.status.success() { 1 } else { 0 },
            failed: if output.status.success() { 0 } else { 1 },
            total: 1,
            run_succeeded: true,
            output: stderr,
            ..Default::default()
        })
    }

    fn name(&self) -> &str {
        "rust"
    }
}

// =============================================================================
// PSP-5 Phase 4: Plugin-Driven Verifier Runner
// =============================================================================

/// Generic verifier runner driven by a plugin's `VerifierProfile`.
///
/// Instead of hardcoding language-specific commands, this runner reads the
/// profile's `VerifierCapability` entries and executes the best available
/// command (primary → fallback → skip) for each stage.
///
/// For languages with detailed output parsers (e.g., pytest, cargo test),
/// prefer the language-specific runners. `PluginVerifierRunner` is the
/// fallback for plugins that don't have a dedicated runner or when the
/// orchestrator wants uniform dispatch across all detected plugins.
pub struct PluginVerifierRunner {
    /// Working directory for command execution.
    working_dir: PathBuf,
    /// Snapshot of the plugin's verifier capabilities.
    profile: VerifierProfile,
}

impl PluginVerifierRunner {
    /// Create a new runner from a plugin's verifier profile.
    pub fn new(working_dir: PathBuf, profile: VerifierProfile) -> Self {
        Self {
            working_dir,
            profile,
        }
    }

    /// Execute a shell command string, returning a `TestResults`.
    ///
    /// The command is split on whitespace for arg parsing. This is
    /// intentionally simple; complex pipelines should use `sh -c`.
    ///
    /// PSP-5 Phase 4: Commands pass through policy sanitization and
    /// workspace-bound validation before execution.
    async fn exec_command(&self, command: &str, stage: VerifierStage) -> Result<TestResults> {
        // Sanitize command through policy
        let sr = perspt_policy::sanitize_command(command)?;
        if sr.rejected {
            anyhow::bail!(
                "{} command rejected by policy: {}",
                stage,
                sr.rejection_reason.unwrap_or_default()
            );
        }
        for warning in &sr.warnings {
            log::warn!(
                "[{}] policy warning for {} stage: {}",
                self.profile.plugin_name,
                stage,
                warning
            );
        }

        // Validate workspace bounds
        perspt_policy::validate_workspace_bound(command, &self.working_dir)?;

        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            anyhow::bail!("empty command for stage {}", stage);
        }

        let program = parts[0];
        let args = &parts[1..];

        log::info!(
            "[{}] running {} stage: {}",
            self.profile.plugin_name,
            stage,
            command
        );

        let output = Command::new(program)
            .args(args)
            .current_dir(&self.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .with_context(|| format!("Failed to run {} for {} stage", command, stage))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(TestResults {
            passed: if output.status.success() { 1 } else { 0 },
            failed: if output.status.success() { 0 } else { 1 },
            total: 1,
            run_succeeded: true,
            output: format!("{}\n{}", stdout, stderr),
            ..Default::default()
        })
    }

    /// Run a verifier stage using the profile's best available command.
    ///
    /// Returns a no-op pass if the stage is not declared or has no available tool.
    async fn run_profile_stage(&self, stage: VerifierStage) -> Result<TestResults> {
        let cap = match self.profile.get(stage) {
            Some(c) => c,
            None => {
                return Ok(TestResults {
                    passed: 1,
                    total: 1,
                    run_succeeded: true,
                    output: format!(
                        "No {} stage declared for {}",
                        stage, self.profile.plugin_name
                    ),
                    ..Default::default()
                });
            }
        };

        match cap.effective_command() {
            Some(cmd) => self.exec_command(cmd, stage).await,
            None => {
                log::warn!(
                    "[{}] {} stage declared but no tool available (degraded)",
                    self.profile.plugin_name,
                    stage
                );
                Ok(TestResults {
                    passed: 0,
                    failed: 0,
                    total: 0,
                    run_succeeded: false,
                    output: format!(
                        "{} stage skipped: no tool available for {}",
                        stage, self.profile.plugin_name
                    ),
                    ..Default::default()
                })
            }
        }
    }

    /// Run all available stages in order, returning results keyed by stage.
    pub async fn run_all_stages(&self) -> Vec<(VerifierStage, Result<TestResults>)> {
        let stages = [
            VerifierStage::SyntaxCheck,
            VerifierStage::Build,
            VerifierStage::Test,
            VerifierStage::Lint,
        ];
        let mut results = Vec::new();
        for stage in stages {
            if self.profile.get(stage).is_some() {
                results.push((stage, self.run_profile_stage(stage).await));
            }
        }
        results
    }

    /// Get the underlying profile.
    pub fn profile(&self) -> &VerifierProfile {
        &self.profile
    }
}

#[async_trait::async_trait]
impl TestRunnerTrait for PluginVerifierRunner {
    async fn run_syntax_check(&self) -> Result<TestResults> {
        self.run_profile_stage(VerifierStage::SyntaxCheck).await
    }

    async fn run_tests(&self) -> Result<TestResults> {
        self.run_profile_stage(VerifierStage::Test).await
    }

    async fn run_build_check(&self) -> Result<TestResults> {
        self.run_profile_stage(VerifierStage::Build).await
    }

    async fn run_lint(&self) -> Result<TestResults> {
        self.run_profile_stage(VerifierStage::Lint).await
    }

    fn name(&self) -> &str {
        &self.profile.plugin_name
    }
}

/// PSP-5: Factory function to create a test runner for a given plugin
pub fn test_runner_for_plugin(plugin_name: &str, working_dir: PathBuf) -> Box<dyn TestRunnerTrait> {
    match plugin_name {
        "rust" => Box::new(RustTestRunner::new(working_dir)),
        "python" => Box::new(PythonTestRunner::new(working_dir)),
        _ => Box::new(PythonTestRunner::new(working_dir)), // Default fallback
    }
}

/// PSP-5 Phase 4: Create a runner from a verifier profile.
///
/// For Rust and Python, this returns the specialised runner (which has
/// detailed output parsing). For anything else it returns a generic
/// `PluginVerifierRunner` that executes whatever commands the profile declares.
pub fn test_runner_for_profile(
    profile: VerifierProfile,
    working_dir: PathBuf,
) -> Box<dyn TestRunnerTrait> {
    match profile.plugin_name.as_str() {
        "rust" => Box::new(RustTestRunner::new(working_dir)),
        "python" => Box::new(PythonTestRunner::new(working_dir)),
        _ => Box::new(PluginVerifierRunner::new(working_dir, profile)),
    }
}

// Re-export PythonTestRunner as TestRunner for backward compatibility
pub type TestRunner = PythonTestRunner;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::WeightedTest;
    use perspt_core::plugin::{
        LanguagePlugin, LspCapability, LspConfig, VerifierCapability, VerifierProfile,
    };

    #[test]
    fn test_parse_pytest_summary() {
        let runner = PythonTestRunner::new(PathBuf::from("."));

        let output = "===== 3 passed, 2 failed, 1 skipped in 0.12s =====";
        let results = runner.parse_pytest_output(output, 120);

        assert_eq!(results.passed, 3);
        assert_eq!(results.failed, 2);
        assert_eq!(results.skipped, 1);
        assert_eq!(results.total, 6);
    }

    #[test]
    fn test_parse_pytest_failure_line() {
        let runner = PythonTestRunner::new(PathBuf::from("."));

        let line = "FAILED test_calculator.py::TestDivide::test_divide_by_zero - ZeroDivisionError";
        let failure = runner.parse_failure_line(line);

        assert_eq!(failure.name, "test_divide_by_zero");
        assert_eq!(failure.file, Some("test_calculator.py".to_string()));
        assert!(failure.message.contains("ZeroDivisionError"));
    }

    #[test]
    fn test_force_failure_on_nonzero_exit_marks_failure() {
        let mut results = TestResults::default();

        force_failure_on_nonzero_exit(&mut results, "pytest", Some(2), "collection error");

        assert_eq!(results.failed, 1);
        assert_eq!(results.total, 1);
        assert_eq!(results.failures.len(), 1);
        assert!(results.failures[0].message.contains("collection error"));
    }

    #[test]
    fn test_calculate_v_log() {
        let runner = PythonTestRunner::new(PathBuf::from("."));

        let results = TestResults {
            failures: vec![TestFailure {
                name: "test_critical_feature".to_string(),
                file: None,
                line: None,
                message: String::new(),
                criticality: Criticality::Critical,
            }],
            ..Default::default()
        };

        let mut contract = BehavioralContract::new();
        contract.weighted_tests = vec![WeightedTest {
            test_name: "test_critical_feature".to_string(),
            criticality: Criticality::Critical,
        }];

        let v_log = runner.calculate_v_log(&results, &contract);
        // gamma (2.0) * Critical weight (10.0) = 20.0
        assert!((v_log - 20.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_cargo_test_output() {
        let runner = RustTestRunner::new(PathBuf::from("."));

        let output = r#"
running 5 tests
test tests::test_add ... ok
test tests::test_sub ... ok
test tests::test_mul ... FAILED
test tests::test_div ... ok
test tests::test_rem ... ignored

test result: ok. 3 passed; 1 failed; 1 ignored; 0 measured; 0 filtered out
"#;
        let results = runner.parse_cargo_test_output(output);
        assert_eq!(results.passed, 3);
        assert_eq!(results.failed, 1);
        assert_eq!(results.skipped, 1);
        assert_eq!(results.total, 5);
    }

    #[test]
    fn test_runner_for_plugin_factory() {
        let rust_runner = test_runner_for_plugin("rust", PathBuf::from("."));
        assert_eq!(rust_runner.name(), "rust");

        let python_runner = test_runner_for_plugin("python", PathBuf::from("."));
        assert_eq!(python_runner.name(), "python");

        // Unknown falls back to Python
        let fallback = test_runner_for_plugin("go", PathBuf::from("."));
        assert_eq!(fallback.name(), "python");
    }

    // =========================================================================
    // PluginVerifierRunner tests
    // =========================================================================

    fn make_test_profile(name: &str, caps: Vec<VerifierCapability>) -> VerifierProfile {
        VerifierProfile {
            plugin_name: name.to_string(),
            capabilities: caps,
            lsp: LspCapability {
                primary: LspConfig {
                    server_binary: "test-ls".to_string(),
                    args: vec![],
                    language_id: name.to_string(),
                },
                primary_available: false,
                fallback: None,
                fallback_available: false,
            },
        }
    }

    #[test]
    fn test_plugin_verifier_runner_name() {
        let profile = make_test_profile("go", vec![]);
        let runner = PluginVerifierRunner::new(PathBuf::from("."), profile);
        assert_eq!(runner.name(), "go");
    }

    #[tokio::test]
    async fn test_plugin_verifier_runner_no_stage_declared() {
        // When no capability is declared for a stage, run_stage returns a no-op pass
        let profile = make_test_profile("go", vec![]);
        let runner = PluginVerifierRunner::new(PathBuf::from("."), profile);
        let result = runner.run_syntax_check().await.unwrap();
        assert_eq!(result.passed, 1);
        assert_eq!(result.total, 1);
        assert!(result.output.contains("No syntax_check stage"));
    }

    #[tokio::test]
    async fn test_plugin_verifier_runner_no_tool_available() {
        // Stage is declared but neither primary nor fallback tool is available
        let profile = make_test_profile(
            "go",
            vec![VerifierCapability {
                stage: VerifierStage::Build,
                command: Some("go build ./...".to_string()),
                available: false,
                fallback_command: None,
                fallback_available: false,
            }],
        );
        let runner = PluginVerifierRunner::new(PathBuf::from("."), profile);
        let result = runner.run_build_check().await.unwrap();
        assert!(!result.run_succeeded);
        assert!(result.output.contains("no tool available"));
    }

    #[tokio::test]
    async fn test_plugin_verifier_runner_echo_command() {
        // Use `echo` as a trivially-available command to test real execution
        let profile = make_test_profile(
            "echo-lang",
            vec![VerifierCapability {
                stage: VerifierStage::SyntaxCheck,
                command: Some("echo syntax-ok".to_string()),
                available: true,
                fallback_command: None,
                fallback_available: false,
            }],
        );
        let runner = PluginVerifierRunner::new(PathBuf::from("."), profile);
        let result = runner.run_syntax_check().await.unwrap();
        assert_eq!(result.passed, 1);
        assert!(result.run_succeeded);
        assert!(result.output.contains("syntax-ok"));
    }

    #[tokio::test]
    async fn test_plugin_verifier_runner_run_all_stages() {
        let profile = make_test_profile(
            "echo-lang",
            vec![
                VerifierCapability {
                    stage: VerifierStage::SyntaxCheck,
                    command: Some("echo check".to_string()),
                    available: true,
                    fallback_command: None,
                    fallback_available: false,
                },
                VerifierCapability {
                    stage: VerifierStage::Lint,
                    command: Some("echo lint".to_string()),
                    available: true,
                    fallback_command: None,
                    fallback_available: false,
                },
            ],
        );
        let runner = PluginVerifierRunner::new(PathBuf::from("."), profile);
        let results = runner.run_all_stages().await;
        // Only the 2 declared stages should appear
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, VerifierStage::SyntaxCheck);
        assert_eq!(results[1].0, VerifierStage::Lint);
        assert!(results[0].1.is_ok());
        assert!(results[1].1.is_ok());
    }

    #[test]
    fn test_runner_for_profile_factory() {
        use perspt_core::plugin::RustPlugin;
        // Known plugins get specialised runners
        let rust_profile = RustPlugin.verifier_profile();
        let runner = test_runner_for_profile(rust_profile, PathBuf::from("."));
        assert_eq!(runner.name(), "rust");

        // Unknown plugins get PluginVerifierRunner
        let custom = make_test_profile("go", vec![]);
        let runner = test_runner_for_profile(custom, PathBuf::from("."));
        assert_eq!(runner.name(), "go");
    }

    #[tokio::test]
    async fn test_exec_command_rejects_dangerous_pattern() {
        let profile = make_test_profile(
            "danger",
            vec![VerifierCapability {
                stage: VerifierStage::SyntaxCheck,
                command: Some("rm -rf /".to_string()),
                available: true,
                fallback_command: None,
                fallback_available: false,
            }],
        );
        let runner = PluginVerifierRunner::new(PathBuf::from("/tmp"), profile);
        let result = runner.run_syntax_check().await;
        // The command should be rejected by policy sanitisation
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_exec_command_rejects_workspace_escape() {
        let profile = make_test_profile(
            "escape",
            vec![VerifierCapability {
                stage: VerifierStage::SyntaxCheck,
                command: Some("cat /etc/passwd".to_string()),
                available: true,
                fallback_command: None,
                fallback_available: false,
            }],
        );
        let runner = PluginVerifierRunner::new(PathBuf::from("/home/user/project"), profile);
        let result = runner.run_syntax_check().await;
        // The command references a path outside the workspace
        assert!(result.is_err());
    }

    #[test]
    fn test_fallback_command_selected_when_primary_unavailable() {
        let cap = VerifierCapability {
            stage: VerifierStage::Test,
            command: Some("uv run pytest".to_string()),
            available: false,
            fallback_command: Some("python -m pytest".to_string()),
            fallback_available: true,
        };
        assert_eq!(cap.effective_command(), Some("python -m pytest"));
    }
}
