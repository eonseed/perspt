//! Sheaf validation and Merkle ledger commit (steps 6 and 7).

use super::*;

impl SRBNOrchestrator {
    /// Step 6: Sheaf Validation
    pub(super) async fn step_sheaf_validate(&mut self, idx: NodeIndex) -> Result<()> {
        log::info!("Step 6: Sheaf Validation - Cross-node consistency check");

        self.graph[idx].state = NodeState::SheafCheck;
        self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
            node_id: self.graph[idx].node_id.clone(),
            status: perspt_core::NodeStatus::SheafCheck,
        });

        // Determine which validators to run for this node.
        let validators = self.select_validators(idx);
        if validators.is_empty() {
            log::info!("No targeted validators selected — skipping sheaf check");
            return Ok(());
        }

        log::info!(
            "Running {} sheaf validators for node {}",
            validators.len(),
            self.graph[idx].node_id
        );

        let mut results = Vec::new();
        for class in &validators {
            let result = self.run_sheaf_validator(idx, *class);
            results.push(result);
        }

        // Persist all validation results.
        let persist_node_id = self.graph[idx].node_id.clone();
        for result in &results {
            if let Err(e) = self
                .ledger
                .record_sheaf_validation(&persist_node_id, result)
            {
                log::warn!("Failed to persist sheaf validation: {}", e);
            }
        }

        // Accumulate V_sheaf and check for failures.
        let total_v_sheaf: f32 = results.iter().map(|r| r.v_sheaf_contribution).sum();
        let failures: Vec<&SheafValidationResult> = results.iter().filter(|r| !r.passed).collect();
        let failure_count = failures.len();

        // Emit sheaf validation event.
        self.emit_event(perspt_core::AgentEvent::SheafValidationComplete {
            node_id: self.graph[idx].node_id.clone(),
            validators_run: results.len(),
            failures: failure_count,
            v_sheaf: total_v_sheaf,
        });

        if !failures.is_empty() {
            let node_id = self.graph[idx].node_id.clone();
            let evidence = failures
                .iter()
                .map(|f| format!("{}: {}", f.validator_class, f.evidence_summary))
                .collect::<Vec<_>>()
                .join("; ");

            self.emit_log(format!(
                "⚠️ Sheaf validation failed for {} (V_sheaf={:.3}): {}",
                node_id, total_v_sheaf, evidence
            ));

            // Collect unique requeue targets from all failures.
            let requeue_targets: Vec<String> = failures
                .iter()
                .flat_map(|f| f.requeue_targets.iter().cloned())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();

            if !requeue_targets.is_empty() {
                self.emit_log(format!(
                    "🔄 Requeuing {} nodes due to sheaf failures",
                    requeue_targets.len()
                ));
                for nid in &requeue_targets {
                    if let Some(&nidx) = self.node_indices.get(nid.as_str()) {
                        self.graph[nidx].state = NodeState::TaskQueued;
                        self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                            node_id: self.graph[nidx].node_id.clone(),
                            status: perspt_core::NodeStatus::Queued,
                        });
                    }
                }
            }

            anyhow::bail!("Sheaf validation failed for node {}: {}", node_id, evidence);
        }

        log::info!("Sheaf validation passed (V_sheaf={:.3})", total_v_sheaf);
        Ok(())
    }

    /// Select which validator classes are relevant for the given node
    /// based on its properties and graph position.
    pub(super) fn select_validators(&self, idx: NodeIndex) -> Vec<SheafValidatorClass> {
        let mut validators = Vec::new();

        // Always run dependency graph consistency — it's cheap and universal.
        validators.push(SheafValidatorClass::DependencyGraphConsistency);

        let node = &self.graph[idx];

        // Interface nodes need export/import consistency checks.
        if node.node_class == perspt_core::types::NodeClass::Interface {
            validators.push(SheafValidatorClass::ExportImportConsistency);
        }

        // Integration nodes cross ownership boundaries.
        if node.node_class == perspt_core::types::NodeClass::Integration {
            validators.push(SheafValidatorClass::ExportImportConsistency);
            validators.push(SheafValidatorClass::SchemaContractCompatibility);
        }

        // Nodes that touch multiple plugins get cross-language validation.
        // Skip when either side is "unknown" — that's an ownership detection
        // gap, not a real cross-language boundary.
        let node_owner = &node.owner_plugin;
        let has_cross_plugin_deps = self
            .graph
            .neighbors_directed(idx, petgraph::Direction::Outgoing)
            .any(|dep_idx| {
                let dep_plugin = &self.graph[dep_idx].owner_plugin;
                dep_plugin != node_owner
                    && !dep_plugin.is_empty()
                    && dep_plugin != "unknown"
                    && node_owner != "unknown"
            });
        if has_cross_plugin_deps {
            validators.push(SheafValidatorClass::CrossLanguageBoundary);
        }

        // If verification result is available and has build failures, check
        // build graph consistency.
        if let Some(ref vr) = self.last_verification_result {
            if !vr.build_ok {
                validators.push(SheafValidatorClass::BuildGraphConsistency);
            }
            // Only check test ownership when tests actually ran and failed
            // (tests_failed > 0), not when tests were skipped due to
            // syntax/build short-circuit (which leaves tests_ok = false
            // with tests_failed = 0).
            if !vr.tests_ok && vr.tests_failed > 0 {
                validators.push(SheafValidatorClass::TestOwnershipConsistency);
            }
        }

        validators
    }

    /// Run a single sheaf validator class against the current node context.
    pub(super) fn run_sheaf_validator(
        &self,
        idx: NodeIndex,
        class: SheafValidatorClass,
    ) -> SheafValidationResult {
        let node = &self.graph[idx];
        let node_id = &node.node_id;

        match class {
            SheafValidatorClass::DependencyGraphConsistency => {
                // Check for cycles in the task graph.
                if petgraph::algo::is_cyclic_directed(&self.graph) {
                    SheafValidationResult::failed(
                        class,
                        "Cyclic dependency detected in task graph",
                        vec![node_id.clone()],
                        self.affected_dependents(idx),
                        0.5,
                    )
                } else {
                    SheafValidationResult::passed(class, vec![node_id.clone()])
                }
            }
            SheafValidatorClass::ExportImportConsistency => {
                // Check that outgoing neighbors' context files include this node's outputs.
                let manifest = &self.context.ownership_manifest;
                let mut mismatched = Vec::new();

                for target in &node.output_targets {
                    let target_str = target.to_string_lossy();
                    if let Some(entry) = manifest.owner_of(&target_str) {
                        if entry.owner_node_id != *node_id {
                            mismatched.push(target_str.to_string());
                        }
                    }
                }

                if mismatched.is_empty() {
                    SheafValidationResult::passed(class, vec![node_id.clone()])
                } else {
                    SheafValidationResult::failed(
                        class,
                        format!(
                            "Ownership mismatch on {} file(s): {}",
                            mismatched.len(),
                            mismatched.join(", ")
                        ),
                        mismatched,
                        vec![node_id.clone()],
                        0.3,
                    )
                }
            }
            SheafValidatorClass::SchemaContractCompatibility => {
                // Check that the node's behavioral contract is not empty.
                let contract = &node.contract;
                if contract.invariants.is_empty() && contract.interface_signature.is_empty() {
                    SheafValidationResult::failed(
                        class,
                        "Integration node has empty contract",
                        node.output_targets
                            .iter()
                            .map(|t| t.to_string_lossy().to_string())
                            .collect(),
                        vec![node_id.clone()],
                        0.2,
                    )
                } else {
                    SheafValidationResult::passed(class, vec![node_id.clone()])
                }
            }
            SheafValidatorClass::BuildGraphConsistency => {
                // When build fails, check if this node's files are referenced
                // by others that might have broken.
                let dependents = self.affected_dependents(idx);
                if dependents.is_empty() {
                    SheafValidationResult::passed(class, vec![node_id.clone()])
                } else {
                    SheafValidationResult::failed(
                        class,
                        format!(
                            "Build failed with {} dependent nodes potentially affected",
                            dependents.len()
                        ),
                        node.output_targets
                            .iter()
                            .map(|t| t.to_string_lossy().to_string())
                            .collect(),
                        dependents,
                        0.4,
                    )
                }
            }
            SheafValidatorClass::TestOwnershipConsistency => {
                // When tests fail, attribute failures to the owning node.
                let owned_files = self.context.ownership_manifest.files_owned_by(node_id);
                if owned_files.is_empty() {
                    SheafValidationResult::passed(class, vec![node_id.clone()])
                } else {
                    // If tests are failing and this node owns files, it's a
                    // candidate for re-examination.
                    SheafValidationResult::failed(
                        class,
                        format!(
                            "Test failures may be attributed to {} owned file(s)",
                            owned_files.len()
                        ),
                        owned_files.iter().map(|s| s.to_string()).collect(),
                        vec![node_id.clone()],
                        0.3,
                    )
                }
            }
            SheafValidatorClass::CrossLanguageBoundary => {
                // Check that cross-plugin dependencies have matching plugins.
                // Skip "unknown" plugins — they indicate undetected ownership,
                // not an actual cross-language boundary issue.
                let mut boundary_issues = Vec::new();
                let node_plugin = &node.owner_plugin;

                for dep_idx in self
                    .graph
                    .neighbors_directed(idx, petgraph::Direction::Outgoing)
                {
                    let dep = &self.graph[dep_idx];
                    if dep.owner_plugin != *node_plugin
                        && !dep.owner_plugin.is_empty()
                        && dep.owner_plugin != "unknown"
                        && *node_plugin != "unknown"
                    {
                        // Cross-plugin edge — check both are active.
                        if !self.context.active_plugins.contains(&dep.owner_plugin) {
                            boundary_issues.push(format!("plugin {} not active", dep.owner_plugin));
                        }
                    }
                }

                if boundary_issues.is_empty() {
                    SheafValidationResult::passed(class, vec![node_id.clone()])
                } else {
                    SheafValidationResult::failed(
                        class,
                        boundary_issues.join("; "),
                        vec![node_id.clone()],
                        self.affected_dependents(idx),
                        0.4,
                    )
                }
            }
            SheafValidatorClass::PolicyInvariantConsistency => {
                // Placeholder: policy checks would consult perspt-policy crate.
                SheafValidationResult::passed(class, vec![node_id.clone()])
            }
        }
    }

    /// Step 7: Merkle Ledger Commit
    pub(super) async fn step_commit(&mut self, idx: NodeIndex) -> Result<()> {
        log::info!("Step 7: Committing stable state to ledger");

        self.graph[idx].state = NodeState::Committing;
        self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
            node_id: self.graph[idx].node_id.clone(),
            status: perspt_core::NodeStatus::Committing,
        });

        // PSP-5 Phase 6: Copy sandbox artifacts back to live workspace before
        // persisting any state, so the commit reflects the final files.
        if let Some(sandbox_dir) = self.sandbox_dir_for_node(idx) {
            match crate::tools::list_sandbox_files(&sandbox_dir) {
                Ok(files) => {
                    for rel in &files {
                        if let Err(e) = crate::tools::copy_from_sandbox(
                            &sandbox_dir,
                            &self.context.working_dir,
                            rel,
                        ) {
                            log::warn!("Failed to export sandbox file {}: {}", rel, e);
                        }
                    }
                    if !files.is_empty() {
                        self.emit_log(format!(
                            "📦 Exported {} file(s) from sandbox to workspace",
                            files.len()
                        ));
                    }
                }
                Err(e) => {
                    log::warn!("Failed to list sandbox files: {}", e);
                }
            }
        }

        // PSP-5 Phase 3: Record context provenance if available
        if let Some(provenance) = self.last_context_provenance.take() {
            if let Err(e) = self.ledger.record_context_provenance(&provenance) {
                log::warn!("Failed to record context provenance: {}", e);
            }
        }

        // PSP-5 Phase 8: Persist verification result before marking completion
        if let Some(ref vr) = self.last_verification_result {
            self.ledger
                .record_verification_result(&self.graph[idx].node_id, vr)
                .map_err(|e| {
                    anyhow::anyhow!(
                        "Commit blocked: failed to persist verification result for {}: {}",
                        self.graph[idx].node_id,
                        e
                    )
                })?;
        }

        // PSP-5 Phase 9: Persist artifact bundle if one was applied for this node
        if let Some(bundle) = self.last_applied_bundle.take() {
            if let Err(e) = self
                .ledger
                .record_artifact_bundle(&self.graph[idx].node_id, &bundle)
            {
                log::warn!(
                    "Failed to persist artifact bundle for {}: {}",
                    self.graph[idx].node_id,
                    e
                );
            }
        }

        // PSP-5 Phase 8: Persist full node snapshot via the rich commit API.
        // This captures graph-structural metadata, retry/error context, and
        // merkle material. Partial-write failure blocks completion.
        let node = &self.graph[idx];
        let children_json = if node.children.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&node.children).unwrap_or_default())
        };

        let payload = crate::ledger::NodeCommitPayload {
            node_id: node.node_id.clone(),
            state: "Completed".to_string(),
            v_total: node.monitor.current_energy(),
            merkle_hash: node.interface_seal_hash.map(|h| h.to_vec()),
            attempt_count: node.monitor.attempt_count as i32,
            node_class: Some(node.node_class.to_string()),
            owner_plugin: if node.owner_plugin.is_empty() {
                None
            } else {
                Some(node.owner_plugin.clone())
            },
            goal: Some(node.goal.clone()),
            parent_id: node.parent_id.clone(),
            children: children_json,
            last_error_type: self
                .last_tool_failure
                .as_ref()
                .map(|f| f.chars().take(200).collect()),
        };

        self.ledger.commit_node_snapshot(&payload).map_err(|e| {
            anyhow::anyhow!(
                "Commit blocked: failed to persist node snapshot for {}: {}",
                self.graph[idx].node_id,
                e
            )
        })?;

        // PSP-5 Phase 6: Emit interface seals for Interface-class nodes
        self.emit_interface_seals(idx);

        self.graph[idx].state = NodeState::Completed;
        self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
            node_id: self.graph[idx].node_id.clone(),
            status: perspt_core::NodeStatus::Completed,
        });

        // PSP-5 Phase 6: Unblock dependents that were waiting on this node's seal
        self.unblock_dependents(idx);

        log::info!("Node {} committed", self.graph[idx].node_id);
        Ok(())
    }
}
