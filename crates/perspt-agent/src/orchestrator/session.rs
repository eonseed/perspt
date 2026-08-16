use super::*;

impl SRBNOrchestrator {
    /// Add a node to the task DAG
    pub fn add_node(&mut self, node: SRBNNode) -> NodeIndex {
        let node_id = node.node_id.clone();
        let idx = self.graph.add_node(node);
        self.node_indices.insert(node_id, idx);
        idx
    }

    /// Connect TUI channels for interactive control
    pub fn connect_tui(
        &mut self,
        event_sender: perspt_core::events::channel::EventSender,
        action_receiver: perspt_core::events::channel::ActionReceiver,
    ) {
        self.tools.set_event_sender(event_sender.clone());
        self.event_sender = Some(event_sender);
        self.action_receiver = Some(action_receiver);
    }

    /// Get a handle to the abort flag for external signal handlers.
    pub fn abort_flag(&self) -> Arc<AtomicBool> {
        self.abort_requested.clone()
    }

    /// Check whether an abort has been requested.
    pub(crate) fn is_abort_requested(&self) -> bool {
        self.abort_requested.load(Ordering::Relaxed)
    }

    /// Finalize the session in the ledger based on the execution result.
    pub(crate) fn finalize_session(&mut self, result: &Result<perspt_core::SessionOutcome>) {
        let status = if self.is_abort_requested() {
            "ABORTED"
        } else {
            match result {
                Ok(perspt_core::SessionOutcome::Success) => "COMPLETED",
                Ok(perspt_core::SessionOutcome::PartialSuccess) => "PARTIAL",
                Ok(perspt_core::SessionOutcome::Failed) | Err(_) => "FAILED",
            }
        };
        if let Err(e) = self.ledger.end_session(status) {
            log::error!("Failed to finalize session as {}: {}", status, e);
        }
    }

    /// Configure the session-level budget envelope.
    ///
    /// Call this before `run()` to set step, cost, or revision caps from CLI
    /// flags.  Uncapped limits remain `None`.
    pub fn set_budget(
        &mut self,
        max_steps: Option<u32>,
        max_revisions: Option<u32>,
        max_cost_usd: Option<f64>,
    ) {
        self.budget.max_steps = max_steps;
        self.budget.max_revisions = max_revisions;
        self.budget.max_cost_usd = max_cost_usd;
    }

    /// Set the preferred package manager for greenfield project init. The value
    /// is interpreted by the active language plugin (e.g. Python → uv/poetry/pdm/
    /// pipenv, JS → npm/pnpm/yarn); an unrecognized value falls back to the
    /// plugin's default.
    pub fn set_package_manager(&mut self, pm: Option<String>) {
        self.package_manager = pm;
    }

    /// Set the PSP-8 energy weights `(α, β, γ)`. These are applied as proportional
    /// scales on the canonical quadratic energy model's per-component class weights
    /// (see [`sdk_bridge::SdkGateState::set_energy_weights`]); they no longer drive
    /// a separate linear aggregation pass.
    pub fn set_energy_weights(&mut self, alpha: f32, beta: f32, gamma: f32) {
        self.energy_alpha = alpha;
        self.energy_beta = beta;
        self.energy_gamma = gamma;
        self.sdk_gate.set_energy_weights(alpha, beta, gamma);
    }

    // =========================================================================
    // PSP-5 Phase 8: Session Rehydration for Resume
    // =========================================================================

