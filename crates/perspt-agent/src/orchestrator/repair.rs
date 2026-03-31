//! Non-convergence classification, repair actions, and subgraph rewriting.

use super::*;

impl SRBNOrchestrator {
    /// Classify why a node failed to converge.
    ///
    /// Uses the last verification result, retry policy, tool failure state,
    /// and graph topology to determine the failure category.
    pub(super) fn classify_non_convergence(&self, idx: NodeIndex) -> EscalationCategory {
        let node = &self.graph[idx];

        // 1. Degraded sensors take priority — we cannot trust any other signal
        if let Some(ref vr) = self.last_verification_result {
            if vr.has_degraded_stages() {
                return EscalationCategory::DegradedSensors;
            }
        }

        // 2. Contract / structural mismatch
        if node.monitor.retry_policy.review_rejections > 0 {
            return EscalationCategory::ContractMismatch;
        }

        // 3. Topology mismatch — node touches files outside its ownership
        if !node.owner_plugin.is_empty() {
            let manifest = &self.context.ownership_manifest;
            for target in &node.output_targets {
                if let Some(entry) = manifest.owner_of(&target.to_string_lossy()) {
                    if entry.owner_node_id != node.node_id {
                        return EscalationCategory::TopologyMismatch;
                    }
                }
            }
        }

        // 4. Compilation errors that persist across retries suggest model inadequacy
        if node.monitor.retry_policy.compilation_failures
            >= node.monitor.retry_policy.max_compilation_retries
        {
            // If energy never decreased, the model may not be capable enough
            if !node.monitor.is_converging() && node.monitor.attempt_count >= 3 {
                return EscalationCategory::InsufficientModelCapability;
            }
        }

        // 5. Default: implementation error (most common case)
        EscalationCategory::ImplementationError
    }

    /// Choose a repair action based on the classified failure category.
    ///
    /// Picks the least-destructive action that is safe given current evidence.
    pub(super) fn choose_repair_action(
        &self,
        idx: NodeIndex,
        category: &EscalationCategory,
    ) -> RewriteAction {
        let node = &self.graph[idx];

        match category {
            EscalationCategory::DegradedSensors => {
                let degraded = self
                    .last_verification_result
                    .as_ref()
                    .map(|vr| vr.degraded_stage_reasons())
                    .unwrap_or_default();
                RewriteAction::DegradedValidationStop {
                    reason: format!(
                        "Cannot verify stability — degraded sensors: {}",
                        degraded.join(", ")
                    ),
                }
            }
            EscalationCategory::ContractMismatch => RewriteAction::ContractRepair {
                fields: vec!["interface_signature".to_string(), "invariants".to_string()],
            },
            EscalationCategory::InsufficientModelCapability => {
                RewriteAction::CapabilityPromotion {
                    from_tier: node.tier,
                    to_tier: ModelTier::Architect, // promote to strongest tier
                }
            }
            EscalationCategory::TopologyMismatch => {
                // Check if a split would help
                if node.output_targets.len() > 1 {
                    RewriteAction::NodeSplit {
                        proposed_children: node
                            .output_targets
                            .iter()
                            .enumerate()
                            .map(|(i, _)| format!("{}_split_{}", node.node_id, i))
                            .collect(),
                    }
                } else {
                    RewriteAction::InterfaceInsertion {
                        boundary: format!(
                            "ownership boundary for {}",
                            node.output_targets
                                .first()
                                .map(|p| p.display().to_string())
                                .unwrap_or_default()
                        ),
                    }
                }
            }
            EscalationCategory::ImplementationError => {
                // If we still have retry budget for a different error type, ground the retry
                if node.monitor.remaining_attempts() > 0 {
                    let evidence = self.build_escalation_evidence(idx);
                    RewriteAction::GroundedRetry {
                        evidence_summary: evidence,
                    }
                } else {
                    RewriteAction::UserEscalation {
                        evidence: self.build_escalation_evidence(idx),
                    }
                }
            }
        }
    }

