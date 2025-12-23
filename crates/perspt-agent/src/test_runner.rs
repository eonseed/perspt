//! Python Test Runner
//!
//! Executes pytest in Python workspaces using `uv` as the package manager.
//! Handles project setup (pyproject.toml) and test execution for V_log calculation.
//!
//! Future phases will add support for other languages (Rust, JavaScript, etc.)

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

use crate::types::{BehavioralContract, Criticality};

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

    /// Create a minimal pyproject.toml with pytest dependency
    pub async fn create_pyproject(&self) -> Result<()> {
        let pyproject_path = self.working_dir.join("pyproject.toml");

        if pyproject_path.exists() {
            log::debug!("pyproject.toml already exists");
            return Ok(());
        }

        log::info!("Creating minimal pyproject.toml with pytest");

        let content = r#"[project]
name = "workspace"
version = "0.1.0"
requires-python = ">=3.10"
dependencies = []

[project.optional-dependencies]
dev = ["pytest>=8.0"]

[tool.pytest.ini_options]
testpaths = ["tests", "."]
python_files = ["test_*.py", "*_test.py"]
python_functions = ["test_*"]
"#;

        tokio::fs::write(&pyproject_path, content)
            .await
            .context("Failed to write pyproject.toml")?;

        Ok(())
    }

    /// Initialize the Python environment with uv
    pub async fn setup_environment(&self) -> Result<()> {
        log::info!("Setting up Python environment with uv");

        // Create pyproject.toml if needed
        if self.auto_setup && !self.has_pyproject() {
            self.create_pyproject().await?;
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
        println!("   🧪 Running tests...");

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

        // Print summary
        if results.all_passed() {
            println!("   ✅ Tests passed: {}/{}", results.passed, results.total);
        } else {
            println!(
                "   ❌ Tests failed: {} passed, {} failed",
                results.passed, results.failed
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

// Re-export PythonTestRunner as TestRunner for now
// In future phases, we'll add a generic TestRunner trait
pub type TestRunner = PythonTestRunner;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::WeightedTest;

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
}
