//! Artifact bundle parsing, transactional application, and path filtering.

use super::*;

impl SRBNOrchestrator {
    /// PSP-5: Parse an artifact bundle from LLM response
    ///
    /// Tries structured JSON bundle first, falls back to legacy `File:`/`Diff:` extraction.
    /// Returns None if no artifacts could be extracted.
    pub fn parse_artifact_bundle(
        &self,
        content: &str,
    ) -> Option<perspt_core::types::ArtifactBundle> {
        // Try structured JSON bundle first
        if let Some(bundle) = self.try_parse_json_bundle(content) {
            if let Ok(()) = bundle.validate() {
                log::info!(
                    "Parsed structured artifact bundle: {} artifacts",
                    bundle.len()
                );
                return Some(bundle);
            } else {
                log::warn!("JSON bundle found but failed validation, falling back to legacy");
            }
        }

        // Fall back to legacy File:/Diff: extraction — collect ALL blocks
        let blocks = self.extract_all_code_blocks_from_response(content);
        if !blocks.is_empty() {
            let artifacts: Vec<perspt_core::types::ArtifactOperation> = blocks
                .into_iter()
                .map(|(filename, code, is_diff)| {
                    if is_diff {
                        perspt_core::types::ArtifactOperation::Diff {
                            path: filename,
                            patch: code,
                        }
                    } else {
                        perspt_core::types::ArtifactOperation::Write {
                            path: filename,
                            content: code,
                        }
                    }
                })
                .collect();
            log::info!(
                "Constructed {}-artifact bundle from legacy extraction",
                artifacts.len()
            );
            let bundle = perspt_core::types::ArtifactBundle {
                artifacts,
                commands: vec![],
            };
            return Some(bundle);
        }

        None
    }

    /// Try to parse a JSON artifact bundle from content
    ///
    /// PSP-5 Phase 4: Uses the provider-neutral normalization layer.
    fn try_parse_json_bundle(&self, content: &str) -> Option<perspt_core::types::ArtifactBundle> {
        match perspt_core::normalize::extract_and_deserialize::<perspt_core::types::ArtifactBundle>(
            content,
        ) {
            Ok((bundle, method)) => {
                log::info!("Parsed ArtifactBundle via normalization ({})", method);
                Some(bundle)
            }
            Err(e) => {
                log::debug!("Normalization could not extract ArtifactBundle: {}", e);
                None
            }
        }
    }

