use super::*;

impl SRBNOrchestrator {
    /// PSP-7: Lightweight sheaf pre-check before full sheaf validation.
    ///
    /// Verifies that every declared output target actually exists on disk and
    /// is non-empty, and that files contain real implementations rather than
    /// stub/placeholder content. Returns `Some(evidence)` if the pre-check
    /// fails, `None` if everything looks good.
    pub(crate) fn sheaf_pre_check(&self, idx: NodeIndex) -> Option<String> {
        let node = &self.graph[idx];
        if node.output_targets.is_empty() {
            return None;
        }

        let work_dir = self.effective_working_dir(idx);
        let mut issues = Vec::new();

        for path in &node.output_targets {
            let full = work_dir.join(path);
            match std::fs::metadata(&full) {
                Ok(m) if m.len() == 0 => {
                    issues.push(format!("empty: {}", path.display()));
                }
                Err(_) => {
                    issues.push(format!("missing: {}", path.display()));
                }
                Ok(_) => {
                    // Check for stub/placeholder content in existing non-empty files.
                    if let Some(reason) = detect_stub_content(&full, &node.owner_plugin) {
                        issues.push(format!("stub content in {}: {}", path.display(), reason));
                    }
                }
            }
        }

        if issues.is_empty() {
            None
        } else {
            Some(format!("Output target issues: {}", issues.join(", ")))
        }
    }

    /// Return the effective working directory for a node: sandbox if the node
    /// has an active provisional branch, otherwise the live workspace.
    pub(crate) fn effective_working_dir(&self, idx: NodeIndex) -> std::path::PathBuf {
        self.sandbox_dir_for_node(idx)
            .unwrap_or_else(|| self.context.working_dir.clone())
    }