    /// Rehydrate the orchestrator from a persisted session, rebuilding the
    /// DAG from stored node snapshots and graph edges.
    ///
    /// Terminal nodes (Completed, Failed, Aborted) will be skipped during
    /// the subsequent `run_resumed()` execution. Non-terminal nodes are
    /// placed back in their persisted state so the executor can continue
    /// from the last durable boundary.
    ///
    /// Returns `Ok(snapshot)` with the loaded session snapshot on success,
    /// or an error when the session cannot be reconstructed.
    pub fn rehydrate_session(
        &mut self,
        session_id: &str,
    ) -> Result<crate::ledger::SessionSnapshot> {
        // Attach the ledger to this session so facades read the right data
        self.context.session_id = session_id.to_string();
        self.ledger.current_session = Some(crate::ledger::SessionRecordLegacy {
            session_id: session_id.to_string(),
            task: String::new(),
            started_at: epoch_seconds(),
            ended_at: None,
            status: "RESUMING".to_string(),
            total_nodes: 0,
            completed_nodes: 0,
        });

        let snapshot = self.ledger.load_session_snapshot()?;

        // PSP-5 Phase 12: Restore budget envelope from persisted state so
        // resume honours the same step/cost/revision caps.
        if let Ok(Some(row)) = self.ledger.get_budget_envelope() {
            self.budget = perspt_core::types::BudgetEnvelope {
                session_id: row.session_id,
                max_steps: row.max_steps.map(|v| v as u32),
                steps_used: row.steps_used as u32,
                max_revisions: row.max_revisions.map(|v| v as u32),
                revisions_used: row.revisions_used as u32,
                max_cost_usd: row.max_cost_usd,
                cost_used_usd: row.cost_used_usd,
            };
            log::info!(
                "Restored budget envelope: steps {}/{:?}, revisions {}/{:?}, cost ${:.2}/{:?}",
                self.budget.steps_used,
                self.budget.max_steps,
                self.budget.revisions_used,
                self.budget.max_revisions,
                self.budget.cost_used_usd,
                self.budget.max_cost_usd,
            );
        }

        // PSP-5 Phase 8: Corruption / backward-compatibility checks
        if snapshot.node_details.is_empty() {
            anyhow::bail!(
                "Session {} has no persisted nodes — cannot resume",
                session_id
            );
        }

        // Detect orphaned edges (references to nodes not in snapshot)
        let node_ids: std::collections::HashSet<&str> = snapshot
            .node_details
            .iter()
            .map(|d| d.record.node_id.as_str())
            .collect();
        let orphaned_edges = snapshot
            .graph_edges
            .iter()
            .filter(|e| {
                !node_ids.contains(e.parent_node_id.as_str())
                    || !node_ids.contains(e.child_node_id.as_str())
            })
            .count();
        if orphaned_edges > 0 {
            log::warn!(
                "Session {} has {} orphaned edge(s) referencing unknown nodes — \
                 edges will be dropped during resume",
                session_id,
                orphaned_edges
            );
            self.emit_log(format!(
                "⚠️ Resume: dropping {} orphaned graph edge(s)",
                orphaned_edges
            ));
        }

        // Rebuild graph: first add all nodes
        let mut node_map: HashMap<String, NodeIndex> = HashMap::new();

        for detail in &snapshot.node_details {
            let rec = &detail.record;

            let state = parse_node_state(&rec.state);
            let node_class = rec
                .node_class
                .as_deref()
                .map(parse_node_class)
                .unwrap_or_default();

            let mut node = SRBNNode::new(
                rec.node_id.clone(),
                rec.goal.clone().unwrap_or_default(),
                ModelTier::Actuator,
            );
            node.state = state;
            node.node_class = node_class;
            node.owner_plugin = rec.owner_plugin.clone().unwrap_or_default();
            node.parent_id = rec.parent_id.clone();
            node.children = rec
                .children
                .as_deref()
                .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
                .unwrap_or_default();
            node.monitor.attempt_count = rec.attempt_count as usize;

            // Restore latest energy if available
            if let Some(last_energy) = detail.energy_history.last() {
                node.monitor.energy_history.push(last_energy.v_total);
            }

            // Restore interface seal hash from persisted seals
            if let Some(seal) = detail.interface_seals.last() {
                if seal.seal_hash.len() == 32 {
                    let mut hash = [0u8; 32];
                    hash.copy_from_slice(&seal.seal_hash);
                    node.interface_seal_hash = Some(hash);
                }
            }

            let idx = self.add_node(node);
            node_map.insert(rec.node_id.clone(), idx);
        }

        // Rebuild edges from persisted graph topology
        for edge in &snapshot.graph_edges {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_map.get(&edge.parent_node_id),
                node_map.get(&edge.child_node_id),
            ) {
                self.graph.add_edge(
                    from_idx,
                    to_idx,
                    Dependency {
                        kind: edge.edge_type.clone(),
                    },
                );
            }
        }

