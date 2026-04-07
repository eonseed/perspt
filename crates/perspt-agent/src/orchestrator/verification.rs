//! Stability verification, plugin-driven checks, and dependency auto-installation.

use super::*;

impl SRBNOrchestrator {
    /// Step 4: Stability Verification
    ///
    /// Computes Lyapunov Energy V(x) from LSP diagnostics, contracts, and tests
    pub(super) async fn step_verify(&mut self, idx: NodeIndex) -> Result<EnergyComponents> {
        log::info!("Step 4: Verification - Computing stability energy");

        // Clear stale verification result from previous nodes to prevent
        // cross-node data leakage into sheaf validators.
        self.last_verification_result = None;
        // Clear stale test output so the correction prompt doesn't show
        // results from a previous node's verification run.
        self.context.last_test_output = None;

        self.graph[idx].state = NodeState::Verifying;
        self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
            node_id: self.graph[idx].node_id.clone(),
            status: perspt_core::NodeStatus::Verifying,
        });

        // Calculate energy components
        let mut energy = EnergyComponents::default();

        // V_syn: From Tool Failures (Critical)
        if let Some(ref err) = self.last_tool_failure {
            energy.v_syn = 10.0; // High energy for tool failure
            log::warn!("Tool failure detected, V_syn set to 10.0: {}", err);
            self.emit_log(format!("⚠️ Tool failure prevents verification: {}", err));
            // We can return early or allow other checks. Usually tool failure means broken state.

            // Store diagnostics mock for correction prompt
            self.context.last_diagnostics = vec![lsp_types::Diagnostic {
                range: lsp_types::Range::default(),
                severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                code: None,
                code_description: None,
                source: Some("tool".to_string()),
                message: format!("Failed to apply changes: {}", err),
                related_information: None,
                tags: None,
                data: None,
            }];
        }

        // V_syn: From LSP diagnostics
        if let Some(ref path) = self.last_written_file {
            // PSP-5 Phase 4: look up LSP client by the node's owner_plugin
            let node_plugin = self.graph[idx].owner_plugin.clone();
            let lsp_key = if node_plugin.is_empty() || node_plugin == "unknown" {
                "python".to_string() // legacy fallback
            } else {
                node_plugin
            };

            if let Some(client) = self.lsp_clients.get(&lsp_key) {
                // Small delay to let LSP analyze the file
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

                let path_str = path.to_string_lossy().to_string();
                let diagnostics = client.get_diagnostics(&path_str).await;

                if !diagnostics.is_empty() {
                    energy.v_syn = LspClient::calculate_syntactic_energy(&diagnostics);
                    log::info!(
                        "LSP found {} diagnostics, V_syn={:.2}",
                        diagnostics.len(),
                        energy.v_syn
                    );
                    self.emit_log(format!("🔍 LSP found {} diagnostics:", diagnostics.len()));
                    for d in &diagnostics {
                        self.emit_log(format!(
                            "   - [{}] {}",
                            severity_to_str(d.severity),
                            d.message
                        ));
                    }

                    // Store diagnostics for correction prompt
                    self.context.last_diagnostics = diagnostics;
                } else {
                    log::info!("LSP reports no errors (diagnostics vector is empty)");
                }
            } else {
                log::debug!("No LSP client available for plugin '{}'", lsp_key);
            }

            // V_str: Check forbidden patterns in written file
            let node = &self.graph[idx];
            if !node.contract.forbidden_patterns.is_empty() {
                if let Ok(content) = std::fs::read_to_string(path) {
                    for pattern in &node.contract.forbidden_patterns {
                        if content.contains(pattern) {
                            energy.v_str += 0.5;
                            log::warn!("Forbidden pattern found: '{}'", pattern);
                            self.emit_log(format!("⚠️ Forbidden pattern: '{}'", pattern));
                        }
                    }
                }
            }

            // PSP-5 Phase 9: Universal plugin verification for all node classes.
            // Replaces the old weighted-test-only block that only ran for Integration nodes.
            let node = &self.graph[idx];
            if self.context.defer_tests {
                self.emit_log("⏭️ Tests deferred (--defer-tests enabled)".to_string());
            } else {
                let plugin_name = node.owner_plugin.clone();
                let verify_dir = self.effective_working_dir(idx);
                let stages = verification_stages_for_node(node);

                if !stages.is_empty() && !plugin_name.is_empty() && plugin_name != "unknown" {
                    // Proactive dependency installation: install packages declared
                    // in the architect's dependency_expectations before running
                    // verification so the first build attempt has a better chance
                    // of succeeding without reactive auto-repair.
                    let dep_exp = node.dependency_expectations.clone();
                    if !dep_exp.required_packages.is_empty() {
                        self.emit_log(format!(
                            "📦 Pre-installing declared dependencies: {}",
                            dep_exp.required_packages.join(", ")
                        ));
                        let installed = if plugin_name == "python" {
                            Self::auto_install_python_deps(&dep_exp.required_packages, &verify_dir)
                                .await
                        } else {
                            Self::auto_install_crate_deps(&dep_exp.required_packages, &verify_dir)
                                .await
                        };
                        if installed > 0 {
                            self.emit_log(format!(
                                "📦 Pre-installed {} declared package(s)",
                                installed
                            ));
                        }
                    }

                    self.emit_log(format!(
                        "🔬 Running verification ({} stages) for {} node '{}'...",
                        stages.len(),
                        node.node_class,
                        node.node_id
                    ));

                    let mut vr = self
                        .run_plugin_verification(&plugin_name, &stages, verify_dir.clone())
                        .await;

                    // Auto-dependency repair: if syntax/build failed, check if the
                    // root cause is missing crate dependencies and auto-install them.
                    if !vr.syntax_ok || !vr.build_ok {
                        if let Some(ref raw) = vr.raw_output {
                            let missing = Self::extract_missing_crates(raw);
                            if !missing.is_empty() {
                                self.emit_log(format!(
                                    "📦 Auto-installing missing dependencies: {}",
                                    missing.join(", ")
                                ));
                                let dep_ok =
                                    Self::auto_install_crate_deps(&missing, &verify_dir).await;
                                if dep_ok > 0 {
                                    self.emit_log(format!(
                                        "📦 Installed {} crate(s), re-running verification...",
                                        dep_ok
                                    ));
                                    // Re-run verification now that deps are installed
                                    vr = self
                                        .run_plugin_verification(
                                            &plugin_name,
                                            &stages,
                                            verify_dir.clone(),
                                        )
                                        .await;
                                }
                            }
                        }
                    }

                    // Auto-dependency repair for Python: parse
                    // ModuleNotFoundError / ImportError from test output and
                    // install missing packages via `uv add`.
                    if plugin_name == "python" && (!vr.syntax_ok || !vr.tests_ok) {
                        // Collect raw output from all stage outcomes for
                        // broader error detection (syntax check may report
                        // import errors too).
                        let all_output: String = vr
                            .stage_outcomes
                            .iter()
                            .filter_map(|so| so.output.as_deref())
                            .collect::<Vec<_>>()
                            .join("\n");
                        let combined = match vr.raw_output.as_deref() {
                            Some(raw) => format!("{}\n{}", raw, all_output),
                            None => all_output,
                        };

                        let missing = Self::extract_missing_python_modules(&combined);
                        if !missing.is_empty() {
                            self.emit_log(format!(
                                "🐍 Auto-installing missing Python packages: {}",
                                missing.join(", ")
                            ));
                            let dep_ok =
                                Self::auto_install_python_deps(&missing, &verify_dir).await;
                            if dep_ok > 0 {
                                self.emit_log(format!(
                                    "🐍 Installed {} package(s), re-running verification...",
                                    dep_ok
                                ));
                                vr = self
                                    .run_plugin_verification(
                                        &plugin_name,
                                        &stages,
                                        verify_dir.clone(),
                                    )
                                    .await;
                            }
                        }
                    }

                    // Map verification result to energy components:
                    // - Syntax fail → V_syn (cap at 5.0, don't override tool-failure 10.0)
                    if !vr.syntax_ok && energy.v_syn < 5.0 {
                        energy.v_syn = 5.0;
                    }
                    // - Build fail → V_syn (cap at 8.0, don't override higher)
                    if !vr.build_ok && energy.v_syn < 8.0 {
                        energy.v_syn = 8.0;
                    }
                    // - Test fail → V_log (weighted calculation)
                    if !vr.tests_ok && vr.tests_failed > 0 {
                        let node = &self.graph[idx];
                        if !node.contract.weighted_tests.is_empty() {
                            // Use weighted test calculation if contract has weights
                            let py_runner = PythonTestRunner::new(verify_dir);
                            let test_results = TestResults {
                                passed: vr.tests_passed,
                                failed: vr.tests_failed,
                                total: vr.tests_passed + vr.tests_failed,
                                output: vr.raw_output.clone().unwrap_or_default(),
                                failures: Vec::new(),
                                run_succeeded: true,
                                skipped: 0,
                                duration_ms: 0,
                            };
                            energy.v_log = py_runner.calculate_v_log(&test_results, &node.contract);
                        } else {
                            // Simple: proportion of failures
                            let total = (vr.tests_passed + vr.tests_failed) as f32;
                            if total > 0.0 {
                                energy.v_log = (vr.tests_failed as f32 / total) * 10.0;
                            }
                        }
                    }
                    // - Tests were expected but never ran (e.g. test compilation
                    //   failed or test stage was skipped) → treat as build failure.
                    //   Without this, nodes with broken test files get V=0 and
                    //   pass verification erroneously.
                    if !vr.tests_ok
                        && vr.tests_failed == 0
                        && vr.tests_passed == 0
                        && stages.contains(&perspt_core::plugin::VerifierStage::Test)
                    {
                        // Check raw output for compilation errors in test targets
                        let test_compile_failed = vr
                            .raw_output
                            .as_deref()
                            .or_else(|| {
                                vr.stage_outcomes
                                    .iter()
                                    .find(|so| so.stage == "test")
                                    .and_then(|so| so.output.as_deref())
                            })
                            .is_some_and(|o| {
                                o.contains("error[E")
                                    || o.contains("could not compile")
                                    || o.contains("FAILED")
                                    || o.contains("ModuleNotFoundError")
                                    || o.contains("ImportError")
                            });
                        if test_compile_failed {
                            log::warn!(
                                "Test compilation failed for node '{}' — treating as build failure",
                                self.graph[idx].node_id
                            );
                            if energy.v_syn < 8.0 {
                                energy.v_syn = 8.0;
                            }
                        } else {
                            // Tests didn't run but no obvious error — moderate penalty
                            energy.v_log = 5.0;
                            log::warn!(
                                "Tests expected but did not produce results for node '{}'",
                                self.graph[idx].node_id
                            );
                        }
                    }
                    // - Lint fail → V_str penalty
                    if !vr.lint_ok
                        && self.context.verifier_strictness
                            == perspt_core::types::VerifierStrictness::Strict
                    {
                        energy.v_str += 0.3;
                    }

                    // PSP-7: V_boot — bootstrap infrastructure failures.
                    // Sensor degradation signals toolchain/environment issues
                    // distinct from code quality captured by V_syn/V_log.
                    // Only computed from the FINAL verification result (after
                    // auto-repair has had its chance to fix missing deps).
                    if vr.degraded && vr.stage_outcomes.is_empty() {
                        // Fully degraded toolchain: no stages ran at all.
                        energy.v_boot = 10.0;
                        log::warn!(
                            "V_boot = 10.0: toolchain fully degraded ({})",
                            vr.degraded_reason.as_deref().unwrap_or("unknown")
                        );
                    }
                    for so in &vr.stage_outcomes {
                        match &so.sensor_status {
                            perspt_core::types::SensorStatus::Unavailable { reason } => {
                                energy.v_boot += 3.0;
                                log::warn!(
                                    "V_boot +3.0: sensor unavailable for stage '{}': {}",
                                    so.stage,
                                    reason
                                );
                            }
                            perspt_core::types::SensorStatus::Fallback { reason, .. } => {
                                energy.v_boot += 1.0;
                                log::info!(
                                    "V_boot +1.0: fallback sensor for stage '{}': {}",
                                    so.stage,
                                    reason
                                );
                            }
                            perspt_core::types::SensorStatus::Available => {}
                        }
                    }

                    // D1: Feed raw output into correction context
                    if let Some(ref raw) = vr.raw_output {
                        self.context.last_test_output = Some(raw.clone());
                    }

                    self.emit_log(format!("📊 Verification: {}", vr.summary));
                }
            }
        }

        let node = &self.graph[idx];
        // Record energy in persistent ledger
        if let Err(e) =
            self.ledger
                .record_energy(&node.node_id, &energy, energy.total(&node.contract))
        {
            log::error!("Failed to record energy: {}", e);
        }

        log::info!(
            "Energy for {}: V_syn={:.2}, V_str={:.2}, V_log={:.2}, V_boot={:.2}, V_sheaf={:.2}, Total={:.2}",
            node.node_id,
            energy.v_syn,
            energy.v_str,
            energy.v_log,
            energy.v_boot,
            energy.v_sheaf,
            energy.total(&node.contract)
        );

        // PSP-5 Phase 7: Emit enriched VerificationComplete event
        {
            let node = &self.graph[idx];
            let total = energy.total(&node.contract);
            let (
                stage_outcomes,
                degraded,
                degraded_reasons,
                summary,
                lint_ok,
                tests_passed,
                tests_failed,
            ) = if let Some(ref vr) = self.last_verification_result {
                (
                    vr.stage_outcomes.clone(),
                    vr.degraded,
                    vr.degraded_stage_reasons(),
                    vr.summary.clone(),
                    vr.lint_ok,
                    vr.tests_passed,
                    vr.tests_failed,
                )
            } else {
                let diag_count = self.context.last_diagnostics.len();
                (
                    Vec::new(),
                    false,
                    Vec::new(),
                    format!("V(x)={:.2} | {} diagnostics", total, diag_count),
                    true,
                    0,
                    0,
                )
            };

            self.emit_event(perspt_core::AgentEvent::VerificationComplete {
                node_id: node.node_id.clone(),
                syntax_ok: energy.v_syn == 0.0,
                build_ok: energy.v_syn < 5.0,
                tests_ok: energy.v_log == 0.0,
                lint_ok,
                diagnostics_count: self.context.last_diagnostics.len(),
                tests_passed,
                tests_failed,
                energy: total,
                energy_components: energy.clone(),
                stage_outcomes,
                degraded,
                degraded_reasons,
                summary,
                node_class: node.node_class.to_string(),
            });
        }

        Ok(energy)
    }

    /// PSP-5: Run plugin-driven verification for a node
    ///
    /// Uses the active language plugin's verifier profile to select commands
    /// for syntax check, build, test, and lint stages. Delegates execution
    /// to `TestRunnerTrait` implementations from `test_runner`.
    ///
    /// Each stage records a `StageOutcome` with `SensorStatus`, enabling
    /// callers to detect fallback / unavailable sensors and block false
    /// stability claims.
    pub async fn run_plugin_verification(
        &mut self,
        plugin_name: &str,
        allowed_stages: &[perspt_core::plugin::VerifierStage],
        working_dir: std::path::PathBuf,
    ) -> perspt_core::types::VerificationResult {
        use perspt_core::plugin::VerifierStage;
        use perspt_core::types::{SensorStatus, StageOutcome};

        let registry = perspt_core::plugin::PluginRegistry::new();
        let plugin = match registry.get(plugin_name) {
            Some(p) => p,
            None => {
                return perspt_core::types::VerificationResult::degraded(format!(
                    "Plugin '{}' not found",
                    plugin_name
                ));
            }
        };

        let profile = plugin.verifier_profile();

        // If fully degraded, report immediately
        if profile.fully_degraded() {
            return perspt_core::types::VerificationResult::degraded(format!(
                "{} toolchain not available on host (all stages degraded)",
                plugin.name()
            ));
        }

        // Derive per-stage sensor status from the profile before moving it.
        let sensor_status_for = |stage: VerifierStage,
                                 profile: &perspt_core::plugin::VerifierProfile|
         -> SensorStatus {
            match profile.get(stage) {
                Some(cap) if cap.available => SensorStatus::Available,
                Some(cap) if cap.fallback_available => SensorStatus::Fallback {
                    actual: cap
                        .fallback_command
                        .clone()
                        .unwrap_or_else(|| "fallback".into()),
                    reason: format!(
                        "primary '{}' not found",
                        cap.command.as_deref().unwrap_or("?")
                    ),
                },
                Some(cap) => SensorStatus::Unavailable {
                    reason: format!(
                        "no tool for {} (tried '{}')",
                        stage,
                        cap.command.as_deref().unwrap_or("?")
                    ),
                },
                None => SensorStatus::Unavailable {
                    reason: format!("{} stage not declared by plugin", stage),
                },
            }
        };

        let syn_sensor = sensor_status_for(VerifierStage::SyntaxCheck, &profile);
        let build_sensor = sensor_status_for(VerifierStage::Build, &profile);
        let test_sensor = sensor_status_for(VerifierStage::Test, &profile);
        let lint_sensor = sensor_status_for(VerifierStage::Lint, &profile);

        let runner = test_runner::test_runner_for_profile(profile, working_dir);

        let mut result = perspt_core::types::VerificationResult::default();

        // PSP-5 Phase 9: Only run stages that are in the allowed filter.
        // Short-circuit: if syntax fails, skip build/test/lint.
        //                if build fails, skip test/lint.

        // Syntax check
        if allowed_stages.contains(&VerifierStage::SyntaxCheck) {
            match runner.run_syntax_check().await {
                Ok(r) => {
                    result.syntax_ok = r.passed > 0 && r.failed == 0;
                    if !result.syntax_ok && r.run_succeeded {
                        result.diagnostics_count = r.output.lines().count();
                        result.raw_output = Some(r.output.clone());
                        self.emit_log(format!(
                            "⚠️ Syntax check failed ({} diagnostics)",
                            result.diagnostics_count
                        ));
                    } else if result.syntax_ok {
                        self.emit_log("✅ Syntax check passed".to_string());
                    }
                    result.stage_outcomes.push(StageOutcome {
                        stage: VerifierStage::SyntaxCheck.to_string(),
                        passed: result.syntax_ok,
                        sensor_status: syn_sensor,
                        output: Some(r.output),
                    });
                }
                Err(e) => {
                    log::warn!("Syntax check failed to run: {}", e);
                    result.syntax_ok = false;
                    result.stage_outcomes.push(StageOutcome {
                        stage: VerifierStage::SyntaxCheck.to_string(),
                        passed: false,
                        sensor_status: SensorStatus::Unavailable {
                            reason: format!("execution error: {}", e),
                        },
                        output: None,
                    });
                }
            }

            // Short-circuit: if syntax fails, skip remaining stages
            if !result.syntax_ok {
                self.emit_log("⏭️ Skipping build/test/lint — syntax check failed".to_string());
                result.build_ok = false;
                result.tests_ok = false;
                self.finalize_verification_result(&mut result, plugin_name);
                return result;
            }
        }

        // Build check
        if allowed_stages.contains(&VerifierStage::Build) {
            match runner.run_build_check().await {
                Ok(r) => {
                    result.build_ok = r.passed > 0 && r.failed == 0;
                    if result.build_ok {
                        self.emit_log("✅ Build passed".to_string());
                    } else if r.run_succeeded {
                        self.emit_log("⚠️ Build failed".to_string());
                        result.raw_output = Some(r.output.clone());
                    }
                    result.stage_outcomes.push(StageOutcome {
                        stage: VerifierStage::Build.to_string(),
                        passed: result.build_ok,
                        sensor_status: build_sensor,
                        output: Some(r.output),
                    });
                }
                Err(e) => {
                    log::warn!("Build check failed to run: {}", e);
                    result.build_ok = false;
                    result.stage_outcomes.push(StageOutcome {
                        stage: VerifierStage::Build.to_string(),
                        passed: false,
                        sensor_status: SensorStatus::Unavailable {
                            reason: format!("execution error: {}", e),
                        },
                        output: None,
                    });
                }
            }

            // Short-circuit: if build fails, skip test/lint
            if !result.build_ok {
                self.emit_log("⏭️ Skipping test/lint — build failed".to_string());
                result.tests_ok = false;
                self.finalize_verification_result(&mut result, plugin_name);
                return result;
            }
        }

        // Tests
        if allowed_stages.contains(&VerifierStage::Test) {
            match runner.run_tests().await {
                Ok(r) => {
                    result.tests_ok = r.all_passed();
                    result.tests_passed = r.passed;
                    result.tests_failed = r.failed;

                    if result.tests_ok {
                        self.emit_log(format!("✅ Tests passed ({})", plugin_name));
                    } else {
                        self.emit_log(format!("❌ Tests failed ({})", plugin_name));
                        result.raw_output = Some(r.output.clone());
                    }
                    result.stage_outcomes.push(StageOutcome {
                        stage: VerifierStage::Test.to_string(),
                        passed: result.tests_ok,
                        sensor_status: test_sensor,
                        output: Some(r.output),
                    });
                }
                Err(e) => {
                    log::warn!("Test command failed to run: {}", e);
                    result.tests_ok = false;
                    result.stage_outcomes.push(StageOutcome {
                        stage: VerifierStage::Test.to_string(),
                        passed: false,
                        sensor_status: SensorStatus::Unavailable {
                            reason: format!("execution error: {}", e),
                        },
                        output: None,
                    });
                }
            }
        } else {
            result.tests_ok = true; // Skip tests when not in allowed stages
        }

        // Lint (only when allowed AND in Strict mode)
        if allowed_stages.contains(&VerifierStage::Lint)
            && self.context.verifier_strictness == perspt_core::types::VerifierStrictness::Strict
        {
            match runner.run_lint().await {
                Ok(r) => {
                    result.lint_ok = r.passed > 0 && r.failed == 0;
                    if result.lint_ok {
                        self.emit_log("✅ Lint passed".to_string());
                    } else if r.run_succeeded {
                        self.emit_log("⚠️ Lint issues found".to_string());
                    }
                    result.stage_outcomes.push(StageOutcome {
                        stage: VerifierStage::Lint.to_string(),
                        passed: result.lint_ok,
                        sensor_status: lint_sensor,
                        output: Some(r.output),
                    });
                }
                Err(e) => {
                    log::warn!("Lint command failed to run: {}", e);
                    result.lint_ok = false;
                    result.stage_outcomes.push(StageOutcome {
                        stage: VerifierStage::Lint.to_string(),
                        passed: false,
                        sensor_status: SensorStatus::Unavailable {
                            reason: format!("execution error: {}", e),
                        },
                        output: None,
                    });
                }
            }
        } else if !allowed_stages.contains(&VerifierStage::Lint) {
            result.lint_ok = true; // Skip lint when not in allowed stages
        } else {
            result.lint_ok = true; // Skip lint in non-strict mode
        }

        self.finalize_verification_result(&mut result, plugin_name);
        result
    }

    // =========================================================================
    // Auto-dependency repair helpers
    // =========================================================================

    /// Parse `cargo check` / `cargo build` stderr and extract crate names that
    /// are missing.  Handles patterns like:
    ///   - `error[E0432]: unresolved import \`thiserror\``
    ///   - `error[E0463]: can't find crate for \`serde\``
    ///   - `use of undeclared crate or module \`clap\``
    fn extract_missing_crates(output: &str) -> Vec<String> {
        use std::collections::HashSet;

        let mut crates: HashSet<String> = HashSet::new();

        for line in output.lines() {
            let lower = line.to_lowercase();

            // Pattern: "use of undeclared crate or module `foo`"
            if lower.contains("undeclared crate or module") {
                if let Some(name) = Self::extract_backtick_ident(line) {
                    if !name.contains("::") {
                        crates.insert(name);
                    }
                }
            }
            // Pattern: "can't find crate for `foo`"
            else if lower.contains("can't find crate for")
                || lower.contains("cant find crate for")
            {
                if let Some(name) = Self::extract_backtick_ident(line) {
                    crates.insert(name);
                }
            }
            // Pattern: "unresolved import `thiserror`" at top level
            else if lower.contains("unresolved import") {
                if let Some(name) = Self::extract_backtick_ident(line) {
                    let root = name.split("::").next().unwrap_or(&name).to_string();
                    if root != "crate" && root != "self" && root != "super" {
                        crates.insert(root);
                    }
                }
            }
        }

        let builtins: HashSet<&str> = ["std", "core", "alloc", "proc_macro", "test"]
            .iter()
            .copied()
            .collect();

        crates
            .into_iter()
            .filter(|c| !builtins.contains(c.as_str()))
            .collect()
    }

    /// Extract the first back-tick–quoted identifier from a line.
    fn extract_backtick_ident(line: &str) -> Option<String> {
        let start = line.find('`')? + 1;
        let rest = &line[start..];
        let end = rest.find('`')?;
        let ident = &rest[..end];
        if ident.is_empty() {
            None
        } else {
            Some(ident.to_string())
        }
    }

    /// Extract dependency commands from a correction LLM response.
    /// PSP-7: Extract dependency commands from correction response, validated by plugin policy.
    ///
    /// Replaces the legacy hardcoded allowlist with plugin `dependency_command_policy()`.
    pub(super) fn extract_commands_from_correction(
        response: &str,
        owner_plugin: &str,
    ) -> Vec<String> {
        let registry = perspt_core::plugin::PluginRegistry::new();
        let plugin = registry.get(owner_plugin);

        let mut commands = Vec::new();
        let mut in_commands_section = false;
        let mut in_code_block = false;

        for line in response.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("Commands:")
                || trimmed.starts_with("**Commands:")
                || trimmed.starts_with("### Commands")
            {
                in_commands_section = true;
                continue;
            }

            if in_commands_section {
                if trimmed.starts_with("```") {
                    in_code_block = !in_code_block;
                    continue;
                }

                if !in_code_block
                    && (trimmed.is_empty()
                        || trimmed.starts_with('#')
                        || trimmed.starts_with("File:")
                        || trimmed.starts_with("Diff:"))
                {
                    in_commands_section = false;
                    continue;
                }

                let cmd = trimmed
                    .trim_start_matches("- ")
                    .trim_start_matches("$ ")
                    .trim();

                if !cmd.is_empty() {
                    let decision = plugin
                        .map(|p| p.dependency_command_policy(cmd))
                        .unwrap_or(perspt_core::types::CommandPolicyDecision::Allow);

                    match decision {
                        perspt_core::types::CommandPolicyDecision::Allow
                        | perspt_core::types::CommandPolicyDecision::RequireApproval => {
                            commands.push(cmd.to_string());
                        }
                        perspt_core::types::CommandPolicyDecision::Deny => {
                            log::warn!(
                                "Command '{}' denied by plugin policy for '{}'",
                                cmd,
                                owner_plugin
                            );
                        }
                    }
                }
            }
        }

        commands
    }

    /// Run `cargo add <crate>` for each missing crate. Returns count of successes.
    async fn auto_install_crate_deps(crates: &[String], working_dir: &std::path::Path) -> usize {
        let mut installed = 0usize;
        for krate in crates {
            log::info!("Auto-installing crate: cargo add {}", krate);
            let result = tokio::process::Command::new("cargo")
                .args(["add", krate])
                .current_dir(working_dir)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .await;

            match result {
                Ok(output) if output.status.success() => {
                    log::info!("Successfully installed crate: {}", krate);
                    installed += 1;
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    log::warn!("Failed to install crate {}: {}", krate, stderr);
                }
                Err(e) => {
                    log::warn!("Failed to run cargo add {}: {}", krate, e);
                }
            }
        }
        installed
    }

    // =========================================================================
    // Python auto-dependency repair helpers (uv-first)
    // =========================================================================

    /// Parse Python test/import output and extract module names that are missing.
    ///
    /// Handles patterns like:
    ///   - `ModuleNotFoundError: No module named 'httpx'`
    ///   - `ImportError: cannot import name 'foo' from 'bar'`
    ///   - `E   ModuleNotFoundError: No module named 'pydantic'`
    pub(super) fn extract_missing_python_modules(output: &str) -> Vec<String> {
        use std::collections::HashSet;

        let mut modules: HashSet<String> = HashSet::new();

        for line in output.lines() {
            let trimmed = line.trim().trim_start_matches("E").trim();

            // Pattern: "ModuleNotFoundError: No module named 'foo'"
            // Also matches: "ModuleNotFoundError: No module named 'foo.bar'"
            // Can appear anywhere in the line (e.g. after FAILED test_x.py::test - ...)
            if trimmed.contains("ModuleNotFoundError: No module named ") {
                // Extract the quoted module name after "No module named "
                if let Some(pos) = trimmed.find("No module named ") {
                    let after = &trimmed[pos + "No module named ".len()..];
                    let name = after.trim().trim_matches('\'').trim_matches('"');
                    let root = name.split('.').next().unwrap_or(name);
                    if !root.is_empty() {
                        modules.insert(root.to_string());
                    }
                }
            }
            // Pattern: "ImportError: cannot import name 'X' from 'Y'"
            // or "ImportError: No module named 'X'"
            else if trimmed.contains("ImportError") && trimmed.contains("No module named") {
                if let Some(start) = trimmed.find('\'') {
                    let rest = &trimmed[start + 1..];
                    if let Some(end) = rest.find('\'') {
                        let name = &rest[..end];
                        let root = name.split('.').next().unwrap_or(name);
                        if !root.is_empty() {
                            modules.insert(root.to_string());
                        }
                    }
                }
            }
        }

        // Filter out standard library modules that are always present
        let stdlib: HashSet<&str> = [
            "os",
            "sys",
            "json",
            "re",
            "math",
            "datetime",
            "collections",
            "itertools",
            "functools",
            "pathlib",
            "typing",
            "abc",
            "io",
            "unittest",
            "logging",
            "argparse",
            "sqlite3",
            "csv",
            "hashlib",
            "tempfile",
            "shutil",
            "copy",
            "contextlib",
            "dataclasses",
            "enum",
            "textwrap",
            "importlib",
            "inspect",
            "traceback",
            "subprocess",
            "threading",
            "multiprocessing",
            "asyncio",
            "socket",
            "http",
            "urllib",
            "xml",
            "html",
            "email",
            "string",
            "struct",
            "array",
            "queue",
            "heapq",
            "bisect",
            "pprint",
            "decimal",
            "fractions",
            "random",
            "secrets",
            "time",
            "calendar",
            "zlib",
            "gzip",
            "zipfile",
            "tarfile",
            "glob",
            "fnmatch",
            "stat",
            "fileinput",
            "codecs",
            "uuid",
            "base64",
            "binascii",
            "pickle",
            "shelve",
            "dbm",
            "platform",
            "signal",
            "mmap",
            "ctypes",
            "configparser",
            "tomllib",
            "warnings",
            "weakref",
            "types",
            "operator",
            "numbers",
            "__future__",
        ]
        .iter()
        .copied()
        .collect();

        modules
            .into_iter()
            .filter(|m| !stdlib.contains(m.as_str()))
            .collect()
    }

    /// Map a Python import name to its PyPI package name.
    ///
    /// Most packages use the same name for import and install, but some
    /// notable exceptions exist. We handle the common ones here.
    pub(super) fn python_import_to_package(import_name: &str) -> &str {
        match import_name {
            "PIL" | "pil" => "pillow",
            "cv2" => "opencv-python",
            "yaml" => "pyyaml",
            "bs4" => "beautifulsoup4",
            "sklearn" => "scikit-learn",
            "attr" | "attrs" => "attrs",
            "dateutil" => "python-dateutil",
            "dotenv" => "python-dotenv",
            "gi" => "PyGObject",
            "serial" => "pyserial",
            "usb" => "pyusb",
            "wx" => "wxPython",
            "lxml" => "lxml",
            "Crypto" => "pycryptodome",
            "jose" => "python-jose",
            "jwt" => "PyJWT",
            "magic" => "python-magic",
            "docx" => "python-docx",
            "pptx" => "python-pptx",
            "git" => "gitpython",
            "psycopg2" => "psycopg2-binary",
            other => other,
        }
    }

    /// Run `uv add <package>` for each missing Python module. Returns count of successes.
    async fn auto_install_python_deps(modules: &[String], working_dir: &std::path::Path) -> usize {
        let mut installed = 0usize;
        for module in modules {
            let package = Self::python_import_to_package(module);
            log::info!("Auto-installing Python package: uv add {}", package);
            let result = tokio::process::Command::new("uv")
                .args(["add", package])
                .current_dir(working_dir)
                .env_remove("VIRTUAL_ENV")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .await;

            match result {
                Ok(output) if output.status.success() => {
                    log::info!("Successfully installed Python package: {}", package);
                    installed += 1;
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    log::warn!("Failed to install Python package {}: {}", package, stderr);
                }
                Err(e) => {
                    log::warn!("Failed to run uv add {}: {}", package, e);
                }
            }
        }

        // Always sync after adding dependencies to ensure venv is up-to-date
        if installed > 0 {
            log::info!("Running uv sync --dev after dependency install...");
            let _ = tokio::process::Command::new("uv")
                .args(["sync", "--dev"])
                .current_dir(working_dir)
                .env_remove("VIRTUAL_ENV")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .await;
        }

        installed
    }

    /// Normalize a dependency command to its uv-first equivalent.
    ///
    /// Converts generic pip/pip3/python -m pip install commands to `uv add`,
    /// leaving already-correct uv commands and non-Python commands unchanged.
    pub(super) fn normalize_command_to_uv(command: &str) -> String {
        let trimmed = command.trim();

        // pip install foo → uv add foo
        // pip3 install foo → uv add foo
        // python -m pip install foo → uv add foo
        // python3 -m pip install foo → uv add foo
        let pip_install_prefixes = [
            "pip install ",
            "pip3 install ",
            "python -m pip install ",
            "python3 -m pip install ",
        ];
        for prefix in &pip_install_prefixes {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let packages = rest.trim();
                if packages.is_empty() {
                    return command.to_string();
                }
                // Strip -r/--requirement flags (uv add doesn't support those directly)
                if packages.starts_with("-r ") || packages.starts_with("--requirement ") {
                    return format!("uv pip install {}", packages);
                }
                return format!("uv add {}", packages);
            }
        }

        // pip install -e . → uv pip install -e .
        if trimmed.starts_with("pip install -") || trimmed.starts_with("pip3 install -") {
            return format!("uv {}", trimmed);
        }

        command.to_string()
    }

    /// PSP-5 Phase 9: Finalize verification result — mark degraded, emit events, build summary.
    fn finalize_verification_result(
        &mut self,
        result: &mut perspt_core::types::VerificationResult,
        plugin_name: &str,
    ) {
        if result.has_degraded_stages() {
            result.degraded = true;
            let reasons = result.degraded_stage_reasons();
            result.degraded_reason = Some(reasons.join("; "));

            // Emit per-stage SensorFallback events
            for outcome in &result.stage_outcomes {
                if let perspt_core::types::SensorStatus::Fallback { actual, reason } =
                    &outcome.sensor_status
                {
                    self.emit_event(perspt_core::AgentEvent::SensorFallback {
                        node_id: plugin_name.to_string(),
                        stage: outcome.stage.clone(),
                        primary: reason.clone(),
                        actual: actual.clone(),
                        reason: reason.clone(),
                    });
                }
            }
        }

        // Store result for convergence-time degraded check
        self.last_verification_result = Some(result.clone());

        // Build summary
        result.summary = format!(
            "{}: syntax={}, build={}, tests={}, lint={}{}",
            plugin_name,
            if result.syntax_ok { "✅" } else { "❌" },
            if result.build_ok { "✅" } else { "❌" },
            if result.tests_ok { "✅" } else { "❌" },
            if result.lint_ok { "✅" } else { "⏭️" },
            if result.degraded { " (degraded)" } else { "" },
        );
    }
}

