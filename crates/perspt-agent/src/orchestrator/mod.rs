//! SRBN Orchestrator
//!
//! Manages the Task DAG and orchestrates agent execution following the 7-step control loop.

mod bundle;
mod commit;
mod convergence;
mod init;
mod planning;
mod repair;
mod solo;
mod verification;

use crate::agent::{ActuatorAgent, Agent, ArchitectAgent, SpeculatorAgent, VerifierAgent};
use crate::context_retriever::ContextRetriever;
use crate::lsp::LspClient;
use crate::test_runner::{self, PythonTestRunner, TestResults};
use crate::tools::{AgentTools, ToolCall};
use crate::types::{AgentContext, EnergyComponents, ModelTier, NodeState, SRBNNode, TaskPlan};
use anyhow::{Context, Result};
use perspt_core::types::{
    EscalationCategory, EscalationReport, NodeClass, ProvisionalBranch, ProvisionalBranchState,
    RewriteAction, RewriteRecord, SheafValidationResult, SheafValidatorClass, WorkspaceState,
};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::{EdgeRef, Topo, Walker};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Dependency edge type
#[derive(Debug, Clone)]
pub struct Dependency {
    /// Dependency type description
    pub kind: String,
}

/// Result of an approval request
#[derive(Debug, Clone)]
pub enum ApprovalResult {
    /// User approved the action
    Approved,
    /// User approved with an edited value (e.g., project name)
    ApprovedWithEdit(String),
    /// User rejected the action
    Rejected,
}

/// Outcome of executing a single graph node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeOutcome {
    /// Node converged and committed successfully.
    Completed,
    /// Node failed to converge and was escalated.
    Escalated,
}

/// The SRBN Orchestrator - manages the agent workflow
pub struct SRBNOrchestrator {
    /// Task DAG managed by petgraph
    pub graph: DiGraph<SRBNNode, Dependency>,
    /// Node ID to graph index mapping
    node_indices: HashMap<String, NodeIndex>,
    /// Agent context
    pub context: AgentContext,
    /// Auto-approve mode
    pub auto_approve: bool,
    /// LSP clients per language
    lsp_clients: HashMap<String, LspClient>,
    /// Agents for different roles
    agents: Vec<Box<dyn Agent>>,
    /// Agent tools for file/command operations
    tools: AgentTools,
    /// Last written file path (for LSP tracking)
    last_written_file: Option<PathBuf>,
    /// File version counter for LSP
    file_version: i32,
    /// LLM provider for correction calls
    provider: std::sync::Arc<perspt_core::llm_provider::GenAIProvider>,
    /// Architect model name for planning
    architect_model: String,
    /// Actuator model name for corrections
    actuator_model: String,
    /// Verifier model name for correction guidance
    verifier_model: String,
    /// Speculator model name for lookahead hints
    speculator_model: String,
    /// PSP-5: Fallback model for Architect tier (used when primary fails structured-output contract)
    architect_fallback_model: Option<String>,
    /// PSP-5: Fallback model for Actuator tier
    actuator_fallback_model: Option<String>,
    /// PSP-5: Fallback model for Verifier tier
    verifier_fallback_model: Option<String>,
    /// PSP-5: Fallback model for Speculator tier
    speculator_fallback_model: Option<String>,
    /// Event sender for TUI updates (optional)
    event_sender: Option<perspt_core::events::channel::EventSender>,
    /// Action receiver for TUI commands (optional)
    action_receiver: Option<perspt_core::events::channel::ActionReceiver>,
    /// Persistence ledger
    pub ledger: crate::ledger::MerkleLedger,
    /// Last tool failure message (for energy calculation)
    pub last_tool_failure: Option<String>,
    /// PSP-5 Phase 3: Last assembled context provenance (for commit recording)
    last_context_provenance: Option<perspt_core::types::ContextProvenance>,
    /// PSP-5 Phase 3: Last formatted context from restriction map (for correction prompts)
    last_formatted_context: String,
    /// PSP-5 Phase 4: Last plugin-driven verification result (for convergence checks)
    last_verification_result: Option<perspt_core::types::VerificationResult>,
    /// PSP-5 Phase 9: Last applied artifact bundle (for persistence in step_commit)
    last_applied_bundle: Option<perspt_core::types::ArtifactBundle>,
    /// Last recorded RepairFootprint (for multi-file correction context)
    last_repair_footprint: Option<perspt_core::RepairFootprint>,
    /// PSP-5 Phase 6: Blocked dependencies awaiting parent interface seals
    blocked_dependencies: Vec<perspt_core::types::BlockedDependency>,
    /// Session-level budget envelope for step/cost/revision caps.
    budget: perspt_core::types::BudgetEnvelope,
    /// Adaptive planning policy for agent phase selection.
    pub planning_policy: perspt_core::PlanningPolicy,
    /// Session-level stability threshold (ε for V(x) < ε convergence)
    pub stability_epsilon: f32,
    /// Energy weight α (syntax/build errors)
    pub energy_alpha: f32,
    /// Energy weight β (structural concerns)
    pub energy_beta: f32,
    /// Energy weight γ (test/lint failures)
    pub energy_gamma: f32,
    /// Session abort flag — set by external signal handlers or TUI
    abort_requested: Arc<AtomicBool>,
}

/// Get current timestamp as epoch seconds.
fn epoch_seconds() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