        // Restore blocked dependencies from non-completed parents of Interface class
        for (child_id, &child_idx) in &node_map {
            let parents: Vec<NodeIndex> = self
                .graph
                .neighbors_directed(child_idx, petgraph::Direction::Incoming)
                .collect();

            for parent_idx in parents {
                let parent = &self.graph[parent_idx];
                if parent.node_class == NodeClass::Interface
                    && parent.interface_seal_hash.is_none()
                    && !parent.state.is_terminal()
                {
                    self.blocked_dependencies
                        .push(perspt_core::types::BlockedDependency {
                            child_node_id: child_id.clone(),
                            parent_node_id: parent.node_id.clone(),
                            required_seal_paths: Vec::new(),
                            blocked_at: epoch_seconds(),
                        });
                }
            }
        }

        let terminal = snapshot
            .node_details
            .iter()
            .filter(|d| {
                let s = parse_node_state(&d.record.state);
                s.is_terminal()
            })
            .count();
        let resumable = snapshot.node_details.len() - terminal;

        log::info!(
            "Rehydrated session {}: {} nodes ({} terminal, {} resumable), {} edges",
            session_id,
            snapshot.node_details.len(),
            terminal,
            resumable,
            snapshot.graph_edges.len()
        );

        // Update legacy session tracker
        if let Some(ref mut sess) = self.ledger.current_session {
            sess.total_nodes = snapshot.node_details.len();
            sess.completed_nodes = terminal;
            sess.status = "RUNNING".to_string();
        }

        // PSP-5 Phase 3: Validate context provenance for non-terminal nodes.
        // Check that files referenced in persisted provenance still exist on
        // disk so the resumed run has a chance to rebuild equivalent context.
        for detail in &snapshot.node_details {
            let state = parse_node_state(&detail.record.state);
            if state.is_terminal() {
                continue;
            }

            if let Some(ref prov) = detail.context_provenance {
                let retriever = ContextRetriever::new(self.context.working_dir.clone());
                let drift = retriever.validate_provenance_record(prov);
                if !drift.is_empty() {
                    log::warn!(
                        "Provenance drift for node '{}': {} file(s) missing: {}",
                        detail.record.node_id,
                        drift.len(),
                        drift.join(", ")
                    );
                    self.emit_log(format!(
                        "⚠️ Provenance drift: node '{}' has {} missing file(s)",
                        detail.record.node_id,
                        drift.len()
                    ));
                    self.emit_event(perspt_core::AgentEvent::ProvenanceDrift {
                        node_id: detail.record.node_id.clone(),
                        missing_files: drift,
                        reason: "Files referenced in persisted context no longer exist".to_string(),
                    });
                }
            }
        }

