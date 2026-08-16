use super::*;

/// Push a typed PSP-8 residual onto the verification residual vector.
///
/// `score` is the raw residual *magnitude* `r_e ≥ 0` (a count or unit penalty);
/// the energy model squares and weights it later (`V = Σ_e w_e‖r_e‖²`). A
/// construction failure (negative/non-finite score) is dropped with a debug log
/// rather than aborting verification.
pub(crate) fn push_residual(
    into: &mut Vec<ResidualEvent>,
    node_id: &str,
    generation: u32,
    class: ResidualClass,
    severity: ResidualSeverity,
    score: f64,
    sensor: SensorRef,
) {
    match ResidualEvent::new(node_id, generation, class, severity, score, sensor) {
        Ok(r) => into.push(r),
        Err(e) => log::debug!("skipped {class:?} residual for {node_id}: {e}"),
    }
}

impl SRBNOrchestrator {
    /// Step 4: Stability Verification
    ///
    /// Computes Lyapunov Energy V(x) from LSP diagnostics, contracts, and tests
    pub(crate) async fn step_verify(&mut self, idx: NodeIndex) -> Result<EnergyComponents> {
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

        // PSP-8 System 2: collect typed residuals from every sensor, then score
        // them once into the canonical quadratic energy `V = Σ_e w_e‖r_e‖²`.
        // Each detection below pushes a residual whose *magnitude* is the natural
        // count / unit penalty; the energy model does the squaring and weighting.
        let node_id = self.graph[idx].node_id.clone();
        let generation = self.graph[idx].monitor.attempt_count as u32;
        let mut residuals: Vec<ResidualEvent> = Vec::new();

        // Tool failures (could not apply changes) — a blocking toolchain fault.
        if let Some(ref err) = self.last_tool_failure {
            push_residual(
                &mut residuals,
                &node_id,
                generation,
                ResidualClass::ToolFailure,
                ResidualSeverity::Blocking,
                3.0,
                SensorRef::new("tool", IndependenceRoute::DeterministicTool),
            );
            log::warn!(
                "Tool failure detected, ToolFailure residual raised: {}",
                err
            );
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
                    // One Type residual whose magnitude is the diagnostic count;
                    // the model squares+weights it into the Syn component.
                    push_residual(
                        &mut residuals,
                        &node_id,
                        generation,
                        ResidualClass::Type,
                        ResidualSeverity::Error,
                        diagnostics.len() as f64,
                        SensorRef::new("lsp", IndependenceRoute::Lsp),
                    );
                    log::info!("LSP found {} diagnostics", diagnostics.len());
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
                            push_residual(
                                &mut residuals,
                                &node_id,
                                generation,
                                ResidualClass::Policy,
                                ResidualSeverity::Error,
                                1.0,
                                SensorRef::new("policy", IndependenceRoute::DeterministicTool),
                            );
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

                    // Map verification result to energy components. CRITICAL:
                    // only penalize a stage that actually RAN for this node.
                    // `VerificationResult` derives Default (all *_ok = false), so a
                    // node that never runs Build (e.g. a syntax-only Interface
                    // node) would otherwise be charged a phantom build failure
                    // with no captured error — an uncorrectable escalation.
                    use perspt_core::plugin::VerifierStage;
                    let ran_syntax = stages.contains(&VerifierStage::SyntaxCheck);
                    let ran_build = stages.contains(&VerifierStage::Build);
                    let ran_lint = stages.contains(&VerifierStage::Lint);

                    // - Syntax fail → Syntax residual (hard check: any failure
                    //   forces a hard-fail regardless of ε).
                    if ran_syntax && !vr.syntax_ok {
                        push_residual(
                            &mut residuals,
                            &node_id,
                            generation,
                            ResidualClass::Syntax,
                            ResidualSeverity::Blocking,
                            1.0,
                            SensorRef::new("syntax", IndependenceRoute::Compiler),
                        );
                    }
                    // - Build fail → Build residual (hard check).
                    if ran_build && !vr.build_ok {
                        push_residual(
                            &mut residuals,
                            &node_id,
                            generation,
                            ResidualClass::Build,
                            ResidualSeverity::Blocking,
                            1.0,
                            SensorRef::new("build", IndependenceRoute::Compiler),
                        );
                    }
                    // - Test fail → TestFailure residual, magnitude = failed count
                    //   (the model squares+weights it into the Log component).
                    if !vr.tests_ok && vr.tests_failed > 0 {
                        push_residual(
                            &mut residuals,
                            &node_id,
                            generation,
                            ResidualClass::TestFailure,
                            ResidualSeverity::Error,
                            vr.tests_failed as f64,
                            SensorRef::new("test", IndependenceRoute::TestOracle),
                        );
                    }
                    // - Tests were expected but never ran (e.g. test compilation
                    //   failed or test stage was skipped). Without this, nodes with
                    //   broken test files get V=0 and pass verification erroneously.
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
                            push_residual(
                                &mut residuals,
                                &node_id,
                                generation,
                                ResidualClass::Build,
                                ResidualSeverity::Blocking,
                                1.0,
                                SensorRef::new("test-compile", IndependenceRoute::Compiler),
                            );
                        } else {
                            // Tests didn't run but no obvious error — moderate penalty.
                            push_residual(
                                &mut residuals,
                                &node_id,
                                generation,
                                ResidualClass::TestFailure,
                                ResidualSeverity::Error,
                                1.0,
                                SensorRef::new("test", IndependenceRoute::TestOracle),
                            );
                            log::warn!(
                                "Tests expected but did not produce results for node '{}'",
                                self.graph[idx].node_id
                            );
                        }
                    }
                    // - Lint fail → Lint residual (only if the Lint stage ran, strict).
                    if ran_lint
                        && !vr.lint_ok
                        && self.context.verifier_strictness
                            == perspt_core::types::VerifierStrictness::Strict
                    {
                        push_residual(
                            &mut residuals,
                            &node_id,
                            generation,
                            ResidualClass::Lint,
                            ResidualSeverity::Warning,
                            1.0,
                            SensorRef::new("lint", IndependenceRoute::DeterministicTool),
                        );
                    }