    /// PSP-5: Apply an artifact bundle transactionally
    ///
    /// All file operations are validated first, then applied.
    /// PSP-5 Phase 2: Validates ownership boundaries before applying.
    /// If any operation fails, the method returns an error describing which operation
    /// failed, and previous successful operations are logged for manual review.
    pub async fn apply_bundle_transactionally(
        &mut self,
        bundle: &perspt_core::types::ArtifactBundle,
        node_id: &str,
        node_class: perspt_core::types::NodeClass,
    ) -> Result<()> {
        let idx =
            self.node_indices.get(node_id).copied().ok_or_else(|| {
                anyhow::anyhow!("Unknown node '{}' for bundle application", node_id)
            })?;
        let node_workdir = self.effective_working_dir(idx);

        // Validate structural integrity first
        bundle.validate().map_err(|e| anyhow::anyhow!(e))?;

        // Filter out undeclared paths instead of failing the entire session
        let filtered = self.filter_bundle_to_declared_paths(bundle, node_id);

        // If filtering removed ALL artifacts, fail so the correction loop can
        // retry with proper paths.  The old fallback applied the *unfiltered*
        // bundle, causing cross-node file pollution (e.g., overwriting root
        // Cargo.toml with a crate-level manifest).
        if filtered.artifacts.is_empty() && !bundle.artifacts.is_empty() {
            let dropped_paths: Vec<String> = bundle
                .artifacts
                .iter()
                .map(|a| a.path().to_string())
                .collect();
            log::warn!(
                "All artifacts stripped for node '{}' — skipping bundle application. \
                 Dropped paths: {}",
                node_id,
                dropped_paths.join(", ")
            );
            self.emit_log(format!(
                "⚠️ All artifacts for '{}' targeted undeclared paths — bundle skipped. \
                 The actuator's output_files don't match the plan.",
                node_id
            ));
            return Err(anyhow::anyhow!(
                "All {} artifact(s) targeted undeclared paths for node '{}': [{}]. \
                 Expected paths: {:?}",
                bundle.artifacts.len(),
                node_id,
                dropped_paths.join(", "),
                self.node_indices
                    .get(node_id)
                    .map(|idx| self.graph[*idx]
                        .output_targets
                        .iter()
                        .map(|p| p.to_string_lossy().to_string())
                        .collect::<Vec<_>>())
                    .unwrap_or_default()
            ));
        }
        let bundle = filtered;

        // PSP-5 Phase 2: Validate ownership boundaries (soft failure)
        // Instead of crashing the session, log ownership conflicts and
        // continue — the LLM often generates shared files (e.g. config.json)
        // from multiple nodes.
        if let Err(e) = self
            .context
            .ownership_manifest
            .validate_bundle(&bundle, node_id, node_class)
        {
            log::warn!("Ownership validation warning for node '{}': {}", node_id, e);
            self.emit_log(format!("⚠️ Ownership warning: {}", e));
        }

        // PSP-5 Phase 2: Determine owner_plugin for new path assignment
        let owner_plugin = self
            .node_indices
            .get(node_id)
            .and_then(|idx| {
                let plugin = &self.graph[*idx].owner_plugin;
                if plugin.is_empty() {
                    None
                } else {
                    Some(plugin.clone())
                }
            })
            .unwrap_or_else(|| "unknown".to_string());

        let mut files_created: Vec<String> = Vec::new();
        let mut files_modified: Vec<String> = Vec::new();
        let mut files_deleted: Vec<String> = Vec::new();

        for op in &bundle.artifacts {
            let mut args = HashMap::new();
            let resolved_path = node_workdir.join(op.path());
            args.insert(
                "path".to_string(),
                resolved_path.to_string_lossy().to_string(),
            );

            let call = match op {
                perspt_core::types::ArtifactOperation::Write { content, .. } => {
                    args.insert("content".to_string(), content.clone());
                    ToolCall {
                        name: "write_file".to_string(),
                        arguments: args,
                    }
                }
                perspt_core::types::ArtifactOperation::Diff { patch, .. } => {
                    args.insert("diff".to_string(), patch.clone());
                    ToolCall {
                        name: "apply_diff".to_string(),
                        arguments: args,
                    }
                }
                perspt_core::types::ArtifactOperation::Delete { path } => {
                    // Validate delete through policy layer
                    if let Err(e) = perspt_policy::sanitize::validate_artifact_mutation(
                        path,
                        &self.context.working_dir,
                        "Delete",
                    ) {
                        log::warn!("Delete blocked by policy: {}", e);
                        self.emit_log(format!("⚠️ Delete blocked: {}", e));
                        continue;
                    }
                    ToolCall {
                        name: "delete_file".to_string(),
                        arguments: args,
                    }
                }
                perspt_core::types::ArtifactOperation::Move { from, to } => {
                    // Validate both source and destination through policy
                    if let Err(e) = perspt_policy::sanitize::validate_artifact_mutation(
                        from,
                        &self.context.working_dir,
                        "Move",
                    ) {
                        log::warn!("Move source blocked by policy: {}", e);
                        self.emit_log(format!("⚠️ Move blocked: {}", e));
                        continue;
                    }
                    if let Err(e) = perspt_policy::sanitize::validate_artifact_mutation(
                        to,
                        &self.context.working_dir,
                        "Move",
                    ) {
                        log::warn!("Move destination blocked by policy: {}", e);
                        self.emit_log(format!("⚠️ Move blocked: {}", e));
                        continue;
                    }
                    let resolved_to = node_workdir.join(to);
                    args.insert("from".to_string(), args["path"].clone());
                    args.insert("to".to_string(), resolved_to.to_string_lossy().to_string());
                    ToolCall {
                        name: "move_file".to_string(),
                        arguments: args,
                    }
                }
            };

            let result = self.tools.execute(&call).await;
            if result.success {
                let full_path = resolved_path.clone();

                if op.is_write() {
                    files_created.push(op.path().to_string());
                } else if op.is_delete() {
                    files_deleted.push(op.path().to_string());
                    self.emit_event(perspt_core::AgentEvent::FileDeleted {
                        node_id: self.graph[idx].node_id.clone(),
                        path: op.path().to_string(),
                    });
                } else if op.is_move() {
                    if let perspt_core::types::ArtifactOperation::Move { to, .. } = op {
                        files_modified.push(format!("{} -> {}", op.path(), to));
                        self.emit_event(perspt_core::AgentEvent::FileMoved {
                            node_id: self.graph[idx].node_id.clone(),
                            from: op.path().to_string(),
                            to: to.to_string(),
                        });
                    }
                } else {
                    files_modified.push(op.path().to_string());
                }

                // Track for LSP notification (skip for deleted files)
                if !op.is_delete() {
                    self.last_written_file = Some(full_path.clone());
                    self.file_version += 1;

                    // Notify LSP of file change
                    let registry = perspt_core::plugin::PluginRegistry::new();
                    for (lang, client) in self.lsp_clients.iter_mut() {
                        // Only notify if the plugin owns this file
                        let should_notify = match registry.get(lang) {
                            Some(plugin) => plugin.owns_file(op.path()),
                            None => true,
                        };
                        if should_notify {
                            if let Ok(content) = std::fs::read_to_string(&full_path) {
                                let _ = client
                                    .did_change(&full_path, &content, self.file_version)
                                    .await;
                            }
                        }
                    }
                }

                log::info!("✓ Applied: {}", op.path());
                self.emit_log(format!("✅ Applied: {}", op.path()));
            } else {
                log::warn!("Failed to apply {}: {:?}", op.path(), result.error);
                self.emit_log(format!("❌ Failed: {} - {:?}", op.path(), result.error));
                self.last_tool_failure = result.error.clone();
                return Err(anyhow::anyhow!(
                    "Bundle application failed at {}: {:?}",
                    op.path(),
                    result.error
                ));
            }
        }

        // PSP-5 Phase 2: Auto-assign unregistered paths to this node
        self.context.ownership_manifest.assign_new_paths(
            &bundle,
            node_id,
            &owner_plugin,
            node_class,
        );

        // Emit BundleApplied event
        self.emit_event(perspt_core::AgentEvent::BundleApplied {
            node_id: node_id.to_string(),
            files_created,
            files_modified,
            writes_count: bundle.writes_count(),
            diffs_count: bundle.diffs_count(),
            node_class: node_class.to_string(),
        });

        self.last_tool_failure = None;
        Ok(())
    }