        Ok(snapshot)
    }

    /// Resume execution from a rehydrated session.
    ///
    /// Walks the DAG in topological order, skipping terminal nodes and
    /// executing any node whose state is not completed/failed/aborted.
    /// Emits a differential resume summary so users can see what will
    /// be replayed vs. skipped.
    pub async fn run_resumed(&mut self) -> Result<()> {
        let result = self.run_resumed_inner().await;
        self.finalize_session(&result);
        result.map(|_| ())
    }

    /// Inner resumed execution logic.
    pub(crate) async fn run_resumed_inner(&mut self) -> Result<perspt_core::SessionOutcome> {
        let topo = Topo::new(&self.graph);
        let indices: Vec<_> = topo.iter(&self.graph).collect();
        let total_nodes = indices.len();
        let mut executed = 0;
        let mut escalated: usize = 0;

        // PSP-5 Phase 8: Emit differential resume summary
        let terminal_count = indices
            .iter()
            .filter(|i| self.graph[**i].state.is_terminal())
            .count();
        let blocked_count = indices
            .iter()
            .filter(|i| !self.graph[**i].state.is_terminal() && self.check_seal_prerequisites(**i))
            .count();
        let resumable_count = total_nodes - terminal_count - blocked_count;
        self.emit_log(format!(
            "📊 Differential resume: {} total, {} skipped (terminal), {} blocked (seal), {} to execute",
            total_nodes, terminal_count, blocked_count, resumable_count
        ));

        for (i, idx) in indices.iter().enumerate() {
            // Abort gate
            if self.is_abort_requested() {
                self.emit_log("⚠️ Session aborted — stopping resumed execution".to_string());
                break;
            }

            // Budget gate: stop execution if step/cost/revision budget exhausted.
            if self.budget.any_exhausted() {
                let node_id = self.graph[*idx].node_id.clone();
                self.emit_log(format!(
                    "⛔ Budget exhausted — skipping node '{}' and remaining nodes",
                    node_id
                ));
                self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                    node_id,
                    status: perspt_core::NodeStatus::Escalated,
                });
                break;
            }

            let node = &self.graph[*idx];

            // Skip terminal nodes
            if node.state.is_terminal() {
                log::debug!("Skipping terminal node {} ({:?})", node.node_id, node.state);
                continue;
            }

            // Check seal prerequisites
            if self.check_seal_prerequisites(*idx) {
                log::warn!(
                    "Node {} blocked on seal prerequisite — skipping",
                    self.graph[*idx].node_id
                );
                continue;
            }

            let node = &self.graph[*idx];
            self.emit_log(format!(
                "📝 [resume {}/{}] {}",
                i + 1,
                total_nodes,
                node.goal
            ));
            self.emit_event(perspt_core::AgentEvent::NodeSelected {
                node_id: node.node_id.clone(),
                goal: node.goal.clone(),
                node_class: node.node_class.to_string(),
            });
            self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                node_id: node.node_id.clone(),
                status: perspt_core::NodeStatus::Running,
            });

            match self.execute_node(*idx).await {
                Ok(NodeOutcome::Completed) => {
                    executed += 1;
                    self.budget.record_step();

                    // Persist budget envelope for auditability.
                    if let Err(e) = self.ledger.upsert_budget_envelope(&self.budget) {
                        log::warn!("Failed to persist budget envelope: {}", e);
                    }

                    if let Some(node) = self.graph.node_weight(*idx) {
                        self.emit_event(perspt_core::AgentEvent::NodeCompleted {
                            node_id: node.node_id.clone(),
                            goal: node.goal.clone(),
                        });
                    }
                }
                Ok(NodeOutcome::Escalated) => {
                    escalated += 1;
                    self.budget.record_step();
                    continue;
                }
                Ok(NodeOutcome::Reworked) => {
                    // A repair was applied during resume; the node is left
                    // non-terminal (Retry). Record the step and move on — a
                    // subsequent resume picks it up. The full closed loop runs in
                    // run_orchestration, not the resume fast-path.
                    self.budget.record_step();
                    continue;
                }
                Err(e) => {
                    escalated += 1;
                    let node_id = self.graph[*idx].node_id.clone();
                    log::error!("Node {} failed on resume: {}", node_id, e);
                    self.emit_log(format!("❌ Node {} failed: {}", node_id, e));
                    self.graph[*idx].state = NodeState::Escalated;
                    self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                        node_id,
                        status: perspt_core::NodeStatus::Escalated,
                    });
                    continue;
                }
            }
        }

        log::info!(
            "Resumed execution completed: {} of {} nodes executed",
            executed,
            total_nodes
        );

        // Derive session outcome from actual node results, same logic as
        // run_orchestration: unattempted nodes count as incomplete.
        let outcome = if escalated == 0 && executed + terminal_count >= total_nodes {
            perspt_core::SessionOutcome::Success
        } else if executed > 0 {
            perspt_core::SessionOutcome::PartialSuccess
        } else {
            perspt_core::SessionOutcome::Failed
        };
        self.emit_event(perspt_core::AgentEvent::Complete {
            success: outcome == perspt_core::SessionOutcome::Success,
            message: format!(
                "Resumed: {}/{} completed, {} escalated",
                executed, total_nodes, escalated
            ),
        });
        Ok(outcome)
    }
}