                    // PSP-7: Boot residuals — bootstrap/toolchain failures, distinct
                    // from code quality. Computed from the FINAL verification result
                    // (after auto-repair has had its chance to fix missing deps).
                    if vr.degraded && vr.stage_outcomes.is_empty() {
                        // Fully degraded toolchain: no stages ran at all.
                        push_residual(
                            &mut residuals,
                            &node_id,
                            generation,
                            ResidualClass::SensorUnavailable,
                            ResidualSeverity::Blocking,
                            3.0,
                            SensorRef::new("toolchain", IndependenceRoute::DeterministicTool),
                        );
                        log::warn!(
                            "Boot residual: toolchain fully degraded ({})",
                            vr.degraded_reason.as_deref().unwrap_or("unknown")
                        );
                    }
                    for so in &vr.stage_outcomes {
                        match &so.sensor_status {
                            perspt_core::types::SensorStatus::Unavailable { reason } => {
                                push_residual(
                                    &mut residuals,
                                    &node_id,
                                    generation,
                                    ResidualClass::SensorUnavailable,
                                    ResidualSeverity::Blocking,
                                    2.0,
                                    SensorRef::new(
                                        format!("stage:{}", so.stage),
                                        IndependenceRoute::DeterministicTool,
                                    ),
                                );
                                log::warn!(
                                    "Boot residual: sensor unavailable for stage '{}': {}",
                                    so.stage,
                                    reason
                                );
                            }
                            perspt_core::types::SensorStatus::Fallback { reason, .. } => {
                                push_residual(
                                    &mut residuals,
                                    &node_id,
                                    generation,
                                    ResidualClass::SensorUnavailable,
                                    ResidualSeverity::Warning,
                                    1.0,
                                    SensorRef::new(
                                        format!("stage:{}", so.stage),
                                        IndependenceRoute::DeterministicTool,
                                    ),
                                );
                                log::info!(
                                    "Boot residual: fallback sensor for stage '{}': {}",
                                    so.stage,
                                    reason
                                );
                            }
                            perspt_core::types::SensorStatus::Available => {}
                        }
                    }

                    // D1: Feed real tool output into the correction context so the
                    // correction prompt always shows the actual error. Prefer the
                    // explicit raw_output, but fall back to any failing stage's
                    // captured output — without this, a genuinely failing stage
                    // could still surface "No specific errors captured".
                    if let Some(ref raw) = vr.raw_output {
                        self.context.last_test_output = Some(raw.clone());
                    } else if let Some(failed_out) = vr
                        .stage_outcomes
                        .iter()
                        .find(|so| {
                            !so.passed && so.output.as_deref().is_some_and(|o| !o.trim().is_empty())
                        })
                        .and_then(|so| so.output.clone())
                    {
                        self.context.last_test_output = Some(failed_out);
                    }

                    self.emit_log(format!("📊 Verification: {}", vr.summary));

