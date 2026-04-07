//! Artifact bundle parsing, transactional application, and path filtering.

use super::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

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

    /// PSP-7: Typed parse pipeline returning structured state for every LLM response.
    ///
    /// Replaces the Option-based `parse_artifact_bundle` with a pipeline that
    /// classifies every response through Layers A→E and returns a typed result.
    ///
    /// - **Layer A**: Raw capture — fingerprints the response (hash + length).
    /// - **Layer B**: Path normalization via the hardened `normalize_artifact_path`.
    /// - **Layer C**: Strict JSON parse via `extract_and_deserialize`.
    /// - **Layer D**: Tolerant file-marker recovery via `extract_file_markers`.
    /// - **Layer E**: Semantic validation — declared paths, plugin support files, command policy.
    ///
    /// Returns `(Option<ArtifactBundle>, ParseResultState, Option<CorrectionAttemptRecord>)`.
    pub fn parse_artifact_bundle_typed(
        &self,
        content: &str,
        node_id: &str,
        attempt: u32,
    ) -> (
        Option<perspt_core::types::ArtifactBundle>,
        perspt_core::types::ParseResultState,
        Option<perspt_core::types::CorrectionAttemptRecord>,
    ) {
        use perspt_core::types::{
            ArtifactBundle, ArtifactOperation, CorrectionAttemptRecord, ParseResultState,
        };

        // Layer A: raw capture — fingerprint the response
        let response_fingerprint = {
            let mut hasher = DefaultHasher::new();
            content.hash(&mut hasher);
            format!("{:016x}", hasher.finish())
        };
        let response_length = content.len();
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let build_record = |state: ParseResultState, accepted: bool, rejection: Option<String>| {
            CorrectionAttemptRecord {
                attempt,
                parse_state: state,
                retry_classification: None,
                response_fingerprint: response_fingerprint.clone(),
                response_length,
                energy_after: None,
                accepted,
                rejection_reason: rejection,
                created_at,
            }
        };

        // Layer C: Strict JSON parse
        if let Some(bundle) = self.try_parse_json_bundle(content) {
            if bundle.validate().is_ok() {
                // Layer B: normalize all paths in the bundle
                let bundle = self.normalize_bundle_paths(bundle);

                if bundle.artifacts.is_empty() {
                    let record = build_record(
                        ParseResultState::EmptyBundle,
                        false,
                        Some("Bundle is empty after path normalization".to_string()),
                    );
                    return (None, ParseResultState::EmptyBundle, Some(record));
                }

                // Layer E: semantic validation
                match self.semantic_validate_bundle(&bundle, node_id) {
                    Ok(filtered) => {
                        if filtered.artifacts.is_empty() {
                            let record = build_record(
                                ParseResultState::SemanticallyRejected,
                                false,
                                Some("All artifacts rejected by semantic validation".to_string()),
                            );
                            return (None, ParseResultState::SemanticallyRejected, Some(record));
                        }
                        let record = build_record(ParseResultState::StrictJsonOk, true, None);
                        return (Some(filtered), ParseResultState::StrictJsonOk, Some(record));
                    }
                    Err(reason) => {
                        let record = build_record(
                            ParseResultState::SemanticallyRejected,
                            false,
                            Some(reason),
                        );
                        return (None, ParseResultState::SemanticallyRejected, Some(record));
                    }
                }
            } else {
                log::warn!("JSON bundle found but failed schema validation");
                let record = build_record(
                    ParseResultState::SchemaInvalid,
                    false,
                    Some("JSON parsed but bundle schema validation failed".to_string()),
                );
                return (None, ParseResultState::SchemaInvalid, Some(record));
            }
        }

        // Layer D: Tolerant file-marker recovery
        let markers = perspt_core::normalize::extract_file_markers(content);
        if !markers.is_empty() {
            let artifacts: Vec<ArtifactOperation> = markers
                .into_iter()
                .filter_map(|m| {
                    let path = m.path?;
                    if m.content.is_empty() {
                        return None;
                    }
                    if m.is_diff {
                        Some(ArtifactOperation::Diff {
                            path,
                            patch: m.content,
                        })
                    } else {
                        Some(ArtifactOperation::Write {
                            path,
                            content: m.content,
                        })
                    }
                })
                .collect();

            if artifacts.is_empty() {
                let record = build_record(
                    ParseResultState::NoStructuredPayload,
                    false,
                    Some("File markers found but no named artifacts extracted".to_string()),
                );
                return (None, ParseResultState::NoStructuredPayload, Some(record));
            }

            let bundle = ArtifactBundle {
                artifacts,
                commands: vec![],
            };
            let bundle = self.normalize_bundle_paths(bundle);

            log::info!(
                "Tolerant recovery extracted {}-artifact bundle via file markers",
                bundle.len()
            );

            // Layer E: semantic validation
            match self.semantic_validate_bundle(&bundle, node_id) {
                Ok(filtered) => {
                    if filtered.artifacts.is_empty() {
                        let record = build_record(
                            ParseResultState::SemanticallyRejected,
                            false,
                            Some("All artifacts rejected by semantic validation".to_string()),
                        );
                        return (None, ParseResultState::SemanticallyRejected, Some(record));
                    }
                    let record = build_record(ParseResultState::TolerantRecoveryOk, true, None);
                    return (
                        Some(filtered),
                        ParseResultState::TolerantRecoveryOk,
                        Some(record),
                    );
                }
                Err(reason) => {
                    let record =
                        build_record(ParseResultState::SemanticallyRejected, false, Some(reason));
                    return (None, ParseResultState::SemanticallyRejected, Some(record));
                }
            }
        }

        // No structured payload at all
        let record = build_record(
            ParseResultState::NoStructuredPayload,
            false,
            Some("No JSON bundle or file markers found in response".to_string()),
        );
        (None, ParseResultState::NoStructuredPayload, Some(record))
    }

    /// Normalize all paths in a bundle through the hardened path normalizer.
    fn normalize_bundle_paths(
        &self,
        mut bundle: perspt_core::types::ArtifactBundle,
    ) -> perspt_core::types::ArtifactBundle {
        bundle.artifacts = bundle
            .artifacts
            .into_iter()
            .filter_map(|op| match op {
                perspt_core::types::ArtifactOperation::Write { path, content } => {
                    match perspt_core::path::normalize_artifact_path(&path) {
                        Ok(normalized) => Some(perspt_core::types::ArtifactOperation::Write {
                            path: normalized,
                            content,
                        }),
                        Err(e) => {
                            log::warn!("Dropping write artifact with bad path '{}': {}", path, e);
                            None
                        }
                    }
                }
                perspt_core::types::ArtifactOperation::Diff { path, patch } => {
                    match perspt_core::path::normalize_artifact_path(&path) {
                        Ok(normalized) => Some(perspt_core::types::ArtifactOperation::Diff {
                            path: normalized,
                            patch,
                        }),
                        Err(e) => {
                            log::warn!("Dropping diff artifact with bad path '{}': {}", path, e);
                            None
                        }
                    }
                }
                perspt_core::types::ArtifactOperation::Delete { path } => {
                    match perspt_core::path::normalize_artifact_path(&path) {
                        Ok(normalized) => {
                            Some(perspt_core::types::ArtifactOperation::Delete { path: normalized })
                        }
                        Err(e) => {
                            log::warn!("Dropping delete artifact with bad path '{}': {}", path, e);
                            None
                        }
                    }
                }
                perspt_core::types::ArtifactOperation::Move { from, to } => {
                    let from_norm = perspt_core::path::normalize_artifact_path(&from);
                    let to_norm = perspt_core::path::normalize_artifact_path(&to);
                    match (from_norm, to_norm) {
                        (Ok(f), Ok(t)) => {
                            Some(perspt_core::types::ArtifactOperation::Move { from: f, to: t })
                        }
                        _ => {
                            log::warn!("Dropping move artifact with bad paths '{}'→'{}'", from, to);
                            None
                        }
                    }
                }
            })
            .collect();
        bundle
    }

    /// PSP-7 Layer E: Semantic validation of a parsed bundle.
    ///
    /// Extends `filter_bundle_to_declared_paths` with plugin-driven checks:
    /// - Legal support files (from plugin `legal_support_files()`)
    /// - Dependency command policy (from plugin `dependency_command_policy()`)
    ///
    /// Returns `Ok(filtered_bundle)` or `Err(reason)` if validation fails hard.
    fn semantic_validate_bundle(
        &self,
        bundle: &perspt_core::types::ArtifactBundle,
        node_id: &str,
    ) -> Result<perspt_core::types::ArtifactBundle, String> {
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

        // If no output targets declared, pass everything through
        if allowed_paths.is_empty() {
            return Ok(bundle.clone());
        }

        // Get legal support files from the plugin
        let legal_support: std::collections::HashSet<String> = self
            .node_indices
            .get(node_id)
            .map(|idx| {
                let plugin_name = &self.graph[*idx].owner_plugin;
                let registry = perspt_core::plugin::PluginRegistry::new();
                registry
                    .get(plugin_name)
                    .map(|p| {
                        p.legal_support_files()
                            .iter()
                            .map(|s| s.to_string())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .unwrap_or_default()
            .into_iter()
            .collect();

        let (kept, dropped): (Vec<_>, Vec<_>) = bundle.artifacts.iter().cloned().partition(|a| {
            let normalized = perspt_core::path::normalize_artifact_path(a.path())
                .unwrap_or_else(|_| a.path().to_string());

            // Accept if in declared output targets
            if allowed_paths.contains(&normalized) {
                return true;
            }

            // Accept if it's a legal support file
            let filename = std::path::Path::new(&normalized)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default();
            if legal_support.contains(&filename) {
                log::info!(
                    "Accepting support file '{}' via plugin legal_support_files",
                    normalized
                );
                return true;
            }

            false
        });

        if !dropped.is_empty() {
            let dropped_paths: Vec<String> = dropped.iter().map(|a| a.path().to_string()).collect();
            log::warn!(
                "Semantic validation stripped {} artifact(s) from node '{}': {}",
                dropped.len(),
                node_id,
                dropped_paths.join(", ")
            );
        }

        // Validate commands via plugin dependency_command_policy
        let mut validated_commands = Vec::new();
        for cmd in &bundle.commands {
            let decision = self
                .node_indices
                .get(node_id)
                .and_then(|idx| {
                    let plugin_name = &self.graph[*idx].owner_plugin;
                    let registry = perspt_core::plugin::PluginRegistry::new();
                    registry
                        .get(plugin_name)
                        .map(|p| p.dependency_command_policy(cmd))
                })
                .unwrap_or(perspt_core::types::CommandPolicyDecision::Allow);

            match decision {
                perspt_core::types::CommandPolicyDecision::Allow => {
                    validated_commands.push(cmd.clone());
                }
                perspt_core::types::CommandPolicyDecision::RequireApproval => {
                    log::info!("Command '{}' requires approval — including with flag", cmd);
                    validated_commands.push(cmd.clone());
                }
                perspt_core::types::CommandPolicyDecision::Deny => {
                    log::warn!("Command '{}' denied by plugin policy", cmd);
                }
            }
        }

        Ok(perspt_core::types::ArtifactBundle {
            artifacts: kept,
            commands: validated_commands,
        })
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
        bundle.validate().map_err(|e| {
            eprintln!(
                "[SRBN-DIAG] Bundle validation failed for '{}': {}",
                node_id, e
            );
            anyhow::anyhow!(e)
        })?;

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
            eprintln!(
                "[SRBN-DIAG] All artifacts stripped for '{}': {:?}",
                node_id, dropped_paths
            );
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
