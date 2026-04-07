//! Convergence and self-correction: correction prompts, LLM calls, and fix routing.

use super::verification::severity_to_str;
use super::*;

impl SRBNOrchestrator {
    /// Step 5: Convergence & Self-Correction
    ///
    /// Returns true if converged, false if should escalate
    pub(super) async fn step_converge(
        &mut self,
        idx: NodeIndex,
        energy: EnergyComponents,
    ) -> Result<bool> {
        log::info!("Step 5: Convergence check");

        // First compute what we need from the node
        let total = {
            let node = &self.graph[idx];
            energy.total(&node.contract)
        };

        // Now mutate
        let node = &mut self.graph[idx];
        node.monitor.record_energy(total);
        let node_id = node.node_id.clone();
        let goal = node.goal.clone();
        let epsilon = node.monitor.stability_epsilon;
        let attempt_count = node.monitor.attempt_count;
        let stable = node.monitor.stable;
        let should_escalate = node.monitor.should_escalate();

        if stable {
            // PSP-5 Phase 4: Block false stability when verification was degraded
            if let Some(ref vr) = self.last_verification_result {
                if vr.has_degraded_stages() {
                    let reasons = vr.degraded_stage_reasons();
                    log::warn!(
                        "Node {} energy is below ε but verification was degraded: {:?}",
                        node_id,
                        reasons
                    );
                    self.emit_log(format!(
                        "⚠️ V(x)={:.2} < ε but stability unconfirmed — degraded sensors: {}",
                        total,
                        reasons.join(", ")
                    ));
                    self.emit_event(perspt_core::AgentEvent::DegradedVerification {
                        node_id: node_id.clone(),
                        degraded_stages: reasons,
                        stability_blocked: true,
                    });
                    // Do NOT return Ok(true) — fall through to correction loop
                    // so the orchestrator retries with awareness that some sensors
                    // were unavailable.
                } else {
                    log::info!(
                        "Node {} is stable (V(x)={:.2} < ε={:.2})",
                        node_id,
                        total,
                        epsilon
                    );
                    self.emit_log(format!("✅ Stable! V(x)={:.2} < ε={:.2}", total, epsilon));
                    return Ok(true);
                }
            } else {
                log::info!(
                    "Node {} is stable (V(x)={:.2} < ε={:.2})",
                    node_id,
                    total,
                    epsilon
                );
                self.emit_log(format!("✅ Stable! V(x)={:.2} < ε={:.2}", total, epsilon));
                return Ok(true);
            }
        }

        if should_escalate {
            log::warn!(
                "Node {} failed to converge after {} attempts (V(x)={:.2})",
                node_id,
                attempt_count,
                total
            );
            self.emit_log(format!(
                "⚠️ Escalating: failed to converge after {} attempts",
                attempt_count
            ));
            return Ok(false);
        }

        // === CORRECTION LOOP ===
        self.graph[idx].state = NodeState::Retry;
        self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
            node_id: self.graph[idx].node_id.clone(),
            status: perspt_core::NodeStatus::Retrying,
        });
        log::info!(
            "V(x)={:.2} > ε={:.2}, regenerating with feedback (attempt {})",
            total,
            epsilon,
            attempt_count
        );
        self.emit_log(format!(
            "🔄 V(x)={:.2} > ε={:.2}, sending errors to LLM (attempt {})",
            total, epsilon, attempt_count
        ));

        // PSP-7: Budget ceiling check before LLM call
        if self.budget.cost_exhausted() {
            log::warn!(
                "Budget exhausted (${:.2} / ${:.2}) — escalating node '{}'",
                self.budget.cost_used_usd,
                self.budget.max_cost_usd.unwrap_or(f64::INFINITY),
                node_id
            );
            self.emit_log(format!(
                "💰 Budget exhausted (${:.2}) — escalating",
                self.budget.cost_used_usd
            ));
            return Ok(false);
        }

        // Build correction prompt with diagnostics
        let correction_prompt = self.build_correction_prompt(&node_id, &goal, &energy)?;

        log::info!(
            "--- CORRECTION PROMPT ---\n{}\n-------------------------",
            correction_prompt
        );
        // Don't emit the full correction prompt to TUI - it's too verbose
        self.emit_log("📤 Sending correction prompt to LLM...".to_string());

        // Call LLM for corrected code
        let corrected = self.call_llm_for_correction(&correction_prompt).await?;

        // PSP-7: Typed parse pipeline replaces legacy Option-based parsing
        let node_class = self.graph[idx].node_class;
        let attempt = self.graph[idx].monitor.attempt_count;
        let diagnosis = self.context.last_diagnostics.clone();
        let owner_plugin = self.graph[idx].owner_plugin.clone();

        let (bundle_opt, parse_state, record_opt) =
            self.parse_artifact_bundle_typed(&corrected, &node_id, attempt as u32);

        // Log structured correction attempt record
        if let Some(ref record) = record_opt {
            log::info!(
                "PSP-7 correction attempt {}: parse_state={}, accepted={}, rejection={:?}",
                record.attempt,
                record.parse_state,
                record.accepted,
                record.rejection_reason
            );
        }

        match parse_state {
            perspt_core::types::ParseResultState::StrictJsonOk
            | perspt_core::types::ParseResultState::TolerantRecoveryOk => {
                let bundle = bundle_opt.expect("Accepted parse must yield a bundle");

                log::info!(
                    "Applying correction bundle ({}): {} artifact(s), {} command(s)",
                    parse_state,
                    bundle.artifacts.len(),
                    bundle.commands.len()
                );
                self.emit_log(format!(
                    "🔧 Applying correction bundle ({} artifact(s))",
                    bundle.artifacts.len()
                ));

                self.apply_bundle_transactionally(&bundle, &node_id, node_class)
                    .await?;
                self.last_tool_failure = None;

                // Track last written file from the bundle for build_correction_prompt
                let node_workdir = self.effective_working_dir(idx);
                if let Some(first_path) = bundle.artifacts.first().map(|a| a.path().to_string()) {
                    self.last_written_file = Some(node_workdir.join(&first_path));
                }
                self.file_version += 1;
                self.last_applied_bundle = Some(bundle.clone());

                // Record repair footprint
                let diagnosis_str = format!("{:?}", diagnosis);
                let footprint = perspt_core::RepairFootprint::new(
                    &self.context.session_id,
                    &node_id,
                    "initial",
                    attempt as u32,
                    &bundle,
                    &diagnosis_str,
                );
                self.last_repair_footprint = Some(footprint.clone());
                if let Err(e) = self.ledger.record_repair_footprint(&footprint) {
                    log::warn!("Failed to record repair footprint: {}", e);
                }

                // Execute bundle post-write commands
                if !bundle.commands.is_empty() {
                    self.emit_log(format!(
                        "🔧 Executing {} bundle command(s)...",
                        bundle.commands.len()
                    ));
                    let work_dir = self.effective_working_dir(idx);
                    let is_python = self.graph[idx].owner_plugin == "python";
                    for raw_command in &bundle.commands {
                        let command = if is_python {
                            Self::normalize_command_to_uv(raw_command)
                        } else {
                            raw_command.clone()
                        };
                        log::info!("Running correction command: {}", command);
                        let parts: Vec<&str> = command.split_whitespace().collect();
                        if parts.is_empty() {
                            continue;
                        }
                        let output = tokio::process::Command::new(parts[0])
                            .args(&parts[1..])
                            .current_dir(&work_dir)
                            .stdout(std::process::Stdio::piped())
                            .stderr(std::process::Stdio::piped())
                            .output()
                            .await;
                        match output {
                            Ok(o) if o.status.success() => {
                                self.emit_log(format!("✅ {}", command));
                            }
                            Ok(o) => {
                                let stderr = String::from_utf8_lossy(&o.stderr);
                                log::warn!("Command failed: {} — {}", command, stderr);
                            }
                            Err(e) => {
                                log::warn!("Failed to run command: {} — {}", command, e);
                            }
                        }
                    }
                }
            }

            perspt_core::types::ParseResultState::SemanticallyRejected => {
                // PSP-7: Classify the rejection for appropriate handling
                let rejection_reason = record_opt
                    .as_ref()
                    .and_then(|r| r.rejection_reason.clone())
                    .unwrap_or_default();

                let classification = if rejection_reason.contains("All artifacts rejected") {
                    perspt_core::types::RetryClassification::Retarget
                } else if rejection_reason.contains("support") {
                    perspt_core::types::RetryClassification::SupportFileViolation
                } else {
                    perspt_core::types::RetryClassification::Replan
                };

                log::warn!(
                    "Correction bundle semantically rejected ({:?}): {}",
                    classification,
                    rejection_reason
                );
                self.emit_log(format!(
                    "⚠️ Correction rejected ({:?}) — will retry",
                    classification
                ));

                // For Retarget: log expected vs dropped for diagnostics
                if matches!(
                    classification,
                    perspt_core::types::RetryClassification::Retarget
                ) {
                    if let Some(idx) = self.node_indices.get(&node_id) {
                        let expected: Vec<String> = self.graph[*idx]
                            .output_targets
                            .iter()
                            .map(|p| p.to_string_lossy().to_string())
                            .collect();
                        log::warn!(
                            "Expected targets: {}, but response targeted wrong files",
                            expected.join(", ")
                        );
                    }
                }
            }

            perspt_core::types::ParseResultState::NoStructuredPayload
            | perspt_core::types::ParseResultState::SchemaInvalid => {
                log::warn!(
                    "Correction response parse failed ({}), will retry with schema guidance",
                    parse_state
                );
                self.emit_log(format!(
                    "⚠️ Response parse failed ({}) — will retry",
                    parse_state
                ));
            }

            perspt_core::types::ParseResultState::EmptyBundle => {
                log::warn!("Correction produced empty bundle, will retry");
                self.emit_log("⚠️ Empty correction bundle — will retry".to_string());
            }
        }

        // PSP-7: Extract and execute standalone dependency commands via plugin policy
        let correction_cmds = Self::extract_commands_from_correction(&corrected, &owner_plugin);
        if !correction_cmds.is_empty() {
            self.emit_log(format!(
                "📦 Running {} dependency command(s) from correction...",
                correction_cmds.len()
            ));
            let work_dir = self.effective_working_dir(idx);
            for cmd in &correction_cmds {
                log::info!("Running correction command: {}", cmd);
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                if parts.is_empty() {
                    continue;
                }
                let output = tokio::process::Command::new(parts[0])
                    .args(&parts[1..])
                    .current_dir(&work_dir)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .output()
                    .await;
                match output {
                    Ok(o) if o.status.success() => {
                        self.emit_log(format!("✅ {}", cmd));
                    }
                    Ok(o) => {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        log::warn!("Command failed: {} — {}", cmd, stderr);
                    }
                    Err(e) => {
                        log::warn!("Failed to run command: {} — {}", cmd, e);
                    }
                }
            }
        }

        // Re-verify (recursive correction loop)
        let new_energy = self.step_verify(idx).await?;
        Box::pin(self.step_converge(idx, new_energy)).await
    }

    /// Build a correction prompt with diagnostic details.
    ///
    /// PSP-5 Phase 3: Language-agnostic, uses the node's actual output targets
    /// and includes formatted restriction-map context so the LLM has structural
    /// awareness during correction.
    ///
    /// When a RepairFootprint is available from a previous correction attempt,
    /// all affected files are included in the prompt so the LLM can see
    /// multi-file context (not just the single last-written file).
    fn build_correction_prompt(
        &self,
        node_id: &str,
        goal: &str,
        energy: &EnergyComponents,
    ) -> Result<String> {
        let diagnostics = &self.context.last_diagnostics;

        // Determine the node's owner plugin for language-specific examples
        let owner_plugin = self
            .node_indices
            .get(node_id)
            .map(|idx| self.graph[*idx].owner_plugin.as_str())
            .unwrap_or("");

        // Collect files to include in the prompt.
        // Priority: node's declared output_targets > repair footprint > last_written_file.
        // The old approach of falling back to last_written_file could show
        // the wrong file (e.g., root src/lib.rs from another node).
        let mut file_sections = Vec::new();
        let mut seen_paths = std::collections::HashSet::new();

        // 1. Include all of the node's declared output_targets (the files it SHOULD produce)
        if let Some(idx) = self.node_indices.get(node_id) {
            let node_workdir = self.effective_working_dir(*idx);
            for target in &self.graph[*idx].output_targets {
                let target_str = target.to_string_lossy().to_string();
                let full_path = node_workdir.join(target);
                if let Ok(content) = std::fs::read_to_string(&full_path) {
                    if !content.is_empty() && seen_paths.insert(target_str.clone()) {
                        file_sections.push((target_str, content));
                    }
                }
            }
        }

        // 2. Supplement with repair footprint files (may include files written by correction)
        if let Some(ref footprint) = self.last_repair_footprint {
            let node_workdir = if let Some(idx) = self.node_indices.get(&footprint.node_id) {
                self.effective_working_dir(*idx)
            } else if let Some(ref path) = self.last_written_file {
                path.parent()
                    .unwrap_or(std::path::Path::new("."))
                    .to_path_buf()
            } else {
                self.context.working_dir.clone()
            };
            for file_path in &footprint.affected_files {
                if seen_paths.insert(file_path.clone()) {
                    let full_path = node_workdir.join(file_path);
                    if let Ok(content) = std::fs::read_to_string(&full_path) {
                        if !content.is_empty() {
                            file_sections.push((file_path.clone(), content));
                        }
                    }
                }
            }
        }

        // 3. Include the workspace root manifest for structural context
        //    (helps the LLM understand crate layout for cross-crate imports)
        let root_manifest_names = ["Cargo.toml", "package.json", "pyproject.toml"];
        for manifest_name in &root_manifest_names {
            let manifest_path = self.context.working_dir.join(manifest_name);
            if manifest_path.exists() {
                let rel = manifest_name.to_string();
                if seen_paths.insert(rel.clone()) {
                    if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                        if !content.is_empty() {
                            file_sections.push((rel, content));
                        }
                    }
                }
                break; // Only include one root manifest
            }
        }

        // 4. Fallback to last_written_file only if nothing else was found
        if file_sections.is_empty() {
            let current_code = if let Some(ref path) = self.last_written_file {
                std::fs::read_to_string(path).unwrap_or_default()
            } else {
                String::new()
            };
            let file_path = self
                .last_written_file
                .as_ref()
                .map(|p| {
                    p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                })
                .unwrap_or_else(|| "unknown".to_string());
            file_sections.push((file_path, current_code));
        }

        // Detect language from first file extension for code fences
        let primary_path = &file_sections[0].0;
        let lang = std::path::Path::new(primary_path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|ext| match ext {
                "py" => "python",
                "rs" => "rust",
                "ts" | "tsx" => "typescript",
                "js" | "jsx" => "javascript",
                "go" => "go",
                "java" => "java",
                "rb" => "ruby",
                "c" | "h" => "c",
                "cpp" | "cc" | "cxx" | "hpp" => "cpp",
                "cs" => "csharp",
                other => other,
            })
            .unwrap_or("text");

        let mut prompt = format!(
            "## Code Correction Required\n\n\
             The code you generated has {} error(s) detected by the language toolchain.\n\
             Your task is to fix ALL errors and return the complete corrected file(s).\n\n\
             ### Original Goal\n{}\n\n\
             ### Current Code (with errors)\n",
            diagnostics.len(),
            goal,
        );

        // Include all affected files
        for (path, content) in &file_sections {
            prompt.push_str(&format!(
                "File: {}\n```{}\n{}\n```\n\n",
                path, lang, content
            ));
        }

        prompt.push_str(&format!(
            "### Detected Errors (V_syn = {:.2})\n",
            energy.v_syn
        ));

        // Add each diagnostic with specific fix direction
        for (i, diag) in diagnostics.iter().enumerate() {
            let fix_direction = self.get_fix_direction(diag);
            prompt.push_str(&format!(
                r#"
#### Error {}
- **Location**: Line {}, Column {}
- **Severity**: {}
- **Message**: {}
- **How to fix**: {}
"#,
                i + 1,
                diag.range.start.line + 1,
                diag.range.start.character + 1,
                severity_to_str(diag.severity),
                diag.message,
                fix_direction
            ));
        }

        // PSP-5 Phase 3: Include restriction-map context so the LLM can
        // reference structural dependencies and sealed interfaces during
        // correction instead of operating blind.
        if !self.last_formatted_context.is_empty() {
            prompt.push_str(&format!(
                "\n### Restriction Map Context\n\n{}\n",
                self.last_formatted_context
            ));
        }

        // Include the sandbox/workspace file tree so corrections target
        // paths that actually exist on disk.
        if let Some(idx) = self.node_indices.get(node_id) {
            let wd = self.effective_working_dir(*idx);
            if let Ok(tree) = crate::tools::list_sandbox_files(&wd) {
                if !tree.is_empty() {
                    prompt.push_str(&format!(
                        "\n### Current Project Tree\n\n```\n{}\n```\n",
                        tree.join("\n")
                    ));
                }
            }
        }

        // Include raw build/test output from plugin verification if available.
        // This is crucial because LSP diagnostics may not report missing crate
        // errors that `cargo check` / `cargo build` would catch.
        if let Some(ref test_output) = self.context.last_test_output {
            if !test_output.is_empty() {
                // Truncate to avoid blowing up the prompt
                let truncated = if test_output.len() > 3000 {
                    &test_output[..3000]
                } else {
                    test_output.as_str()
                };
                prompt.push_str(&format!(
                    "\n### Build / Test Output\nThe following is the raw output from the build toolchain (e.g. `cargo check` / `cargo build`). \
                     Use this to identify missing dependencies, unresolved imports, or type errors:\n```\n{}\n```\n",
                    truncated
                ));
            }
        }

        let multi_file = file_sections.len() > 1;
        let file_instruction = if multi_file {
            "Return ALL affected files as a JSON artifact bundle"
        } else {
            "Return the COMPLETE corrected file, not just snippets"
        };

        // Generate language-specific dependency command examples
        let commands_example = match owner_plugin {
            "rust" => "cargo add thiserror\ncargo add clap --features derive",
            "python" => "uv add httpx\nuv add --dev pytest",
            "javascript" => "npm install express\nnpm install --save-dev jest",
            _ => "cargo add thiserror\nuv add httpx",
        };

        prompt.push_str(&format!(
            r#"
### Fix Requirements
1. Fix ALL errors listed above - do not leave any unfixed
2. Maintain the original functionality and goal
3. Follow {} language conventions and idioms
4. Import any missing modules or dependencies
5. {}
6. If errors mention missing crates/packages (e.g. "can't find crate", "unresolved import" for an external dependency, "ModuleNotFoundError", "No module named"), list the required install commands

### Output Format
Provide the complete corrected file(s) followed by any dependency commands needed:

File: [same filename]
```{}
[complete corrected code]
```

Commands: [optional, one per line]
```
{}
```
"#,
            lang, file_instruction, lang, commands_example
        ));

        Ok(prompt)
    }

    /// Map diagnostic message patterns to specific fix directions
    fn get_fix_direction(&self, diag: &lsp_types::Diagnostic) -> String {
        let msg = diag.message.to_lowercase();

        if msg.contains("undefined") || msg.contains("unresolved") || msg.contains("not defined") {
            if msg.contains("crate") || msg.contains("module") {
                "The crate may not be in Cargo.toml. Add it with `cargo add <crate>` in the Commands section, or use `crate::` for intra-crate imports".into()
            } else {
                "Define the missing variable/function, or import it from the correct module".into()
            }
        } else if msg.contains("type") && (msg.contains("expected") || msg.contains("incompatible"))
        {
            "Change the value or add a type conversion to match the expected type".into()
        } else if msg.contains("import") || msg.contains("no module named") {
            "Add the correct import statement at the top of the file. For Python: use `uv add <pkg>` for external packages; use relative imports (`from . import mod`) inside package modules.".into()
        } else if msg.contains("argument") && (msg.contains("missing") || msg.contains("expected"))
        {
            "Provide all required arguments to the function call".into()
        } else if msg.contains("return") && msg.contains("type") {
            "Ensure the return statement returns a value of the declared return type".into()
        } else if msg.contains("attribute") {
            "Check if the object has this attribute, or fix the object type".into()
        } else if msg.contains("syntax") {
            "Fix the syntax error - check for missing colons, parentheses, or indentation".into()
        } else if msg.contains("indentation") {
            "Fix the indentation to match Python's indentation rules (4 spaces per level)".into()
        } else if msg.contains("parameter") {
            "Check the function signature and update parameter types/names".into()
        } else {
            format!("Review and fix: {}", diag.message)
        }
    }

    /// Call LLM for code correction using a verifier-guided two-stage flow.
    ///
    /// Stage 1 (verifier tier): Analyze the failure diagnostics and produce
    /// structured correction guidance — root cause, which lines/functions to
    /// change, and constraints to preserve.
    ///
    /// Stage 2 (actuator tier): Apply the verifier's guidance to produce
    /// the corrected code artifact.
    async fn call_llm_for_correction(&mut self, prompt: &str) -> Result<String> {
        // Stage 1: Verifier analyzes the failure
        let verifier_prompt = format!(
            "{}{}",
            crate::prompt_compiler::VERIFIER_ANALYSIS_PREAMBLE,
            prompt
        );

        log::debug!(
            "Stage 1: Sending analysis to verifier model: {}",
            self.verifier_model
        );
        let guidance = self
            .call_llm_with_logging(&self.verifier_model.clone(), &verifier_prompt, None)
            .await
            .unwrap_or_else(|e| {
                log::warn!(
                    "Verifier analysis failed ({}), falling back to actuator-only correction",
                    e
                );
                String::new()
            });

        // Stage 2: Actuator applies the guidance
        let actuator_prompt = if guidance.is_empty() {
            prompt.to_string()
        } else {
            format!(
                "{}\n\n## Verifier Analysis\n{}\n\nApply the above analysis to produce corrected code.",
                prompt, guidance
            )
        };

        log::debug!(
            "Stage 2: Sending correction to actuator model: {}",
            self.actuator_model
        );
        let response = self
            .call_llm_with_logging(&self.actuator_model.clone(), &actuator_prompt, None)
            .await?;
        log::debug!("Received correction response with {} chars", response.len());

        Ok(response)
    }

    /// Call LLM, always record token usage, and optionally persist full request/response text.
    pub(super) async fn call_llm_with_logging(
        &mut self,
        model: &str,
        prompt: &str,
        node_id: Option<&str>,
    ) -> Result<String> {
        let start = Instant::now();

        let llm_response = self
            .provider
            .generate_response_simple(model, prompt)
            .await?;

        let latency_ms = start.elapsed().as_millis() as i32;
        let tokens_in = llm_response.tokens_in.unwrap_or(0);
        let tokens_out = llm_response.tokens_out.unwrap_or(0);

        // Always record lightweight token/latency metrics regardless of --log-llm.
        if let Err(e) = self
            .ledger
            .record_llm_usage(model, node_id, latency_ms, tokens_in, tokens_out)
        {
            log::warn!("Failed to persist LLM usage metrics: {}", e);
        }

        // Always update budget envelope with estimated cost.
        // Rough estimate: $0.01 per 1K input tokens, $0.03 per 1K output tokens.
        let estimated_cost = (tokens_in as f64 * 0.00001) + (tokens_out as f64 * 0.00003);
        self.budget.record_cost(estimated_cost);

        // Optionally persist full prompt/response text when --log-llm is active.
        if self.context.log_llm {
            if let Err(e) = self.ledger.record_llm_request(
                model,
                prompt,
                &llm_response.text,
                node_id,
                latency_ms,
                tokens_in,
                tokens_out,
            ) {
                log::warn!("Failed to persist full LLM request: {}", e);
            }
        }

        log::debug!(
            "LLM call: model={}, latency={}ms, tokens_in={}, tokens_out={}, est_cost=${:.4}",
            model,
            latency_ms,
            tokens_in,
            tokens_out,
            estimated_cost,
        );

        Ok(llm_response.text)
    }

    /// PSP-5 Phase 1/4: Call LLM with tier-aware fallback.
    ///
    /// If the primary model returns a response that fails structured-output
    /// contract validation (`validator` returns `Err`), and a fallback model
    /// is configured for the given tier, retry with the fallback. Emits a
    /// `ModelFallback` event on switch. Returns the raw response string.
    pub(super) async fn call_llm_with_tier_fallback<F>(
        &mut self,
        primary_model: &str,
        prompt: &str,
        node_id: Option<&str>,
        tier: ModelTier,
        validator: F,
    ) -> Result<String>
    where
        F: Fn(&str) -> std::result::Result<(), String>,
    {
        // Try primary model
        let response = self
            .call_llm_with_logging(primary_model, prompt, node_id)
            .await?;

        // Validate structured output
        if validator(&response).is_ok() {
            return Ok(response);
        }

        let validation_err = validator(&response).unwrap_err();
        log::warn!(
            "Primary model '{}' failed structured-output contract for {:?}: {}",
            primary_model,
            tier,
            validation_err
        );

        // Look up fallback model for this tier (clone to avoid borrow conflict).
        let fallback_model = match tier {
            ModelTier::Architect => self.architect_fallback_model.clone(),
            ModelTier::Actuator => self.actuator_fallback_model.clone(),
            ModelTier::Verifier => self.verifier_fallback_model.clone(),
            ModelTier::Speculator => self.speculator_fallback_model.clone(),
        };

        // If no explicit fallback configured, retry with the same primary model.
        let fallback_model = fallback_model
            .as_deref()
            .unwrap_or(primary_model)
            .to_string();

        log::info!(
            "Falling back to model '{}' for {:?} tier",
            fallback_model,
            tier
        );
        self.emit_event_ref(perspt_core::AgentEvent::ModelFallback {
            node_id: node_id.unwrap_or("").to_string(),
            tier: format!("{:?}", tier),
            primary_model: primary_model.to_string(),
            fallback_model: fallback_model.to_string(),
            reason: validation_err,
        });

        self.call_llm_with_logging(&fallback_model, prompt, node_id)
            .await
    }

    /// Emit an event from a &self context (non-mutable).
    pub(super) fn emit_event_ref(&self, event: perspt_core::AgentEvent) {
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(event);
        }
    }
}