                    // PSP-8 runtime probe: once syntax/build/tests are clean,
                    // actually RUN the built artifact's entrypoints so runtime
                    // failures (panics, import errors, crashes) become Runtime
                    // residuals instead of slipping past build+test.
                    let code_clean = !residuals.iter().any(|r| {
                        matches!(
                            r.class,
                            ResidualClass::Syntax
                                | ResidualClass::Type
                                | ResidualClass::Build
                                | ResidualClass::TestFailure
                                | ResidualClass::ToolFailure
                                | ResidualClass::Runtime
                        )
                    });
                    if code_clean {
                        self.run_runtime_smoke(idx, &verify_dir, &mut residuals)
                            .await;
                    }
                }
            }
        }

        // PSP-8: Goal-presence sensor — the verifier that refuses false
        // stability. An empty or placeholder output file compiles, runs no
        // failing tests, and emits no diagnostics, so every code-quality sensor
        // reports zero and the node would be declared "stable" without the
        // requested work ever being done. The sensor extracts the symbols the
        // node must produce (from its contract signature + goal) and the symbols
        // actually defined in its output targets; any absent required symbol
        // raises structural energy so the node cannot converge until the goal is
        // satisfied, and injects a directed diagnostic for the correction loop.
        self.apply_goal_presence_energy(idx, &mut residuals);

        // PSP-8 System 2: score the full residual vector into the canonical
        // quadratic energy `V = Σ_e w_e‖r_e‖²` and project it onto the five
        // component rollups. This is the single acceptance energy — no separate
        // α/β/γ aggregation pass.
        let (_v_total, energy) = self.sdk_gate.score_components(&residuals);

        let node = &self.graph[idx];
        // Record energy in persistent ledger
        if let Err(e) = self
            .ledger
            .record_energy(&node.node_id, &energy, energy.total())
        {
            log::error!("Failed to record energy: {}", e);
        }

        log::info!(
            "Energy for {}: V_syn={:.2}, V_str={:.2}, V_log={:.2}, V_boot={:.2}, V_sheaf={:.2}, \
                Total={:.2}",
            node.node_id,
            energy.v_syn,
            energy.v_str,
            energy.v_log,
            energy.v_boot,
            energy.v_sheaf,
            energy.total()
        );

        // PSP-5 Phase 7: Emit enriched VerificationComplete event
        {
            let node = &self.graph[idx];
            let total = energy.total();
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

            // Prefer the plugin verification result for the ok-flags; fall back to
            // the quadratic component rollups (a zero component means that class
            // of residual is absent).
            let (syntax_ok, build_ok, tests_ok) = match self.last_verification_result {
                Some(ref vr) => (vr.syntax_ok, vr.build_ok, vr.tests_ok),
                None => (
                    energy.v_syn == 0.0,
                    energy.v_syn == 0.0,
                    energy.v_log == 0.0,
                ),
            };
            self.emit_event(perspt_core::AgentEvent::VerificationComplete {
                node_id: node.node_id.clone(),
                syntax_ok,
                build_ok,
                tests_ok,
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

    /// PSP-8 goal-presence sensor: raise structural energy when the symbols a
    /// node must produce are absent from its delivered output targets.
    ///
    /// Runs the `sdk_bridge::goal_presence_check` (perspt-coding symbol
    /// extraction + perspt-sdk blocking residual). When required symbols are
    /// missing it adds a strong `V_str` penalty so the node cannot be declared
    /// stable, records the blocking residual on the SDK gate state, injects a
    /// directed diagnostic into the correction context, and emits a log line.
    /// It is a no-op when the node owns no code source files (e.g. a manifest or
    /// scaffold node), declares no checkable symbols (a command or prose-only
    /// goal), or when the goal is already satisfied, so it never invents a false
    /// obligation.
    pub(crate) fn apply_goal_presence_energy(
        &mut self,
        idx: NodeIndex,
        residuals: &mut Vec<ResidualEvent>,
    ) {
        let node = &self.graph[idx];
        if node.output_targets.is_empty() {
            return;
        }
        let node_id = node.node_id.clone();
        let goal = node.goal.clone();
        let interface_signature = node.contract.interface_signature.clone();
        let generation = node.monitor.attempt_count as u32;

        // Restrict to code source files. A node that only owns a manifest
        // (pyproject.toml, package.json, Cargo.toml) or other non-code asset has
        // no code-symbol obligation, so the symbol sensor must not fire on it.
        let output_targets: Vec<PathBuf> = node
            .output_targets
            .iter()
            .filter(|t| is_code_source_file(t))
            .cloned()
            .collect();
        if output_targets.is_empty() {
            return;
        }

        // Read the current contents of every code output target the node owns.
        let work_dir = self.effective_working_dir(idx);
        let sources: Vec<String> = output_targets
            .iter()
            .filter_map(|target| std::fs::read_to_string(work_dir.join(target)).ok())
            .collect();

        let report = match super::sdk_bridge::goal_presence_check(
            &node_id,
            generation,
            &interface_signature,
            &goal,
            &sources,
        ) {
            Ok(Some(report)) => report,
            Ok(None) => return,
            Err(e) => {
                log::debug!("Goal-presence sensor skipped for '{}': {}", node_id, e);
                return;
            }
        };

        let line = report.summary();
        log::warn!("Node {}: {}", node_id, line);
        self.emit_log(format!("🚧 {} — raising structural energy", line));

        // Inject a directed diagnostic so the correction loop tells the actuator
        // exactly which symbols to define (build_correction_prompt reads these).
        let missing_list = report.missing.join(", ");
        let target_hint = output_targets
            .first()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        self.context.last_diagnostics.push(lsp_types::Diagnostic {
            range: lsp_types::Range::default(),
            severity: Some(lsp_types::DiagnosticSeverity::ERROR),
            code: None,
            code_description: None,
            source: Some("goal-presence".to_string()),
            message: format!(
                "Required symbol(s) not defined in {}: {}. The goal is not satisfied until \
                 each is implemented.",
                if target_hint.is_empty() {
                    "the output"
                } else {
                    &target_hint
                },
                missing_list
            ),
            related_information: None,
            tags: None,
            data: None,
        });

        // The blocking residual contributes to the node's quadratic energy
        // (SymbolMismatch → Str component) so the node cannot converge while the
        // requested work is missing, and is mirrored on the SDK gate telemetry.
        self.sdk_gate.record_goal_residual(report.residual.clone());
        residuals.push(report.residual);
    }

    /// PSP-8 runtime probe: run the built artifact's smoke invocations (from the
    /// language adapter in `perspt-coding`) and translate runtime failures into
    /// `V_log` energy + a directed-correction context. Generic across languages:
    /// each adapter declares its own smoke invocations and output classification.
    pub(crate) async fn run_runtime_smoke(
        &mut self,
        idx: NodeIndex,
        work_dir: &std::path::Path,
        residuals: &mut Vec<ResidualEvent>,
    ) {
        let owner_plugin = self.graph[idx].owner_plugin.clone();
        let Some(lang) = super::sdk_bridge::coding_language_for(&owner_plugin) else {
            return;
        };
        let node_id = self.graph[idx].node_id.clone();
        let generation = self.graph[idx].monitor.attempt_count as u32;

        let adapter = perspt_coding::lang::adapter_for(lang);
        let invocations = adapter.smoke_invocations(work_dir);
        if invocations.is_empty() {
            return;
        }
        self.emit_log(format!(
            "🚦 Runtime smoke: exercising {} entrypoint(s)",
            invocations.len()
        ));

        let mut residual_total = 0usize;
        let mut failure_report = String::new();
        for inv in &invocations {
            let (exit_ok, output) = Self::run_smoke_command(&inv.command, work_dir).await;
            let runtime_residuals =
                adapter.classify_runtime(&node_id, generation, inv, exit_ok, &output);
            if runtime_residuals.is_empty() {
                self.emit_log(format!("   ✅ {}", inv.description));
            } else {
                residual_total += runtime_residuals.len();
                self.emit_log(format!("   ❌ {} — runtime failure", inv.description));
                let tail: String = output
                    .lines()
                    .rev()
                    .take(20)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join("\n");
                failure_report.push_str(&format!(
                    "Runtime smoke failed: `{}` ({})\n{}\n\n",
                    inv.command, inv.description, tail
                ));
                // Runtime residuals (class Runtime → Log component) join the node's
                // residual vector so they are squared+weighted into the energy, and
                // are mirrored on the SDK gate telemetry.
                for r in runtime_residuals {
                    self.sdk_gate.record_goal_residual(r.clone());
                    residuals.push(r);
                }
            }
        }

        if residual_total > 0 {
            // Feed the real runtime error into the correction context so the
            // directed-correction path produces a Runtime fix instruction.
            self.context.last_test_output = Some(failure_report);
            self.emit_log(format!(
                "🚧 Runtime smoke found {} failure(s) — raising runtime energy",
                residual_total
            ));
        }
    }

    /// Run a single smoke shell command from `work_dir` with a timeout. Returns
    /// `(exit_success, combined_stdout_stderr)`; a timeout/spawn error is a failure.
    pub(crate) async fn run_smoke_command(
        command: &str,
        work_dir: &std::path::Path,
    ) -> (bool, String) {
        use tokio::process::Command;
        let child = Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(work_dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output();
        match tokio::time::timeout(std::time::Duration::from_secs(120), child).await {
            Ok(Ok(out)) => {
                let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
                combined.push_str(&String::from_utf8_lossy(&out.stderr));
                (out.status.success(), combined)
            }
            Ok(Err(e)) => (false, format!("failed to spawn smoke command: {e}")),
            Err(_) => (
                false,
                "runtime smoke command timed out after 120s".to_string(),
            ),
        }
    }
}
