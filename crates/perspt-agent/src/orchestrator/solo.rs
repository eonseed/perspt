//! Solo mode: single-file generation, verification, and correction loop.

use super::*;

impl SRBNOrchestrator {
    /// Run Solo Mode: A tight loop for single-file tasks
    ///
    /// Bypasses DAG sheafification and directly generates, verifies, and corrects
    /// a single Python file with embedded doctests for V_log.
    pub(super) async fn run_solo_mode(&mut self, task: String) -> Result<()> {
        const MAX_ATTEMPTS: usize = 3;
        const EPSILON: f32 = 0.1;

        // Initial prompt - will be replaced with correction prompt on retries
        let mut current_prompt = self.build_solo_prompt(&task);
        let mut attempt = 0;

        // Track state for correction
        let mut last_filename: String;
        let mut last_code: String;

        loop {
            attempt += 1;

            if attempt > MAX_ATTEMPTS {
                self.emit_log(format!(
                    "Solo Mode failed after {} attempts, consider Team Mode",
                    MAX_ATTEMPTS
                ));
                self.emit_event(perspt_core::AgentEvent::Complete {
                    success: false,
                    message: "Solo Mode exhausted retries".to_string(),
                });
                return Ok(());
            }

            self.emit_log(format!("Solo Mode attempt {}/{}", attempt, MAX_ATTEMPTS));

            // Step 1: Generate code
            let response = self
                .call_llm_with_logging(&self.actuator_model.clone(), &current_prompt, Some("solo"))
                .await?;

            // Step 2: Extract code from response
            let (filename, code) = match self.extract_code_from_response(&response) {
                Some((f, c, _)) => (f, c),
                None => {
                    self.emit_log("No code block found in LLM response".to_string());
                    continue;
                }
            };

            last_filename = filename.clone();
            last_code = code.clone();

            // Step 3: Write file
            let full_path = self.context.working_dir.join(&filename);

            let mut args = HashMap::new();
            args.insert("path".to_string(), filename.clone());
            args.insert("content".to_string(), code.clone());

            let call = ToolCall {
                name: "write_file".to_string(),
                arguments: args,
            };

            let result = self.tools.execute(&call).await;
            if !result.success {
                self.emit_log(format!("Failed to write {}: {:?}", filename, result.error));
                continue;
            }

            self.emit_log(format!("Created: {}", filename));
            self.last_written_file = Some(full_path.clone());

            // Step 4: Verify - Calculate Lyapunov Energy
            let energy = self.solo_verify(&full_path).await;
            let v_total = energy.total_simple();

            self.emit_log(format!(
                "V(x) = {:.2} (V_syn={:.2}, V_log={:.2}, V_boot={:.2})",
                v_total, energy.v_syn, energy.v_log, energy.v_boot
            ));

            // Step 5: Check convergence
            if v_total < EPSILON {
                self.emit_log(format!(
                    "Solo Mode complete! V(x)={:.2} < epsilon={:.2}",
                    v_total, EPSILON
                ));
                self.emit_event(perspt_core::AgentEvent::Complete {
                    success: true,
                    message: format!("Created {}", filename),
                });
                return Ok(());
            }

            // Step 6: Build correction prompt with errors (THE KEY FIX!)
            self.emit_log(format!(
                "Unstable (V={:.2} > epsilon={:.2}), building correction prompt...",
                v_total, EPSILON
            ));

            current_prompt =
                self.build_solo_correction_prompt(&task, &last_filename, &last_code, &energy);
        }
    }

    /// Verify a Solo Mode file and calculate energy components
    async fn solo_verify(&mut self, path: &std::path::Path) -> EnergyComponents {
        let mut energy = EnergyComponents::default();

        // V_syn: LSP Diagnostics
        let lsp_key = self.lsp_key_for_file(&path.to_string_lossy());
        if let Some(client) = lsp_key.as_deref().and_then(|k| self.lsp_clients.get(k)) {
            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
            let path_str = path.to_string_lossy().to_string();

            let diagnostics = client.get_diagnostics(&path_str).await;
            energy.v_syn = LspClient::calculate_syntactic_energy(&diagnostics);

            if !diagnostics.is_empty() {
                self.emit_log(format!(
                    "LSP: {} diagnostics (V_syn={:.2})",
                    diagnostics.len(),
                    energy.v_syn
                ));
                self.context.last_diagnostics = diagnostics;
            }
        }

        // V_log: Doctests
        energy.v_log = self.run_doctest(path).await;

        // V_boot: Execution verification
        energy.v_boot = self.run_script_check(path).await;

        energy
    }