/// Convert diagnostic severity to string
pub(super) fn severity_to_str(severity: Option<lsp_types::DiagnosticSeverity>) -> &'static str {
    match severity {
        Some(lsp_types::DiagnosticSeverity::ERROR) => "ERROR",
        Some(lsp_types::DiagnosticSeverity::WARNING) => "WARNING",
        Some(lsp_types::DiagnosticSeverity::INFORMATION) => "INFO",
        Some(lsp_types::DiagnosticSeverity::HINT) => "HINT",
        Some(_) => "OTHER",
        None => "UNKNOWN",
    }
}

/// PSP-5 Phase 9: Determine which verification stages to run based on NodeClass.
///
/// - **Interface**: SyntaxCheck only (signatures/schemas)
/// - **Implementation**: SyntaxCheck + Build (+ Test if weighted_tests non-empty
///   OR output targets include test files)
/// - **Integration**: Full pipeline (SyntaxCheck + Build + Test + Lint)
pub(super) fn verification_stages_for_node(
    node: &SRBNNode,
) -> Vec<perspt_core::plugin::VerifierStage> {
    use perspt_core::plugin::VerifierStage;
    match node.node_class {
        perspt_core::types::NodeClass::Interface => {
            vec![VerifierStage::SyntaxCheck]
        }
        perspt_core::types::NodeClass::Implementation => {
            let mut stages = vec![VerifierStage::SyntaxCheck, VerifierStage::Build];
            // Include Test stage if the node has weighted tests OR if the
            // node's output targets include test files.  Without this, nodes
            // that produce test files (tests/*.rs, test_*.py, *.test.ts, etc.)
            // only get SyntaxCheck+Build which don't compile/run test targets.
            let has_test_outputs = node.output_targets.iter().any(|p| {
                let s = p.to_string_lossy();
                // Check the filename (last component) for test patterns rather
                // than the full path, to avoid false positives from directory
                // names like "test_seismic/" matching "/test_".
                let filename = p
                    .file_name()
                    .map(|f| f.to_string_lossy())
                    .unwrap_or_default();
                s.contains("/tests/")
                    || filename.starts_with("test_")
                    || filename.contains(".test.")
                    || filename.contains(".spec.")
                    || filename.ends_with("_test.rs")
                    || filename.ends_with("_test.py")
                    || filename.ends_with("_tests.rs")
                    || filename.ends_with("_tests.py")
            });
            if !node.contract.weighted_tests.is_empty() || has_test_outputs {
                stages.push(VerifierStage::Test);
            }
            stages
        }
        perspt_core::types::NodeClass::Integration => {
            vec![
                VerifierStage::SyntaxCheck,
                VerifierStage::Build,
                VerifierStage::Test,
                VerifierStage::Lint,
            ]
        }
    }
}