    /// Create a provisional branch if the node has graph parents (i.e., it
    /// depends on another node's output). Returns the branch ID if created.
    pub(crate) fn maybe_create_provisional_branch(&mut self, idx: NodeIndex) -> Option<String> {
        // Find incoming edges (parents this node depends on)
        let parents: Vec<NodeIndex> = self
            .graph
            .neighbors_directed(idx, petgraph::Direction::Incoming)
            .collect();

        let node = &self.graph[idx];
        let node_id = node.node_id.clone();
        let session_id = self.context.session_id.clone();

        // Root nodes and child nodes both get sandboxes.  Root nodes use
        // "root" as the parent identifier since they have no graph parent.
        let parent_node_id = if parents.is_empty() {
            "root".to_string()
        } else {
            self.graph[parents[0]].node_id.clone()
        };

        let branch_id = format!("branch_{}_{}", node_id, uuid::Uuid::new_v4());
        let branch = ProvisionalBranch::new(
            branch_id.clone(),
            session_id.clone(),
            node_id.clone(),
            parent_node_id.clone(),
        );

        // Persist via ledger
        if let Err(e) = self.ledger.record_provisional_branch(&branch) {
            log::warn!("Failed to record provisional branch: {}", e);
        }

        // Record lineage edges for every parent (skipped for root nodes)
        for pidx in &parents {
            let parent_id = self.graph[*pidx].node_id.clone();
            // Determine if this parent is an Interface node (seal dependency)
            let depends_on_seal = self.graph[*pidx].node_class == NodeClass::Interface;
            let lineage = perspt_core::types::BranchLineage {
                lineage_id: format!("lin_{}_{}", branch_id, parent_id),
                parent_branch_id: parent_id,
                child_branch_id: branch_id.clone(),
                depends_on_seal,
            };
            if let Err(e) = self.ledger.record_branch_lineage(&lineage) {
                log::warn!("Failed to record branch lineage: {}", e);
            }
        }

        // Store branch ID on the node for tracking
        self.graph[idx].provisional_branch_id = Some(branch_id.clone());

        // PSP-5 Phase 6: Create sandbox workspace for this branch and seed it
        // with any existing files the node will read or modify.
        match crate::tools::create_sandbox(&self.context.working_dir, &session_id, &branch_id) {
            Ok(sandbox_path) => {
                log::debug!("Sandbox created at {}", sandbox_path.display());

                // Seed sandbox with plugin-identified project manifests
                // (Cargo.toml, pyproject.toml, etc.) so build/test commands work.
                let plugin_refs: Vec<&str> = self
                    .context
                    .active_plugins
                    .iter()
                    .map(|s| s.as_str())
                    .collect();
                if let Err(e) = crate::tools::seed_sandbox_manifests(
                    &self.context.working_dir,
                    &sandbox_path,
                    &plugin_refs,
                ) {
                    log::warn!("Failed to seed sandbox manifests: {}", e);
                }

                // Copy node's owned output targets into the sandbox so
                // verification and builds can find them.
                let node = &self.graph[idx];
                for target in &node.output_targets {
                    if let Some(rel) = target.to_str() {
                        if let Err(e) = crate::tools::copy_to_sandbox(
                            &self.context.working_dir,
                            &sandbox_path,
                            rel,
                        ) {
                            log::debug!("Could not seed sandbox with {}: {}", rel, e);
                        }
                    }
                }
                // Also copy output targets from ALL ancestors (not just
                // direct parents) so transitive dependencies are available.
                // e.g. task_test_solver depends on task_solver which depends
                // on task_cfd_core — the solver test sandbox needs cfd-core
                // source files to build.
                let mut ancestor_queue: Vec<NodeIndex> = parents.clone();
                let mut visited = std::collections::HashSet::new();
                while let Some(ancestor_idx) = ancestor_queue.pop() {
                    if !visited.insert(ancestor_idx) {
                        continue;
                    }
                    for target in &self.graph[ancestor_idx].output_targets {
                        if let Some(rel) = target.to_str() {
                            if let Err(e) = crate::tools::copy_to_sandbox(
                                &self.context.working_dir,
                                &sandbox_path,
                                rel,
                            ) {
                                log::debug!(
                                    "Could not seed sandbox with ancestor file {}: {}",
                                    rel,
                                    e
                                );
                            }
                        }
                    }
                    // Walk further up the graph
                    for grandparent in self
                        .graph
                        .neighbors_directed(ancestor_idx, petgraph::Direction::Incoming)
                    {
                        ancestor_queue.push(grandparent);
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to create sandbox for branch {}: {}", branch_id, e);
            }
        }

        self.emit_event(perspt_core::AgentEvent::BranchCreated {
            branch_id: branch_id.clone(),
            node_id,
            parent_node_id,
        });
        log::info!("Created provisional branch {} for node", branch_id);

        Some(branch_id)
    }

    /// Merge a provisional branch after successful commit.
    pub(crate) fn merge_provisional_branch(&mut self, branch_id: &str, idx: NodeIndex) {
        let node_id = self.graph[idx].node_id.clone();
        if let Err(e) = self
            .ledger
            .update_branch_state(branch_id, &ProvisionalBranchState::Merged.to_string())
        {
            log::warn!("Failed to merge branch {}: {}", branch_id, e);
        }

        // Clean up sandbox directory — artifacts were already exported in step_commit
        let sandbox_path = self
            .context
            .working_dir
            .join(".perspt")
            .join("sandboxes")
            .join(&self.context.session_id)
            .join(branch_id);
        if let Err(e) = crate::tools::cleanup_sandbox(&sandbox_path) {
            log::warn!(
                "Failed to cleanup sandbox for merged branch {}: {}",
                branch_id,
                e
            );
        }

        self.emit_event(perspt_core::AgentEvent::BranchMerged {
            branch_id: branch_id.to_string(),
            node_id,
        });
        log::info!("Merged provisional branch {}", branch_id);
    }

    /// Flush a provisional branch on escalation / non-convergence.
    pub(crate) fn flush_provisional_branch(&mut self, branch_id: &str, node_id: &str) {
        if let Err(e) = self
            .ledger
            .update_branch_state(branch_id, &ProvisionalBranchState::Flushed.to_string())
        {
            log::warn!("Failed to flush branch {}: {}", branch_id, e);
        }

        // Clean up sandbox directory — speculative work is discarded
        let sandbox_path = self
            .context
            .working_dir
            .join(".perspt")
            .join("sandboxes")
            .join(&self.context.session_id)
            .join(branch_id);
        if let Err(e) = crate::tools::cleanup_sandbox(&sandbox_path) {
            log::warn!(
                "Failed to cleanup sandbox for flushed branch {}: {}",
                branch_id,
                e
            );
        }

        log::info!(
            "Flushed provisional branch {} for node {}",
            branch_id,
            node_id
        );
    }

    /// Flush all descendant provisional branches when a parent node fails.
    ///
    /// Walks the DAG outward from `idx`, finds all child nodes that have
    /// active provisional branches, flushes them, and persists a
    /// BranchFlushRecord documenting the cascade.
    pub(crate) fn flush_descendant_branches(&mut self, idx: NodeIndex) {
        let parent_node_id = self.graph[idx].node_id.clone();
        let session_id = self.context.session_id.clone();

        // Collect all transitive dependents
        let descendant_indices = self.collect_descendants(idx);

        let mut flushed_branch_ids = Vec::new();
        let mut requeue_node_ids = Vec::new();

        for desc_idx in &descendant_indices {
            let desc_node = &self.graph[*desc_idx];
            if let Some(ref bid) = desc_node.provisional_branch_id {
                // Flush the branch
                let bid_clone = bid.clone();
                let nid_clone = desc_node.node_id.clone();
                self.flush_provisional_branch(&bid_clone, &nid_clone);
                flushed_branch_ids.push(bid_clone);
                requeue_node_ids.push(nid_clone);
            }
        }

        if flushed_branch_ids.is_empty() {
            return;
        }

        // Persist the flush decision
        let flush_record = perspt_core::types::BranchFlushRecord::new(
            &session_id,
            &parent_node_id,
            flushed_branch_ids.clone(),
            requeue_node_ids.clone(),
            format!(
                "Parent node {} failed verification/convergence",
                parent_node_id
            ),
        );
        if let Err(e) = self.ledger.record_branch_flush(&flush_record) {
            log::warn!("Failed to record branch flush: {}", e);
        }

        self.emit_event(perspt_core::AgentEvent::BranchFlushed {
            parent_node_id: parent_node_id.clone(),
            flushed_branch_ids,
            reason: format!("Parent {} failed", parent_node_id),
        });

        log::info!(
            "Flushed {} descendant branches for parent {}; {} nodes eligible for requeue",
            flush_record.flushed_branch_ids.len(),
            parent_node_id,
            requeue_node_ids.len(),
        );
    }

    /// Collect all transitive dependent node indices reachable from `idx`
    /// via outgoing edges (children, grandchildren, etc.).
    pub(crate) fn collect_descendants(&self, idx: NodeIndex) -> Vec<NodeIndex> {
        let mut descendants = Vec::new();
        let mut stack = vec![idx];
        let mut visited = std::collections::HashSet::new();
        visited.insert(idx);

        while let Some(current) = stack.pop() {
            for child in self
                .graph
                .neighbors_directed(current, petgraph::Direction::Outgoing)
            {
                if visited.insert(child) {
                    descendants.push(child);
                    stack.push(child);
                }
            }
        }
        descendants
    }

    /// Emit interface seals from an Interface-class node's output artifacts.
    ///
    /// Called during step_commit for nodes whose `node_class` is `Interface`.
    /// Computes structural digests of owned output files and persists seal
    /// records so dependent nodes can assemble context from sealed interfaces.
    pub(crate) fn emit_interface_seals(&mut self, idx: NodeIndex) {
        let node = &self.graph[idx];
        if node.node_class != NodeClass::Interface {
            return;
        }

        let node_id = node.node_id.clone();
        let session_id = self.context.session_id.clone();
        let output_targets: Vec<_> = node.output_targets.clone();
        let mut sealed_paths = Vec::new();
        let mut seal_hash = [0u8; 32];

        let retriever = ContextRetriever::new(self.context.working_dir.clone());

        for target in &output_targets {
            let path_str = target.to_string_lossy().to_string();
            match retriever.compute_structural_digest(
                &path_str,
                perspt_core::types::ArtifactKind::InterfaceSeal,
                &node_id,
            ) {
                Ok(digest) => {
                    let seal = perspt_core::types::InterfaceSealRecord::from_digest(
                        &session_id,
                        &node_id,
                        &digest,
                    );
                    seal_hash = seal.seal_hash;
                    sealed_paths.push(path_str);

                    if let Err(e) = self.ledger.record_interface_seal(&seal) {
                        log::warn!("Failed to record interface seal: {}", e);
                    }
                }
                Err(e) => {
                    log::debug!("Skipping seal for {}: {}", path_str, e);
                }
            }
        }

        if !sealed_paths.is_empty() {
            // Store seal hash on the node
            self.graph[idx].interface_seal_hash = Some(seal_hash);

            self.emit_event(perspt_core::AgentEvent::InterfaceSealed {
                node_id: node_id.clone(),
                sealed_paths: sealed_paths.clone(),
                seal_hash: seal_hash
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>(),
            });
            log::info!(
                "Sealed {} interface artifact(s) for node {}",
                sealed_paths.len(),
                node_id
            );
        }
    }

    /// Unblock child nodes that were waiting on this node's interface seal.
    pub(crate) fn unblock_dependents(&mut self, idx: NodeIndex) {
        let node_id = self.graph[idx].node_id.clone();

        // Drain blocked dependencies that match this parent
        let (unblocked, remaining): (Vec<_>, Vec<_>) = self
            .blocked_dependencies
            .drain(..)
            .partition(|dep| dep.parent_node_id == node_id);

        self.blocked_dependencies = remaining;

        for dep in unblocked {
            self.emit_event(perspt_core::AgentEvent::DependentUnblocked {
                child_node_id: dep.child_node_id.clone(),
                parent_node_id: node_id.clone(),
            });
            log::info!(
                "Unblocked dependent {} (parent {} sealed)",
                dep.child_node_id,
                node_id
            );
        }
    }

    /// Check whether a node should be blocked because a parent Interface node
    /// has not yet produced a seal.  Returns `true` if the node is blocked.
    pub(crate) fn check_seal_prerequisites(&mut self, idx: NodeIndex) -> bool {
        let parents: Vec<NodeIndex> = self
            .graph
            .neighbors_directed(idx, petgraph::Direction::Incoming)
            .collect();

        for pidx in parents {
            let parent = &self.graph[pidx];
            if parent.node_class == NodeClass::Interface
                && parent.interface_seal_hash.is_none()
                && parent.state != NodeState::Completed
            {
                // Parent Interface node hasn't sealed yet — block this child
                let child_node_id = self.graph[idx].node_id.clone();
                let parent_node_id = parent.node_id.clone();
                let sealed_paths: Vec<String> = parent
                    .output_targets
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();

                let dep = perspt_core::types::BlockedDependency::new(
                    &child_node_id,
                    &parent_node_id,
                    sealed_paths,
                );
                self.blocked_dependencies.push(dep);

                log::info!(
                    "Node {} blocked: waiting on interface seal from {}",
                    child_node_id,
                    parent_node_id
                );
                return true;
            }
        }
        false
    }

    /// PSP-5 Phase 3: Check that required structural dependencies have
    /// machine-verifiable digests, not just prose summaries.
    ///
    /// Returns a list of (dependency_node_id, reason) for dependencies that
    /// only have semantic/advisory summaries with no structural evidence.
    pub(crate) fn check_structural_dependencies(
        &self,
        node: &SRBNNode,
        restriction_map: &perspt_core::types::RestrictionMap,
    ) -> Vec<(String, String)> {
        use perspt_core::types::{ArtifactKind, NodeClass};

        let mut prose_only = Vec::new();

        // Only enforce for Implementation nodes that depend on Interface nodes
        if node.node_class != NodeClass::Implementation {
            return prose_only;
        }

        // Collect parent Interface node IDs from the DAG
        let idx = match self.node_indices.get(&node.node_id) {
            Some(i) => *i,
            None => return prose_only,
        };

        let parents: Vec<NodeIndex> = self
            .graph
            .neighbors_directed(idx, petgraph::Direction::Incoming)
            .collect();

        for pidx in parents {
            let parent = &self.graph[pidx];
            if parent.node_class != NodeClass::Interface {
                continue;
            }

            // Check if we have at least one structural digest from this parent
            let has_structural = restriction_map.structural_digests.iter().any(|d| {
                d.source_node_id == parent.node_id
                    && matches!(
                        d.artifact_kind,
                        ArtifactKind::Signature
                            | ArtifactKind::Schema
                            | ArtifactKind::InterfaceSeal
                    )
            });

            if !has_structural {
                prose_only.push((
                    parent.node_id.clone(),
                    format!(
                        "Interface node '{}' has no Signature/Schema/InterfaceSeal digest in the \
                            restriction map",
                        parent.node_id
                    ),
                ));
            }
        }

        prose_only
    }

    /// Inject sealed interface digests from parent nodes into a restriction map.
    ///
    /// For each parent that has a recorded interface seal in the ledger, replace
    /// the mutable file reference in the sealed_interfaces list with a
    /// structural digest derived from the persisted seal.  This ensures the
    /// child context is assembled from immutable sealed data.
    pub(crate) fn inject_sealed_interfaces(
        &self,
        idx: NodeIndex,
        restriction_map: &mut perspt_core::types::RestrictionMap,
    ) {
        let parents: Vec<NodeIndex> = self
            .graph
            .neighbors_directed(idx, petgraph::Direction::Incoming)
            .collect();

        for pidx in parents {
            let parent = &self.graph[pidx];
            if parent.interface_seal_hash.is_none() {
                continue;
            }

            let parent_node_id = &parent.node_id;

            // Query persisted seal records for this parent
            let seals = match self.ledger.get_interface_seals(parent_node_id) {
                Ok(rows) => rows,
                Err(e) => {
                    log::debug!("Could not query seals for {}: {}", parent_node_id, e);
                    continue;
                }
            };

            for seal in seals {
                // Remove the path from sealed_interfaces (it will be replaced by digest)
                restriction_map
                    .sealed_interfaces
                    .retain(|p| *p != seal.sealed_path);

                // Convert Vec<u8> seal_hash to [u8; 32]
                let mut hash = [0u8; 32];
                let len = seal.seal_hash.len().min(32);
                hash[..len].copy_from_slice(&seal.seal_hash[..len]);

                // Add a structural digest instead
                let digest = perspt_core::types::StructuralDigest {
                    digest_id: format!("seal_{}_{}", seal.node_id, seal.sealed_path),
                    source_node_id: seal.node_id.clone(),
                    source_path: seal.sealed_path.clone(),
                    artifact_kind: perspt_core::types::ArtifactKind::InterfaceSeal,
                    hash,
                    version: seal.version as u32,
                };
                restriction_map.structural_digests.push(digest);

                log::debug!(
                    "Injected sealed digest for {} from parent {}",
                    seal.sealed_path,
                    parent_node_id,
                );
            }
        }
    }
}