    /// Run the script and check for execution errors (V_boot)
    async fn run_script_check(&mut self, path: &std::path::Path) -> f32 {
        let output = tokio::process::Command::new("python")
            .arg(path)
            .current_dir(&self.context.working_dir)
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                self.emit_log("Script execution: OK".to_string());
                0.0
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let stdout = String::from_utf8_lossy(&out.stdout);
                let error_output = if !stderr.is_empty() {
                    stderr.to_string()
                } else {
                    stdout.to_string()
                };

                // Truncate long output
                let truncated = if error_output.len() > 500 {
                    format!("{}...(truncated)", &error_output[..500])
                } else {
                    error_output.clone()
                };

                self.emit_log(format!("Script execution: FAILED\n{}", truncated));
                self.context.last_test_output = Some(error_output);
                5.0 // High energy penalty for runtime errors
            }
            Err(e) => {
                self.emit_log(format!("Script execution: ERROR ({})", e));
                5.0
            }
        }
    }

    /// Build a minimal prompt for Solo Mode (with dynamic filename instruction)
    fn build_solo_prompt(&self, task: &str) -> String {
        crate::prompts::SOLO_GENERATE.replace("{task}", task)
    }

    /// Build a correction prompt for Solo Mode with error feedback
    fn build_solo_correction_prompt(
        &self,
        task: &str,
        filename: &str,
        current_code: &str,
        energy: &EnergyComponents,
    ) -> String {
        let mut errors = Vec::new();

        // Collect LSP diagnostics
        for diag in &self.context.last_diagnostics {
            let severity = match diag.severity {
                Some(lsp_types::DiagnosticSeverity::ERROR) => "ERROR",
                Some(lsp_types::DiagnosticSeverity::WARNING) => "WARNING",
                Some(lsp_types::DiagnosticSeverity::INFORMATION) => "INFO",
                Some(lsp_types::DiagnosticSeverity::HINT) => "HINT",
                _ => "DIAGNOSTIC",
            };
            errors.push(format!(
                "- Line {}: {} [{}]",
                diag.range.start.line + 1,
                diag.message,
                severity
            ));
        }

        // Collect test/execution output
        if let Some(ref output) = self.context.last_test_output {
            if !output.is_empty() {
                // Truncate if too long
                let truncated = if output.len() > 1000 {
                    format!("{}...(truncated)", &output[..1000])
                } else {
                    output.clone()
                };
                errors.push(format!("- Runtime/Test Error:\n{}", truncated));
            }
        }

        let error_list = if errors.is_empty() {
            "No specific errors captured, but energy is still too high.".to_string()
        } else {
            errors.join("\n")
        };

        crate::prompts::render_solo_correction(
            task,
            filename,
            current_code,
            &format!("{:.2}", energy.v_syn),
            &format!("{:.2}", energy.v_log),
            &format!("{:.2}", energy.v_boot),
            &error_list,
        )
    }

    /// Run Python doctest on a file and return V_log energy
    async fn run_doctest(&mut self, file_path: &std::path::Path) -> f32 {
        let output = tokio::process::Command::new("python")
            .args(["-m", "doctest", "-v"])
            .arg(file_path)
            .current_dir(&self.context.working_dir)
            .output()
            .await;

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);

                // Parse doctest output for failures
                let failed = stderr.matches("FAILED").count() + stdout.matches("FAILED").count();
                let passed = stdout.matches("ok").count();

                if failed > 0 {
                    self.emit_log(format!("Doctest: {} passed, {} failed", passed, failed));
                    // Store doctest output for correction prompt
                    let doctest_output = format!("{}\n{}", stdout, stderr);
                    self.context.last_test_output = Some(doctest_output);
                    // Weight failures at gamma=2.0 per SRBN spec
                    2.0 * (failed as f32)
                } else if passed > 0 {
                    self.emit_log(format!("Doctest: {} passed", passed));
                    0.0
                } else {
                    // No doctests found - that's okay for Solo Mode, v_log = 0
                    log::debug!("No doctests found in file");
                    0.0
                }
            }
            Err(e) => {
                log::warn!("Failed to run doctest: {}", e);
                0.0 // Don't penalize if doctest runner fails
            }
        }
    }
}