    /// Apply a chosen repair action.  Returns `true` if the action was
    /// handled locally; `false` if the orchestrator should escalate to user.
    pub(super) async fn apply_repair_action(
        &mut self,
        idx: NodeIndex,
        action: &RewriteAction,
    ) -> bool {
        let node_id = self.graph[idx].node_id.clone();

        // PSP-5 Phase 5: Churn guardrail — limit repeated rewrites on the
        // same node lineage. Count prior rewrites for this node (and its
        // parent lineage) and refuse further rewrites beyond the limit.
        const MAX_REWRITES_PER_LINEAGE: usize = 3;
        let lineage_rewrites = self.count_lineage_rewrites(&node_id);
        if lineage_rewrites >= MAX_REWRITES_PER_LINEAGE {
            log::warn!(
                "Rewrite churn limit reached for node {} ({} prior rewrites) — refusing rewrite",
                node_id,
                lineage_rewrites
            );
            self.emit_log(format!(
                "⛔ Rewrite churn limit ({}) reached for {} — escalating",
                MAX_REWRITES_PER_LINEAGE, node_id
            ));
            return false;
        }

        let category = self.classify_non_convergence(idx);

        match action {
            RewriteAction::DegradedValidationStop { reason } => {
                self.emit_log(format!("⛔ Degraded-validation stop: {}", reason));
                self.graph[idx].state = NodeState::Escalated;
                self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                    node_id: self.graph[idx].node_id.clone(),
                    status: perspt_core::NodeStatus::Escalated,
                });
                self.persist_rewrite_record(&node_id, action, &category, &[]);
                false
            }
            RewriteAction::UserEscalation { evidence } => {
                self.emit_log(format!("⚠️ User escalation required: {}", evidence));
                self.persist_rewrite_record(&node_id, action, &category, &[]);
                false
            }
            RewriteAction::GroundedRetry { evidence_summary } => {
                log::info!(
                    "Applying grounded retry for node {}: {}",
                    node_id,
                    evidence_summary
                );
                self.emit_log(format!(
                    "🔄 Grounded retry for {}: {}",
                    node_id,
                    &evidence_summary[..evidence_summary.len().min(120)]
                ));
                self.graph[idx].state = NodeState::Retry;
                self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                    node_id: self.graph[idx].node_id.clone(),
                    status: perspt_core::NodeStatus::Retrying,
                });
                self.persist_rewrite_record(&node_id, action, &category, &[]);
                true
            }
            RewriteAction::ContractRepair { fields } => {
                log::info!("Contract repair for node {}: fields {:?}", node_id, fields);
                self.emit_log(format!(
                    "🔧 Contract repair for {}: {}",
                    node_id,
                    fields.join(", ")
                ));
                self.graph[idx].state = NodeState::Retry;
                self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                    node_id: self.graph[idx].node_id.clone(),
                    status: perspt_core::NodeStatus::Retrying,
                });
                self.persist_rewrite_record(&node_id, action, &category, &[]);
                true
            }
            RewriteAction::CapabilityPromotion { from_tier, to_tier } => {
                log::info!(
                    "Promoting node {} from {:?} to {:?}",
                    node_id,
                    from_tier,
                    to_tier
                );
                self.emit_log(format!(
                    "⬆️ Promoting {} from {:?} to {:?}",
                    node_id, from_tier, to_tier
                ));
                self.graph[idx].tier = *to_tier;
                self.graph[idx].state = NodeState::Retry;
                self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                    node_id: self.graph[idx].node_id.clone(),
                    status: perspt_core::NodeStatus::Retrying,
                });
                self.persist_rewrite_record(&node_id, action, &category, &[]);
                true
            }
            RewriteAction::SensorRecovery { degraded_stages } => {
                log::info!(
                    "Sensor recovery for node {}: {:?}",
                    node_id,
                    degraded_stages
                );
                self.emit_log(format!("🔧 Attempting sensor recovery for {}", node_id));
                self.graph[idx].state = NodeState::Retry;
                self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                    node_id: self.graph[idx].node_id.clone(),
                    status: perspt_core::NodeStatus::Retrying,
                });
                self.persist_rewrite_record(&node_id, action, &category, &[]);
                true
            }
            RewriteAction::NodeSplit { proposed_children } => {
                log::info!(
                    "Node split requested for {}: {:?}",
                    node_id,
                    proposed_children
                );
                if proposed_children.is_empty() {
                    self.emit_log(format!(
                        "✂️ NodeSplit for {} requested with no children — escalating",
                        node_id
                    ));
                    return false;
                }
                self.emit_log(format!(
                    "✂️ Splitting {} into {} sub-nodes",
                    node_id,
                    proposed_children.len()
                ));
                let count = proposed_children.len();
                let inserted = self.split_node(idx, proposed_children);
                if !inserted.is_empty() {
                    self.persist_rewrite_record(&node_id, action, &category, &inserted);
                    self.emit_event(perspt_core::AgentEvent::GraphRewriteApplied {
                        trigger_node: node_id.clone(),
                        action: "node_split".to_string(),
                        nodes_affected: count,
                    });
                    true
                } else {
                    false
                }
            }
            RewriteAction::InterfaceInsertion { boundary } => {
                log::info!("Interface insertion for {}: {}", node_id, boundary);
                self.emit_log(format!(
                    "📐 Inserting interface adapter at boundary: {}",
                    boundary
                ));
                let inserted = self.insert_interface_node(idx, boundary);
                if let Some(ref adapter_id) = inserted {
                    self.persist_rewrite_record(
                        &node_id,
                        action,
                        &category,
                        std::slice::from_ref(adapter_id),
                    );
                    self.emit_event(perspt_core::AgentEvent::GraphRewriteApplied {
                        trigger_node: node_id.clone(),
                        action: "interface_insertion".to_string(),
                        nodes_affected: 1,
                    });
                    true
                } else {
                    false
                }
            }
            RewriteAction::SubgraphReplan { affected_nodes } => {
                log::info!("Subgraph replan for {}: {:?}", node_id, affected_nodes);
                let count = affected_nodes.len();
                self.emit_log(format!(
                    "🗺️ Replanning subgraph around {} ({} affected nodes)",
                    node_id,
                    affected_nodes.len()
                ));
                let applied = self.replan_subgraph(idx, affected_nodes);
                if applied {
                    self.persist_rewrite_record(&node_id, action, &category, &[]);
                    self.emit_event(perspt_core::AgentEvent::GraphRewriteApplied {
                        trigger_node: node_id.clone(),
                        action: "subgraph_replan".to_string(),
                        nodes_affected: count + 1,
                    });
                }
                applied
            }
        }
    }

    /// Persist a rewrite record with the actual inserted node IDs.
    fn persist_rewrite_record(
        &self,
        node_id: &str,
        action: &RewriteAction,
        category: &perspt_core::types::EscalationCategory,
        inserted_nodes: &[String],
    ) {
        let rewrite = RewriteRecord {
            node_id: node_id.to_string(),
            session_id: self.context.session_id.clone(),
            action: action.clone(),
            category: *category,
            requeued_nodes: Vec::new(),
            inserted_nodes: inserted_nodes.to_vec(),
            timestamp: epoch_seconds(),
        };
        if let Err(e) = self.ledger.record_rewrite(&rewrite) {
            log::warn!("Failed to persist rewrite record: {}", e);
        }
    }

    /// Count how many rewrites have been applied to this node or its lineage
    /// (nodes sharing the same base ID prefix before `__split_` or `__iface`).
    pub(super) fn count_lineage_rewrites(&self, node_id: &str) -> usize {
        // Extract the root lineage ID (strip __split_N and __iface suffixes)
        let base = node_id
            .split("__split_")
            .next()
            .unwrap_or(node_id)
            .split("__iface")
            .next()
            .unwrap_or(node_id);

        // Count rewrite records for this lineage from the ledger
        match self.ledger.get_rewrite_count_for_lineage(base) {
            Ok(count) => count,
            Err(e) => {
                log::warn!(
                    "Failed to query rewrite count for lineage '{}': {}",
                    base,
                    e
                );
                0
            }
        }
    }

    /// Build a human-readable evidence string for an escalation.
    pub(super) fn build_escalation_evidence(&self, idx: NodeIndex) -> String {
        let node = &self.graph[idx];
        let mut parts = Vec::new();

        parts.push(format!("node: {}", node.node_id));
        parts.push(format!("goal: {}", node.goal));
        parts.push(format!("energy: {:.2}", node.monitor.current_energy()));
        parts.push(format!("attempts: {}", node.monitor.attempt_count));
        parts.push(node.monitor.retry_policy.summary());

        if let Some(ref vr) = self.last_verification_result {
            parts.push(format!(
                "verification: syn={}, build={}, tests={}, diag={}",
                vr.syntax_ok, vr.build_ok, vr.tests_ok, vr.diagnostics_count
            ));
            if vr.has_degraded_stages() {
                parts.push(format!(
                    "degraded: {}",
                    vr.degraded_stage_reasons().join("; ")
                ));
            }
        }

        if let Some(ref failure) = self.last_tool_failure {
            parts.push(format!("last tool failure: {}", failure));
        }

        parts.join(" | ")
    }

    /// Collect node IDs that directly depend on the given node.
    pub(super) fn affected_dependents(&self, idx: NodeIndex) -> Vec<String> {
        self.graph
            .neighbors_directed(idx, petgraph::Direction::Outgoing)
            .map(|dep_idx| self.graph[dep_idx].node_id.clone())
            .collect()
    }

    /// Split a node into multiple child nodes, inheriting its contracts and
    /// edges. The original node is removed and replaced with the children.
    ///
    /// Each proposed child string is treated as a sub-goal description.
    /// Returns the list of inserted child node IDs (empty on failure).
    pub(super) fn split_node(
        &mut self,
        idx: NodeIndex,
        proposed_children: &[String],
    ) -> Vec<String> {
        if proposed_children.is_empty() {
            return Vec::new();
        }
        let parent = self.graph[idx].clone();
        let parent_id = parent.node_id.clone();

        // Collect existing incoming and outgoing edges before mutation.
        let incoming: Vec<(NodeIndex, Dependency)> = self
            .graph
            .neighbors_directed(idx, petgraph::Direction::Incoming)
            .map(|src| {
                let edge = self.graph.edges_connecting(src, idx).next().unwrap();
                (src, edge.weight().clone())
            })
            .collect();
        let outgoing: Vec<(NodeIndex, Dependency)> = self
            .graph
            .neighbors_directed(idx, petgraph::Direction::Outgoing)
            .map(|dst| {
                let edge = self.graph.edges_connecting(idx, dst).next().unwrap();
                (dst, edge.weight().clone())
            })
            .collect();

        // Create child nodes.
        let mut child_indices = Vec::with_capacity(proposed_children.len());
        let mut child_ids = Vec::with_capacity(proposed_children.len());
        for (i, sub_goal) in proposed_children.iter().enumerate() {
            let child_id = format!("{}__split_{}", parent_id, i);
            let mut child = SRBNNode::new(child_id.clone(), sub_goal.clone(), parent.tier);
            child.parent_id = Some(parent_id.clone());
            child.contract = parent.contract.clone();
            child.node_class = parent.node_class;
            child.owner_plugin = parent.owner_plugin.clone();
            // Distribute output targets round-robin across children so each
            // child handles a subset of the original scope.
            child.output_targets = parent
                .output_targets
                .iter()
                .skip(i)
                .step_by(proposed_children.len())
                .cloned()
                .collect();
            child.context_files = parent.context_files.clone();
            let c_idx = self.graph.add_node(child);
            self.node_indices.insert(child_id.clone(), c_idx);
            child_indices.push(c_idx);
            child_ids.push(child_id);
        }

        // Wire incoming edges → first child, outgoing edges from last child.
        if let Some(&first) = child_indices.first() {
            for (src, dep) in &incoming {
                self.graph.add_edge(*src, first, dep.clone());
                // Persist new edge
                let src_id = self.graph[*src].node_id.clone();
                let dst_id = self.graph[first].node_id.clone();
                let _ = self
                    .ledger
                    .record_task_graph_edge(&src_id, &dst_id, &dep.kind);
            }
        }
        if let Some(&last) = child_indices.last() {
            for (dst, dep) in &outgoing {
                self.graph.add_edge(last, *dst, dep.clone());
                let src_id = self.graph[last].node_id.clone();
                let dst_id = self.graph[*dst].node_id.clone();
                let _ = self
                    .ledger
                    .record_task_graph_edge(&src_id, &dst_id, &dep.kind);
            }
        }

        // Chain children sequentially and persist edges.
        for pair in child_indices.windows(2) {
            self.graph.add_edge(
                pair[0],
                pair[1],
                Dependency {
                    kind: "split_sequence".to_string(),
                },
            );
            let src_id = self.graph[pair[0]].node_id.clone();
            let dst_id = self.graph[pair[1]].node_id.clone();
            let _ = self
                .ledger
                .record_task_graph_edge(&src_id, &dst_id, "split_sequence");
        }

        // Remove original node.
        self.node_indices.remove(&parent_id);
        self.graph.remove_node(idx);

        log::info!(
            "Split node {} into {} children: {:?}",
            parent_id,
            proposed_children.len(),
            child_ids
        );
        child_ids
    }

    /// Insert an interface/adapter node on the edge between the given node
    /// and its dependents.  The boundary string describes the interface
    /// contract for the newly created adapter node.
    /// Returns the adapter node ID on success, or None on failure.
    pub(super) fn insert_interface_node(
        &mut self,
        idx: NodeIndex,
        boundary: &str,
    ) -> Option<String> {
        let source_id = self.graph[idx].node_id.clone();
        let adapter_id = format!("{}__iface", source_id);
        let source_node = &self.graph[idx];

        let mut adapter = SRBNNode::new(
            adapter_id.clone(),
            format!("Interface adapter: {}", boundary),
            source_node.tier,
        );
        adapter.parent_id = Some(source_id.clone());
        adapter.node_class = perspt_core::types::NodeClass::Interface;
        adapter.owner_plugin = source_node.owner_plugin.clone();

        let adapter_idx = self.graph.add_node(adapter);
        self.node_indices.insert(adapter_id.clone(), adapter_idx);

        // Collect outgoing edges from the source node.
        let outgoing: Vec<(NodeIndex, Dependency)> = self
            .graph
            .neighbors_directed(idx, petgraph::Direction::Outgoing)
            .map(|dst| {
                let edge = self.graph.edges_connecting(idx, dst).next().unwrap();
                (dst, edge.weight().clone())
            })
            .collect();

        // Remove old outgoing edges and re-route through adapter.
        let edge_ids: Vec<_> = self
            .graph
            .edges_directed(idx, petgraph::Direction::Outgoing)
            .map(|e| e.id())
            .collect();
        for eid in edge_ids {
            self.graph.remove_edge(eid);
        }

        // source → adapter
        self.graph.add_edge(
            idx,
            adapter_idx,
            Dependency {
                kind: "interface_boundary".to_string(),
            },
        );
        let _ = self
            .ledger
            .record_task_graph_edge(&source_id, &adapter_id, "interface_boundary");

        // adapter → original dependents
        for (dst, dep) in outgoing {
            self.graph.add_edge(adapter_idx, dst, dep.clone());
            let dst_id = self.graph[dst].node_id.clone();
            let _ = self
                .ledger
                .record_task_graph_edge(&adapter_id, &dst_id, &dep.kind);
        }

        log::info!("Inserted interface node {} after {}", adapter_id, source_id);
        Some(adapter_id)
    }

    /// Reset the specified affected nodes back to `TaskQueued` so they get
    /// re-executed.  The triggering node itself is also reset.  Returns `true`
    /// if at least one node was replanned.
    pub(super) fn replan_subgraph(
        &mut self,
        trigger_idx: NodeIndex,
        affected_nodes: &[String],
    ) -> bool {
        let mut replanned = 0;

        // Reset the trigger node itself.
        self.graph[trigger_idx].state = NodeState::Retry;
        self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
            node_id: self.graph[trigger_idx].node_id.clone(),
            status: perspt_core::NodeStatus::Retrying,
        });
        self.graph[trigger_idx].monitor.reset_for_replan();
        replanned += 1;

        // Reset each referenced affected node.
        for nid in affected_nodes {
            if let Some(&nidx) = self.node_indices.get(nid.as_str()) {
                self.graph[nidx].state = NodeState::TaskQueued;
                self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                    node_id: self.graph[nidx].node_id.clone(),
                    status: perspt_core::NodeStatus::Queued,
                });
                self.graph[nidx].monitor.reset_for_replan();
                replanned += 1;
            } else {
                log::warn!("Subgraph replan: unknown node {}", nid);
            }
        }

        log::info!(
            "Replanned {} nodes starting from {}",
            replanned,
            self.graph[trigger_idx].node_id
        );
        replanned > 0
    }
}
