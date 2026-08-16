use super::*;

impl SRBNOrchestrator {
    /// Step 3: Speculative Generation
    pub(crate) async fn step_speculate(&mut self, idx: NodeIndex) -> Result<()> {
        log::info!("Step 3: Speculation - Generating implementation");

        // PSP-5 Phase 3: Build context package for this node.
        // Use the sandbox directory when available so the LLM sees files
        // it will actually write to, falling back to the workspace root.
        let retriever = ContextRetriever::new(self.effective_working_dir(idx))
            .with_max_file_bytes(8 * 1024)
            .with_max_context_bytes(100 * 1024); // 100KB default budget

        let node = &self.graph[idx];
        let mut restriction_map =
            retriever.build_restriction_map(node, &self.context.ownership_manifest);

        // PSP-5 Phase 6: Inject sealed interface digests from parent nodes.
        // For each parent Interface node that has a recorded seal, add the
        // seal's structural digest to the restriction map so the context
        // package uses immutable sealed data instead of mutable parent files.
        self.inject_sealed_interfaces(idx, &mut restriction_map);

        let node = &self.graph[idx];
        let context_package = retriever.assemble_context_package(node, &restriction_map);
        let formatted_context = retriever.format_context_package(&context_package);

        // PSP-5 Phase 3: Enforce context budget — emit degradation event when
        // budget is exceeded or required owned files are missing.
        let node = &self.graph[idx];
        let missing_owned: Vec<String> = restriction_map
            .owned_files
            .iter()
            .filter(|f| {
                // Only treat as missing if not planned for creation by this node
                !context_package.included_files.contains_key(*f)
                    && !node
                        .output_targets
                        .iter()
                        .any(|ot| ot.to_string_lossy() == **f)
            })
            .cloned()
            .collect();

        if context_package.budget_exceeded || !missing_owned.is_empty() {
            let reason = if context_package.budget_exceeded && !missing_owned.is_empty() {
                format!(
                    "Budget exceeded and {} owned file(s) missing",
                    missing_owned.len()
                )
            } else if context_package.budget_exceeded {
                "Context budget exceeded; some files replaced with structural digests".to_string()
            } else {
                format!(
                    "{} owned file(s) could not be read: {}",
                    missing_owned.len(),
                    missing_owned.join(", ")
                )
            };

            log::warn!("Context degraded for node '{}': {}", node.node_id, reason);
            self.emit_log(format!("⚠️ Context degraded: {}", reason));
            self.emit_event(perspt_core::AgentEvent::ContextDegraded {
                node_id: node.node_id.clone(),
                budget_exceeded: context_package.budget_exceeded,
                missing_owned_files: missing_owned.clone(),
                included_file_count: context_package.included_files.len(),
                total_bytes: context_package.total_bytes,
                reason: reason.clone(),
            });

            // PSP-5 Phase 3: Block execution when required owned files are missing.
            // Budget-exceeded-but-all-owned-files-present is a warning, not a block.
            if !missing_owned.is_empty() {
                self.emit_event(perspt_core::AgentEvent::ContextBlocked {
                    node_id: node.node_id.clone(),
                    missing_owned_files: missing_owned,
                    reason: reason.clone(),
                });
                self.graph[idx].state = NodeState::Escalated;
                self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                    node_id: self.graph[idx].node_id.clone(),
                    status: perspt_core::NodeStatus::Escalated,
                });
                let err_msg = format!(
                    "Context blocked for node '{}': {}. Node escalated.",
                    self.graph[idx].node_id, reason
                );
                eprintln!("[SRBN-DIAG] {}", err_msg);
                return Err(anyhow::anyhow!(err_msg));
            }
        }

        // PSP-5 Phase 3: Pre-execution structural dependency check.
        // A node SHALL NOT proceed when only prose exists for a required dependency.
        {
            let node = &self.graph[idx];
            let prose_only_deps = self.check_structural_dependencies(node, &restriction_map);
            if !prose_only_deps.is_empty() {
                for (dep_node_id, dep_reason) in &prose_only_deps {
                    self.emit_event(perspt_core::AgentEvent::StructuralDependencyMissing {
                        node_id: node.node_id.clone(),
                        dependency_node_id: dep_node_id.clone(),
                        reason: dep_reason.clone(),
                    });
                }
                let dep_names: Vec<&str> =
                    prose_only_deps.iter().map(|(id, _)| id.as_str()).collect();
                let block_reason = format!(
                    "Required structural dependencies lack machine-verifiable digests (only prose \
                        summaries): [{}]",
                    dep_names.join(", ")
                );
                eprintln!(
                    "[SRBN-DIAG] Structural dependency check failed for '{}': {}",
                    self.graph[idx].node_id, block_reason
                );
                self.emit_log(format!("🚫 {}", block_reason));
                self.graph[idx].state = NodeState::Escalated;
                self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                    node_id: self.graph[idx].node_id.clone(),
                    status: perspt_core::NodeStatus::Escalated,
                });
                return Err(anyhow::anyhow!(
                    "Structural dependency check failed for node '{}': {}",
                    self.graph[idx].node_id,
                    block_reason
                ));
            }
        }

        // Record provenance for later commit
        self.last_context_provenance = Some(context_package.provenance());
        // Store formatted context for reuse in correction prompts
        self.last_formatted_context = formatted_context.clone();

        // PSP-5: Speculator lookahead — ask the speculator tier for bounded
        // hints about potential risks and downstream impacts before the
        // actuator generates code. Stored as ephemeral context, not committed.
        // Gated by planning policy: only LargeFeature/Greenfield/ArchitecturalRevision activate it.
        let speculator_hints = if self.planning_policy.needs_speculator() {
            let node_id = self.graph[idx].node_id.clone();
            let node_goal = self.graph[idx].goal.clone();
            let child_goals: Vec<String> = self
                .graph
                .edges(idx)
                .filter_map(|edge| {
                    let child = &self.graph[edge.target()];
                    if child.state == NodeState::TaskQueued {
                        Some(format!("- {}: {}", child.node_id, child.goal))
                    } else {
                        None
                    }
                })
                .collect();

            if !child_goals.is_empty() {
                let ev = perspt_core::types::PromptEvidence {
                    node_goal: Some(node_goal.clone()),
                    context_files: vec![node_id.clone()],
                    output_files: child_goals.clone(),
                    ..Default::default()
                };
                let speculator_prompt = crate::prompt_compiler::compile(
                    perspt_core::types::PromptIntent::SpeculatorLookahead,
                    &ev,
                )
                .text;

                log::debug!(
                    "Speculator lookahead for node {} using model {}",
                    node_id,
                    self.speculator_model
                );
                self.call_llm_with_logging(
                    &self.speculator_model.clone(),
                    &speculator_prompt,
                    Some(&node_id),
                )
                .await
                .unwrap_or_else(|e| {
                    log::warn!(
                        "Speculator lookahead failed ({}), proceeding without hints",
                        e
                    );
                    String::new()
                })
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let actuator = &self.agents[1];
        let node = &self.graph[idx];
        let node_id = node.node_id.clone();

        // Build prompt enriched with context package and speculator hints
        let base_prompt = actuator.build_prompt(node, &self.context);
        let mut prompt = if formatted_context.is_empty() {
            base_prompt
        } else {
            format!(
                "{}\n\n## Node Context (PSP-5 Restriction Map)\n\n{}",
                base_prompt, formatted_context
            )
        };

        if !speculator_hints.is_empty() {
            prompt = format!(
                "{}\n\n## Speculator Lookahead Hints\n\n{}",
                prompt, speculator_hints
            );
        }

        // Include sandbox/workspace file tree so the LLM has structural
        // awareness of the actual directory layout it is writing into.
        let wd = self.effective_working_dir(idx);
        if let Ok(tree) = crate::tools::list_sandbox_files(&wd) {
            if !tree.is_empty() {
                prompt = format!(
                    "{}\n\n## Current Project Tree\n\n```\n{}\n```",
                    prompt,
                    tree.join("\n")
                );
            }
        }

        let model = actuator.model().to_string();

        let response = self
            .call_llm_with_logging(&model, &prompt, Some(&node_id))
            .await?;

        let message = crate::types::AgentMessage::new(crate::types::ModelTier::Actuator, response);
        let content = &message.content;

        // Check for [COMMAND] blocks first (for TaskType::Command)
        if let Some(command) = self.extract_command_from_response(content) {
            log::info!("Extracted command: {}", command);
            self.emit_log(format!("🔧 Command proposed: {}", command));

            // Request approval before executing command
            let node_id = self.graph[idx].node_id.clone();
            let approval_result = self
                .await_approval_for_node(
                    perspt_core::ActionType::Command {
                        command: command.clone(),
                    },
                    format!("Execute shell command: {}", command),
                    None,
                    Some(&node_id),
                )
                .await;

            if !matches!(
                approval_result,
                ApprovalResult::Approved | ApprovalResult::ApprovedWithEdit(_)
            ) {
                self.emit_log("⏭️ Command skipped (not approved)");
                return Ok(());
            }

            // Execute command via AgentTools
            let mut args = HashMap::new();
            args.insert("command".to_string(), command.clone());

            let call = ToolCall {
                name: "run_command".to_string(),
                arguments: args,
            };

            let result = self.tools.execute(&call).await;
            if result.success {
                log::info!("✓ Command succeeded: {}", command);
                self.emit_log(format!("✅ Command succeeded: {}", command));
                self.emit_log(result.output);
            } else {
                log::warn!("Command failed: {:?}", result.error);
                self.emit_log(format!("❌ Command failed: {:?}", result.error));
            }
        }
        // PSP-7: Typed parse pipeline for initial generation
        else {
            let (bundle_opt, parse_state, record_opt) =
                self.parse_artifact_bundle_typed(content, &node_id, 0);

            if let Some(ref record) = record_opt {
                log::info!(
                    "PSP-7 initial gen: parse_state={}, accepted={}",
                    record.parse_state,
                    record.accepted
                );
            }

            match parse_state {
                perspt_core::types::ParseResultState::StrictJsonOk
                | perspt_core::types::ParseResultState::TolerantRecoveryOk => {
                    let bundle = bundle_opt.expect("Accepted parse must yield a bundle");
                    let affected_files: Vec<String> = bundle
                        .affected_paths()
                        .into_iter()
                        .map(ToString::to_string)
                        .collect();
                    log::info!(
                        "Parsed artifact bundle for node {} ({}): {} artifacts, {} commands",
                        node_id,
                        parse_state,
                        bundle.artifacts.len(),
                        bundle.commands.len()
                    );
                    self.emit_log(format!(
                        "📝 Bundle proposed: {} artifact(s) across {} file(s)",
                        bundle.artifacts.len(),
                        affected_files.len()
                    ));

                    let approval_result = self
                        .await_approval_for_node(
                            perspt_core::ActionType::BundleWrite {
                                node_id: node_id.clone(),
                                files: affected_files.clone(),
                            },
                            format!("Apply bundle touching: {}", affected_files.join(", ")),
                            serde_json::to_string_pretty(&bundle).ok(),
                            Some(&node_id),
                        )
                        .await;

                    if !matches!(
                        approval_result,
                        ApprovalResult::Approved | ApprovalResult::ApprovedWithEdit(_)
                    ) {
                        self.emit_log("⏭️ Bundle application skipped (not approved)");
                        return Ok(());
                    }

                    let node_class = self.graph[idx].node_class;
                    match self
                        .apply_bundle_transactionally(&bundle, &node_id, node_class)
                        .await
                    {
                        Ok(()) => {
                            self.last_tool_failure = None;
                            self.last_applied_bundle = Some(bundle.clone());
                        }
                        Err(e) => return Err(e),
                    }

                    // PSP-5 Phase 9: Execute post-write commands from the effective bundle
                    let effective_commands = self
                        .last_applied_bundle
                        .as_ref()
                        .map(|b| b.commands.clone())
                        .unwrap_or_default();
                    if !effective_commands.is_empty() {
                        self.emit_log(format!(
                            "🔧 Executing {} bundle command(s)...",
                            effective_commands.len()
                        ));
                        let work_dir = self.effective_working_dir(idx);
                        let is_python = self.graph[idx].owner_plugin == "python";
                        for raw_command in &effective_commands {
                            let command = if is_python {
                                Self::normalize_command_to_uv(raw_command)
                            } else {
                                raw_command.clone()
                            };

                            let cmd_approval = self
                                .await_approval_for_node(
                                    perspt_core::ActionType::Command {
                                        command: command.clone(),
                                    },
                                    format!("Execute bundle command: {}", command),
                                    None,
                                    Some(&node_id),
                                )
                                .await;

                            if !matches!(
                                cmd_approval,
                                ApprovalResult::Approved | ApprovalResult::ApprovedWithEdit(_)
                            ) {
                                self.emit_log(format!(
                                    "⏭️ Bundle command skipped (not approved): {}",
                                    command
                                ));
                                continue;
                            }

                            let mut args = HashMap::new();
                            args.insert("command".to_string(), command.clone());
                            args.insert(
                                "working_dir".to_string(),
                                work_dir.to_string_lossy().to_string(),
                            );

                            let call = ToolCall {
                                name: "run_command".to_string(),
                                arguments: args,
                            };

                            let result = self.tools.execute(&call).await;
                            if result.success {
                                log::info!("✓ Bundle command succeeded: {}", command);
                                self.emit_log(format!("✅ {}", command));
                                if !result.output.is_empty() {
                                    let truncated: String =
                                        result.output.chars().take(500).collect();
                                    self.emit_log(truncated);
                                }
                            } else {
                                let err_msg = result.error.unwrap_or_else(|| result.output.clone());
                                log::warn!("Bundle command failed: {} — {}", command, err_msg);
                                self.emit_log(format!(
                                    "❌ Command failed: {} — {}",
                                    command, err_msg
                                ));
                                self.last_tool_failure = Some(format!(
                                    "Bundle command '{}' failed: {}",
                                    command, err_msg
                                ));
                            }
                        }

                        if is_python {
                            log::info!("Running uv sync --dev after bundle commands...");
                            let sync_result = tokio::process::Command::new("uv")
                                .args(["sync", "--dev"])
                                .current_dir(&work_dir)
                                .stdout(std::process::Stdio::piped())
                                .stderr(std::process::Stdio::piped())
                                .output()
                                .await;
                            match sync_result {
                                Ok(output) if output.status.success() => {
                                    self.emit_log("🐍 uv sync --dev completed".to_string());
                                }
                                Ok(output) => {
                                    let stderr = String::from_utf8_lossy(&output.stderr);
                                    log::warn!("uv sync --dev failed: {}", stderr);
                                }
                                Err(e) => {
                                    log::warn!("Failed to run uv sync --dev: {}", e);
                                }
                            }
                        }
                    }
                }

                perspt_core::types::ParseResultState::SemanticallyRejected => {
                    // PSP-7: Retarget — extract raw paths and retry with focused prompt
                    log::warn!(
                        "Bundle for '{}' semantically rejected, retrying with retarget prompt",
                        node_id
                    );
                    self.emit_log(format!(
                        "🔄 Bundle for '{}' targeted wrong files — retrying...",
                        node_id
                    ));

                    let raw_paths: Vec<String> =
                        perspt_core::normalize::extract_file_markers(content)
                            .iter()
                            .filter_map(|m| m.path.clone())
                            .collect();
                    let expected: Vec<String> = self.graph[idx]
                        .output_targets
                        .iter()
                        .map(|p| p.to_string_lossy().to_string())
                        .collect();
                    let ev = perspt_core::types::PromptEvidence {
                        output_files: expected.clone(),
                        existing_file_contents: vec![(raw_paths.join(", "), prompt.clone())],
                        ..Default::default()
                    };
                    let retry_prompt = crate::prompt_compiler::compile(
                        perspt_core::types::PromptIntent::BundleRetarget,
                        &ev,
                    )
                    .text;

                    let retry_response = self
                        .call_llm_with_logging(&model, &retry_prompt, Some(&node_id))
                        .await?;

                    let (retry_bundle_opt, retry_state, _) =
                        self.parse_artifact_bundle_typed(&retry_response, &node_id, 1);

                    if let Some(retry_bundle) = retry_bundle_opt {
                        let node_class = self.graph[idx].node_class;
                        self.apply_bundle_transactionally(&retry_bundle, &node_id, node_class)
                            .await?;
                        self.last_tool_failure = None;
                        self.last_applied_bundle = Some(retry_bundle);
                    } else {
                        return Err(anyhow::anyhow!(
                            "Retry for '{}' did not produce a valid bundle ({})",
                            node_id,
                            retry_state
                        ));
                    }
                }

                _ => {
                    // NoStructuredPayload, SchemaInvalid, EmptyBundle
                    log::debug!(
                        "No artifact bundle found in response ({}), response length: {}",
                        parse_state,
                        content.len()
                    );
                    self.emit_log("ℹ️ No file changes detected in response".to_string());
                }
            }
        }

        self.context.history.push(message);
        Ok(())
    }

    /// Extract command from LLM response
    /// Looks for [COMMAND] pattern
    pub(crate) fn extract_command_from_response(&self, content: &str) -> Option<String> {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("[COMMAND]") {
                return Some(trimmed.trim_start_matches("[COMMAND]").trim().to_string());
            }
            // Also support ```bash blocks with a command annotation
            if trimmed.starts_with("$ ") || trimmed.starts_with("➜ ") {
                return Some(
                    trimmed
                        .trim_start_matches("$ ")
                        .trim_start_matches("➜ ")
                        .trim()
                        .to_string(),
                );
            }
        }
        None
    }

    // =========================================================================
    // PSP-5 Phase 5: Non-Convergence Classification and Repair
    // =========================================================================

    /// Get the current session ID
    pub fn session_id(&self) -> &str {
        &self.context.session_id
    }

    /// Get node count
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Start LSP clients for the given plugin names.
    ///
    /// For each name, looks up the plugin's `LspConfig` (with fallback)
    /// and starts a client keyed by the plugin name.
    pub async fn start_lsp_for_plugins(&mut self, plugin_names: &[&str]) -> Result<()> {
        let registry = perspt_core::plugin::PluginRegistry::new();

        for &name in plugin_names {
            if self.lsp_clients.contains_key(name) {
                log::debug!("LSP client already running for {}", name);
                continue;
            }

            let plugin = match registry.get(name) {
                Some(p) => p,
                None => {
                    log::warn!("No plugin found for '{}', skipping LSP startup", name);
                    continue;
                }
            };

            let profile = plugin.verifier_profile();
            let lsp_config = match profile.lsp.effective_config() {
                Some(cfg) => cfg.clone(),
                None => {
                    log::warn!(
                        "No available LSP for {} (primary and fallback unavailable)",
                        name
                    );
                    continue;
                }
            };

            log::info!(
                "Starting LSP for {}: {} {:?}",
                name,
                lsp_config.server_binary,
                lsp_config.args
            );

            let mut client = LspClient::from_config(&lsp_config);
            match client
                .start_with_config(&lsp_config, &self.context.working_dir)
                .await
            {
                Ok(()) => {
                    log::info!("{} LSP started successfully", name);
                    self.lsp_clients.insert(name.to_string(), client);
                }
                Err(e) => {
                    log::warn!(
                        "Failed to start {} LSP: {} (continuing without it)",
                        name,
                        e
                    );
                }
            }
        }

        Ok(())
    }

    /// Resolve the LSP client key for a given file path.
    ///
    /// Checks which registered plugin owns the file and returns its name,
    /// falling back to the first available LSP client.
    pub(crate) fn lsp_key_for_file(&self, path: &str) -> Option<String> {
        let registry = perspt_core::plugin::PluginRegistry::new();

        // First, try to find a plugin that owns this file
        for plugin in registry.all() {
            if plugin.owns_file(path) {
                let name = plugin.name().to_string();
                if self.lsp_clients.contains_key(&name) {
                    return Some(name);
                }
            }
        }

        // Fallback: return the first available client
        self.lsp_clients.keys().next().cloned()
    }

    // =========================================================================
    // PSP-000005: Multi-Artifact Bundle Parsing & Application
    // =========================================================================

    // =========================================================================
    // PSP-5 Phase 6: Provisional Branch Lifecycle
    // =========================================================================

    /// Resolve the sandbox directory for a node that has a provisional branch.
    /// Returns `None` for root nodes or nodes without branches.
    pub(crate) fn sandbox_dir_for_node(&self, idx: NodeIndex) -> Option<std::path::PathBuf> {
        let branch_id = self.graph[idx].provisional_branch_id.as_ref()?;
        let sandbox_path = self
            .context
            .working_dir
            .join(".perspt")
            .join("sandboxes")
            .join(&self.context.session_id)
            .join(branch_id);
        if sandbox_path.exists() {
            Some(sandbox_path)
        } else {
            None
        }
    }
}
