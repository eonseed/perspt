use super::*;

impl SRBNOrchestrator {
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
}