    /// Validate and strip undeclared paths from a bundle.
    ///
    /// Instead of failing the entire session, this method removes artifacts
    /// targeting paths not listed in the node's `output_targets` and logs
    /// warnings.  Returns the filtered bundle.
    fn filter_bundle_to_declared_paths(
        &self,
        bundle: &perspt_core::types::ArtifactBundle,
        node_id: &str,
    ) -> perspt_core::types::ArtifactBundle {
        let allowed_paths: std::collections::HashSet<String> = self
            .node_indices
            .get(node_id)
            .map(|idx| {
                self.graph[*idx]
                    .output_targets
                    .iter()
                    .map(|p| {
                        let raw = p.to_string_lossy();
                        perspt_core::path::normalize_artifact_path(&raw)
                            .unwrap_or_else(|_| raw.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default();

        if allowed_paths.is_empty() {
            return bundle.clone();
        }

        let (kept, dropped): (Vec<_>, Vec<_>) = bundle.artifacts.iter().cloned().partition(|a| {
            let normalized = perspt_core::path::normalize_artifact_path(a.path())
                .unwrap_or_else(|_| a.path().to_string());
            allowed_paths.contains(&normalized)
        });

        if !dropped.is_empty() {
            let dropped_paths: Vec<String> = dropped.iter().map(|a| a.path().to_string()).collect();
            log::warn!(
                "Stripped {} undeclared artifact(s) from node '{}': {}",
                dropped.len(),
                node_id,
                dropped_paths.join(", ")
            );
            self.emit_log(format!(
                "⚠️ Stripped {} undeclared path(s) from bundle: {}",
                dropped.len(),
                dropped_paths.join(", ")
            ));
        }

        perspt_core::types::ArtifactBundle {
            artifacts: kept,
            commands: bundle.commands.clone(),
        }
    }
}