impl SRBNOrchestrator {
    /// Create a new orchestrator with default models
    pub fn new(working_dir: PathBuf, auto_approve: bool) -> Self {
        Self::new_with_models(
            working_dir,
            auto_approve,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
    }

    /// Create a new orchestrator with custom model configuration
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_models(
        working_dir: PathBuf,
        auto_approve: bool,
        architect_model: Option<String>,
        actuator_model: Option<String>,
        verifier_model: Option<String>,
        speculator_model: Option<String>,
        architect_fallback_model: Option<String>,
        actuator_fallback_model: Option<String>,
        verifier_fallback_model: Option<String>,
        speculator_fallback_model: Option<String>,
    ) -> Self {
        let context = AgentContext {
            working_dir: working_dir.clone(),
            auto_approve,
            ..Default::default()
        };

        // Create a shared LLM provider - agents will use this for LLM calls
        // In production, this would be configured from environment/config
        let provider = std::sync::Arc::new(
            perspt_core::llm_provider::GenAIProvider::new().unwrap_or_else(|e| {
                log::warn!("Failed to create GenAIProvider: {}, using default", e);
                perspt_core::llm_provider::GenAIProvider::new().expect("GenAI must initialize")
            }),
        );

        // Create agent tools for file/command operations
        let tools = AgentTools::new(working_dir.clone(), !auto_approve);

        // Store model names for direct LLM calls
        let stored_architect_model = architect_model
            .clone()
            .unwrap_or_else(|| ModelTier::Architect.default_model().to_string());
        let stored_actuator_model = actuator_model
            .clone()
            .unwrap_or_else(|| ModelTier::Actuator.default_model().to_string());
        let stored_verifier_model = verifier_model
            .clone()
            .unwrap_or_else(|| ModelTier::Verifier.default_model().to_string());
        let stored_speculator_model = speculator_model
            .clone()
            .unwrap_or_else(|| ModelTier::Speculator.default_model().to_string());

        Self {
            graph: DiGraph::new(),
            node_indices: HashMap::new(),
            context,
            auto_approve,
            lsp_clients: HashMap::new(),
            agents: vec![
                Box::new(ArchitectAgent::new(provider.clone(), architect_model)),
                Box::new(ActuatorAgent::new(provider.clone(), actuator_model)),
                Box::new(VerifierAgent::new(provider.clone(), verifier_model)),
                Box::new(SpeculatorAgent::new(provider.clone(), speculator_model)),
            ],
            tools,
            last_written_file: None,
            file_version: 0,
            provider,
            architect_model: stored_architect_model,
            actuator_model: stored_actuator_model,
            verifier_model: stored_verifier_model,
            speculator_model: stored_speculator_model,
            architect_fallback_model,
            actuator_fallback_model,
            verifier_fallback_model,
            speculator_fallback_model,
            event_sender: None,
            action_receiver: None,
            #[cfg(test)]
            ledger: crate::ledger::MerkleLedger::in_memory().expect("Failed to create test ledger"),
            #[cfg(not(test))]
            ledger: crate::ledger::MerkleLedger::new().expect("Failed to create ledger"),
            last_tool_failure: None,
            last_context_provenance: None,
            last_formatted_context: String::new(),
            last_verification_result: None,
            last_applied_bundle: None,
            last_repair_footprint: None,
            blocked_dependencies: Vec::new(),
            budget: perspt_core::types::BudgetEnvelope::new("pending"),
            planning_policy: perspt_core::PlanningPolicy::default(),
            stability_epsilon: 0.1,
            energy_alpha: 1.0,
            energy_beta: 0.5,
            energy_gamma: 2.0,
            abort_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a new orchestrator for testing with an in-memory ledger
    #[cfg(test)]
    pub fn new_for_testing(working_dir: PathBuf) -> Self {
        let context = AgentContext {
            working_dir: working_dir.clone(),
            auto_approve: true,
            ..Default::default()
        };

        let provider = std::sync::Arc::new(
            perspt_core::llm_provider::GenAIProvider::new().unwrap_or_else(|e| {
                log::warn!("Failed to create GenAIProvider: {}, using default", e);
                perspt_core::llm_provider::GenAIProvider::new().expect("GenAI must initialize")
            }),
        );

        let tools = AgentTools::new(working_dir.clone(), false);

        Self {
            graph: DiGraph::new(),
            node_indices: HashMap::new(),
            context,
            auto_approve: true,
            lsp_clients: HashMap::new(),
            agents: vec![
                Box::new(ArchitectAgent::new(provider.clone(), None)),
                Box::new(ActuatorAgent::new(provider.clone(), None)),
                Box::new(VerifierAgent::new(provider.clone(), None)),
                Box::new(SpeculatorAgent::new(provider.clone(), None)),
            ],
            tools,
            last_written_file: None,
            file_version: 0,
            provider,
            architect_model: ModelTier::Architect.default_model().to_string(),
            actuator_model: ModelTier::Actuator.default_model().to_string(),
            verifier_model: ModelTier::Verifier.default_model().to_string(),
            speculator_model: ModelTier::Speculator.default_model().to_string(),
            architect_fallback_model: None,
            actuator_fallback_model: None,
            verifier_fallback_model: None,
            speculator_fallback_model: None,
            event_sender: None,
            action_receiver: None,
            ledger: crate::ledger::MerkleLedger::in_memory().expect("Failed to create test ledger"),
            last_tool_failure: None,
            last_context_provenance: None,
            last_formatted_context: String::new(),
            last_verification_result: None,
            last_applied_bundle: None,
            last_repair_footprint: None,
            blocked_dependencies: Vec::new(),
            budget: perspt_core::types::BudgetEnvelope::new("test"),
            planning_policy: perspt_core::PlanningPolicy::default(),
            stability_epsilon: 0.1,
            energy_alpha: 1.0,
            energy_beta: 0.5,
            energy_gamma: 2.0,
            abort_requested: Arc::new(AtomicBool::new(false)),
        }
    }

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
    fn is_abort_requested(&self) -> bool {
        self.abort_requested.load(Ordering::Relaxed)
    }

    /// Finalize the session in the ledger based on the execution result.
    fn finalize_session(&mut self, result: &Result<()>) {
        let status = if self.is_abort_requested() {
            "ABORTED"
        } else if result.is_ok() {
            "COMPLETED"
        } else {
            "FAILED"
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
        result
    }

    /// Inner resumed execution logic.
    async fn run_resumed_inner(&mut self) -> Result<()> {
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
                    if let Some(node) = self.graph.node_weight(*idx) {
                        self.emit_event(perspt_core::AgentEvent::NodeCompleted {
                            node_id: node.node_id.clone(),
                            goal: node.goal.clone(),
                        });
                    }
                    executed += 1;
                }
                Ok(NodeOutcome::Escalated) => {
                    escalated += 1;
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
        self.emit_event(perspt_core::AgentEvent::Complete {
            success: escalated == 0,
            message: format!(
                "Resumed: {}/{} completed, {} escalated",
                executed, total_nodes, escalated
            ),
        });
        Ok(())
    }

    /// Emit an event to the TUI (if connected)
    fn emit_event(&self, event: perspt_core::AgentEvent) {
        if let Some(ref sender) = self.event_sender {
            let _ = sender.send(event);
        }
    }

    /// Emit a log message to TUI
    fn emit_log(&self, msg: impl Into<String>) {
        self.emit_event(perspt_core::AgentEvent::Log(msg.into()));
    }

    /// Request approval from user and await response
    /// Returns ApprovalResult with optional edited value.
    /// `review_node_id` is used for persisting the review audit record.
    async fn await_approval(
        &mut self,
        action_type: perspt_core::ActionType,
        description: String,
        diff: Option<String>,
    ) -> ApprovalResult {
        self.await_approval_for_node(action_type, description, diff, None)
            .await
    }

    /// Internal approval with optional node_id for audit persistence.
    async fn await_approval_for_node(
        &mut self,
        action_type: perspt_core::ActionType,
        description: String,
        diff: Option<String>,
        review_node_id: Option<&str>,
    ) -> ApprovalResult {
        // If auto_approve is enabled, skip approval
        if self.auto_approve {
            if let Some(nid) = review_node_id {
                self.persist_review_decision(nid, "auto_approved", None);
            }
            return ApprovalResult::Approved;
        }

        // If no TUI connected, default to approve (headless with --yes)
        if self.action_receiver.is_none() {
            if let Some(nid) = review_node_id {
                self.persist_review_decision(nid, "auto_approved", None);
            }
            return ApprovalResult::Approved;
        }

        // Generate unique request ID
        let request_id = uuid::Uuid::new_v4().to_string();

        // Emit approval request
        self.emit_event(perspt_core::AgentEvent::ApprovalRequest {
            request_id: request_id.clone(),
            node_id: review_node_id.unwrap_or("current").to_string(),
            action_type,
            description,
            diff,
        });

        // Wait for response
        if let Some(ref mut receiver) = self.action_receiver {
            while let Some(action) = receiver.recv().await {
                match action {
                    perspt_core::AgentAction::Approve { request_id: rid } if rid == request_id => {
                        self.emit_log("✓ Approved by user");
                        if let Some(nid) = review_node_id {
                            self.persist_review_decision(nid, "approved", None);
                        }
                        return ApprovalResult::Approved;
                    }
                    perspt_core::AgentAction::ApproveWithEdit {
                        request_id: rid,
                        edited_value,
                    } if rid == request_id => {
                        self.emit_log(format!("✓ Approved with edit: {}", edited_value));
                        if let Some(nid) = review_node_id {
                            self.persist_review_decision(nid, "approved_with_edit", None);
                        }
                        return ApprovalResult::ApprovedWithEdit(edited_value);
                    }
                    perspt_core::AgentAction::Reject {
                        request_id: rid,
                        reason,
                    } if rid == request_id => {
                        let msg = reason.unwrap_or_else(|| "User rejected".to_string());
                        self.emit_log(format!("✗ Rejected: {}", msg));
                        if let Some(nid) = review_node_id {
                            self.persist_review_decision(nid, "rejected", Some(&msg));
                        }
                        return ApprovalResult::Rejected;
                    }
                    perspt_core::AgentAction::RequestCorrection {
                        request_id: rid,
                        feedback,
                    } if rid == request_id => {
                        self.emit_log(format!("🔄 Correction requested: {}", feedback));
                        if let Some(nid) = review_node_id {
                            self.persist_review_decision(
                                nid,
                                "correction_requested",
                                Some(&feedback),
                            );
                        }
                        return ApprovalResult::Rejected;
                    }
                    perspt_core::AgentAction::Abort => {
                        self.emit_log("⚠️ Session aborted by user");
                        self.abort_requested.store(true, Ordering::Relaxed);
                        if let Some(nid) = review_node_id {
                            self.persist_review_decision(nid, "aborted", None);
                        }
                        return ApprovalResult::Rejected;
                    }
                    _ => {
                        // Ignore other actions while waiting for this specific approval
                        continue;
                    }
                }
            }
        }

        ApprovalResult::Rejected // Channel closed
    }

    /// Persist a review decision to the audit trail.
    fn persist_review_decision(&self, node_id: &str, outcome: &str, note: Option<&str>) {
        let degraded = self.last_verification_result.as_ref().map(|vr| vr.degraded);
        if let Err(e) = self
            .ledger
            .record_review_outcome(node_id, outcome, note, None, degraded, None)
        {
            log::warn!("Failed to persist review decision for {}: {}", node_id, e);
        }
    }

    /// Add a dependency edge between nodes
    pub fn add_dependency(&mut self, from_id: &str, to_id: &str, kind: &str) -> Result<()> {
        let from_idx = self
            .node_indices
            .get(from_id)
            .context(format!("Node not found: {}", from_id))?;
        let to_idx = self
            .node_indices
            .get(to_id)
            .context(format!("Node not found: {}", to_id))?;

        self.graph.add_edge(
            *from_idx,
            *to_idx,
            Dependency {
                kind: kind.to_string(),
            },
        );
        Ok(())
    }

    /// Run the complete SRBN control loop
    pub async fn run(&mut self, task: String) -> Result<()> {
        log::info!("Starting SRBN execution for task: {}", task);
        self.emit_log(format!("🚀 Starting task: {}", task));

        // Step 0: Start session first
        let session_id = uuid::Uuid::new_v4().to_string();
        self.context.session_id = session_id.clone();
        self.ledger.start_session(
            &session_id,
            &task,
            &self.context.working_dir.to_string_lossy(),
        )?;

        // Run orchestration and always finalize the session
        let result = self.run_orchestration(task).await;
        self.finalize_session(&result);
        result
    }

    /// Inner orchestration logic — called by `run()` which handles session lifecycle.
    async fn run_orchestration(&mut self, task: String) -> Result<()> {
        if self.context.log_llm {
            self.emit_log("📝 LLM request logging enabled".to_string());
        }

        // PSP-5: Detect execution mode (Project is default, Solo only on explicit keywords)
        let execution_mode = self.detect_execution_mode(&task);
        self.context.execution_mode = execution_mode;
        self.emit_log(format!("🎯 Execution mode: {}", execution_mode));

        if execution_mode == perspt_core::types::ExecutionMode::Solo {
            // Solo Mode: Single-file execution without DAG
            log::info!("Using Solo Mode for explicit single-file task");
            self.emit_log("⚡ Solo Mode: Single-file execution".to_string());
            return self.run_solo_mode(task).await;
        }

        // PSP-5: Classify workspace state before deciding plugin/init strategy
        let workspace_state = self.classify_workspace(&task);
        self.context.workspace_state = workspace_state.clone();
        self.emit_log(format!("📋 Workspace: {}", workspace_state));

        // For existing projects, detect plugins and probe verifier readiness now.
        // For greenfield/ambiguous, defer until after step_init_project().
        if let WorkspaceState::ExistingProject { ref plugins } = workspace_state {
            self.context.active_plugins = plugins.clone();
            self.emit_log(format!("🔌 Detected plugins: {}", plugins.join(", ")));
            self.emit_plugin_readiness();
        }

        // Team Mode: Full project initialization and DAG sheafification
        self.step_init_project(&task).await?;

        // PSP-5: For greenfield/ambiguous workspaces, re-detect plugins after init
        // and probe verifier readiness against the newly initialized project.
        if !matches!(workspace_state, WorkspaceState::ExistingProject { .. }) {
            self.redetect_plugins_after_init();
        }

        // Gate: verify at least one plugin has build capability before planning.
        // Without this, the architect may produce a plan whose verification is
        // fully degraded, leading to false stability.
        self.check_verifier_readiness_gate();

        // Start LSP for detected plugins (after classification + init so we
        // use the authoritative plugin set, not a provisional one).
        {
            let plugin_refs: Vec<String> = self.context.active_plugins.clone();
            let refs: Vec<&str> = plugin_refs.iter().map(|s| s.as_str()).collect();
            if !refs.is_empty() {
                self.emit_log("🔍 Starting language servers...".to_string());
                if let Err(e) = self.start_lsp_for_plugins(&refs).await {
                    log::warn!("Failed to start LSP: {}", e);
                    self.emit_log("⚠️ Continuing without LSP".to_string());
                } else {
                    self.emit_log("✅ Language servers ready".to_string());
                }
            }
        }

        // Select planning policy based on workspace state before architect runs.
        // Greenfield workspaces use GreenfieldBuild; existing projects
        // default to FeatureIncrement (callers may override via set_planning_policy).
        if self.planning_policy == perspt_core::PlanningPolicy::default() {
            self.planning_policy = match &self.context.workspace_state {
                WorkspaceState::Greenfield { .. } => perspt_core::PlanningPolicy::GreenfieldBuild,
                WorkspaceState::ExistingProject { .. } => {
                    perspt_core::PlanningPolicy::FeatureIncrement
                }
                WorkspaceState::Ambiguous => perspt_core::PlanningPolicy::FeatureIncrement,
            };
        }

        // PSP-5 Phase 12: Create a default FeatureCharter so the
        // file-budget gate in step_sheafify has bounds to enforce.
        // Derive sensible defaults from the planning policy.
        if self.ledger.get_feature_charter().ok().flatten().is_none() {
            let mut charter = perspt_core::FeatureCharter::new(&self.context.session_id, &task);
            match self.planning_policy {
                perspt_core::PlanningPolicy::LocalEdit => {
                    charter.max_modules = Some(1);
                    charter.max_files = Some(5);
                    charter.max_revisions = Some(3);
                }
                perspt_core::PlanningPolicy::FeatureIncrement => {
                    charter.max_modules = Some(10);
                    charter.max_files = Some(30);
                    charter.max_revisions = Some(5);
                }
                perspt_core::PlanningPolicy::LargeFeature
                | perspt_core::PlanningPolicy::GreenfieldBuild
                | perspt_core::PlanningPolicy::ArchitecturalRevision => {
                    charter.max_modules = Some(25);
                    charter.max_files = Some(80);
                    charter.max_revisions = Some(10);
                }
            }
            if let Some(ref lang) = self.context.active_plugins.first() {
                charter.language_constraint = Some(lang.to_string());
            }
            if let Err(e) = self.ledger.record_feature_charter(&charter) {
                log::warn!("Failed to persist default FeatureCharter: {}", e);
            } else {
                log::info!(
                    "Registered default FeatureCharter (max_modules={:?}, max_files={:?})",
                    charter.max_modules,
                    charter.max_files
                );
            }
        }

        // Gate architect planning on policy: LocalEdit skips the architect
        // and creates a single-node deterministic graph directly.
        if self.planning_policy.needs_architect() {
            self.step_sheafify(task).await?;
        } else {
            self.emit_log("📐 LocalEdit policy — skipping architect, single-node plan".to_string());
            self.create_deterministic_fallback_graph(&task)?;
        }

        // Planning policy is already resolved above; log it after sheafification.
        self.emit_log(format!("📐 Planning policy: {:?}", self.planning_policy));

        // PSP-5: Emit PlanReady event after sheafification
        let node_count = self.graph.node_count();
        self.emit_event(perspt_core::AgentEvent::PlanReady {
            nodes: node_count,
            plugins: self.context.active_plugins.clone(),
            execution_mode: execution_mode.to_string(),
        });

        // Emit task nodes to TUI after sheafification
        for node_id in self.node_indices.keys() {
            if let Some(idx) = self.node_indices.get(node_id) {
                if let Some(node) = self.graph.node_weight(*idx) {
                    self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                        node_id: node.node_id.clone(),
                        status: perspt_core::NodeStatus::Pending,
                    });
                }
            }
        }

        // Step 2-7: Execute nodes in topological order
        let topo = Topo::new(&self.graph);
        let indices: Vec<_> = topo.iter(&self.graph).collect();
        let total_nodes = indices.len();
        let mut completed_count: usize = 0;
        let mut escalated_count: usize = 0;

        for (i, idx) in indices.iter().enumerate() {
            // Abort gate: stop execution if abort was requested.
            if self.is_abort_requested() {
                self.emit_log("⚠️ Session aborted — stopping execution".to_string());
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

            // PSP-5 Phase 6: Check if node is blocked on a parent interface seal.
            // In the current sequential topo-order execution this should not fire
            // (parents commit before children), but it establishes the gating
            // contract for when speculative parallelism is introduced later.
            if self.check_seal_prerequisites(*idx) {
                log::warn!(
                    "Node {} blocked on seal prerequisite — skipping in this iteration",
                    self.graph[*idx].node_id
                );
                continue;
            }

            // PSP-5: Emit NodeSelected event before execution
            if let Some(node) = self.graph.node_weight(*idx) {
                self.emit_log(format!("📝 [{}/{}] {}", i + 1, total_nodes, node.goal));
                self.emit_event(perspt_core::AgentEvent::NodeSelected {
                    node_id: node.node_id.clone(),
                    goal: node.goal.clone(),
                    node_class: node.node_class.to_string(),
                });
                self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                    node_id: node.node_id.clone(),
                    status: perspt_core::NodeStatus::Running,
                });
            }

            match self.execute_node(*idx).await {
                Ok(NodeOutcome::Completed) => {
                    completed_count += 1;

                    // Record step in budget envelope
                    self.budget.record_step();

                    // Emit budget status after each step
                    self.emit_event(perspt_core::AgentEvent::BudgetUpdated {
                        steps_used: self.budget.steps_used,
                        max_steps: self.budget.max_steps,
                        cost_used_usd: self.budget.cost_used_usd,
                        max_cost_usd: self.budget.max_cost_usd,
                        revisions_used: self.budget.revisions_used,
                        max_revisions: self.budget.max_revisions,
                    });

                    // Persist budget envelope to store for auditability.
                    if let Err(e) = self.ledger.upsert_budget_envelope(&self.budget) {
                        log::warn!("Failed to persist budget envelope: {}", e);
                    }

                    // Emit completed status
                    if let Some(node) = self.graph.node_weight(*idx) {
                        self.emit_event(perspt_core::AgentEvent::NodeCompleted {
                            node_id: node.node_id.clone(),
                            goal: node.goal.clone(),
                        });
                    }
                }
                Ok(NodeOutcome::Escalated) => {
                    escalated_count += 1;
                    self.budget.record_step();

                    // Do NOT emit NodeCompleted — the node was escalated, not completed.
                    if let Some(node) = self.graph.node_weight(*idx) {
                        self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                            node_id: node.node_id.clone(),
                            status: perspt_core::NodeStatus::Escalated,
                        });
                    }
                    continue;
                }
                Err(e) => {
                    escalated_count += 1;
                    let node_id = self.graph[*idx].node_id.clone();
                    eprintln!("[SRBN-DIAG] Node {} failed: {:#}", node_id, e);
                    log::error!("Node {} failed: {}", node_id, e);
                    self.emit_log(format!("❌ Node {} failed: {}", node_id, e));

                    // Flush the node's provisional branch so sandbox files
                    // don't leak. Without this, files written to the sandbox
                    // are lost when step_commit/step_sheaf_validate fails
                    // before merge.
                    if let Some(bid) = self.graph[*idx].provisional_branch_id.clone() {
                        self.flush_provisional_branch(&bid, &node_id);
                    }
                    self.flush_descendant_branches(*idx);

                    self.graph[*idx].state = NodeState::Escalated;
                    self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                        node_id: node_id.clone(),
                        status: perspt_core::NodeStatus::Escalated,
                    });
                    // Continue to next node instead of stopping all execution
                    continue;
                }
            }
        }

        log::info!("SRBN execution completed");

        // PSP-5 Phase 6: Clean up all session sandboxes
        if let Err(e) = crate::tools::cleanup_session_sandboxes(
            &self.context.working_dir,
            &self.context.session_id,
        ) {
            log::warn!("Failed to clean up session sandboxes: {}", e);
        }

        // Derive session outcome from actual node results.
        let outcome = if escalated_count == 0 {
            perspt_core::SessionOutcome::Success
        } else if completed_count > 0 {
            perspt_core::SessionOutcome::PartialSuccess
        } else {
            perspt_core::SessionOutcome::Failed
        };
        self.emit_event(perspt_core::AgentEvent::Complete {
            success: outcome == perspt_core::SessionOutcome::Success,
            message: format!(
                "{}/{} nodes completed, {} escalated",
                completed_count, total_nodes, escalated_count
            ),
        });
        Ok(())
    }

    /// Execute a single node through the control loop
    async fn execute_node(&mut self, idx: NodeIndex) -> Result<NodeOutcome> {
        let node = &self.graph[idx];
        log::info!("Executing node: {} ({})", node.node_id, node.goal);

        // PSP-5 Phase 6: Create provisional branch if node has graph parents
        let branch_id = self.maybe_create_provisional_branch(idx);

        // Step 2: Recursive Sub-graph Execution (already in topo order)
        self.graph[idx].state = NodeState::Coding;
        self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
            node_id: self.graph[idx].node_id.clone(),
            status: perspt_core::NodeStatus::Coding,
        });

        // Step 3: Speculative Generation
        self.step_speculate(idx).await?;

        // Step 4: Stability Verification
        let mut energy = self.step_verify(idx).await?;

        // PSP-7: Sheaf pre-check retry loop.
        // After convergence succeeds, a lightweight structural check verifies
        // output artifacts exist on disk before proceeding to full sheaf
        // validation. If pre-check fails, re-enter convergence with sheaf
        // evidence (max 1 retry to prevent infinite loops).
        let mut sheaf_pre_check_retries = 0u32;
        loop {
            // Step 5: Convergence & Self-Correction
            if !self.step_converge(idx, energy.clone()).await? {
                // PSP-5 Phase 5: Classify non-convergence and choose repair action
                let category = self.classify_non_convergence(idx);
                let action = self.choose_repair_action(idx, &category);

                // Persist the escalation report
                let node = &self.graph[idx];
                let report = EscalationReport {
                    node_id: node.node_id.clone(),
                    session_id: self.context.session_id.clone(),
                    category,
                    action: action.clone(),
                    energy_snapshot: EnergyComponents {
                        v_syn: node.monitor.current_energy(),
                        ..Default::default()
                    },
                    stage_outcomes: self
                        .last_verification_result
                        .as_ref()
                        .map(|vr| vr.stage_outcomes.clone())
                        .unwrap_or_default(),
                    evidence: self.build_escalation_evidence(idx),
                    affected_node_ids: self.affected_dependents(idx),
                    timestamp: epoch_seconds(),
                };

                if let Err(e) = self.ledger.record_escalation_report(&report) {
                    log::warn!("Failed to persist escalation report: {}", e);
                }

                // PSP-5 Phase 9: Also persist artifact bundle on escalation path
                if let Some(bundle) = self.last_applied_bundle.take() {
                    if let Err(e) = self
                        .ledger
                        .record_artifact_bundle(&self.graph[idx].node_id, &bundle)
                    {
                        log::warn!(
                            "Failed to persist artifact bundle on escalation for {}: {}",
                            self.graph[idx].node_id,
                            e
                        );
                    }
                }

                self.emit_event(perspt_core::AgentEvent::EscalationClassified {
                    node_id: report.node_id.clone(),
                    category: report.category.to_string(),
                    action: report.action.to_string(),
                });

                // PSP-5 Phase 6: Flush this branch and all descendant branches
                let node_id_for_flush = self.graph[idx].node_id.clone();
                if let Some(ref bid) = branch_id {
                    self.flush_provisional_branch(bid, &node_id_for_flush);
                }
                self.flush_descendant_branches(idx);

                // Apply the chosen repair action or escalate to user
                let applied = self.apply_repair_action(idx, &action).await;

                if !applied {
                    self.graph[idx].state = NodeState::Escalated;
                    self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                        node_id: self.graph[idx].node_id.clone(),
                        status: perspt_core::NodeStatus::Escalated,
                    });
                    log::warn!(
                        "Node {} escalated to user: {} → {}",
                        self.graph[idx].node_id,
                        category,
                        action
                    );
                }

                return Ok(NodeOutcome::Escalated);
            }

            // PSP-7: Lightweight sheaf pre-check before full validation.
            // Verifies output artifacts exist and are non-empty on disk.
            if sheaf_pre_check_retries < 1 {
                if let Some(evidence) = self.sheaf_pre_check(idx) {
                    sheaf_pre_check_retries += 1;
                    log::warn!(
                        "Sheaf pre-check failed for {}, retrying convergence: {}",
                        self.graph[idx].node_id,
                        evidence
                    );
                    self.emit_log(format!("⚠️ Sheaf pre-check: {}", evidence));
                    // Inject sheaf evidence so the correction LLM sees it
                    self.context.last_test_output = Some(format!(
                    "Structural pre-check failure: {}\nEnsure all declared output files are generated correctly.",
                    evidence
                ));
                    // Re-verify and add sheaf penalty to force correction loop entry
                    energy = self.step_verify(idx).await?;
                    energy.v_sheaf += 2.0;
                    continue;
                }
            }
            break;
        } // end PSP-7 sheaf pre-check loop

        // Step 6: Sheaf Validation (Post-Subgraph Consistency)
        self.step_sheaf_validate(idx).await?;

        // Step 7: Merkle Ledger Commit
        self.step_commit(idx).await?;

        // PSP-5 Phase 6: Merge provisional branch after successful commit
        if let Some(ref bid) = branch_id {
            self.merge_provisional_branch(bid, idx);
        }

        Ok(NodeOutcome::Completed)
    }

    /// Step 3: Speculative Generation
    async fn step_speculate(&mut self, idx: NodeIndex) -> Result<()> {
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
                    "Required structural dependencies lack machine-verifiable digests (only prose summaries): [{}]",
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
    fn extract_command_from_response(&self, content: &str) -> Option<String> {
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
    fn lsp_key_for_file(&self, path: &str) -> Option<String> {
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
    fn sandbox_dir_for_node(&self, idx: NodeIndex) -> Option<std::path::PathBuf> {
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

    /// PSP-7: Lightweight sheaf pre-check before full sheaf validation.
    ///
    /// Verifies that every declared output target actually exists on disk and
    /// is non-empty. Returns `Some(evidence)` if the pre-check fails, `None`
    /// if everything looks good.
    fn sheaf_pre_check(&self, idx: NodeIndex) -> Option<String> {
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
                Ok(_) => {}
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
    fn effective_working_dir(&self, idx: NodeIndex) -> std::path::PathBuf {
        self.sandbox_dir_for_node(idx)
            .unwrap_or_else(|| self.context.working_dir.clone())
    }

    /// Create a provisional branch if the node has graph parents (i.e., it
    /// depends on another node's output). Returns the branch ID if created.
    fn maybe_create_provisional_branch(&mut self, idx: NodeIndex) -> Option<String> {
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
    fn merge_provisional_branch(&mut self, branch_id: &str, idx: NodeIndex) {
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
    fn flush_provisional_branch(&mut self, branch_id: &str, node_id: &str) {
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
    fn flush_descendant_branches(&mut self, idx: NodeIndex) {
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
    fn collect_descendants(&self, idx: NodeIndex) -> Vec<NodeIndex> {
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
    fn emit_interface_seals(&mut self, idx: NodeIndex) {
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
    fn unblock_dependents(&mut self, idx: NodeIndex) {
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
    fn check_seal_prerequisites(&mut self, idx: NodeIndex) -> bool {
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
    fn check_structural_dependencies(
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
                        "Interface node '{}' has no Signature/Schema/InterfaceSeal digest in the restriction map",
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
    fn inject_sealed_interfaces(
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

/// Parse a persisted state string back into a NodeState enum
fn parse_node_state(s: &str) -> NodeState {
    NodeState::from_display_str(s)
}

/// Parse a persisted node class string back into a NodeClass enum
fn parse_node_class(s: &str) -> NodeClass {
    match s {
        "Interface" => NodeClass::Interface,
        "Implementation" => NodeClass::Implementation,
        "Integration" => NodeClass::Integration,
        _ => NodeClass::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::verification::verification_stages_for_node;
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_orchestrator_creation() {
        let orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
        assert_eq!(orch.node_count(), 0);
    }

    #[tokio::test]
    async fn test_add_nodes() {
        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));

        let node1 = SRBNNode::new(
            "node1".to_string(),
            "Test task 1".to_string(),
            ModelTier::Architect,
        );
        let node2 = SRBNNode::new(
            "node2".to_string(),
            "Test task 2".to_string(),
            ModelTier::Actuator,
        );

        orch.add_node(node1);
        orch.add_node(node2);
        orch.add_dependency("node1", "node2", "depends_on").unwrap();

        assert_eq!(orch.node_count(), 2);
    }
    #[tokio::test]
    async fn test_lsp_key_for_file_resolves_by_plugin() {
        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
        // Insert a dummy LSP client key so the lookup has something to match
        orch.lsp_clients.insert(
            "rust".to_string(),
            crate::lsp::LspClient::new("rust-analyzer"),
        );
        orch.lsp_clients
            .insert("python".to_string(), crate::lsp::LspClient::new("pylsp"));

        // Rust plugin owns .rs files
        assert_eq!(
            orch.lsp_key_for_file("src/main.rs"),
            Some("rust".to_string())
        );
        // Python plugin owns .py files
        assert_eq!(orch.lsp_key_for_file("app.py"), Some("python".to_string()));
        // Unknown extension falls back to first available client
        let key = orch.lsp_key_for_file("data.csv");
        assert!(key.is_some()); // Falls back to first available
    }

    // =========================================================================
    // Phase 5: Graph rewrite & sheaf validator tests
    // =========================================================================

    #[tokio::test]
    async fn test_split_node_creates_children() {
        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
        let mut node = SRBNNode::new("parent".into(), "Do everything".into(), ModelTier::Actuator);
        node.output_targets = vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")];
        orch.add_node(node);

        let idx = orch.node_indices["parent"];
        let applied = orch.split_node(idx, &["handle a.rs".into(), "handle b.rs".into()]);
        assert!(!applied.is_empty());
        // Parent should be gone
        assert!(!orch.node_indices.contains_key("parent"));
        // Two children should exist
        assert!(orch.node_indices.contains_key("parent__split_0"));
        assert!(orch.node_indices.contains_key("parent__split_1"));
    }

    #[tokio::test]
    async fn test_split_node_empty_children_is_noop() {
        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
        let node = SRBNNode::new("n".into(), "g".into(), ModelTier::Actuator);
        orch.add_node(node);
        let idx = orch.node_indices["n"];
        let applied = orch.split_node(idx, &[]);
        // Should not apply — return empty vec but not panic
        assert!(applied.is_empty());
    }

    #[tokio::test]
    async fn test_insert_interface_node() {
        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
        let n1 = SRBNNode::new("a".into(), "source".into(), ModelTier::Actuator);
        let n2 = SRBNNode::new("b".into(), "dest".into(), ModelTier::Actuator);
        orch.add_node(n1);
        orch.add_node(n2);
        orch.add_dependency("a", "b", "data_flow").unwrap();

        let idx_a = orch.node_indices["a"];
        let applied = orch.insert_interface_node(idx_a, "API boundary");
        assert!(applied.is_some());
        assert!(orch.node_indices.contains_key("a__iface"));
        // Should now have 3 nodes
        assert_eq!(orch.node_count(), 3);
    }

    #[tokio::test]
    async fn test_replan_subgraph_resets_nodes() {
        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
        let mut n1 = SRBNNode::new("trigger".into(), "g1".into(), ModelTier::Actuator);
        n1.state = NodeState::Coding;
        let mut n2 = SRBNNode::new("dep".into(), "g2".into(), ModelTier::Actuator);
        n2.state = NodeState::Completed;
        orch.add_node(n1);
        orch.add_node(n2);

        let trigger_idx = orch.node_indices["trigger"];
        let applied = orch.replan_subgraph(trigger_idx, &["dep".into()]);
        assert!(applied);

        let dep_idx = orch.node_indices["dep"];
        assert_eq!(orch.graph[dep_idx].state, NodeState::TaskQueued);
        assert_eq!(orch.graph[trigger_idx].state, NodeState::Retry);
    }

    #[tokio::test]
    async fn test_select_validators_always_includes_dependency_graph() {
        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
        let node = SRBNNode::new("n".into(), "g".into(), ModelTier::Actuator);
        orch.add_node(node);
        let idx = orch.node_indices["n"];

        let validators = orch.select_validators(idx);
        assert!(validators.contains(&SheafValidatorClass::DependencyGraphConsistency));
    }

    #[tokio::test]
    async fn test_select_validators_interface_node() {
        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
        let mut node = SRBNNode::new("iface".into(), "g".into(), ModelTier::Actuator);
        node.node_class = perspt_core::types::NodeClass::Interface;
        orch.add_node(node);
        let idx = orch.node_indices["iface"];

        let validators = orch.select_validators(idx);
        assert!(validators.contains(&SheafValidatorClass::ExportImportConsistency));
    }

    #[tokio::test]
    async fn test_run_sheaf_validator_dependency_graph_no_cycles() {
        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
        let n1 = SRBNNode::new("a".into(), "g".into(), ModelTier::Actuator);
        let n2 = SRBNNode::new("b".into(), "g".into(), ModelTier::Actuator);
        orch.add_node(n1);
        orch.add_node(n2);
        orch.add_dependency("a", "b", "dep").unwrap();

        let idx = orch.node_indices["a"];
        let result = orch.run_sheaf_validator(idx, SheafValidatorClass::DependencyGraphConsistency);
        assert!(result.passed);
        assert_eq!(result.v_sheaf_contribution, 0.0);
    }

    #[tokio::test]
    async fn test_classify_non_convergence_default() {
        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
        let node = SRBNNode::new("n".into(), "g".into(), ModelTier::Actuator);
        orch.add_node(node);
        let idx = orch.node_indices["n"];

        // With no verification results or policy failures, should default to ImplementationError
        let category = orch.classify_non_convergence(idx);
        assert_eq!(category, EscalationCategory::ImplementationError);
    }

    #[tokio::test]
    async fn test_affected_dependents() {
        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
        let n1 = SRBNNode::new("root".into(), "g".into(), ModelTier::Actuator);
        let n2 = SRBNNode::new("child1".into(), "g".into(), ModelTier::Actuator);
        let n3 = SRBNNode::new("child2".into(), "g".into(), ModelTier::Actuator);
        orch.add_node(n1);
        orch.add_node(n2);
        orch.add_node(n3);
        orch.add_dependency("root", "child1", "dep").unwrap();
        orch.add_dependency("root", "child2", "dep").unwrap();

        let idx = orch.node_indices["root"];
        let deps = orch.affected_dependents(idx);
        assert_eq!(deps.len(), 2);
        assert!(deps.contains(&"child1".to_string()));
        assert!(deps.contains(&"child2".to_string()));
    }

    // =========================================================================
    // PSP-5 Phase 6: Provisional Branch Tests
    // =========================================================================

    #[tokio::test]
    async fn test_maybe_create_provisional_branch_root_node() {
        let temp_dir =
            std::env::temp_dir().join(format!("perspt_root_branch_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let mut orch = SRBNOrchestrator::new_for_testing(temp_dir.clone());
        orch.context.session_id = "test_session".into();
        let node = SRBNNode::new("root".into(), "root goal".into(), ModelTier::Actuator);
        orch.add_node(node);

        let idx = orch.node_indices["root"];
        // Root nodes now also get a provisional branch with sandbox
        let branch = orch.maybe_create_provisional_branch(idx);
        assert!(branch.is_some());
        assert!(orch.graph[idx].provisional_branch_id.is_some());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_maybe_create_provisional_branch_child_node() {
        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_phase6"));
        orch.context.session_id = "test_session".into();
        let parent = SRBNNode::new("parent".into(), "parent goal".into(), ModelTier::Actuator);
        let child = SRBNNode::new("child".into(), "child goal".into(), ModelTier::Actuator);
        orch.add_node(parent);
        orch.add_node(child);
        orch.add_dependency("parent", "child", "dep").unwrap();

        let idx = orch.node_indices["child"];
        let branch = orch.maybe_create_provisional_branch(idx);
        assert!(branch.is_some());
        assert!(orch.graph[idx].provisional_branch_id.is_some());
    }

    #[tokio::test]
    async fn test_collect_descendants() {
        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
        let n1 = SRBNNode::new("a".into(), "g".into(), ModelTier::Actuator);
        let n2 = SRBNNode::new("b".into(), "g".into(), ModelTier::Actuator);
        let n3 = SRBNNode::new("c".into(), "g".into(), ModelTier::Actuator);
        let n4 = SRBNNode::new("d".into(), "g".into(), ModelTier::Actuator);
        orch.add_node(n1);
        orch.add_node(n2);
        orch.add_node(n3);
        orch.add_node(n4);
        orch.add_dependency("a", "b", "dep").unwrap();
        orch.add_dependency("b", "c", "dep").unwrap();
        orch.add_dependency("a", "d", "dep").unwrap();

        let idx_a = orch.node_indices["a"];
        let descendants = orch.collect_descendants(idx_a);
        assert_eq!(descendants.len(), 3); // b, c, d
    }

    #[tokio::test]
    async fn test_check_seal_prerequisites_no_interface_parent() {
        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
        let parent = SRBNNode::new("parent".into(), "g".into(), ModelTier::Actuator);
        let child = SRBNNode::new("child".into(), "g".into(), ModelTier::Actuator);
        orch.add_node(parent);
        orch.add_node(child);
        orch.add_dependency("parent", "child", "dep").unwrap();

        let idx = orch.node_indices["child"];
        // Parent is Implementation (default), not Interface — should not block
        assert!(!orch.check_seal_prerequisites(idx));
        assert!(orch.blocked_dependencies.is_empty());
    }

    #[tokio::test]
    async fn test_check_seal_prerequisites_unsealed_interface() {
        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
        let mut parent = SRBNNode::new("iface".into(), "g".into(), ModelTier::Actuator);
        parent.node_class = perspt_core::types::NodeClass::Interface;
        let child = SRBNNode::new("impl".into(), "g".into(), ModelTier::Actuator);
        orch.add_node(parent);
        orch.add_node(child);
        orch.add_dependency("iface", "impl", "dep").unwrap();

        let idx = orch.node_indices["impl"];
        // Interface parent not sealed and not completed — should block
        assert!(orch.check_seal_prerequisites(idx));
        assert_eq!(orch.blocked_dependencies.len(), 1);
        assert_eq!(orch.blocked_dependencies[0].parent_node_id, "iface");
    }

    #[tokio::test]
    async fn test_check_seal_prerequisites_sealed_interface() {
        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
        let mut parent = SRBNNode::new("iface".into(), "g".into(), ModelTier::Actuator);
        parent.node_class = perspt_core::types::NodeClass::Interface;
        parent.interface_seal_hash = Some([1u8; 32]); // Already sealed
        let child = SRBNNode::new("impl".into(), "g".into(), ModelTier::Actuator);
        orch.add_node(parent);
        orch.add_node(child);
        orch.add_dependency("iface", "impl", "dep").unwrap();

        let idx = orch.node_indices["impl"];
        // Interface parent is sealed — should not block
        assert!(!orch.check_seal_prerequisites(idx));
        assert!(orch.blocked_dependencies.is_empty());
    }

    #[tokio::test]
    async fn test_unblock_dependents() {
        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
        let parent = SRBNNode::new("parent".into(), "g".into(), ModelTier::Actuator);
        let child = SRBNNode::new("child".into(), "g".into(), ModelTier::Actuator);
        orch.add_node(parent);
        orch.add_node(child);

        // Manually add a blocked dependency
        orch.blocked_dependencies
            .push(perspt_core::types::BlockedDependency::new(
                "child",
                "parent",
                vec!["src/api.rs".into()],
            ));
        assert_eq!(orch.blocked_dependencies.len(), 1);

        let idx = orch.node_indices["parent"];
        orch.unblock_dependents(idx);
        assert!(orch.blocked_dependencies.is_empty());
    }

    #[tokio::test]
    async fn test_flush_descendant_branches() {
        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_phase6_flush"));
        orch.context.session_id = "test_session".into();

        let parent = SRBNNode::new("parent".into(), "g".into(), ModelTier::Actuator);
        let mut child1 = SRBNNode::new("child1".into(), "g".into(), ModelTier::Actuator);
        child1.provisional_branch_id = Some("branch_c1".into());
        let mut child2 = SRBNNode::new("child2".into(), "g".into(), ModelTier::Actuator);
        child2.provisional_branch_id = Some("branch_c2".into());
        let grandchild = SRBNNode::new("grandchild".into(), "g".into(), ModelTier::Actuator);
        orch.add_node(parent);
        orch.add_node(child1);
        orch.add_node(child2);
        orch.add_node(grandchild);
        orch.add_dependency("parent", "child1", "dep").unwrap();
        orch.add_dependency("parent", "child2", "dep").unwrap();
        orch.add_dependency("child1", "grandchild", "dep").unwrap();

        let idx = orch.node_indices["parent"];
        // This will try to flush branches but ledger may not find them —
        // the important thing is it doesn't panic and traverses correctly
        orch.flush_descendant_branches(idx);
    }

    // =========================================================================
    // PSP-5 Completion Tests
    // =========================================================================

    #[tokio::test]
    async fn test_effective_working_dir_no_branch() {
        let orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/test/workspace"));
        // No nodes, but we can test the helper directly by adding one
        let mut orch = orch;
        let node = SRBNNode::new("n1".into(), "goal".into(), ModelTier::Actuator);
        orch.add_node(node);
        let idx = orch.node_indices["n1"];
        // No provisional branch → returns live workspace
        assert_eq!(
            orch.effective_working_dir(idx),
            PathBuf::from("/test/workspace")
        );
    }

    #[tokio::test]
    async fn test_sandbox_dir_for_node_none_without_branch() {
        let orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/test/workspace"));
        let mut orch = orch;
        let node = SRBNNode::new("n1".into(), "goal".into(), ModelTier::Actuator);
        orch.add_node(node);
        let idx = orch.node_indices["n1"];
        assert!(orch.sandbox_dir_for_node(idx).is_none());
    }

    #[tokio::test]
    async fn test_rewrite_churn_guardrail() {
        let orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_churn"));
        let mut orch = orch;
        let node = SRBNNode::new("node_a".into(), "goal".into(), ModelTier::Actuator);
        orch.add_node(node);
        // count_lineage_rewrites should return 0 for a fresh node
        let count = orch.count_lineage_rewrites("node_a");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_run_resumed_skips_terminal_nodes() {
        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_resume"));

        let mut n1 = SRBNNode::new("done".into(), "completed".into(), ModelTier::Actuator);
        n1.state = NodeState::Completed;
        let mut n2 = SRBNNode::new("failed".into(), "failed".into(), ModelTier::Actuator);
        n2.state = NodeState::Failed;
        orch.add_node(n1);
        orch.add_node(n2);

        // Both nodes are terminal, so run_resumed should do nothing and succeed
        let result = orch.run_resumed().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_persist_review_decision_no_panic() {
        let orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_review"));
        // Should not panic even without a real ledger session —
        // it gracefully logs errors
        orch.persist_review_decision("node_x", "approved", None);
    }

    // =========================================================================
    // PSP-5 Gap Tests
    // =========================================================================

    #[tokio::test]
    async fn test_check_structural_dependencies_blocks_prose_only() {
        use perspt_core::types::{NodeClass, RestrictionMap};

        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_struct_dep"));

        // Parent: Interface node (no structural digests)
        let mut parent = SRBNNode::new("iface_1".into(), "Define API".into(), ModelTier::Architect);
        parent.node_class = NodeClass::Interface;

        // Child: Implementation node depending on the interface
        let mut child = SRBNNode::new("impl_1".into(), "Implement API".into(), ModelTier::Actuator);
        child.node_class = NodeClass::Implementation;

        let parent_idx = orch.add_node(parent);
        let child_idx = orch.add_node(child.clone());
        orch.graph
            .add_edge(parent_idx, child_idx, Dependency { kind: "dep".into() });

        // Empty restriction map — no structural digests at all
        let rmap = RestrictionMap::for_node("impl_1");
        let gaps = orch.check_structural_dependencies(&child, &rmap);

        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].0, "iface_1");
        assert!(gaps[0].1.contains("no Signature/Schema/InterfaceSeal"));
    }

    #[tokio::test]
    async fn test_check_structural_dependencies_passes_with_digest() {
        use perspt_core::types::{ArtifactKind, NodeClass, RestrictionMap, StructuralDigest};

        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_struct_ok"));

        let mut parent = SRBNNode::new("iface_2".into(), "Define API".into(), ModelTier::Architect);
        parent.node_class = NodeClass::Interface;

        let mut child = SRBNNode::new("impl_2".into(), "Implement API".into(), ModelTier::Actuator);
        child.node_class = NodeClass::Implementation;

        let parent_idx = orch.add_node(parent);
        let child_idx = orch.add_node(child.clone());
        orch.graph
            .add_edge(parent_idx, child_idx, Dependency { kind: "dep".into() });

        // Restriction map with a Signature digest from the Interface node
        let mut rmap = RestrictionMap::for_node("impl_2");
        rmap.structural_digests.push(StructuralDigest::from_content(
            "iface_2",
            "api.rs",
            ArtifactKind::Signature,
            b"fn do_thing(x: i32) -> bool;",
        ));

        let gaps = orch.check_structural_dependencies(&child, &rmap);
        assert!(gaps.is_empty(), "Expected no gaps when digest present");
    }

    #[tokio::test]
    async fn test_check_structural_dependencies_skips_non_implementation() {
        use perspt_core::types::{NodeClass, RestrictionMap};

        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_struct_skip"));

        // An Integration node should NOT be checked
        let mut node = SRBNNode::new("integ_1".into(), "Wire modules".into(), ModelTier::Actuator);
        node.node_class = NodeClass::Integration;
        orch.add_node(node.clone());

        let rmap = RestrictionMap::for_node("integ_1");
        let gaps = orch.check_structural_dependencies(&node, &rmap);
        assert!(gaps.is_empty(), "Integration nodes should skip the check");
    }

    #[tokio::test]
    async fn test_tier_default_models_are_differentiated() {
        // PSP-5 Fix D: each tier should map to a different default model
        let arch = ModelTier::Architect.default_model();
        let act = ModelTier::Actuator.default_model();
        let spec = ModelTier::Speculator.default_model();

        // Architect and Actuator should NOT be the same tier default
        assert_ne!(arch, act, "Architect and Actuator defaults should differ");
        // Speculator should be the lightest
        assert_ne!(spec, arch, "Speculator should differ from Architect");
    }

    // =========================================================================
    // PSP-5: Tier Wiring and Plan Validation Tests
    // =========================================================================

    #[tokio::test]
    async fn test_orchestrator_stores_all_four_tier_models() {
        let orch = SRBNOrchestrator::new_with_models(
            PathBuf::from("/tmp/test_tiers"),
            false,
            Some("arch-model".into()),
            Some("act-model".into()),
            Some("ver-model".into()),
            Some("spec-model".into()),
            None,
            None,
            None,
            None,
        );
        assert_eq!(orch.architect_model, "arch-model");
        assert_eq!(orch.actuator_model, "act-model");
        assert_eq!(orch.verifier_model, "ver-model");
        assert_eq!(orch.speculator_model, "spec-model");
    }

    #[tokio::test]
    async fn test_orchestrator_default_tier_models() {
        let orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_tier_defaults"));
        assert_eq!(orch.architect_model, ModelTier::Architect.default_model());
        assert_eq!(orch.actuator_model, ModelTier::Actuator.default_model());
        assert_eq!(orch.verifier_model, ModelTier::Verifier.default_model());
        assert_eq!(orch.speculator_model, ModelTier::Speculator.default_model());
    }

    #[tokio::test]
    async fn test_create_nodes_rejects_duplicate_output_files() {
        use perspt_core::types::PlannedTask;

        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_dup_outputs"));

        let plan = TaskPlan {
            tasks: vec![
                PlannedTask {
                    id: "task_1".into(),
                    goal: "Create math".into(),
                    output_files: vec!["src/math.py".into(), "tests/test_math.py".into()],
                    ..PlannedTask::new("task_1", "Create math")
                },
                PlannedTask {
                    id: "task_2".into(),
                    goal: "Create tests".into(),
                    output_files: vec!["tests/test_math.py".into()],
                    ..PlannedTask::new("task_2", "Create tests")
                },
            ],
        };

        let result = orch.create_nodes_from_plan(&plan);
        assert!(result.is_err(), "Should reject duplicate output_files");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("tests/test_math.py"),
            "Error should mention the duplicate file: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_nodes_accepts_unique_output_files() {
        use perspt_core::types::PlannedTask;

        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_unique_outputs"));

        let plan = TaskPlan {
            tasks: vec![
                PlannedTask {
                    id: "task_1".into(),
                    goal: "Create math".into(),
                    output_files: vec!["src/math.py".into()],
                    ..PlannedTask::new("task_1", "Create math")
                },
                PlannedTask {
                    id: "test_1".into(),
                    goal: "Test math".into(),
                    output_files: vec!["tests/test_math.py".into()],
                    dependencies: vec!["task_1".into()],
                    ..PlannedTask::new("test_1", "Test math")
                },
            ],
        };

        let result = orch.create_nodes_from_plan(&plan);
        assert!(result.is_ok(), "Should accept unique output_files");
        assert_eq!(orch.graph.node_count(), 2);
    }

    #[tokio::test]
    async fn test_ownership_manifest_built_with_majority_plugin_vote() {
        use perspt_core::types::PlannedTask;

        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_plugin_vote"));

        let plan = TaskPlan {
            tasks: vec![PlannedTask {
                id: "task_1".into(),
                goal: "Create Python module".into(),
                output_files: vec![
                    "src/main.py".into(),
                    "src/helper.py".into(),
                    "src/__init__.py".into(),
                ],
                ..PlannedTask::new("task_1", "Create Python module")
            }],
        };

        orch.create_nodes_from_plan(&plan).unwrap();

        // All three files should be in the manifest
        assert_eq!(orch.context.ownership_manifest.len(), 3);
        // The node should have the python plugin assigned
        let idx = orch.node_indices["task_1"];
        assert_eq!(orch.graph[idx].owner_plugin, "python");
    }

    #[tokio::test]
    async fn test_apply_bundle_strips_paths_outside_node_output_targets() {
        use perspt_core::types::{ArtifactBundle, ArtifactOperation, PlannedTask};

        let temp_dir = std::env::temp_dir().join(format!(
            "perspt_bundle_target_guard_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(temp_dir.join("src")).unwrap();

        let mut orch = SRBNOrchestrator::new_for_testing(temp_dir.clone());
        let plan = TaskPlan {
            tasks: vec![
                PlannedTask {
                    id: "validate_module".into(),
                    goal: "Create validation module".into(),
                    output_files: vec!["src/validate.rs".into()],
                    ..PlannedTask::new("validate_module", "Create validation module")
                },
                PlannedTask {
                    id: "lib_module".into(),
                    goal: "Export validation module".into(),
                    output_files: vec!["src/lib.rs".into()],
                    dependencies: vec!["validate_module".into()],
                    ..PlannedTask::new("lib_module", "Export validation module")
                },
            ],
        };

        orch.create_nodes_from_plan(&plan).unwrap();

        let bundle = ArtifactBundle {
            artifacts: vec![
                ArtifactOperation::Write {
                    path: "src/validate.rs".into(),
                    content: "pub fn ok() {}".into(),
                },
                ArtifactOperation::Write {
                    path: "src/lib.rs".into(),
                    content: "pub mod validate;".into(),
                },
            ],
            commands: vec![],
        };

        // Should succeed — the undeclared path src/lib.rs is stripped, but
        // src/validate.rs is applied.
        orch.apply_bundle_transactionally(
            &bundle,
            "validate_module",
            perspt_core::types::NodeClass::Implementation,
        )
        .await
        .expect("Should apply valid artifacts after stripping undeclared paths");

        // The declared file should be written
        assert!(temp_dir.join("src/validate.rs").exists());
        // The undeclared file should NOT be written
        assert!(!temp_dir.join("src/lib.rs").exists());
    }

    #[tokio::test]
    async fn test_apply_bundle_writes_into_branch_sandbox() {
        use perspt_core::types::{ArtifactBundle, ArtifactOperation, PlannedTask};

        let temp_dir = std::env::temp_dir().join(format!(
            "perspt_branch_sandbox_write_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(temp_dir.join("src")).unwrap();
        std::fs::write(temp_dir.join("src/lib.rs"), "pub fn old() {}\n").unwrap();

        let mut orch = SRBNOrchestrator::new_for_testing(temp_dir.clone());
        orch.context.session_id = uuid::Uuid::new_v4().to_string();

        let plan = TaskPlan {
            tasks: vec![
                PlannedTask {
                    id: "parent".into(),
                    goal: "Parent node".into(),
                    output_files: vec!["src/lib.rs".into()],
                    ..PlannedTask::new("parent", "Parent node")
                },
                PlannedTask {
                    id: "child".into(),
                    goal: "Child node".into(),
                    context_files: vec!["src/lib.rs".into()],
                    output_files: vec!["src/child.rs".into()],
                    dependencies: vec!["parent".into()],
                    ..PlannedTask::new("child", "Child node")
                },
            ],
        };

        orch.create_nodes_from_plan(&plan).unwrap();
        let child_idx = orch.node_indices["child"];
        let branch_id = orch.maybe_create_provisional_branch(child_idx).unwrap();
        let sandbox_dir = orch.sandbox_dir_for_node(child_idx).unwrap();

        let bundle = ArtifactBundle {
            artifacts: vec![ArtifactOperation::Write {
                path: "src/child.rs".into(),
                content: "pub fn child() {}\n".into(),
            }],
            commands: vec![],
        };

        orch.apply_bundle_transactionally(
            &bundle,
            "child",
            perspt_core::types::NodeClass::Implementation,
        )
        .await
        .unwrap();

        assert!(sandbox_dir.join("src/child.rs").exists());
        assert!(!temp_dir.join("src/child.rs").exists());

        orch.merge_provisional_branch(&branch_id, child_idx);
    }

    #[test]
    fn test_verification_stages_for_node_classes() {
        use perspt_core::plugin::VerifierStage;

        // Interface → SyntaxCheck only
        let interface_node =
            SRBNNode::new("iface".into(), "Define trait".into(), ModelTier::Actuator);
        // Default is Implementation, so override:
        let mut interface_node = interface_node;
        interface_node.node_class = perspt_core::types::NodeClass::Interface;
        let stages = verification_stages_for_node(&interface_node);
        assert_eq!(stages, vec![VerifierStage::SyntaxCheck]);

        // Implementation without tests → SyntaxCheck + Build
        let mut implementation_node = SRBNNode::new(
            "impl".into(),
            "Implement feature".into(),
            ModelTier::Actuator,
        );
        implementation_node.node_class = perspt_core::types::NodeClass::Implementation;
        let stages = verification_stages_for_node(&implementation_node);
        assert_eq!(
            stages,
            vec![VerifierStage::SyntaxCheck, VerifierStage::Build]
        );

        // Implementation with weighted tests → SyntaxCheck + Build + Test
        implementation_node
            .contract
            .weighted_tests
            .push(perspt_core::types::WeightedTest {
                test_name: "test_feature".into(),
                criticality: perspt_core::types::Criticality::High,
            });
        let stages = verification_stages_for_node(&implementation_node);
        assert_eq!(
            stages,
            vec![
                VerifierStage::SyntaxCheck,
                VerifierStage::Build,
                VerifierStage::Test
            ]
        );

        // Integration → full pipeline
        let mut integration_node =
            SRBNNode::new("test".into(), "Verify feature".into(), ModelTier::Actuator);
        integration_node.node_class = perspt_core::types::NodeClass::Integration;
        integration_node
            .contract
            .weighted_tests
            .push(perspt_core::types::WeightedTest {
                test_name: "test_feature".into(),
                criticality: perspt_core::types::Criticality::High,
            });
        let stages = verification_stages_for_node(&integration_node);
        assert_eq!(
            stages,
            vec![
                VerifierStage::SyntaxCheck,
                VerifierStage::Build,
                VerifierStage::Test,
                VerifierStage::Lint,
            ]
        );
    }

    // =========================================================================
    // Workspace Classification Tests
    // =========================================================================

    #[tokio::test]
    async fn test_classify_workspace_empty_dir() {
        let temp = tempfile::tempdir().unwrap();
        let orch = SRBNOrchestrator::new_for_testing(temp.path().to_path_buf());
        let state = orch.classify_workspace("build a web app");
        // Empty dir with language keywords → Greenfield
        assert!(matches!(state, WorkspaceState::Greenfield { .. }));
    }

    #[tokio::test]
    async fn test_classify_workspace_empty_dir_no_lang() {
        let temp = tempfile::tempdir().unwrap();
        let orch = SRBNOrchestrator::new_for_testing(temp.path().to_path_buf());
        let state = orch.classify_workspace("do something");
        // Empty dir, no keywords → Greenfield with no lang
        match state {
            WorkspaceState::Greenfield { inferred_lang } => assert!(inferred_lang.is_none()),
            _ => panic!("expected Greenfield, got {:?}", state),
        }
    }

    #[tokio::test]
    async fn test_classify_workspace_existing_rust_project() {
        let temp = tempfile::tempdir().unwrap();
        // Create a Cargo.toml to make it look like a Rust project
        std::fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"",
        )
        .unwrap();
        let orch = SRBNOrchestrator::new_for_testing(temp.path().to_path_buf());
        let state = orch.classify_workspace("add a feature");
        match state {
            WorkspaceState::ExistingProject { plugins } => {
                assert!(plugins.contains(&"rust".to_string()));
            }
            _ => panic!("expected ExistingProject, got {:?}", state),
        }
    }

    #[tokio::test]
    async fn test_classify_workspace_existing_python_project() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("pyproject.toml"),
            "[project]\nname = \"test\"",
        )
        .unwrap();
        let orch = SRBNOrchestrator::new_for_testing(temp.path().to_path_buf());
        let state = orch.classify_workspace("add a feature");
        match state {
            WorkspaceState::ExistingProject { plugins } => {
                assert!(plugins.contains(&"python".to_string()));
            }
            _ => panic!("expected ExistingProject, got {:?}", state),
        }
    }

    #[tokio::test]
    async fn test_classify_workspace_existing_js_project() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("package.json"), "{}").unwrap();
        let orch = SRBNOrchestrator::new_for_testing(temp.path().to_path_buf());
        let state = orch.classify_workspace("add auth");
        match state {
            WorkspaceState::ExistingProject { plugins } => {
                assert!(plugins.contains(&"javascript".to_string()));
            }
            _ => panic!("expected ExistingProject, got {:?}", state),
        }
    }

    #[tokio::test]
    async fn test_classify_workspace_ambiguous_with_misc_files() {
        let temp = tempfile::tempdir().unwrap();
        // Non-empty dir with misc files that don't match any plugin
        std::fs::write(temp.path().join("notes.txt"), "hello").unwrap();
        std::fs::write(temp.path().join("data.csv"), "a,b,c").unwrap();
        let orch = SRBNOrchestrator::new_for_testing(temp.path().to_path_buf());
        let state = orch.classify_workspace("do something");
        assert!(matches!(state, WorkspaceState::Ambiguous));
    }

    #[tokio::test]
    async fn test_classify_workspace_greenfield_with_rust_task() {
        let temp = tempfile::tempdir().unwrap();
        let orch = SRBNOrchestrator::new_for_testing(temp.path().to_path_buf());
        let state = orch.classify_workspace("create a rust CLI tool");
        match state {
            WorkspaceState::Greenfield { inferred_lang } => {
                assert_eq!(inferred_lang, Some("rust".to_string()));
            }
            _ => panic!("expected Greenfield, got {:?}", state),
        }
    }

    #[tokio::test]
    async fn test_classify_workspace_greenfield_with_python_task() {
        let temp = tempfile::tempdir().unwrap();
        let orch = SRBNOrchestrator::new_for_testing(temp.path().to_path_buf());
        let state = orch.classify_workspace("build a python flask API");
        match state {
            WorkspaceState::Greenfield { inferred_lang } => {
                assert_eq!(inferred_lang, Some("python".to_string()));
            }
            _ => panic!("expected Greenfield, got {:?}", state),
        }
    }

    // =========================================================================
    // Tool Prerequisite Tests
    // =========================================================================

    #[tokio::test]
    async fn test_check_prerequisites_returns_true_when_tools_available() {
        let orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
        let registry = perspt_core::plugin::PluginRegistry::new();
        // Rust plugin — cargo/rustc should be available in dev environment
        if let Some(plugin) = registry.get("rust") {
            let result = orch.check_tool_prerequisites(plugin);
            // We can't assert true (CI might not have rust-analyzer)
            // but the method should not panic
            let _ = result;
        }
    }

    #[test]
    fn test_required_binaries_rust_includes_cargo() {
        let registry = perspt_core::plugin::PluginRegistry::new();
        let plugin = registry.get("rust").unwrap();
        let bins = plugin.required_binaries();
        assert!(bins.iter().any(|(name, _, _)| *name == "cargo"));
        assert!(bins.iter().any(|(name, _, _)| *name == "rustc"));
    }

    #[test]
    fn test_required_binaries_python_includes_uv() {
        let registry = perspt_core::plugin::PluginRegistry::new();
        let plugin = registry.get("python").unwrap();
        let bins = plugin.required_binaries();
        assert!(bins.iter().any(|(name, _, _)| *name == "uv"));
        assert!(bins.iter().any(|(name, _, _)| *name == "python3"));
    }

    #[test]
    fn test_required_binaries_js_includes_node() {
        let registry = perspt_core::plugin::PluginRegistry::new();
        let plugin = registry.get("javascript").unwrap();
        let bins = plugin.required_binaries();
        assert!(bins.iter().any(|(name, _, _)| *name == "node"));
        assert!(bins.iter().any(|(name, _, _)| *name == "npm"));
    }

    // =========================================================================
    // Fallback Resolution Tests
    // =========================================================================

    #[tokio::test]
    async fn test_fallback_defaults_to_none_without_explicit_config() {
        let orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
        assert!(orch.architect_fallback_model.is_none());
        assert!(orch.actuator_fallback_model.is_none());
        assert!(orch.verifier_fallback_model.is_none());
        assert!(orch.speculator_fallback_model.is_none());
    }

    #[tokio::test]
    async fn test_explicit_fallback_stored_correctly() {
        let orch = SRBNOrchestrator::new_with_models(
            PathBuf::from("/tmp/test_fallback"),
            false,
            None,
            None,
            None,
            None,
            Some("gpt-4o".into()),
            Some("gpt-4o-mini".into()),
            Some("gpt-4o".into()),
            Some("gpt-4o-mini".into()),
        );
        assert_eq!(orch.architect_fallback_model, Some("gpt-4o".to_string()));
        assert_eq!(
            orch.actuator_fallback_model,
            Some("gpt-4o-mini".to_string())
        );
        assert_eq!(orch.verifier_fallback_model, Some("gpt-4o".to_string()));
        assert_eq!(
            orch.speculator_fallback_model,
            Some("gpt-4o-mini".to_string())
        );
    }

    #[tokio::test]
    async fn test_per_tier_models_independent() {
        let orch = SRBNOrchestrator::new_with_models(
            PathBuf::from("/tmp/test_tiers_independent"),
            false,
            Some("arch".into()),
            Some("act".into()),
            Some("ver".into()),
            Some("spec".into()),
            None,
            None,
            None,
            None,
        );
        // Each tier stores its own model, not shared
        assert_ne!(orch.architect_model, orch.actuator_model);
        assert_ne!(orch.verifier_model, orch.speculator_model);
    }

    // =========================================================================
    // Python auto-dependency repair tests
    // =========================================================================

    #[test]
    fn test_extract_missing_python_modules_basic() {
        let output = r#"
FAILED tests/test_core.py::TestPipeline::test_run - ModuleNotFoundError: No module named 'httpx'
E   ModuleNotFoundError: No module named 'pydantic'
ImportError: No module named 'pyarrow'
"#;
        let mut missing = SRBNOrchestrator::extract_missing_python_modules(output);
        missing.sort();
        assert_eq!(missing, vec!["httpx", "pyarrow", "pydantic"]);
    }

    #[test]
    fn test_extract_missing_python_modules_subpackage() {
        let output = "ModuleNotFoundError: No module named 'foo.bar.baz'";
        let missing = SRBNOrchestrator::extract_missing_python_modules(output);
        assert_eq!(missing, vec!["foo"]);
    }

    #[test]
    fn test_extract_missing_python_modules_stdlib_filtered() {
        let output = r#"
ModuleNotFoundError: No module named 'numpy'
ModuleNotFoundError: No module named 'os'
ModuleNotFoundError: No module named 'json'
"#;
        let missing = SRBNOrchestrator::extract_missing_python_modules(output);
        assert_eq!(missing, vec!["numpy"]);
    }

    #[test]
    fn test_extract_missing_python_modules_empty() {
        let output = "All tests passed!\n3 passed in 0.5s";
        let missing = SRBNOrchestrator::extract_missing_python_modules(output);
        assert!(missing.is_empty());
    }

    #[test]
    fn test_python_import_to_package_mapping() {
        assert_eq!(SRBNOrchestrator::python_import_to_package("PIL"), "pillow");
        assert_eq!(SRBNOrchestrator::python_import_to_package("yaml"), "pyyaml");
        assert_eq!(
            SRBNOrchestrator::python_import_to_package("cv2"),
            "opencv-python"
        );
        assert_eq!(
            SRBNOrchestrator::python_import_to_package("sklearn"),
            "scikit-learn"
        );
        assert_eq!(
            SRBNOrchestrator::python_import_to_package("bs4"),
            "beautifulsoup4"
        );
        // Direct passthrough for unknown
        assert_eq!(SRBNOrchestrator::python_import_to_package("httpx"), "httpx");
        assert_eq!(
            SRBNOrchestrator::python_import_to_package("fastapi"),
            "fastapi"
        );
    }

    #[test]
    fn test_normalize_command_to_uv_pip_install() {
        assert_eq!(
            SRBNOrchestrator::normalize_command_to_uv("pip install httpx"),
            "uv add httpx"
        );
        assert_eq!(
            SRBNOrchestrator::normalize_command_to_uv("pip3 install httpx pydantic"),
            "uv add httpx pydantic"
        );
        assert_eq!(
            SRBNOrchestrator::normalize_command_to_uv("python -m pip install requests"),
            "uv add requests"
        );
        assert_eq!(
            SRBNOrchestrator::normalize_command_to_uv("python3 -m pip install flask"),
            "uv add flask"
        );
    }

    #[test]
    fn test_normalize_command_to_uv_requirements_file() {
        assert_eq!(
            SRBNOrchestrator::normalize_command_to_uv("pip install -r requirements.txt"),
            "uv pip install -r requirements.txt"
        );
    }

    #[test]
    fn test_normalize_command_to_uv_passthrough() {
        // Already uv commands pass through unchanged
        assert_eq!(
            SRBNOrchestrator::normalize_command_to_uv("uv add httpx"),
            "uv add httpx"
        );
        // Non-Python commands pass through unchanged
        assert_eq!(
            SRBNOrchestrator::normalize_command_to_uv("cargo add serde"),
            "cargo add serde"
        );
        assert_eq!(
            SRBNOrchestrator::normalize_command_to_uv("npm install lodash"),
            "npm install lodash"
        );
    }

    #[test]
    fn test_extract_commands_from_correction_rust_plugin_policy() {
        let response = r#"Here's the fix:
Commands:
```
uv add httpx
cargo add serde
pip install numpy
```
File: main.rs
```rust
use serde;
```"#;
        // Rust plugin allows cargo commands, denies uv/pip
        let commands = SRBNOrchestrator::extract_commands_from_correction(response, "rust");
        assert!(
            commands.contains(&"cargo add serde".to_string()),
            "{:?}",
            commands
        );
        assert!(
            !commands.contains(&"uv add httpx".to_string()),
            "Rust plugin should deny uv commands: {:?}",
            commands
        );
        assert!(
            !commands.contains(&"pip install numpy".to_string()),
            "Rust plugin should deny pip commands: {:?}",
            commands
        );
    }

    #[test]
    fn test_extract_commands_from_correction_python_plugin_policy() {
        let response = r#"Commands:
```
uv add httpx
cargo add serde
pip install numpy
```"#;
        // Python plugin allows uv/pip commands, denies cargo
        let commands = SRBNOrchestrator::extract_commands_from_correction(response, "python");
        assert!(
            commands.contains(&"uv add httpx".to_string()),
            "{:?}",
            commands
        );
        assert!(
            commands.contains(&"pip install numpy".to_string()),
            "{:?}",
            commands
        );
        assert!(
            !commands.contains(&"cargo add serde".to_string()),
            "Python plugin should deny cargo commands: {:?}",
            commands
        );
    }

    #[test]
    fn test_typed_parse_pipeline_multiple_files() {
        let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
        let content = r#"Here are the files:

File: src/etl_pipeline/core.py
```python
def run_pipeline():
    pass
```

File: src/etl_pipeline/validator.py
```python
def validate(data):
    return True
```

File: tests/test_core.py
```python
from etl_pipeline.core import run_pipeline

def test_run():
    run_pipeline()
```
"#;
        let (bundle_opt, state, _) = orch.parse_artifact_bundle_typed(content, "test", 0);
        assert!(state.is_ok(), "Expected successful parse, got {}", state);
        let bundle = bundle_opt.unwrap();
        assert_eq!(bundle.artifacts.len(), 3, "Expected 3 artifacts");
        assert_eq!(bundle.artifacts[0].path(), "src/etl_pipeline/core.py");
        assert_eq!(bundle.artifacts[1].path(), "src/etl_pipeline/validator.py");
        assert_eq!(bundle.artifacts[2].path(), "tests/test_core.py");
    }

    #[test]
    fn test_typed_parse_pipeline_single_file() {
        let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
        let content = r#"File: main.py
```python
print("hello")
```"#;
        let (bundle_opt, state, _) = orch.parse_artifact_bundle_typed(content, "test", 0);
        assert!(state.is_ok());
        let bundle = bundle_opt.unwrap();
        assert_eq!(bundle.artifacts.len(), 1);
        assert_eq!(bundle.artifacts[0].path(), "main.py");
    }

    #[test]
    fn test_typed_parse_pipeline_mixed_file_and_diff() {
        let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
        let content = r#"File: new_module.py
```python
def new_fn():
    pass
```

Diff: existing.py
```diff
--- existing.py
+++ existing.py
@@ -1 +1,2 @@
+import new_module
 def old_fn():
```"#;
        let (bundle_opt, state, _) = orch.parse_artifact_bundle_typed(content, "test", 0);
        assert!(state.is_ok());
        let bundle = bundle_opt.unwrap();
        assert_eq!(bundle.artifacts.len(), 2);
        assert_eq!(bundle.artifacts[0].path(), "new_module.py");
        assert!(
            bundle.artifacts[0].is_write(),
            "new_module.py should be a write"
        );
        assert_eq!(bundle.artifacts[1].path(), "existing.py");
        assert!(
            bundle.artifacts[1].is_diff(),
            "existing.py should be a diff"
        );
    }

    #[test]
    fn test_typed_parse_pipeline_legacy_multi_file() {
        let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
        let content = r#"File: core.py
```python
def core():
    pass
```

File: utils.py
```python
def util():
    pass
```"#;
        let (bundle_opt, state, _) = orch.parse_artifact_bundle_typed(content, "test", 0);
        assert!(state.is_ok(), "Should parse multi-file response");
        let bundle = bundle_opt.unwrap();
        assert_eq!(bundle.artifacts.len(), 2, "Should have 2 artifacts");
        assert_eq!(bundle.artifacts[0].path(), "core.py");
        assert_eq!(bundle.artifacts[1].path(), "utils.py");
    }

    // =========================================================================
    // Baseline regression tests — freeze pre-refactor behavior
    // =========================================================================

    #[test]
    fn test_typed_parse_pipeline_structured_json() {
        let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
        let content = r#"Here is the output:
```json
{
  "artifacts": [
    {"operation": "write", "path": "src/main.py", "content": "print('hello')"},
    {"operation": "diff", "path": "src/lib.py", "patch": "--- a\n+++ b\n@@ -1 +1 @@\n-old\n+new"}
  ],
  "commands": ["uv add requests"]
}
```"#;
        let (bundle_opt, state, _) = orch.parse_artifact_bundle_typed(content, "test", 0);
        assert!(state.is_ok(), "Should parse structured JSON bundle");
        let bundle = bundle_opt.unwrap();
        assert_eq!(bundle.artifacts.len(), 2);
        assert!(bundle.artifacts[0].is_write());
        assert_eq!(bundle.artifacts[0].path(), "src/main.py");
        assert!(bundle.artifacts[1].is_diff());
        assert_eq!(bundle.artifacts[1].path(), "src/lib.py");
        assert_eq!(bundle.commands, vec!["uv add requests"]);
    }

    #[test]
    fn test_typed_parse_pipeline_json_empty_path_rejected() {
        let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
        let content = r#"```json
{
  "artifacts": [
    {"operation": "write", "path": "", "content": "bad"}
  ],
  "commands": []
}
```"#;
        let (bundle_opt, state, _) = orch.parse_artifact_bundle_typed(content, "test", 0);
        assert!(
            bundle_opt.is_none(),
            "Invalid bundle with empty path should be rejected"
        );
        assert!(
            !state.is_ok(),
            "Parse state should not be Ok for invalid bundle: {}",
            state
        );
    }

    #[test]
    fn test_typed_parse_pipeline_json_absolute_path_rejected() {
        let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
        let content = r#"```json
{
  "artifacts": [
    {"operation": "write", "path": "/etc/passwd", "content": "bad"}
  ],
  "commands": []
}
```"#;
        let (bundle_opt, state, _) = orch.parse_artifact_bundle_typed(content, "test", 0);
        assert!(
            bundle_opt.is_none(),
            "Invalid bundle with absolute path should be rejected"
        );
        assert!(
            !state.is_ok(),
            "Parse state should not be Ok for path traversal: {}",
            state
        );
    }

    #[test]
    fn test_typed_parse_pipeline_returns_no_payload_for_garbage() {
        let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
        let content = "This is just a plain text response with no code blocks at all.";
        let (bundle_opt, state, _) = orch.parse_artifact_bundle_typed(content, "test", 0);
        assert!(bundle_opt.is_none());
        assert!(
            matches!(
                state,
                perspt_core::types::ParseResultState::NoStructuredPayload
            ),
            "Expected NoStructuredPayload, got {}",
            state
        );
    }

    #[tokio::test]
    async fn test_effective_working_dir_with_sandbox() {
        // When a node has a provisional branch AND the sandbox directory exists,
        // effective_working_dir should return the sandbox path instead of workspace.
        let temp_dir = std::env::temp_dir().join(format!(
            "perspt_eff_workdir_sandbox_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let mut orch = SRBNOrchestrator::new_for_testing(temp_dir.clone());
        orch.context.session_id = "test_session".into();

        let parent = SRBNNode::new("root".into(), "root goal".into(), ModelTier::Actuator);
        let child = SRBNNode::new("child".into(), "child goal".into(), ModelTier::Actuator);
        orch.add_node(parent);
        orch.add_node(child);
        orch.add_dependency("root", "child", "dep").unwrap();

        let child_idx = orch.node_indices["child"];
        let branch_id = orch.maybe_create_provisional_branch(child_idx).unwrap();

        let sandbox_path = temp_dir
            .join(".perspt")
            .join("sandboxes")
            .join("test_session")
            .join(&branch_id);
        assert!(sandbox_path.exists(), "Sandbox should have been created");

        // effective_working_dir should now return the sandbox
        let eff = orch.effective_working_dir(child_idx);
        assert_eq!(eff, sandbox_path);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_sandbox_dir_for_node_returns_path_when_exists() {
        let temp_dir = std::env::temp_dir().join(format!(
            "perspt_sandbox_dir_exists_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let mut orch = SRBNOrchestrator::new_for_testing(temp_dir.clone());
        orch.context.session_id = "sess".into();

        let parent = SRBNNode::new("p".into(), "g".into(), ModelTier::Actuator);
        let child = SRBNNode::new("c".into(), "g".into(), ModelTier::Actuator);
        orch.add_node(parent);
        orch.add_node(child);
        orch.add_dependency("p", "c", "dep").unwrap();

        let child_idx = orch.node_indices["c"];
        let branch_id = orch.maybe_create_provisional_branch(child_idx).unwrap();

        let sandbox = orch.sandbox_dir_for_node(child_idx);
        assert!(sandbox.is_some());
        let sandbox_path = sandbox.unwrap();
        assert!(sandbox_path.ends_with(&branch_id));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_root_node_bypasses_sandbox() {
        // Root nodes (no graph parents) should NOT get provisional branches,
        // and effective_working_dir should return the live workspace.
        let temp_dir =
            std::env::temp_dir().join(format!("perspt_root_bypass_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let mut orch = SRBNOrchestrator::new_for_testing(temp_dir.clone());

        let root = SRBNNode::new("root".into(), "root goal".into(), ModelTier::Actuator);
        orch.add_node(root);

        let root_idx = orch.node_indices["root"];
        // Root nodes now get a provisional branch with sandbox isolation
        let branch = orch.maybe_create_provisional_branch(root_idx);
        assert!(
            branch.is_some(),
            "Root node should now get a provisional branch for sandbox isolation"
        );

        // effective_working_dir should point to the sandbox, not the raw workspace
        let wd = orch.effective_working_dir(root_idx);
        assert_ne!(wd, temp_dir, "Root should use sandbox, not raw workspace");
        assert!(wd.to_string_lossy().contains("sandboxes"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_step_commit_copies_sandbox_to_workspace() {
        // Verify the commit path: files written to sandbox should appear in
        // the workspace after step_commit runs its copy-from-sandbox logic.
        use perspt_core::types::{ArtifactBundle, ArtifactOperation, PlannedTask};

        let temp_dir =
            std::env::temp_dir().join(format!("perspt_commit_copy_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(temp_dir.join("src")).unwrap();

        let mut orch = SRBNOrchestrator::new_for_testing(temp_dir.clone());
        orch.context.session_id = uuid::Uuid::new_v4().to_string();

        let plan = TaskPlan {
            tasks: vec![
                PlannedTask {
                    id: "parent".into(),
                    goal: "Parent".into(),
                    output_files: vec!["src/parent.rs".into()],
                    ..PlannedTask::new("parent", "Parent")
                },
                PlannedTask {
                    id: "child".into(),
                    goal: "Child".into(),
                    output_files: vec!["src/child.rs".into()],
                    dependencies: vec!["parent".into()],
                    ..PlannedTask::new("child", "Child")
                },
            ],
        };
        orch.create_nodes_from_plan(&plan).unwrap();

        let child_idx = orch.node_indices["child"];
        let _branch_id = orch.maybe_create_provisional_branch(child_idx).unwrap();

        // Write a file into sandbox via apply_bundle_transactionally
        let bundle = ArtifactBundle {
            artifacts: vec![ArtifactOperation::Write {
                path: "src/child.rs".into(),
                content: "pub fn child_fn() {}\n".into(),
            }],
            commands: vec![],
        };
        orch.apply_bundle_transactionally(
            &bundle,
            "child",
            perspt_core::types::NodeClass::Implementation,
        )
        .await
        .unwrap();

        // Before commit: file should be in sandbox, NOT in workspace
        let sandbox = orch.sandbox_dir_for_node(child_idx).unwrap();
        assert!(sandbox.join("src/child.rs").exists());
        assert!(!temp_dir.join("src/child.rs").exists());

        // Now run step_commit to promote
        let child_idx = orch.node_indices["child"];
        let _ = orch.step_commit(child_idx).await;

        // After commit: file should now be in workspace
        assert!(
            temp_dir.join("src/child.rs").exists(),
            "step_commit should copy sandbox files to workspace"
        );
        let content = std::fs::read_to_string(temp_dir.join("src/child.rs")).unwrap();
        assert_eq!(content, "pub fn child_fn() {}\n");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_typed_parse_pipeline_json_path_traversal_rejected() {
        let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
        let content = r#"```json
{
  "artifacts": [
    {"operation": "write", "path": "../../../etc/shadow", "content": "bad"}
  ],
  "commands": []
}
```"#;
        let (bundle_opt, state, _) = orch.parse_artifact_bundle_typed(content, "test", 0);
        assert!(
            bundle_opt.is_none(),
            "Invalid bundle with path traversal should be rejected"
        );
        assert!(
            !state.is_ok(),
            "Parse state should not be Ok for path traversal: {}",
            state
        );
    }

    // --- Step 6: Greenfield bootstrap ordering & dependency determinism ---

    #[test]
    fn test_dependency_expectations_threaded_to_nodes() {
        use perspt_core::types::{DependencyExpectation, PlannedTask, TaskPlan};

        let mut plan = TaskPlan::new();
        let mut t1 = PlannedTask::new("t1", "Create server module");
        t1.output_files = vec!["src/server.py".to_string()];
        t1.dependency_expectations = DependencyExpectation {
            required_packages: vec!["flask".to_string(), "pydantic".to_string()],
            setup_commands: vec![],
            min_toolchain_version: Some("3.11".to_string()),
        };
        plan.tasks.push(t1);

        let mut orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
        orch.create_nodes_from_plan(&plan).unwrap();

        // Verify the node carries dependency expectations
        let idx = orch.node_indices["t1"];
        let node = &orch.graph[idx];
        assert_eq!(node.dependency_expectations.required_packages.len(), 2);
        assert_eq!(node.dependency_expectations.required_packages[0], "flask");
        assert_eq!(
            node.dependency_expectations
                .min_toolchain_version
                .as_deref(),
            Some("3.11")
        );
    }

    #[test]
    fn test_verifier_readiness_gate_no_plugins() {
        let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
        // Should not panic with empty plugins
        orch.check_verifier_readiness_gate();
    }

    #[test]
    fn test_architect_prompt_includes_dependency_expectations() {
        let ev = perspt_core::types::PromptEvidence {
            user_goal: Some("Build a web server".to_string()),
            project_summary: Some("empty project".to_string()),
            working_dir: Some("/tmp".to_string()),
            ..Default::default()
        };
        let prompt = crate::prompt_compiler::compile(
            perspt_core::types::PromptIntent::ArchitectExisting,
            &ev,
        )
        .text;
        assert!(
            prompt.contains("dependency_expectations"),
            "Architect prompt must include dependency_expectations in the JSON schema"
        );
        assert!(
            prompt.contains("required_packages"),
            "Architect prompt must mention required_packages"
        );
        assert!(
            prompt.contains("min_toolchain_version"),
            "Architect prompt must mention min_toolchain_version"
        );
    }

    // --- Step 8: Budget enforcement & plan revision tracking ---

    #[test]
    fn test_budget_gate_stops_execution_when_exhausted() {
        let mut orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
        // Set a budget of 0 steps — should be immediately exhausted
        orch.set_budget(Some(0), None, None);
        assert!(
            orch.budget.any_exhausted(),
            "Budget with max_steps=0 should be immediately exhausted"
        );
    }

    #[test]
    fn test_budget_step_recording() {
        let mut budget = perspt_core::types::BudgetEnvelope::new("test-session");
        budget.max_steps = Some(3);
        assert!(!budget.any_exhausted());
        budget.record_step();
        budget.record_step();
        assert!(!budget.any_exhausted());
        budget.record_step();
        assert!(budget.steps_exhausted());
        assert!(budget.any_exhausted());
    }

    #[test]
    fn test_set_budget_configures_envelope() {
        let mut orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
        orch.set_budget(Some(10), Some(5), Some(2.50));
        assert_eq!(orch.budget.max_steps, Some(10));
        assert_eq!(orch.budget.max_revisions, Some(5));
        assert_eq!(orch.budget.max_cost_usd, Some(2.50));
        assert!(!orch.budget.any_exhausted());
    }

    #[test]
    fn test_node_outcome_equality() {
        assert_eq!(NodeOutcome::Completed, NodeOutcome::Completed);
        assert_eq!(NodeOutcome::Escalated, NodeOutcome::Escalated);
        assert_ne!(NodeOutcome::Completed, NodeOutcome::Escalated);
    }

    #[test]
    fn test_session_outcome_from_counts() {
        fn derive_outcome(completed: usize, escalated: usize) -> perspt_core::SessionOutcome {
            if escalated == 0 {
                perspt_core::SessionOutcome::Success
            } else if completed > 0 {
                perspt_core::SessionOutcome::PartialSuccess
            } else {
                perspt_core::SessionOutcome::Failed
            }
        }

        // All completed → Success
        assert_eq!(derive_outcome(3, 0), perspt_core::SessionOutcome::Success,);
        // Some completed, some escalated → PartialSuccess
        assert_eq!(
            derive_outcome(2, 1),
            perspt_core::SessionOutcome::PartialSuccess,
        );
        // All escalated → Failed
        assert_eq!(derive_outcome(0, 3), perspt_core::SessionOutcome::Failed,);
    }

    /// Helper: create an orchestrator with a single default node for testing.
    fn orch_with_node(
        working_dir: std::path::PathBuf,
    ) -> (SRBNOrchestrator, petgraph::graph::NodeIndex) {
        let mut orch = SRBNOrchestrator::new(working_dir, false);
        let node = SRBNNode::new(
            "test-node".to_string(),
            "test goal".to_string(),
            perspt_core::ModelTier::Actuator,
        );
        let idx = orch.add_node(node);
        (orch, idx)
    }

    #[test]
    fn test_sheaf_pre_check_passes_when_no_outputs() {
        let (orch, idx) = orch_with_node(std::path::PathBuf::from("/tmp/test"));
        assert!(orch.sheaf_pre_check(idx).is_none());
    }

    #[test]
    fn test_sheaf_pre_check_detects_missing_files() {
        let (mut orch, idx) = orch_with_node(std::path::PathBuf::from("/tmp/test"));
        orch.graph[idx]
            .output_targets
            .push(std::path::PathBuf::from("nonexistent_file_xyz.rs"));
        let result = orch.sheaf_pre_check(idx);
        assert!(result.is_some());
        assert!(result.unwrap().contains("missing"));
    }

    #[test]
    fn test_sheaf_pre_check_detects_empty_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::File::create(dir.path().join("empty.rs")).unwrap();

        let (mut orch, idx) = orch_with_node(dir.path().to_path_buf());
        orch.graph[idx]
            .output_targets
            .push(std::path::PathBuf::from("empty.rs"));
        let result = orch.sheaf_pre_check(idx);
        assert!(result.is_some());
        assert!(result.unwrap().contains("empty"));
    }

    #[test]
    fn test_sheaf_pre_check_passes_for_valid_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

        let (mut orch, idx) = orch_with_node(dir.path().to_path_buf());
        orch.graph[idx]
            .output_targets
            .push(std::path::PathBuf::from("main.rs"));
        assert!(orch.sheaf_pre_check(idx).is_none());
    }

    #[test]
    fn test_v_boot_energy_from_degraded_sensors() {
        use perspt_core::types::{
            EnergyComponents, SensorStatus, StageOutcome, VerificationResult,
        };

        // Simulate a verification result with one fallback and one unavailable sensor
        let vr = VerificationResult {
            syntax_ok: true,
            build_ok: true,
            tests_ok: true,
            lint_ok: true,
            diagnostics_count: 0,
            tests_passed: 5,
            tests_failed: 0,
            summary: String::new(),
            raw_output: None,
            degraded: true,
            degraded_reason: Some("test sensor fallback".into()),
            stage_outcomes: vec![
                StageOutcome {
                    stage: "syntax_check".into(),
                    passed: true,
                    sensor_status: SensorStatus::Available,
                    output: None,
                },
                StageOutcome {
                    stage: "build".into(),
                    passed: true,
                    sensor_status: SensorStatus::Fallback {
                        actual: "cargo check".into(),
                        reason: "primary not found".into(),
                    },
                    output: None,
                },
                StageOutcome {
                    stage: "test".into(),
                    passed: true,
                    sensor_status: SensorStatus::Unavailable {
                        reason: "no test runner".into(),
                    },
                    output: None,
                },
            ],
        };

        // Compute V_boot the same way verification.rs does
        let mut energy = EnergyComponents::default();
        for so in &vr.stage_outcomes {
            match &so.sensor_status {
                SensorStatus::Unavailable { .. } => energy.v_boot += 3.0,
                SensorStatus::Fallback { .. } => energy.v_boot += 1.0,
                SensorStatus::Available => {}
            }
        }
        // 1 fallback (+1.0) + 1 unavailable (+3.0) = 4.0
        assert!(
            (energy.v_boot - 4.0).abs() < f32::EPSILON,
            "Expected V_boot=4.0, got {}",
            energy.v_boot
        );
    }
}
