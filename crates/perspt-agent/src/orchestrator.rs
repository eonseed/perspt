//! SRBN Orchestrator
//!
//! Manages the Task DAG and orchestrates agent execution following the 7-step control loop.

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
    /// PSP-5 Phase 6: Blocked dependencies awaiting parent interface seals
    blocked_dependencies: Vec<perspt_core::types::BlockedDependency>,
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
            blocked_dependencies: Vec::new(),
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
            blocked_dependencies: Vec::new(),
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
        let topo = Topo::new(&self.graph);
        let indices: Vec<_> = topo.iter(&self.graph).collect();
        let total_nodes = indices.len();
        let mut executed = 0;

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
                Ok(()) => {
                    if let Some(node) = self.graph.node_weight(*idx) {
                        self.emit_event(perspt_core::AgentEvent::NodeCompleted {
                            node_id: node.node_id.clone(),
                            goal: node.goal.clone(),
                        });
                    }
                    executed += 1;
                }
                Err(e) => {
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
            success: true,
            message: format!("Resumed: executed {} of {} nodes", executed, total_nodes),
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

        // Log that LLM request logging is enabled (persistence happens immediately per-request)
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

        self.step_sheafify(task).await?;

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

        for (i, idx) in indices.iter().enumerate() {
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
                Ok(()) => {
                    // Emit completed status
                    if let Some(node) = self.graph.node_weight(*idx) {
                        self.emit_event(perspt_core::AgentEvent::NodeCompleted {
                            node_id: node.node_id.clone(),
                            goal: node.goal.clone(),
                        });
                    }
                }
                Err(e) => {
                    let node_id = self.graph[*idx].node_id.clone();
                    log::error!("Node {} failed: {}", node_id, e);
                    self.emit_log(format!("❌ Node {} failed: {}", node_id, e));
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

        self.emit_event(perspt_core::AgentEvent::Complete {
            success: true,
            message: format!("Completed {} nodes", total_nodes),
        });
        Ok(())
    }

    /// Step 0: Project Initialization
    ///
    /// Check that required OS and language tools are available before
    /// running init commands. Emits install instructions for any missing tools.
    ///
    /// Returns `true` if all critical tools (needed for init) are present.
    /// Optional tools (LSP, linters) emit warnings but don't block.
    fn check_tool_prerequisites(&self, plugin: &dyn perspt_core::plugin::LanguagePlugin) -> bool {
        // Common OS tools that perspt uses for context retrieval and code manipulation
        let common_tools: &[(&str, &str)] = &[
            (
                "grep",
                "Install coreutils: brew install grep (macOS) or apt install grep (Linux)",
            ),
            (
                "sed",
                "Install coreutils: brew install gnu-sed (macOS) or apt install sed (Linux)",
            ),
            (
                "awk",
                "Install coreutils: brew install gawk (macOS) or apt install gawk (Linux)",
            ),
        ];

        let mut missing_critical = Vec::new();
        let mut missing_optional = Vec::new();
        let mut install_instructions = Vec::new();

        // Check common OS tools
        for (binary, hint) in common_tools {
            if !perspt_core::plugin::host_binary_available(binary) {
                missing_optional.push((*binary, "OS utility"));
                install_instructions.push(format!("  {} ({}): {}", binary, "OS utility", hint));
            }
        }

        // Check language-specific tools
        let required = plugin.required_binaries();
        for (binary, role, hint) in &required {
            if !perspt_core::plugin::host_binary_available(binary) {
                // init and build tools are critical; LSP and linters are optional
                if *role == "language server" || role.contains("lint") {
                    missing_optional.push((*binary, role));
                } else {
                    missing_critical.push((*binary, role));
                }
                install_instructions.push(format!("  {} ({}): {}", binary, role, hint));
            }
        }

        // Emit results
        if !missing_critical.is_empty() {
            let names: Vec<String> = missing_critical
                .iter()
                .map(|(b, r)| format!("{} ({})", b, r))
                .collect();
            self.emit_log(format!("🚫 Missing critical tools: {}", names.join(", ")));
        }
        if !missing_optional.is_empty() {
            let names: Vec<String> = missing_optional
                .iter()
                .map(|(b, r)| format!("{} ({})", b, r))
                .collect();
            self.emit_log(format!(
                "⚠️ Missing optional tools (degraded mode): {}",
                names.join(", ")
            ));
        }
        if !install_instructions.is_empty() {
            self.emit_log(format!(
                "📋 Install instructions:\n{}",
                install_instructions.join("\n")
            ));
        }

        if missing_critical.is_empty() {
            if missing_optional.is_empty() {
                self.emit_log(format!("✅ All {} tools available", plugin.name()));
            }
            true
        } else {
            self.emit_log("❌ Cannot proceed with project initialization — install missing critical tools first.".to_string());
            false
        }
    }

    /// Uses the pre-computed `WorkspaceState` to branch between:
    /// - ExistingProject: check tooling sync, gather context, never re-init.
    /// - Greenfield: create isolated project dir, run language-native init.
    /// - Ambiguous: create a child project dir to avoid polluting misc files.
    async fn step_init_project(&mut self, task: &str) -> Result<()> {
        let registry = perspt_core::plugin::PluginRegistry::new();
        log::info!(
            "step_init_project: workspace_state={}",
            self.context.workspace_state
        );

        match self.context.workspace_state.clone() {
            WorkspaceState::ExistingProject { ref plugins } => {
                // Existing project — check tooling sync, never re-init
                let plugin_name = plugins.first().map(|s| s.as_str()).unwrap_or("");
                if let Some(plugin) = registry.get(plugin_name) {
                    self.emit_log(format!("📂 Detected existing {} project", plugin.name()));

                    // Pre-flight: check required tools (non-blocking for existing projects)
                    self.check_tool_prerequisites(plugin);

                    match plugin.check_tooling_action(&self.context.working_dir) {
                        perspt_core::plugin::ProjectAction::ExecCommand {
                            command,
                            description,
                        } => {
                            self.emit_log(format!("🔧 Tooling action needed: {}", description));

                            let approval_result = self
                                .await_approval(
                                    perspt_core::ActionType::Command {
                                        command: command.clone(),
                                    },
                                    description.clone(),
                                    None,
                                )
                                .await;

                            if matches!(
                                approval_result,
                                ApprovalResult::Approved | ApprovalResult::ApprovedWithEdit(_)
                            ) {
                                let mut args = HashMap::new();
                                args.insert("command".to_string(), command.clone());
                                let call = ToolCall {
                                    name: "run_command".to_string(),
                                    arguments: args,
                                };
                                let result = self.tools.execute(&call).await;
                                if result.success {
                                    self.emit_log(format!("✅ {}", description));
                                } else {
                                    self.emit_log(format!("❌ Failed: {:?}", result.error));
                                }
                            }
                        }
                        perspt_core::plugin::ProjectAction::NoAction => {
                            self.emit_log("✓ Project tooling is up to date".to_string());
                        }
                    }
                }
            }

            WorkspaceState::Greenfield { ref inferred_lang } => {
                let lang = match inferred_lang.as_deref() {
                    Some(l) => l,
                    None => {
                        // Try to infer from task at init time
                        match self.detect_language_from_task(task) {
                            Some(l) => l,
                            None => {
                                self.emit_log(
                                    "ℹ️ No language detected, skipping project init".to_string(),
                                );
                                return Ok(());
                            }
                        }
                    }
                };

                // In Team/Project mode we always scaffold — the Solo check
                // already ran in run() and would have short-circuited.
                // Only consult the LLM heuristic when execution mode was not
                // explicitly set (defensive guard for future callers).
                if self.context.execution_mode == perspt_core::types::ExecutionMode::Solo {
                    self.emit_log("ℹ️ Solo mode, skipping project scaffolding.".to_string());
                    return Ok(());
                }

                if let Some(plugin) = registry.get(lang) {
                    log::info!(
                        "step_init_project: Greenfield lang={}, initializing project",
                        lang
                    );
                    self.emit_log(format!("🌱 Initializing new {} project", lang));

                    // Pre-flight: check required tools before attempting init
                    if !self.check_tool_prerequisites(plugin) {
                        log::warn!(
                            "step_init_project: tool prerequisites check failed for {}",
                            lang
                        );
                        return Ok(());
                    }

                    // Determine if working directory is empty
                    let is_empty = std::fs::read_dir(&self.context.working_dir)
                        .map(|mut i| i.next().is_none())
                        .unwrap_or(true);

                    // Isolation: if dir is non-empty, create a child directory
                    // to avoid polluting existing files.
                    let project_name = if is_empty {
                        ".".to_string()
                    } else {
                        self.suggest_project_name(task).await
                    };

                    let opts = perspt_core::plugin::InitOptions {
                        name: project_name.clone(),
                        is_empty_dir: is_empty,
                        ..Default::default()
                    };

                    match plugin.get_init_action(&opts) {
                        perspt_core::plugin::ProjectAction::ExecCommand {
                            command,
                            description,
                        } => {
                            log::info!(
                                "step_init_project: init command='{}', awaiting approval",
                                command
                            );
                            let result = self
                                .await_approval(
                                    perspt_core::ActionType::ProjectInit {
                                        command: command.clone(),
                                        suggested_name: project_name.clone(),
                                    },
                                    description.clone(),
                                    None,
                                )
                                .await;

                            let final_name = match &result {
                                ApprovalResult::ApprovedWithEdit(edited) => edited.clone(),
                                _ => project_name.clone(),
                            };
                            log::info!(
                                "step_init_project: approval result={:?}, final_name={}",
                                result,
                                final_name
                            );

                            if matches!(
                                result,
                                ApprovalResult::Approved | ApprovalResult::ApprovedWithEdit(_)
                            ) {
                                let final_command = if final_name != project_name {
                                    let edited_opts = perspt_core::plugin::InitOptions {
                                        name: final_name.clone(),
                                        is_empty_dir: is_empty,
                                        ..Default::default()
                                    };
                                    match plugin.get_init_action(&edited_opts) {
                                        perspt_core::plugin::ProjectAction::ExecCommand {
                                            command,
                                            ..
                                        } => command,
                                        _ => command.clone(),
                                    }
                                } else {
                                    command.clone()
                                };

                                let mut args = HashMap::new();
                                args.insert("command".to_string(), final_command.clone());
                                let call = ToolCall {
                                    name: "run_command".to_string(),
                                    arguments: args,
                                };
                                let exec_result = self.tools.execute(&call).await;
                                if exec_result.success {
                                    self.emit_log(format!(
                                        "✅ Project '{}' initialized",
                                        final_name
                                    ));

                                    // Update working directory to point at the
                                    // isolated project root if we created a child dir.
                                    if final_name != "." {
                                        let new_dir = self.context.working_dir.join(&final_name);
                                        if new_dir.is_dir() {
                                            self.context.working_dir = new_dir.clone();
                                            self.tools =
                                                AgentTools::new(new_dir, !self.auto_approve);
                                            if let Some(ref sender) = self.event_sender {
                                                self.tools.set_event_sender(sender.clone());
                                            }
                                            self.emit_log(format!(
                                                "📁 Working directory: {}",
                                                self.context.working_dir.display()
                                            ));
                                        }
                                    }
                                } else {
                                    self.emit_log(format!(
                                        "❌ Init failed: {:?}",
                                        exec_result.error
                                    ));
                                }
                            }
                        }
                        perspt_core::plugin::ProjectAction::NoAction => {
                            self.emit_log("ℹ️ No initialization action needed".to_string());
                        }
                    }
                }
            }

            WorkspaceState::Ambiguous => {
                // Ambiguous workspace — try to infer language and create a child
                // project directory rather than initializing in place.
                if let Some(lang) = self.detect_language_from_task(task) {
                    // Same as Greenfield: in Team/Project mode always scaffold.
                    if self.context.execution_mode == perspt_core::types::ExecutionMode::Solo {
                        self.emit_log("ℹ️ Solo mode, skipping project scaffolding.".to_string());
                        return Ok(());
                    }

                    if let Some(plugin) = registry.get(lang) {
                        let project_name = self.suggest_project_name(task).await;
                        self.emit_log(format!(
                            "🌱 Ambiguous workspace — creating isolated {} project '{}'",
                            lang, project_name
                        ));

                        // Pre-flight: check required tools before attempting init
                        if !self.check_tool_prerequisites(plugin) {
                            return Ok(());
                        }

                        let opts = perspt_core::plugin::InitOptions {
                            name: project_name.clone(),
                            is_empty_dir: false,
                            ..Default::default()
                        };

                        match plugin.get_init_action(&opts) {
                            perspt_core::plugin::ProjectAction::ExecCommand {
                                command,
                                description,
                            } => {
                                let result = self
                                    .await_approval(
                                        perspt_core::ActionType::ProjectInit {
                                            command: command.clone(),
                                            suggested_name: project_name.clone(),
                                        },
                                        description.clone(),
                                        None,
                                    )
                                    .await;

                                let final_name = match &result {
                                    ApprovalResult::ApprovedWithEdit(edited) => edited.clone(),
                                    _ => project_name.clone(),
                                };

                                if matches!(
                                    result,
                                    ApprovalResult::Approved | ApprovalResult::ApprovedWithEdit(_)
                                ) {
                                    let final_command = if final_name != project_name {
                                        let edited_opts = perspt_core::plugin::InitOptions {
                                            name: final_name.clone(),
                                            is_empty_dir: false,
                                            ..Default::default()
                                        };
                                        match plugin.get_init_action(&edited_opts) {
                                            perspt_core::plugin::ProjectAction::ExecCommand {
                                                command,
                                                ..
                                            } => command,
                                            _ => command.clone(),
                                        }
                                    } else {
                                        command.clone()
                                    };

                                    let mut args = HashMap::new();
                                    args.insert("command".to_string(), final_command.clone());
                                    let call = ToolCall {
                                        name: "run_command".to_string(),
                                        arguments: args,
                                    };
                                    let exec_result = self.tools.execute(&call).await;
                                    if exec_result.success {
                                        self.emit_log(format!(
                                            "✅ Project '{}' initialized",
                                            final_name
                                        ));

                                        let new_dir = self.context.working_dir.join(&final_name);
                                        if new_dir.is_dir() {
                                            self.context.working_dir = new_dir.clone();
                                            self.tools =
                                                AgentTools::new(new_dir, !self.auto_approve);
                                            if let Some(ref sender) = self.event_sender {
                                                self.tools.set_event_sender(sender.clone());
                                            }
                                            self.emit_log(format!(
                                                "📁 Working directory: {}",
                                                self.context.working_dir.display()
                                            ));
                                        }
                                    } else {
                                        self.emit_log(format!(
                                            "❌ Init failed: {:?}",
                                            exec_result.error
                                        ));
                                    }
                                }
                            }
                            perspt_core::plugin::ProjectAction::NoAction => {
                                self.emit_log("ℹ️ No initialization action needed".to_string());
                            }
                        }
                    }
                } else {
                    self.emit_log(
                        "ℹ️ Ambiguous workspace and no language detected, skipping project init"
                            .to_string(),
                    );
                }
            }
        }

        Ok(())
    }

    /// Determine if Solo Mode should be used for this task
    /// Solo Mode is for simple single-file tasks that don't need project scaffolding
    /// PSP-5: Detect execution mode from task description
    ///
    /// Project mode is the DEFAULT. Solo mode only activates on explicit
    /// single-file intent keywords or via the `--single-file` CLI flag.
    fn detect_execution_mode(&self, task: &str) -> perspt_core::types::ExecutionMode {
        // If execution mode was set explicitly (e.g., CLI flag), honor it
        if self.context.execution_mode != perspt_core::types::ExecutionMode::Project {
            return self.context.execution_mode;
        }

        let task_lower = task.to_lowercase();

        // Only these explicit keywords trigger Solo mode
        let solo_keywords = [
            "single file",
            "single-file",
            "snippet",
            "standalone script",
            "standalone file",
            "one file only",
            "just a file",
        ];

        if solo_keywords.iter().any(|&k| task_lower.contains(k)) {
            log::debug!("Task contains explicit solo keyword, using Solo Mode");
            return perspt_core::types::ExecutionMode::Solo;
        }

        // Default: Project mode for everything else
        log::debug!("Defaulting to Project Mode (PSP-5)");
        perspt_core::types::ExecutionMode::Project
    }

    /// PSP-5: Classify the workspace as existing project, greenfield, or ambiguous.
    ///
    /// This is the single source of truth that drives init/bootstrap/context
    /// strategy for the session. Called once at the start of `run()`.
    fn classify_workspace(&self, task: &str) -> WorkspaceState {
        let registry = perspt_core::plugin::PluginRegistry::new();
        let detected: Vec<String> = registry
            .detect_all(&self.context.working_dir)
            .iter()
            .map(|p| p.name().to_string())
            .collect();

        if !detected.is_empty() {
            return WorkspaceState::ExistingProject { plugins: detected };
        }

        // No project metadata found — check if we can infer language from task
        let inferred = self.detect_language_from_task(task).map(|s| s.to_string());

        if inferred.is_some() {
            return WorkspaceState::Greenfield {
                inferred_lang: inferred,
            };
        }

        // Check if directory is empty (empty dirs are greenfield with unknown lang)
        let is_empty = std::fs::read_dir(&self.context.working_dir)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(true);

        if is_empty {
            return WorkspaceState::Greenfield {
                inferred_lang: None,
            };
        }

        // Has files but no project metadata and no language inferred
        WorkspaceState::Ambiguous
    }

    /// Probe verifier readiness for active plugins and emit `ToolReadiness` event.
    ///
    /// Reads `self.context.active_plugins` and emits available/degraded stage
    /// info so TUI/headless surfaces know which verification stages are ready.
    fn emit_plugin_readiness(&self) {
        let registry = perspt_core::plugin::PluginRegistry::new();
        let mut plugin_readiness = Vec::new();

        for plugin_name in &self.context.active_plugins {
            if let Some(plugin) = registry.get(plugin_name) {
                let profile = plugin.verifier_profile();
                let available: Vec<String> = profile
                    .capabilities
                    .iter()
                    .filter(|c| c.available)
                    .map(|c| c.stage.to_string())
                    .collect();
                let degraded: Vec<String> = profile
                    .capabilities
                    .iter()
                    .filter(|c| !c.available && c.fallback_available)
                    .map(|c| format!("{} (fallback)", c.stage))
                    .chain(
                        profile
                            .capabilities
                            .iter()
                            .filter(|c| !c.any_available())
                            .map(|c| format!("{} (unavailable)", c.stage)),
                    )
                    .collect();
                let lsp_status = if profile.lsp.primary_available {
                    format!("{} (primary)", profile.lsp.primary.server_binary)
                } else if profile.lsp.fallback_available {
                    profile
                        .lsp
                        .fallback
                        .as_ref()
                        .map(|f| format!("{} (fallback)", f.server_binary))
                        .unwrap_or_else(|| "fallback".to_string())
                } else {
                    "unavailable".to_string()
                };

                if !degraded.is_empty() {
                    self.emit_log(format!(
                        "⚠️ Plugin '{}' degraded: {}",
                        plugin_name,
                        degraded.join(", ")
                    ));
                }

                plugin_readiness.push(perspt_core::events::PluginReadiness {
                    plugin_name: plugin_name.clone(),
                    available_stages: available,
                    degraded_stages: degraded,
                    lsp_status,
                });
            }
        }

        self.emit_event_ref(perspt_core::AgentEvent::ToolReadiness {
            plugins: plugin_readiness,
            strictness: format!("{:?}", self.context.verifier_strictness),
        });
    }

    /// Re-detect plugins after greenfield project initialization.
    ///
    /// Updates `self.context.active_plugins` and `workspace_state`, then
    /// emits plugin readiness so the verifier stack matches the new project.
    fn redetect_plugins_after_init(&mut self) {
        let registry = perspt_core::plugin::PluginRegistry::new();
        let detected: Vec<String> = registry
            .detect_all(&self.context.working_dir)
            .iter()
            .map(|p| p.name().to_string())
            .collect();

        if !detected.is_empty() {
            self.emit_log(format!("🔌 Post-init plugins: {}", detected.join(", ")));
            self.context.active_plugins = detected.clone();
            self.context.workspace_state = WorkspaceState::ExistingProject { plugins: detected };
        } else {
            self.emit_log("⚠️ No plugins detected after project init".to_string());
        }

        self.emit_plugin_readiness();
    }

    /// Run Solo Mode: A tight loop for single-file tasks
    ///
    /// Bypasses DAG sheafification and directly generates, verifies, and corrects
    /// a single Python file with embedded doctests for V_log.
    async fn run_solo_mode(&mut self, task: String) -> Result<()> {
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
        format!(
            r#"You are an expert Python developer. Complete this task with a SINGLE, self-contained Python file.

## Task
{task}

## Requirements
1. Choose a DESCRIPTIVE filename based on the task (e.g., `fibonacci.py` for a fibonacci script, `prime_checker.py` for checking primes)
2. Write ONE Python file that accomplishes the task
3. Include docstrings with doctest examples for all functions
4. Make the file directly runnable with `if __name__ == "__main__":` block
5. Use type hints for all function parameters and return values

## Output Format
File: <your_descriptive_filename.py>
```python
# your complete code here
```

IMPORTANT: Do NOT use generic names like `script.py` or `main.py`. Choose a name that reflects the task."#,
            task = task
        )
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

        format!(
            r#"## Code Correction Required

The code you generated has errors. Fix ALL of them.

### Original Task
{task}

### Current Code ({filename})
```python
{current_code}
```

### Errors Found
Energy: V_syn={v_syn:.2}, V_log={v_log:.2}, V_boot={v_boot:.2}

{error_list}

### Instructions
1. Fix ALL errors listed above
2. Maintain the original functionality
3. Ensure the script runs without errors
4. Ensure all doctests pass
5. Return the COMPLETE corrected file

### Output Format
File: {filename}
```python
[complete corrected code]
```"#,
            task = task,
            filename = filename,
            current_code = current_code,
            v_syn = energy.v_syn,
            v_log = energy.v_log,
            v_boot = energy.v_boot,
            error_list = error_list
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

    /// Detect language from task description using heuristics
    fn detect_language_from_task(&self, task: &str) -> Option<&'static str> {
        let task_lower = task.to_lowercase();

        if task_lower.contains("rust") || task_lower.contains("cargo") {
            Some("rust")
        } else if task_lower.contains("python")
            || task_lower.contains("flask")
            || task_lower.contains("django")
            || task_lower.contains("fastapi")
            || task_lower.contains("pytorch")
            || task_lower.contains("tensorflow")
            || task_lower.contains("pandas")
            || task_lower.contains("numpy")
            || task_lower.contains("scikit")
            || task_lower.contains("sklearn")
            || task_lower.contains("ml ")
            || task_lower.contains("machine learning")
            || task_lower.contains("deep learning")
            || task_lower.contains("neural")
            || task_lower.contains("dcf")
            || task_lower.contains("data science")
            || task_lower.contains("jupyter")
            || task_lower.contains("notebook")
            || task_lower.contains("streamlit")
            || task_lower.contains("gradio")
            || task_lower.contains("huggingface")
            || task_lower.contains("transformers")
            || task_lower.contains("llm")
            || task_lower.contains("langchain")
            || task_lower.contains("pydantic")
        {
            Some("python")
        } else if task_lower.contains("javascript")
            || task_lower.contains("typescript")
            || task_lower.contains("node")
            || task_lower.contains("react")
            || task_lower.contains("vue")
            || task_lower.contains("angular")
            || task_lower.contains("next.js")
            || task_lower.contains("nextjs")
        {
            Some("javascript")
        } else if task_lower.contains("app") || task_lower.contains("application") {
            // Default to Python for generic "app" or "application" tasks
            Some("python")
        } else {
            None
        }
    }

    /// Suggest a meaningful project name from the task description
    async fn suggest_project_name(&self, task: &str) -> String {
        // 1. Try heuristic extraction first (fast, no LLM)
        if let Some(name) = self.extract_name_heuristic(task) {
            self.emit_log(format!("📁 Suggested project folder: {}", name));
            return name;
        }

        // 2. Fallback to LLM for complex tasks
        let prompt = format!(
            r#"Extract a short project name from this task description.
Rules:
- Use snake_case (lowercase with underscores)
- Maximum 30 characters
- Must be a valid folder name (letters, numbers, underscores only)
- Return ONLY the name, nothing else

Task: "{}"

Project name:"#,
            task
        );

        match self
            .call_llm_with_logging(&self.actuator_model.clone(), &prompt, None)
            .await
        {
            Ok(response) => {
                let suggested = response.trim().to_lowercase();
                if let Some(validated) = self.validate_project_name(&suggested) {
                    self.emit_log(format!("📁 Suggested project folder: {}", validated));
                    return validated;
                }
            }
            Err(e) => {
                log::warn!("Failed to get project name from LLM: {}", e);
            }
        }

        // 3. Final fallback
        let fallback = "perspt_app".to_string();
        self.emit_log(format!("📁 Using default folder: {}", fallback));
        fallback
    }

    /// Extract project name from task using stop-word removal (no LLM)
    fn extract_name_heuristic(&self, task: &str) -> Option<String> {
        let task_lower = task.to_lowercase();

        // Stop words to remove
        let stop_words = [
            // Verbs
            "create",
            "build",
            "make",
            "implement",
            "develop",
            "write",
            "design",
            "add",
            "setup",
            "set",
            "up",
            "generate",
            "please",
            "can",
            "you",
            // Articles
            "a",
            "an",
            "the",
            "this",
            "that",
            // Prepositions
            "in",
            "on",
            "for",
            "with",
            "using",
            "to",
            "from",
            // Languages (we don't want these in folder names)
            "python",
            "rust",
            "javascript",
            "typescript",
            "node",
            "nodejs",
            "react",
            "vue",
            "angular",
            "django",
            "flask",
            "fastapi",
            // Generic terms
            "simple",
            "basic",
            "new",
            "my",
            "our",
            "your",
        ];

        // Split into words and filter
        let words: Vec<&str> = task_lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .filter(|w| !stop_words.contains(w))
            .filter(|w| w.len() > 1) // Skip single chars
            .take(3) // Max 3 words
            .collect();

        if words.is_empty() {
            return None;
        }

        // Join words with underscore
        let name = words.join("_");

        // Validate
        self.validate_project_name(&name)
    }

    /// Validate and sanitize a project name
    fn validate_project_name(&self, name: &str) -> Option<String> {
        // Must start with letter, contain only letters/numbers/underscores
        let cleaned: String = name
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .take(30)
            .collect();

        if cleaned.is_empty() {
            return None;
        }

        // Ensure it starts with a letter
        let first = cleaned.chars().next()?;
        if !first.is_alphabetic() {
            return None;
        }

        Some(cleaned)
    }

    ///
    /// The Architect analyzes the task and produces a structured Task DAG.
    /// This step retries until a valid JSON plan is produced or max attempts reached.
    async fn step_sheafify(&mut self, task: String) -> Result<()> {
        log::info!("Step 1: Sheafification - Planning task decomposition");
        self.emit_log("🏗️ Architect is analyzing the task...".to_string());

        const MAX_ATTEMPTS: usize = 3;
        let mut last_error: Option<String> = None;

        for attempt in 1..=MAX_ATTEMPTS {
            log::info!(
                "Sheafification attempt {}/{}: requesting structured plan",
                attempt,
                MAX_ATTEMPTS
            );

            // Build the structured prompt
            let prompt = self.build_architect_prompt(&task, last_error.as_deref())?;

            // Call the Architect with tier-aware fallback for structured-output failures
            let response = self
                .call_llm_with_tier_fallback(
                    &self.get_architect_model(),
                    &prompt,
                    None,
                    ModelTier::Architect,
                    |resp| {
                        // Validate that the response contains parseable JSON plan
                        if resp.contains("tasks") && (resp.contains('{') && resp.contains('}')) {
                            Ok(())
                        } else {
                            Err("Response does not contain a JSON task plan".to_string())
                        }
                    },
                )
                .await
                .context("Failed to get Architect response")?;

            log::debug!("Architect response length: {} chars", response.len());

            // Try to parse the JSON plan
            match self.parse_task_plan(&response) {
                Ok(plan) => {
                    // Validate the plan
                    if let Err(e) = plan.validate() {
                        log::warn!("Plan validation failed (attempt {}): {}", attempt, e);
                        last_error = Some(format!("Validation error: {}", e));

                        if attempt >= MAX_ATTEMPTS {
                            self.emit_log(format!(
                                "❌ Failed to get valid plan after {} attempts",
                                MAX_ATTEMPTS
                            ));
                            // Fall back to single-task execution
                            return self.create_deterministic_fallback_graph(&task);
                        }
                        continue;
                    }

                    // Check complexity gating
                    if plan.len() > self.context.complexity_k && !self.auto_approve {
                        self.emit_log(format!(
                            "⚠️ Plan has {} tasks (exceeds K={})",
                            plan.len(),
                            self.context.complexity_k
                        ));
                        // TODO: Implement interactive approval
                        // For now, auto-approve in headless mode
                    }

                    self.emit_log(format!(
                        "✅ Architect produced plan with {} task(s)",
                        plan.len()
                    ));

                    // Emit plan generated event
                    self.emit_event(perspt_core::AgentEvent::PlanGenerated(plan.clone()));

                    // Create nodes from the plan
                    self.create_nodes_from_plan(&plan)?;
                    return Ok(());
                }
                Err(e) => {
                    log::warn!("Plan parsing failed (attempt {}): {}", attempt, e);
                    last_error = Some(format!("JSON parse error: {}", e));

                    if attempt >= MAX_ATTEMPTS {
                        self.emit_log(
                            "⚠️ Could not parse structured plan, using single task".to_string(),
                        );
                        return self.create_deterministic_fallback_graph(&task);
                    }
                }
            }
        }

        // Should not reach here
        self.create_deterministic_fallback_graph(&task)
    }

    /// Build the Architect prompt requesting structured JSON output
    ///
    /// PSP-5 Fix F: Delegates to `ArchitectAgent::build_task_decomposition_prompt`
    /// so the JSON schema contract lives in one place.
    fn build_architect_prompt(&self, task: &str, last_error: Option<&str>) -> Result<String> {
        let mut project_context = self.gather_project_context();

        // PSP-5: For existing projects, prepend a structured project summary
        // so the architect respects existing structure rather than scaffolding.
        if matches!(
            self.context.workspace_state,
            WorkspaceState::ExistingProject { .. }
        ) {
            let retriever = ContextRetriever::new(self.context.working_dir.clone());
            let summary = retriever.get_project_summary();
            if !summary.is_empty() {
                project_context = format!("{}\n\n{}", summary, project_context);
            }
        }

        Ok(
            crate::agent::ArchitectAgent::build_task_decomposition_prompt(
                task,
                &self.context.working_dir,
                &project_context,
                last_error,
            ),
        )
    }

    /// Gather existing project context for the Architect prompt
    /// Uses ContextRetriever to read key configuration files
    fn gather_project_context(&self) -> String {
        let mut context_parts = Vec::new();
        let working_dir = &self.context.working_dir;
        let retriever = ContextRetriever::new(working_dir.clone())
            .with_max_file_bytes(8 * 1024) // 8KB per file for config files
            .with_max_context_bytes(32 * 1024); // 32KB total context

        // Key config files to read (in priority order)
        let config_files = [
            "pyproject.toml",
            "Cargo.toml",
            "package.json",
            "requirements.txt",
        ];

        // Read and include config file contents (up to max_file_bytes)
        let mut found_configs = Vec::new();
        for file in &config_files {
            let path = working_dir.join(file);
            if path.exists() {
                if let Ok(content) = retriever.read_file_truncated(&path) {
                    context_parts.push(format!("### {}\n```\n{}\n```", file, content));
                    found_configs.push(*file);
                }
            }
        }

        // List directory structure
        if let Ok(entries) = std::fs::read_dir(working_dir) {
            let mut dirs = Vec::new();
            let mut files = Vec::new();
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue; // Skip hidden files/dirs
                }
                if entry.path().is_dir() {
                    dirs.push(name);
                } else if !found_configs.contains(&name.as_str()) {
                    files.push(name);
                }
            }

            if !dirs.is_empty() {
                context_parts.push(format!("### Directories\n{}", dirs.join(", ")));
            }
            if !files.is_empty() && files.len() <= 15 {
                context_parts.push(format!("### Other Files\n{}", files.join(", ")));
            } else if !files.is_empty() {
                context_parts.push(format!(
                    "### Other Files\n{} files (not listed)",
                    files.len()
                ));
            }
        }

        if context_parts.is_empty() {
            "Empty directory (greenfield project)".to_string()
        } else {
            context_parts.join("\n\n")
        }
    }

    /// Parse JSON response into TaskPlan
    ///
    /// PSP-5 Phase 4: Uses the provider-neutral normalization layer to extract
    /// JSON from LLM responses regardless of fencing, wrapper text, or provider
    /// formatting quirks. Falls back to raw content parsing if normalization
    /// finds no JSON.
    fn parse_task_plan(&self, content: &str) -> Result<TaskPlan> {
        // PSP-5 Phase 4: Use normalized extraction
        match perspt_core::normalize::extract_and_deserialize::<TaskPlan>(content) {
            Ok((plan, method)) => {
                log::info!("Parsed TaskPlan via normalization ({})", method);
                return Ok(plan);
            }
            Err(e) => {
                log::warn!(
                    "Normalization could not extract TaskPlan: {}. Attempting raw parse.",
                    e
                );
            }
        }

        // Legacy fallback: try direct deserialization of trimmed content
        let trimmed = content.trim();
        log::debug!(
            "Attempting legacy JSON parse: {}...",
            &trimmed[..trimmed.len().min(200)]
        );
        serde_json::from_str(trimmed).context("Failed to parse TaskPlan JSON")
    }

    /// Create SRBN nodes from a parsed TaskPlan
    fn create_nodes_from_plan(&mut self, plan: &TaskPlan) -> Result<()> {
        // PSP-5: Validate plan structure including ownership closure before creating nodes
        plan.validate()
            .map_err(|e| anyhow::anyhow!("Plan validation failed: {}", e))?;

        log::info!("Creating {} nodes from plan", plan.len());

        // Create all nodes first
        let mut node_map: HashMap<String, NodeIndex> = HashMap::new();

        for task in &plan.tasks {
            let node = task.to_srbn_node(ModelTier::Actuator);
            let idx = self.add_node(node);
            node_map.insert(task.id.clone(), idx);
            log::info!("  Created node: {} - {}", task.id, task.goal);
        }

        // Wire up dependencies
        for task in &plan.tasks {
            for dep_id in &task.dependencies {
                if let (Some(&from_idx), Some(&to_idx)) =
                    (node_map.get(dep_id), node_map.get(&task.id))
                {
                    self.graph.add_edge(
                        from_idx,
                        to_idx,
                        Dependency {
                            kind: "depends_on".to_string(),
                        },
                    );
                    log::debug!("  Wired dependency: {} -> {}", dep_id, task.id);

                    // PSP-5 Phase 8: Persist graph edge for resume reconstruction
                    if let Err(e) =
                        self.ledger
                            .record_task_graph_edge(dep_id, &task.id, "depends_on")
                    {
                        log::warn!(
                            "Failed to persist graph edge {} -> {}: {}",
                            dep_id,
                            task.id,
                            e
                        );
                    }
                }
            }
        }

        // PSP-5 Phase 2: Build ownership manifest from plan output_files
        self.build_ownership_manifest_from_plan(plan);

        Ok(())
    }

    /// PSP-5 Phase 2: Build ownership manifest from a TaskPlan
    ///
    /// Assigns each task's output_files to the owning node, detecting the
    /// language plugin from file extension via the plugin registry.
    /// Uses majority-vote across ALL output files instead of first-file-only heuristic.
    fn build_ownership_manifest_from_plan(&mut self, plan: &TaskPlan) {
        let registry = perspt_core::plugin::PluginRegistry::new();

        for task in &plan.tasks {
            // Detect plugin via majority vote across ALL output files
            let mut plugin_votes: HashMap<String, usize> = HashMap::new();
            for f in &task.output_files {
                let detected = registry
                    .all()
                    .iter()
                    .find(|p| p.owns_file(f))
                    .map(|p| p.name().to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                *plugin_votes.entry(detected).or_insert(0) += 1;
            }

            let plugin_name = plugin_votes
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(name, _)| name)
                .unwrap_or_else(|| "unknown".to_string());

            // Warn on mixed-plugin tasks (non-Integration nodes)
            if task.node_class != perspt_core::types::NodeClass::Integration {
                let mixed: Vec<String> = task
                    .output_files
                    .iter()
                    .filter_map(|f| {
                        let det = registry
                            .all()
                            .iter()
                            .find(|p| p.owns_file(f))
                            .map(|p| p.name().to_string())
                            .unwrap_or_else(|| "unknown".to_string());
                        if det != plugin_name {
                            Some(format!("'{}' ({})", f, det))
                        } else {
                            None
                        }
                    })
                    .collect();
                if !mixed.is_empty() {
                    log::warn!(
                        "Task '{}' has mixed-plugin outputs (primary: {}): {}",
                        task.id,
                        plugin_name,
                        mixed.join(", ")
                    );
                }
            }

            // Set owner_plugin on the node if we can find it
            if let Some(idx) = self.node_indices.get(&task.id) {
                self.graph[*idx].owner_plugin = plugin_name.clone();
            }

            // Register each output file in the manifest
            for file in &task.output_files {
                // Detect per-file plugin for accurate manifest entries
                let file_plugin = registry
                    .all()
                    .iter()
                    .find(|p| p.owns_file(file))
                    .map(|p| p.name().to_string())
                    .unwrap_or_else(|| plugin_name.clone());

                self.context.ownership_manifest.assign(
                    file.clone(),
                    task.id.clone(),
                    file_plugin,
                    task.node_class,
                );
            }
        }

        log::info!(
            "Built ownership manifest: {} entries",
            self.context.ownership_manifest.len()
        );
    }

    /// Get the Architect model name
    fn get_architect_model(&self) -> String {
        self.architect_model.clone()
    }

    /// Execute a single node through the control loop
    async fn execute_node(&mut self, idx: NodeIndex) -> Result<()> {
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
        let energy = self.step_verify(idx).await?;

        // Step 5: Convergence & Self-Correction
        if !self.step_converge(idx, energy).await? {
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

            return Ok(());
        }

        // Step 6: Sheaf Validation (Post-Subgraph Consistency)
        self.step_sheaf_validate(idx).await?;

        // Step 7: Merkle Ledger Commit
        self.step_commit(idx).await?;

        // PSP-5 Phase 6: Merge provisional branch after successful commit
        if let Some(ref bid) = branch_id {
            self.merge_provisional_branch(bid, idx);
        }

        Ok(())
    }

    /// Step 3: Speculative Generation
    async fn step_speculate(&mut self, idx: NodeIndex) -> Result<()> {
        log::info!("Step 3: Speculation - Generating implementation");

        // PSP-5 Phase 3: Build context package for this node
        let retriever = ContextRetriever::new(self.context.working_dir.clone())
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
                return Err(anyhow::anyhow!(
                    "Context blocked for node '{}': {}. Node escalated.",
                    self.graph[idx].node_id,
                    reason
                ));
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
        let speculator_hints = {
            let node = &self.graph[idx];
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
                let speculator_prompt = format!(
                    "You are a Speculator agent. Given this task and its downstream dependents, \
                     produce a brief (3-5 bullet) list of:\n\
                     1. Interface contracts the current task must satisfy for dependents\n\
                     2. Common pitfalls (e.g., import paths, missing exports)\n\
                     3. Edge cases dependents may need\n\n\
                     Current task: {} — {}\n\
                     Downstream tasks:\n{}\n\n\
                     Be concise. No code.",
                    node.node_id,
                    node.goal,
                    child_goals.join("\n")
                );

                log::debug!(
                    "Speculator lookahead for node {} using model {}",
                    node.node_id,
                    self.speculator_model
                );
                self.call_llm_with_logging(
                    &self.speculator_model.clone(),
                    &speculator_prompt,
                    Some(&node.node_id),
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
        // Then check for PSP-5 artifact bundles (with legacy single-file fallback inside)
        else if let Some(bundle) = self.parse_artifact_bundle(content) {
            let affected_files: Vec<String> = bundle
                .affected_paths()
                .into_iter()
                .map(ToString::to_string)
                .collect();
            log::info!(
                "Parsed artifact bundle for node {}: {} artifacts, {} commands",
                node_id,
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
            self.apply_bundle_transactionally(&bundle, &node_id, node_class)
                .await?;
            self.last_tool_failure = None;

            // PSP-5 Phase 9: Store bundle for persistence in step_commit
            self.last_applied_bundle = Some(bundle.clone());

            // PSP-5 Phase 9: Execute post-write commands from the bundle
            if !bundle.commands.is_empty() {
                self.emit_log(format!(
                    "🔧 Executing {} bundle command(s)...",
                    bundle.commands.len()
                ));
                let work_dir = self.effective_working_dir(idx);
                let is_python = self.graph[idx].owner_plugin == "python";
                for raw_command in &bundle.commands {
                    // Normalize Python install commands to uv equivalents
                    let command = if is_python {
                        Self::normalize_command_to_uv(raw_command)
                    } else {
                        raw_command.clone()
                    };

                    // Request approval for each command (respects --yes auto-approve)
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
                            // Truncate verbose output for log
                            let truncated: String = result.output.chars().take(500).collect();
                            self.emit_log(truncated);
                        }
                    } else {
                        let err_msg = result.error.unwrap_or_else(|| result.output.clone());
                        log::warn!("Bundle command failed: {} — {}", command, err_msg);
                        self.emit_log(format!("❌ Command failed: {} — {}", command, err_msg));
                        // Record as tool failure so step_verify picks it up via V_syn
                        self.last_tool_failure =
                            Some(format!("Bundle command '{}' failed: {}", command, err_msg));
                    }
                }

                // After all bundle commands, sync Python venv so new deps are available
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
        } else {
            log::debug!(
                "No code block or command found in response, response length: {}",
                content.len()
            );
            self.emit_log("ℹ️ No file changes detected in response".to_string());
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

    /// Extract code from LLM response
    /// Returns (filename, code_content) if found
    /// Extract code from LLM response
    /// Returns (filename, code_content, is_diff) if found
    fn extract_code_from_response(&self, content: &str) -> Option<(String, String, bool)> {
        // Return only the first block for backward compatibility
        self.extract_all_code_blocks_from_response(content)
            .into_iter()
            .next()
    }

    /// Extract ALL File:/Diff: code blocks from an LLM response.
    ///
    /// Unlike `extract_code_from_response` which returns only the first block,
    /// this collects every named code block so multi-file legacy responses are
    /// not silently truncated to a single artifact.
    fn extract_all_code_blocks_from_response(&self, content: &str) -> Vec<(String, String, bool)> {
        let lines: Vec<&str> = content.lines().collect();
        let mut results: Vec<(String, String, bool)> = Vec::new();
        let mut file_path: Option<String> = None;
        let mut is_diff_marker = false;
        let mut in_code_block = false;
        let mut code_lines: Vec<&str> = Vec::new();
        let mut code_lang = String::new();

        for line in &lines {
            // Look for file path patterns
            if line.starts_with("File:") || line.starts_with("**File:") || line.starts_with("file:")
            {
                let path = line
                    .trim_start_matches("File:")
                    .trim_start_matches("**File:")
                    .trim_start_matches("file:")
                    .trim_start_matches("**")
                    .trim_end_matches("**")
                    .trim();
                if !path.is_empty() {
                    file_path = Some(path.to_string());
                    is_diff_marker = false;
                }
            }

            // Look for Diff patterns
            if line.starts_with("Diff:") || line.starts_with("**Diff:") || line.starts_with("diff:")
            {
                let path = line
                    .trim_start_matches("Diff:")
                    .trim_start_matches("**Diff:")
                    .trim_start_matches("diff:")
                    .trim_start_matches("**")
                    .trim_end_matches("**")
                    .trim();
                if !path.is_empty() {
                    file_path = Some(path.to_string());
                    is_diff_marker = true;
                }
            }

            // Parse code blocks
            if line.starts_with("```") && !in_code_block {
                in_code_block = true;
                code_lang = line.trim_start_matches('`').to_string();
                continue;
            }

            if line.starts_with("```") && in_code_block {
                in_code_block = false;
                if !code_lines.is_empty() {
                    let code = code_lines.join("\n");
                    let filename = match file_path.take() {
                        Some(p) => p,
                        None => match code_lang.as_str() {
                            "python" | "py" => "main.py".to_string(),
                            "rust" | "rs" => "main.rs".to_string(),
                            "javascript" | "js" => "index.js".to_string(),
                            "typescript" | "ts" => "index.ts".to_string(),
                            "toml" => "Cargo.toml".to_string(),
                            "json" => "config.json".to_string(),
                            "yaml" | "yml" => "config.yaml".to_string(),
                            other => {
                                log::warn!(
                                    "Skipping unnamed code block with unrecognized language tag '{}'",
                                    other
                                );
                                code_lines.clear();
                                code_lang.clear();
                                is_diff_marker = false;
                                continue;
                            }
                        },
                    };
                    let is_diff = is_diff_marker || code_lang == "diff" || code.starts_with("---");
                    results.push((filename, code, is_diff));
                }
                code_lines.clear();
                code_lang.clear();
                is_diff_marker = false;
                continue;
            }

            if in_code_block {
                code_lines.push(line);
            }
        }

        results
    }

    /// Step 4: Stability Verification
    ///
    /// Computes Lyapunov Energy V(x) from LSP diagnostics, contracts, and tests
    async fn step_verify(&mut self, idx: NodeIndex) -> Result<EnergyComponents> {
        log::info!("Step 4: Verification - Computing stability energy");

        self.graph[idx].state = NodeState::Verifying;
        self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
            node_id: self.graph[idx].node_id.clone(),
            status: perspt_core::NodeStatus::Verifying,
        });

        // Calculate energy components
        let mut energy = EnergyComponents::default();

        // V_syn: From Tool Failures (Critical)
        if let Some(ref err) = self.last_tool_failure {
            energy.v_syn = 10.0; // High energy for tool failure
            log::warn!("Tool failure detected, V_syn set to 10.0: {}", err);
            self.emit_log(format!("⚠️ Tool failure prevents verification: {}", err));
            // We can return early or allow other checks. Usually tool failure means broken state.

            // Store diagnostics mock for correction prompt
            self.context.last_diagnostics = vec![lsp_types::Diagnostic {
                range: lsp_types::Range::default(),
                severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                code: None,
                code_description: None,
                source: Some("tool".to_string()),
                message: format!("Failed to apply changes: {}", err),
                related_information: None,
                tags: None,
                data: None,
            }];
        }

        // V_syn: From LSP diagnostics
        if let Some(ref path) = self.last_written_file {
            // PSP-5 Phase 4: look up LSP client by the node's owner_plugin
            let node_plugin = self.graph[idx].owner_plugin.clone();
            let lsp_key = if node_plugin.is_empty() || node_plugin == "unknown" {
                "python".to_string() // legacy fallback
            } else {
                node_plugin
            };

            if let Some(client) = self.lsp_clients.get(&lsp_key) {
                // Small delay to let LSP analyze the file
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

                let path_str = path.to_string_lossy().to_string();
                let diagnostics = client.get_diagnostics(&path_str).await;

                if !diagnostics.is_empty() {
                    energy.v_syn = LspClient::calculate_syntactic_energy(&diagnostics);
                    log::info!(
                        "LSP found {} diagnostics, V_syn={:.2}",
                        diagnostics.len(),
                        energy.v_syn
                    );
                    self.emit_log(format!("🔍 LSP found {} diagnostics:", diagnostics.len()));
                    for d in &diagnostics {
                        self.emit_log(format!(
                            "   - [{}] {}",
                            severity_to_str(d.severity),
                            d.message
                        ));
                    }

                    // Store diagnostics for correction prompt
                    self.context.last_diagnostics = diagnostics;
                } else {
                    log::info!("LSP reports no errors (diagnostics vector is empty)");
                }
            } else {
                log::debug!("No LSP client available for plugin '{}'", lsp_key);
            }

            // V_str: Check forbidden patterns in written file
            let node = &self.graph[idx];
            if !node.contract.forbidden_patterns.is_empty() {
                if let Ok(content) = std::fs::read_to_string(path) {
                    for pattern in &node.contract.forbidden_patterns {
                        if content.contains(pattern) {
                            energy.v_str += 0.5;
                            log::warn!("Forbidden pattern found: '{}'", pattern);
                            self.emit_log(format!("⚠️ Forbidden pattern: '{}'", pattern));
                        }
                    }
                }
            }

            // PSP-5 Phase 9: Universal plugin verification for all node classes.
            // Replaces the old weighted-test-only block that only ran for Integration nodes.
            let node = &self.graph[idx];
            if self.context.defer_tests {
                self.emit_log("⏭️ Tests deferred (--defer-tests enabled)".to_string());
            } else {
                let plugin_name = node.owner_plugin.clone();
                let verify_dir = self.effective_working_dir(idx);
                let stages = verification_stages_for_node(node);

                if !stages.is_empty() && !plugin_name.is_empty() && plugin_name != "unknown" {
                    self.emit_log(format!(
                        "🔬 Running verification ({} stages) for {} node '{}'...",
                        stages.len(),
                        node.node_class,
                        node.node_id
                    ));

                    let mut vr = self
                        .run_plugin_verification(&plugin_name, &stages, verify_dir.clone())
                        .await;

                    // Auto-dependency repair: if syntax/build failed, check if the
                    // root cause is missing crate dependencies and auto-install them.
                    if !vr.syntax_ok || !vr.build_ok {
                        if let Some(ref raw) = vr.raw_output {
                            let missing = Self::extract_missing_crates(raw);
                            if !missing.is_empty() {
                                self.emit_log(format!(
                                    "📦 Auto-installing missing dependencies: {}",
                                    missing.join(", ")
                                ));
                                let dep_ok =
                                    Self::auto_install_crate_deps(&missing, &verify_dir).await;
                                if dep_ok > 0 {
                                    self.emit_log(format!(
                                        "📦 Installed {} crate(s), re-running verification...",
                                        dep_ok
                                    ));
                                    // Re-run verification now that deps are installed
                                    vr = self
                                        .run_plugin_verification(
                                            &plugin_name,
                                            &stages,
                                            verify_dir.clone(),
                                        )
                                        .await;
                                }
                            }
                        }
                    }

                    // Auto-dependency repair for Python: parse
                    // ModuleNotFoundError / ImportError from test output and
                    // install missing packages via `uv add`.
                    if plugin_name == "python" && (!vr.syntax_ok || !vr.tests_ok) {
                        // Collect raw output from all stage outcomes for
                        // broader error detection (syntax check may report
                        // import errors too).
                        let all_output: String = vr
                            .stage_outcomes
                            .iter()
                            .filter_map(|so| so.output.as_deref())
                            .collect::<Vec<_>>()
                            .join("\n");
                        let combined = match vr.raw_output.as_deref() {
                            Some(raw) => format!("{}\n{}", raw, all_output),
                            None => all_output,
                        };

                        let missing = Self::extract_missing_python_modules(&combined);
                        if !missing.is_empty() {
                            self.emit_log(format!(
                                "🐍 Auto-installing missing Python packages: {}",
                                missing.join(", ")
                            ));
                            let dep_ok =
                                Self::auto_install_python_deps(&missing, &verify_dir).await;
                            if dep_ok > 0 {
                                self.emit_log(format!(
                                    "🐍 Installed {} package(s), re-running verification...",
                                    dep_ok
                                ));
                                vr = self
                                    .run_plugin_verification(
                                        &plugin_name,
                                        &stages,
                                        verify_dir.clone(),
                                    )
                                    .await;
                            }
                        }
                    }

                    // Map verification result to energy components:
                    // - Syntax fail → V_syn (cap at 5.0, don't override tool-failure 10.0)
                    if !vr.syntax_ok && energy.v_syn < 5.0 {
                        energy.v_syn = 5.0;
                    }
                    // - Build fail → V_syn (cap at 8.0, don't override higher)
                    if !vr.build_ok && energy.v_syn < 8.0 {
                        energy.v_syn = 8.0;
                    }
                    // - Test fail → V_log (weighted calculation)
                    if !vr.tests_ok && vr.tests_failed > 0 {
                        let node = &self.graph[idx];
                        if !node.contract.weighted_tests.is_empty() {
                            // Use weighted test calculation if contract has weights
                            let py_runner = PythonTestRunner::new(verify_dir);
                            let test_results = TestResults {
                                passed: vr.tests_passed,
                                failed: vr.tests_failed,
                                total: vr.tests_passed + vr.tests_failed,
                                output: vr.raw_output.clone().unwrap_or_default(),
                                failures: Vec::new(),
                                run_succeeded: true,
                                skipped: 0,
                                duration_ms: 0,
                            };
                            energy.v_log = py_runner.calculate_v_log(&test_results, &node.contract);
                        } else {
                            // Simple: proportion of failures
                            let total = (vr.tests_passed + vr.tests_failed) as f32;
                            if total > 0.0 {
                                energy.v_log = (vr.tests_failed as f32 / total) * 10.0;
                            }
                        }
                    }
                    // - Lint fail → V_str penalty
                    if !vr.lint_ok
                        && self.context.verifier_strictness
                            == perspt_core::types::VerifierStrictness::Strict
                    {
                        energy.v_str += 0.3;
                    }

                    // D1: Feed raw output into correction context
                    if let Some(ref raw) = vr.raw_output {
                        self.context.last_test_output = Some(raw.clone());
                    }

                    self.emit_log(format!("📊 Verification: {}", vr.summary));
                }
            }
        }

        let node = &self.graph[idx];
        // Record energy in persistent ledger
        if let Err(e) =
            self.ledger
                .record_energy(&node.node_id, &energy, energy.total(&node.contract))
        {
            log::error!("Failed to record energy: {}", e);
        }

        log::info!(
            "Energy for {}: V_syn={:.2}, V_str={:.2}, V_log={:.2}, V_boot={:.2}, V_sheaf={:.2}, Total={:.2}",
            node.node_id,
            energy.v_syn,
            energy.v_str,
            energy.v_log,
            energy.v_boot,
            energy.v_sheaf,
            energy.total(&node.contract)
        );

        // PSP-5 Phase 7: Emit enriched VerificationComplete event
        {
            let node = &self.graph[idx];
            let total = energy.total(&node.contract);
            let (
                stage_outcomes,
                degraded,
                degraded_reasons,
                summary,
                lint_ok,
                tests_passed,
                tests_failed,
            ) = if let Some(ref vr) = self.last_verification_result {
                (
                    vr.stage_outcomes.clone(),
                    vr.degraded,
                    vr.degraded_stage_reasons(),
                    vr.summary.clone(),
                    vr.lint_ok,
                    vr.tests_passed,
                    vr.tests_failed,
                )
            } else {
                let diag_count = self.context.last_diagnostics.len();
                (
                    Vec::new(),
                    false,
                    Vec::new(),
                    format!("V(x)={:.2} | {} diagnostics", total, diag_count),
                    true,
                    0,
                    0,
                )
            };

            self.emit_event(perspt_core::AgentEvent::VerificationComplete {
                node_id: node.node_id.clone(),
                syntax_ok: energy.v_syn == 0.0,
                build_ok: energy.v_syn < 5.0,
                tests_ok: energy.v_log == 0.0,
                lint_ok,
                diagnostics_count: self.context.last_diagnostics.len(),
                tests_passed,
                tests_failed,
                energy: total,
                energy_components: energy.clone(),
                stage_outcomes,
                degraded,
                degraded_reasons,
                summary,
                node_class: node.node_class.to_string(),
            });
        }

        Ok(energy)
    }

    /// Step 5: Convergence & Self-Correction
    ///
    /// Returns true if converged, false if should escalate
    async fn step_converge(&mut self, idx: NodeIndex, energy: EnergyComponents) -> Result<bool> {
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

        // Extract and apply diff
        if let Some((filename, new_code, is_diff)) = self.extract_code_from_response(&corrected) {
            let full_path = self.context.working_dir.join(&filename);

            // Write corrected file
            let mut args = HashMap::new();
            args.insert("path".to_string(), filename.clone());

            let call = if is_diff {
                args.insert("diff".to_string(), new_code.clone());
                ToolCall {
                    name: "apply_diff".to_string(),
                    arguments: args,
                }
            } else {
                args.insert("content".to_string(), new_code.clone());
                ToolCall {
                    name: "write_file".to_string(),
                    arguments: args,
                }
            };

            let result = self.tools.execute(&call).await;
            if result.success {
                log::info!("✓ Applied correction to: {}", filename);
                self.emit_log(format!("📝 Applied correction to: {}", filename));
                self.last_tool_failure = None;

                // Update tracking
                self.last_written_file = Some(full_path.clone());
                self.file_version += 1;

                // Notify LSP of file change
                let lsp_key = self.lsp_key_for_file(&full_path.to_string_lossy());
                if let Some(client) = lsp_key.and_then(|k| self.lsp_clients.get_mut(&k)) {
                    if let Ok(content) = std::fs::read_to_string(&full_path) {
                        let _ = client
                            .did_change(&full_path, &content, self.file_version)
                            .await;
                    }
                }
            } else {
                self.last_tool_failure = result.error;
            }
        }

        // Extract and execute any dependency commands from the correction response
        let correction_cmds = Self::extract_commands_from_correction(&corrected);
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
    fn build_correction_prompt(
        &self,
        _node_id: &str,
        goal: &str,
        energy: &EnergyComponents,
    ) -> Result<String> {
        let diagnostics = &self.context.last_diagnostics;

        // Read current code from the last written file
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

        // Detect language from file extension for code fences and instructions
        let lang = self
            .last_written_file
            .as_ref()
            .and_then(|p| p.extension())
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
            r#"## Code Correction Required

The code you generated has {} error(s) detected by the language toolchain.
Your task is to fix ALL errors and return the complete corrected file.

### Original Goal
{}

### Current Code (with errors)
File: {}
```{}
{}
```

### Detected Errors (V_syn = {:.2})
"#,
            diagnostics.len(),
            goal,
            file_path,
            lang,
            current_code,
            energy.v_syn
        );

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

        prompt.push_str(&format!(
            r#"
### Fix Requirements
1. Fix ALL errors listed above - do not leave any unfixed
2. Maintain the original functionality and goal
3. Follow {} language conventions and idioms
4. Import any missing modules or dependencies
5. Return the COMPLETE corrected file, not just snippets
6. If errors mention missing crates/packages (e.g. "can't find crate", "unresolved import" for an external dependency, "ModuleNotFoundError", "No module named"), list the required install commands

### Output Format
Provide the complete corrected file followed by any dependency commands needed:

File: [same filename]
```{}
[complete corrected code]
```

Commands: [optional, one per line]
```
cargo add thiserror
cargo add clap --features derive
uv add httpx
uv add --dev pytest
```
"#,
            lang, lang
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
    async fn call_llm_for_correction(&self, prompt: &str) -> Result<String> {
        // Stage 1: Verifier analyzes the failure
        let verifier_prompt = format!(
            "You are a Verifier agent. Analyze the following correction request and produce \
             concise, structured guidance for the code fixer. Identify:\n\
             1. Root cause of each failure\n\
             2. Which specific functions/lines need changes\n\
             3. Constraints that must be preserved\n\
             Do NOT produce code — only analysis and guidance.\n\n{}",
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

    /// Call LLM and immediately persist the request/response to database if logging is enabled
    async fn call_llm_with_logging(
        &self,
        model: &str,
        prompt: &str,
        node_id: Option<&str>,
    ) -> Result<String> {
        let start = Instant::now();

        let response = self
            .provider
            .generate_response_simple(model, prompt)
            .await?;

        // Immediately persist to database if logging is enabled
        if self.context.log_llm {
            let latency_ms = start.elapsed().as_millis() as i32;
            if let Err(e) = self
                .ledger
                .record_llm_request(model, prompt, &response, node_id, latency_ms)
            {
                log::warn!("Failed to persist LLM request: {}", e);
            } else {
                log::debug!(
                    "Persisted LLM request: model={}, latency={}ms",
                    model,
                    latency_ms
                );
            }
        }

        Ok(response)
    }

    /// PSP-5 Phase 1/4: Call LLM with tier-aware fallback.
    ///
    /// If the primary model returns a response that fails structured-output
    /// contract validation (`validator` returns `Err`), and a fallback model
    /// is configured for the given tier, retry with the fallback. Emits a
    /// `ModelFallback` event on switch. Returns the raw response string.
    async fn call_llm_with_tier_fallback<F>(
        &self,
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

        // Look up fallback model for this tier
        let fallback = match tier {
            ModelTier::Architect => self.architect_fallback_model.as_deref(),
            ModelTier::Actuator => self.actuator_fallback_model.as_deref(),
            ModelTier::Verifier => self.verifier_fallback_model.as_deref(),
            ModelTier::Speculator => self.speculator_fallback_model.as_deref(),
        };

        // If no explicit fallback configured, retry with the same primary model.
        // This gives the LLM a second chance at structured output without
        // requiring explicit fallback config for every tier.
        let fallback_model = fallback.unwrap_or(primary_model);

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

        self.call_llm_with_logging(fallback_model, prompt, node_id)
            .await
    }

    /// Emit an event from a &self context (non-mutable).
    fn emit_event_ref(&self, event: perspt_core::AgentEvent) {
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(event);
        }
    }

    /// Step 6: Sheaf Validation
    async fn step_sheaf_validate(&mut self, idx: NodeIndex) -> Result<()> {
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
    fn select_validators(&self, idx: NodeIndex) -> Vec<SheafValidatorClass> {
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
        let node_owner = &node.owner_plugin;
        let has_cross_plugin_deps = self
            .graph
            .neighbors_directed(idx, petgraph::Direction::Outgoing)
            .any(|dep_idx| self.graph[dep_idx].owner_plugin != *node_owner);
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
    fn run_sheaf_validator(
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
                let mut boundary_issues = Vec::new();
                let node_plugin = &node.owner_plugin;

                for dep_idx in self
                    .graph
                    .neighbors_directed(idx, petgraph::Direction::Outgoing)
                {
                    let dep = &self.graph[dep_idx];
                    if dep.owner_plugin != *node_plugin && !dep.owner_plugin.is_empty() {
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
    async fn step_commit(&mut self, idx: NodeIndex) -> Result<()> {
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

    // =========================================================================
    // PSP-5 Phase 5: Non-Convergence Classification and Repair
    // =========================================================================

    /// Classify why a node failed to converge.
    ///
    /// Uses the last verification result, retry policy, tool failure state,
    /// and graph topology to determine the failure category.
    fn classify_non_convergence(&self, idx: NodeIndex) -> EscalationCategory {
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
    fn choose_repair_action(&self, idx: NodeIndex, category: &EscalationCategory) -> RewriteAction {
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
    async fn apply_repair_action(&mut self, idx: NodeIndex, action: &RewriteAction) -> bool {
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
    fn count_lineage_rewrites(&self, node_id: &str) -> usize {
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
    fn build_escalation_evidence(&self, idx: NodeIndex) -> String {
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
    fn affected_dependents(&self, idx: NodeIndex) -> Vec<String> {
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
    fn split_node(&mut self, idx: NodeIndex, proposed_children: &[String]) -> Vec<String> {
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
    fn insert_interface_node(&mut self, idx: NodeIndex, boundary: &str) -> Option<String> {
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
    fn replan_subgraph(&mut self, trigger_idx: NodeIndex, affected_nodes: &[String]) -> bool {
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

    /// Get the current session ID
    pub fn session_id(&self) -> &str {
        &self.context.session_id
    }

    /// Get node count
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Start Python LSP (ty) for type checking
    ///
    /// Legacy entry point — delegates to `start_lsp_for_plugins` with just Python.
    pub async fn start_python_lsp(&mut self) -> Result<()> {
        self.start_lsp_for_plugins(&["python"]).await
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

        // If filtering removed ALL artifacts, fall back to the original bundle
        // with a warning — the architect/actuator path mismatch shouldn't kill
        // the entire session.  Ownership validation below still guards against
        // true cross-node conflicts.
        let bundle = if filtered.artifacts.is_empty() && !bundle.artifacts.is_empty() {
            log::warn!(
                "All artifacts stripped for node '{}' — falling back to original bundle",
                node_id
            );
            self.emit_log(format!(
                "⚠️ Path mismatch: all artifacts for '{}' targeted unplanned paths — applying anyway",
                node_id
            ));
            bundle.clone()
        } else {
            filtered
        };

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
            };

            let result = self.tools.execute(&call).await;
            if result.success {
                let full_path = resolved_path.clone();

                if op.is_write() {
                    files_created.push(op.path().to_string());
                } else {
                    files_modified.push(op.path().to_string());
                }

                // Track for LSP notification
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

    /// PSP-5: Create a deterministic fallback execution graph
    ///
    /// When the Architect fails to produce a valid JSON plan after MAX_ATTEMPTS,
    /// this creates a minimal 3-node graph: scaffold → implement → test.
    fn create_deterministic_fallback_graph(&mut self, task: &str) -> Result<()> {
        log::warn!("Using deterministic fallback graph (PSP-5)");
        self.emit_log("📦 Using deterministic fallback plan");

        // Emit FallbackPlanner event
        self.emit_event(perspt_core::AgentEvent::FallbackPlanner {
            reason: "Architect failed to produce valid JSON plan".to_string(),
        });

        // Detect language for file extensions
        let lang = self.detect_language_from_task(task).unwrap_or("python");
        let ext = match lang {
            "rust" => "rs",
            "javascript" => "js",
            _ => "py",
        };

        // Determine file names based on language
        let (main_file, test_file) = match lang {
            "rust" => (
                "src/main.rs".to_string(),
                "tests/integration_test.rs".to_string(),
            ),
            "javascript" => ("index.js".to_string(), "test/index.test.js".to_string()),
            _ => ("main.py".to_string(), format!("tests/test_main.{}", ext)),
        };

        // Node 1: Scaffold/structure
        let scaffold_task = perspt_core::types::PlannedTask {
            id: "scaffold".to_string(),
            goal: format!("Set up project structure for: {}", task),
            context_files: vec![],
            output_files: vec![main_file.clone()],
            dependencies: vec![],
            task_type: perspt_core::types::TaskType::Code,
            contract: Default::default(),
            command_contract: None,
            node_class: perspt_core::types::NodeClass::Interface,
        };

        // Node 2: Core implementation
        let impl_task = perspt_core::types::PlannedTask {
            id: "implement".to_string(),
            goal: format!("Implement core logic for: {}", task),
            context_files: vec![main_file.clone()],
            output_files: vec![main_file],
            dependencies: vec!["scaffold".to_string()],
            task_type: perspt_core::types::TaskType::Code,
            contract: Default::default(),
            command_contract: None,
            node_class: perspt_core::types::NodeClass::Implementation,
        };

        // Node 3: Tests
        let test_task = perspt_core::types::PlannedTask {
            id: "test".to_string(),
            goal: format!("Write tests for: {}", task),
            context_files: vec![],
            output_files: vec![test_file],
            dependencies: vec!["implement".to_string()],
            task_type: perspt_core::types::TaskType::UnitTest,
            contract: Default::default(),
            command_contract: None,
            node_class: perspt_core::types::NodeClass::Integration,
        };

        // Build plan and emit
        let mut plan = perspt_core::types::TaskPlan::new();
        plan.tasks.push(scaffold_task);
        plan.tasks.push(impl_task);
        plan.tasks.push(test_task);

        self.emit_event(perspt_core::AgentEvent::PlanGenerated(plan.clone()));
        self.create_nodes_from_plan(&plan)?;

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
                    .map(|p| p.to_string_lossy().to_string())
                    .collect()
            })
            .unwrap_or_default();

        if allowed_paths.is_empty() {
            return bundle.clone();
        }

        let (kept, dropped): (Vec<_>, Vec<_>) = bundle
            .artifacts
            .iter()
            .cloned()
            .partition(|a| allowed_paths.contains(a.path()));

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

    /// PSP-5: Run plugin-driven verification for a node
    ///
    /// Uses the active language plugin's verifier profile to select commands
    /// for syntax check, build, test, and lint stages. Delegates execution
    /// to `TestRunnerTrait` implementations from `test_runner`.
    ///
    /// Each stage records a `StageOutcome` with `SensorStatus`, enabling
    /// callers to detect fallback / unavailable sensors and block false
    /// stability claims.
    pub async fn run_plugin_verification(
        &mut self,
        plugin_name: &str,
        allowed_stages: &[perspt_core::plugin::VerifierStage],
        working_dir: std::path::PathBuf,
    ) -> perspt_core::types::VerificationResult {
        use perspt_core::plugin::VerifierStage;
        use perspt_core::types::{SensorStatus, StageOutcome};

        let registry = perspt_core::plugin::PluginRegistry::new();
        let plugin = match registry.get(plugin_name) {
            Some(p) => p,
            None => {
                return perspt_core::types::VerificationResult::degraded(format!(
                    "Plugin '{}' not found",
                    plugin_name
                ));
            }
        };

        let profile = plugin.verifier_profile();

        // If fully degraded, report immediately
        if profile.fully_degraded() {
            return perspt_core::types::VerificationResult::degraded(format!(
                "{} toolchain not available on host (all stages degraded)",
                plugin.name()
            ));
        }

        // Derive per-stage sensor status from the profile before moving it.
        let sensor_status_for = |stage: VerifierStage,
                                 profile: &perspt_core::plugin::VerifierProfile|
         -> SensorStatus {
            match profile.get(stage) {
                Some(cap) if cap.available => SensorStatus::Available,
                Some(cap) if cap.fallback_available => SensorStatus::Fallback {
                    actual: cap
                        .fallback_command
                        .clone()
                        .unwrap_or_else(|| "fallback".into()),
                    reason: format!(
                        "primary '{}' not found",
                        cap.command.as_deref().unwrap_or("?")
                    ),
                },
                Some(cap) => SensorStatus::Unavailable {
                    reason: format!(
                        "no tool for {} (tried '{}')",
                        stage,
                        cap.command.as_deref().unwrap_or("?")
                    ),
                },
                None => SensorStatus::Unavailable {
                    reason: format!("{} stage not declared by plugin", stage),
                },
            }
        };

        let syn_sensor = sensor_status_for(VerifierStage::SyntaxCheck, &profile);
        let build_sensor = sensor_status_for(VerifierStage::Build, &profile);
        let test_sensor = sensor_status_for(VerifierStage::Test, &profile);
        let lint_sensor = sensor_status_for(VerifierStage::Lint, &profile);

        let runner = test_runner::test_runner_for_profile(profile, working_dir);

        let mut result = perspt_core::types::VerificationResult::default();

        // PSP-5 Phase 9: Only run stages that are in the allowed filter.
        // Short-circuit: if syntax fails, skip build/test/lint.
        //                if build fails, skip test/lint.

        // Syntax check
        if allowed_stages.contains(&VerifierStage::SyntaxCheck) {
            match runner.run_syntax_check().await {
                Ok(r) => {
                    result.syntax_ok = r.passed > 0 && r.failed == 0;
                    if !result.syntax_ok && r.run_succeeded {
                        result.diagnostics_count = r.output.lines().count();
                        result.raw_output = Some(r.output.clone());
                        self.emit_log(format!(
                            "⚠️ Syntax check failed ({} diagnostics)",
                            result.diagnostics_count
                        ));
                    } else if result.syntax_ok {
                        self.emit_log("✅ Syntax check passed".to_string());
                    }
                    result.stage_outcomes.push(StageOutcome {
                        stage: VerifierStage::SyntaxCheck.to_string(),
                        passed: result.syntax_ok,
                        sensor_status: syn_sensor,
                        output: Some(r.output),
                    });
                }
                Err(e) => {
                    log::warn!("Syntax check failed to run: {}", e);
                    result.syntax_ok = false;
                    result.stage_outcomes.push(StageOutcome {
                        stage: VerifierStage::SyntaxCheck.to_string(),
                        passed: false,
                        sensor_status: SensorStatus::Unavailable {
                            reason: format!("execution error: {}", e),
                        },
                        output: None,
                    });
                }
            }

            // Short-circuit: if syntax fails, skip remaining stages
            if !result.syntax_ok {
                self.emit_log("⏭️ Skipping build/test/lint — syntax check failed".to_string());
                result.build_ok = false;
                result.tests_ok = false;
                self.finalize_verification_result(&mut result, plugin_name);
                return result;
            }
        }

        // Build check
        if allowed_stages.contains(&VerifierStage::Build) {
            match runner.run_build_check().await {
                Ok(r) => {
                    result.build_ok = r.passed > 0 && r.failed == 0;
                    if result.build_ok {
                        self.emit_log("✅ Build passed".to_string());
                    } else if r.run_succeeded {
                        self.emit_log("⚠️ Build failed".to_string());
                        result.raw_output = Some(r.output.clone());
                    }
                    result.stage_outcomes.push(StageOutcome {
                        stage: VerifierStage::Build.to_string(),
                        passed: result.build_ok,
                        sensor_status: build_sensor,
                        output: Some(r.output),
                    });
                }
                Err(e) => {
                    log::warn!("Build check failed to run: {}", e);
                    result.build_ok = false;
                    result.stage_outcomes.push(StageOutcome {
                        stage: VerifierStage::Build.to_string(),
                        passed: false,
                        sensor_status: SensorStatus::Unavailable {
                            reason: format!("execution error: {}", e),
                        },
                        output: None,
                    });
                }
            }

            // Short-circuit: if build fails, skip test/lint
            if !result.build_ok {
                self.emit_log("⏭️ Skipping test/lint — build failed".to_string());
                result.tests_ok = false;
                self.finalize_verification_result(&mut result, plugin_name);
                return result;
            }
        }

        // Tests
        if allowed_stages.contains(&VerifierStage::Test) {
            match runner.run_tests().await {
                Ok(r) => {
                    result.tests_ok = r.all_passed();
                    result.tests_passed = r.passed;
                    result.tests_failed = r.failed;

                    if result.tests_ok {
                        self.emit_log(format!("✅ Tests passed ({})", plugin_name));
                    } else {
                        self.emit_log(format!("❌ Tests failed ({})", plugin_name));
                        result.raw_output = Some(r.output.clone());
                    }
                    result.stage_outcomes.push(StageOutcome {
                        stage: VerifierStage::Test.to_string(),
                        passed: result.tests_ok,
                        sensor_status: test_sensor,
                        output: Some(r.output),
                    });
                }
                Err(e) => {
                    log::warn!("Test command failed to run: {}", e);
                    result.tests_ok = false;
                    result.stage_outcomes.push(StageOutcome {
                        stage: VerifierStage::Test.to_string(),
                        passed: false,
                        sensor_status: SensorStatus::Unavailable {
                            reason: format!("execution error: {}", e),
                        },
                        output: None,
                    });
                }
            }
        }

        // Lint (only when allowed AND in Strict mode)
        if allowed_stages.contains(&VerifierStage::Lint)
            && self.context.verifier_strictness == perspt_core::types::VerifierStrictness::Strict
        {
            match runner.run_lint().await {
                Ok(r) => {
                    result.lint_ok = r.passed > 0 && r.failed == 0;
                    if result.lint_ok {
                        self.emit_log("✅ Lint passed".to_string());
                    } else if r.run_succeeded {
                        self.emit_log("⚠️ Lint issues found".to_string());
                    }
                    result.stage_outcomes.push(StageOutcome {
                        stage: VerifierStage::Lint.to_string(),
                        passed: result.lint_ok,
                        sensor_status: lint_sensor,
                        output: Some(r.output),
                    });
                }
                Err(e) => {
                    log::warn!("Lint command failed to run: {}", e);
                    result.lint_ok = false;
                    result.stage_outcomes.push(StageOutcome {
                        stage: VerifierStage::Lint.to_string(),
                        passed: false,
                        sensor_status: SensorStatus::Unavailable {
                            reason: format!("execution error: {}", e),
                        },
                        output: None,
                    });
                }
            }
        } else if !allowed_stages.contains(&VerifierStage::Lint) {
            result.lint_ok = true; // Skip lint when not in allowed stages
        } else {
            result.lint_ok = true; // Skip lint in non-strict mode
        }

        self.finalize_verification_result(&mut result, plugin_name);
        result
    }

    // =========================================================================
    // Auto-dependency repair helpers
    // =========================================================================

    /// Parse `cargo check` / `cargo build` stderr and extract crate names that
    /// are missing.  Handles patterns like:
    ///   - `error[E0432]: unresolved import \`thiserror\``
    ///   - `error[E0463]: can't find crate for \`serde\``
    ///   - `use of undeclared crate or module \`clap\``
    fn extract_missing_crates(output: &str) -> Vec<String> {
        use std::collections::HashSet;

        let mut crates: HashSet<String> = HashSet::new();

        for line in output.lines() {
            let lower = line.to_lowercase();

            // Pattern: "use of undeclared crate or module `foo`"
            if lower.contains("undeclared crate or module") {
                if let Some(name) = Self::extract_backtick_ident(line) {
                    if !name.contains("::") {
                        crates.insert(name);
                    }
                }
            }
            // Pattern: "can't find crate for `foo`"
            else if lower.contains("can't find crate for")
                || lower.contains("cant find crate for")
            {
                if let Some(name) = Self::extract_backtick_ident(line) {
                    crates.insert(name);
                }
            }
            // Pattern: "unresolved import `thiserror`" at top level
            else if lower.contains("unresolved import") {
                if let Some(name) = Self::extract_backtick_ident(line) {
                    let root = name.split("::").next().unwrap_or(&name).to_string();
                    if root != "crate" && root != "self" && root != "super" {
                        crates.insert(root);
                    }
                }
            }
        }

        let builtins: HashSet<&str> = ["std", "core", "alloc", "proc_macro", "test"]
            .iter()
            .copied()
            .collect();

        crates
            .into_iter()
            .filter(|c| !builtins.contains(c.as_str()))
            .collect()
    }

    /// Extract the first back-tick–quoted identifier from a line.
    fn extract_backtick_ident(line: &str) -> Option<String> {
        let start = line.find('`')? + 1;
        let rest = &line[start..];
        let end = rest.find('`')?;
        let ident = &rest[..end];
        if ident.is_empty() {
            None
        } else {
            Some(ident.to_string())
        }
    }

    /// Extract dependency commands from a correction LLM response.
    fn extract_commands_from_correction(response: &str) -> Vec<String> {
        let mut commands = Vec::new();
        let mut in_commands_section = false;
        let mut in_code_block = false;

        let allowed_prefixes = [
            "cargo add",
            "pip install",
            "pip3 install",
            "uv add",
            "uv pip install",
            "npm install",
            "yarn add",
            "pnpm add",
        ];

        for line in response.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("Commands:")
                || trimmed.starts_with("**Commands:")
                || trimmed.starts_with("### Commands")
            {
                in_commands_section = true;
                continue;
            }

            if in_commands_section {
                if trimmed.starts_with("```") {
                    in_code_block = !in_code_block;
                    continue;
                }

                if !in_code_block
                    && (trimmed.is_empty()
                        || trimmed.starts_with('#')
                        || trimmed.starts_with("File:")
                        || trimmed.starts_with("Diff:"))
                {
                    in_commands_section = false;
                    continue;
                }

                let cmd = trimmed
                    .trim_start_matches("- ")
                    .trim_start_matches("$ ")
                    .trim();

                if !cmd.is_empty() && allowed_prefixes.iter().any(|p| cmd.starts_with(p)) {
                    commands.push(cmd.to_string());
                }
            }
        }

        commands
    }

    /// Run `cargo add <crate>` for each missing crate. Returns count of successes.
    async fn auto_install_crate_deps(crates: &[String], working_dir: &std::path::Path) -> usize {
        let mut installed = 0usize;
        for krate in crates {
            log::info!("Auto-installing crate: cargo add {}", krate);
            let result = tokio::process::Command::new("cargo")
                .args(["add", krate])
                .current_dir(working_dir)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .await;

            match result {
                Ok(output) if output.status.success() => {
                    log::info!("Successfully installed crate: {}", krate);
                    installed += 1;
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    log::warn!("Failed to install crate {}: {}", krate, stderr);
                }
                Err(e) => {
                    log::warn!("Failed to run cargo add {}: {}", krate, e);
                }
            }
        }
        installed
    }

    // =========================================================================
    // Python auto-dependency repair helpers (uv-first)
    // =========================================================================

    /// Parse Python test/import output and extract module names that are missing.
    ///
    /// Handles patterns like:
    ///   - `ModuleNotFoundError: No module named 'httpx'`
    ///   - `ImportError: cannot import name 'foo' from 'bar'`
    ///   - `E   ModuleNotFoundError: No module named 'pydantic'`
    fn extract_missing_python_modules(output: &str) -> Vec<String> {
        use std::collections::HashSet;

        let mut modules: HashSet<String> = HashSet::new();

        for line in output.lines() {
            let trimmed = line.trim().trim_start_matches("E").trim();

            // Pattern: "ModuleNotFoundError: No module named 'foo'"
            // Also matches: "ModuleNotFoundError: No module named 'foo.bar'"
            // Can appear anywhere in the line (e.g. after FAILED test_x.py::test - ...)
            if trimmed.contains("ModuleNotFoundError: No module named ") {
                // Extract the quoted module name after "No module named "
                if let Some(pos) = trimmed.find("No module named ") {
                    let after = &trimmed[pos + "No module named ".len()..];
                    let name = after.trim().trim_matches('\'').trim_matches('"');
                    let root = name.split('.').next().unwrap_or(name);
                    if !root.is_empty() {
                        modules.insert(root.to_string());
                    }
                }
            }
            // Pattern: "ImportError: cannot import name 'X' from 'Y'"
            // or "ImportError: No module named 'X'"
            else if trimmed.contains("ImportError") && trimmed.contains("No module named") {
                if let Some(start) = trimmed.find('\'') {
                    let rest = &trimmed[start + 1..];
                    if let Some(end) = rest.find('\'') {
                        let name = &rest[..end];
                        let root = name.split('.').next().unwrap_or(name);
                        if !root.is_empty() {
                            modules.insert(root.to_string());
                        }
                    }
                }
            }
        }

        // Filter out standard library modules that are always present
        let stdlib: HashSet<&str> = [
            "os",
            "sys",
            "json",
            "re",
            "math",
            "datetime",
            "collections",
            "itertools",
            "functools",
            "pathlib",
            "typing",
            "abc",
            "io",
            "unittest",
            "logging",
            "argparse",
            "sqlite3",
            "csv",
            "hashlib",
            "tempfile",
            "shutil",
            "copy",
            "contextlib",
            "dataclasses",
            "enum",
            "textwrap",
            "importlib",
            "inspect",
            "traceback",
            "subprocess",
            "threading",
            "multiprocessing",
            "asyncio",
            "socket",
            "http",
            "urllib",
            "xml",
            "html",
            "email",
            "string",
            "struct",
            "array",
            "queue",
            "heapq",
            "bisect",
            "pprint",
            "decimal",
            "fractions",
            "random",
            "secrets",
            "time",
            "calendar",
            "zlib",
            "gzip",
            "zipfile",
            "tarfile",
            "glob",
            "fnmatch",
            "stat",
            "fileinput",
            "codecs",
            "uuid",
            "base64",
            "binascii",
            "pickle",
            "shelve",
            "dbm",
            "platform",
            "signal",
            "mmap",
            "ctypes",
            "configparser",
            "tomllib",
            "warnings",
            "weakref",
            "types",
            "operator",
            "numbers",
            "__future__",
        ]
        .iter()
        .copied()
        .collect();

        modules
            .into_iter()
            .filter(|m| !stdlib.contains(m.as_str()))
            .collect()
    }

    /// Map a Python import name to its PyPI package name.
    ///
    /// Most packages use the same name for import and install, but some
    /// notable exceptions exist. We handle the common ones here.
    fn python_import_to_package(import_name: &str) -> &str {
        match import_name {
            "PIL" | "pil" => "pillow",
            "cv2" => "opencv-python",
            "yaml" => "pyyaml",
            "bs4" => "beautifulsoup4",
            "sklearn" => "scikit-learn",
            "attr" | "attrs" => "attrs",
            "dateutil" => "python-dateutil",
            "dotenv" => "python-dotenv",
            "gi" => "PyGObject",
            "serial" => "pyserial",
            "usb" => "pyusb",
            "wx" => "wxPython",
            "lxml" => "lxml",
            "Crypto" => "pycryptodome",
            "jose" => "python-jose",
            "jwt" => "PyJWT",
            "magic" => "python-magic",
            "docx" => "python-docx",
            "pptx" => "python-pptx",
            "git" => "gitpython",
            "psycopg2" => "psycopg2-binary",
            other => other,
        }
    }

    /// Run `uv add <package>` for each missing Python module. Returns count of successes.
    async fn auto_install_python_deps(modules: &[String], working_dir: &std::path::Path) -> usize {
        let mut installed = 0usize;
        for module in modules {
            let package = Self::python_import_to_package(module);
            log::info!("Auto-installing Python package: uv add {}", package);
            let result = tokio::process::Command::new("uv")
                .args(["add", package])
                .current_dir(working_dir)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .await;

            match result {
                Ok(output) if output.status.success() => {
                    log::info!("Successfully installed Python package: {}", package);
                    installed += 1;
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    log::warn!("Failed to install Python package {}: {}", package, stderr);
                }
                Err(e) => {
                    log::warn!("Failed to run uv add {}: {}", package, e);
                }
            }
        }

        // Always sync after adding dependencies to ensure venv is up-to-date
        if installed > 0 {
            log::info!("Running uv sync --dev after dependency install...");
            let _ = tokio::process::Command::new("uv")
                .args(["sync", "--dev"])
                .current_dir(working_dir)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .await;
        }

        installed
    }

    /// Normalize a dependency command to its uv-first equivalent.
    ///
    /// Converts generic pip/pip3/python -m pip install commands to `uv add`,
    /// leaving already-correct uv commands and non-Python commands unchanged.
    fn normalize_command_to_uv(command: &str) -> String {
        let trimmed = command.trim();

        // pip install foo → uv add foo
        // pip3 install foo → uv add foo
        // python -m pip install foo → uv add foo
        // python3 -m pip install foo → uv add foo
        let pip_install_prefixes = [
            "pip install ",
            "pip3 install ",
            "python -m pip install ",
            "python3 -m pip install ",
        ];
        for prefix in &pip_install_prefixes {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let packages = rest.trim();
                if packages.is_empty() {
                    return command.to_string();
                }
                // Strip -r/--requirement flags (uv add doesn't support those directly)
                if packages.starts_with("-r ") || packages.starts_with("--requirement ") {
                    return format!("uv pip install {}", packages);
                }
                return format!("uv add {}", packages);
            }
        }

        // pip install -e . → uv pip install -e .
        if trimmed.starts_with("pip install -") || trimmed.starts_with("pip3 install -") {
            return format!("uv {}", trimmed);
        }

        command.to_string()
    }

    /// PSP-5 Phase 9: Finalize verification result — mark degraded, emit events, build summary.
    fn finalize_verification_result(
        &mut self,
        result: &mut perspt_core::types::VerificationResult,
        plugin_name: &str,
    ) {
        if result.has_degraded_stages() {
            result.degraded = true;
            let reasons = result.degraded_stage_reasons();
            result.degraded_reason = Some(reasons.join("; "));

            // Emit per-stage SensorFallback events
            for outcome in &result.stage_outcomes {
                if let perspt_core::types::SensorStatus::Fallback { actual, reason } =
                    &outcome.sensor_status
                {
                    self.emit_event(perspt_core::AgentEvent::SensorFallback {
                        node_id: plugin_name.to_string(),
                        stage: outcome.stage.clone(),
                        primary: reason.clone(),
                        actual: actual.clone(),
                        reason: reason.clone(),
                    });
                }
            }
        }

        // Store result for convergence-time degraded check
        self.last_verification_result = Some(result.clone());

        // Build summary
        result.summary = format!(
            "{}: syntax={}, build={}, tests={}, lint={}{}",
            plugin_name,
            if result.syntax_ok { "✅" } else { "❌" },
            if result.build_ok { "✅" } else { "❌" },
            if result.tests_ok { "✅" } else { "❌" },
            if result.lint_ok { "✅" } else { "⏭️" },
            if result.degraded { " (degraded)" } else { "" },
        );
    }

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

        if parents.is_empty() {
            return None; // Root node — no provisional branch needed
        }

        let node = &self.graph[idx];
        let node_id = node.node_id.clone();
        let session_id = self.context.session_id.clone();

        // Use the first parent as the primary dependency.
        // For multi-parent nodes the branch tracks the first parent;
        // lineage records capture all parent→child edges.
        let parent_idx = parents[0];
        let parent_node_id = self.graph[parent_idx].node_id.clone();

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

        // Record lineage edges for every parent
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
                // Also copy parent output targets that this node depends on
                for pidx in &parents {
                    for target in &self.graph[*pidx].output_targets {
                        if let Some(rel) = target.to_str() {
                            if let Err(e) = crate::tools::copy_to_sandbox(
                                &self.context.working_dir,
                                &sandbox_path,
                                rel,
                            ) {
                                log::debug!(
                                    "Could not seed sandbox with parent file {}: {}",
                                    rel,
                                    e
                                );
                            }
                        }
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

/// Convert diagnostic severity to string
fn severity_to_str(severity: Option<lsp_types::DiagnosticSeverity>) -> &'static str {
    match severity {
        Some(lsp_types::DiagnosticSeverity::ERROR) => "ERROR",
        Some(lsp_types::DiagnosticSeverity::WARNING) => "WARNING",
        Some(lsp_types::DiagnosticSeverity::INFORMATION) => "INFO",
        Some(lsp_types::DiagnosticSeverity::HINT) => "HINT",
        Some(_) => "OTHER",
        None => "UNKNOWN",
    }
}

/// PSP-5 Phase 9: Determine which verification stages to run based on NodeClass.
///
/// - **Interface**: SyntaxCheck only (signatures/schemas)
/// - **Implementation**: SyntaxCheck + Build (+ Test if weighted_tests non-empty)
/// - **Integration**: Full pipeline (SyntaxCheck + Build + Test + Lint)
fn verification_stages_for_node(node: &SRBNNode) -> Vec<perspt_core::plugin::VerifierStage> {
    use perspt_core::plugin::VerifierStage;
    match node.node_class {
        perspt_core::types::NodeClass::Interface => {
            vec![VerifierStage::SyntaxCheck]
        }
        perspt_core::types::NodeClass::Implementation => {
            let mut stages = vec![VerifierStage::SyntaxCheck, VerifierStage::Build];
            if !node.contract.weighted_tests.is_empty() {
                stages.push(VerifierStage::Test);
            }
            stages
        }
        perspt_core::types::NodeClass::Integration => {
            vec![
                VerifierStage::SyntaxCheck,
                VerifierStage::Build,
                VerifierStage::Test,
                VerifierStage::Lint,
            ]
        }
    }
}

/// Parse a persisted state string back into a NodeState enum
fn parse_node_state(s: &str) -> NodeState {
    match s {
        "TaskQueued" => NodeState::TaskQueued,
        "Planning" => NodeState::Planning,
        "Coding" => NodeState::Coding,
        "Verifying" => NodeState::Verifying,
        "Retry" => NodeState::Retry,
        "SheafCheck" => NodeState::SheafCheck,
        "Committing" => NodeState::Committing,
        "Escalated" => NodeState::Escalated,
        "Completed" | "COMPLETED" | "STABLE" => NodeState::Completed,
        "Failed" | "FAILED" => NodeState::Failed,
        "Aborted" | "ABORTED" => NodeState::Aborted,
        _ => NodeState::TaskQueued, // Default for unknown states
    }
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
        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_phase6"));
        orch.context.session_id = "test_session".into();
        let node = SRBNNode::new("root".into(), "root goal".into(), ModelTier::Actuator);
        orch.add_node(node);

        let idx = orch.node_indices["root"];
        // Root node has no parents — should not create a branch
        let branch = orch.maybe_create_provisional_branch(idx);
        assert!(branch.is_none());
        assert!(orch.graph[idx].provisional_branch_id.is_none());
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
    fn test_extract_commands_from_correction_includes_uv() {
        let response = r#"Here's the fix:
Commands:
```
uv add httpx
uv add --dev pytest
cargo add serde
pip install numpy
```
File: main.py
```python
import httpx
```"#;
        let commands = SRBNOrchestrator::extract_commands_from_correction(response);
        assert!(
            commands.contains(&"uv add httpx".to_string()),
            "{:?}",
            commands
        );
        assert!(
            commands.contains(&"cargo add serde".to_string()),
            "{:?}",
            commands
        );
        assert!(
            commands.contains(&"pip install numpy".to_string()),
            "{:?}",
            commands
        );
    }

    #[test]
    fn test_extract_all_code_blocks_multiple_files() {
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
        let blocks = orch.extract_all_code_blocks_from_response(content);
        assert_eq!(blocks.len(), 3, "Expected 3 blocks, got {:?}", blocks);
        assert_eq!(blocks[0].0, "src/etl_pipeline/core.py");
        assert_eq!(blocks[1].0, "src/etl_pipeline/validator.py");
        assert_eq!(blocks[2].0, "tests/test_core.py");
        assert!(!blocks[0].2, "core.py should not be a diff");
    }

    #[test]
    fn test_extract_all_code_blocks_single_file() {
        let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
        let content = r#"File: main.py
```python
print("hello")
```"#;
        let blocks = orch.extract_all_code_blocks_from_response(content);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, "main.py");
    }

    #[test]
    fn test_extract_all_code_blocks_mixed_file_and_diff() {
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
        let blocks = orch.extract_all_code_blocks_from_response(content);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].0, "new_module.py");
        assert!(!blocks[0].2, "new_module.py should be a write");
        assert_eq!(blocks[1].0, "existing.py");
        assert!(blocks[1].2, "existing.py should be a diff");
    }

    #[test]
    fn test_parse_artifact_bundle_legacy_multi_file() {
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
        let bundle = orch.parse_artifact_bundle(content);
        assert!(bundle.is_some(), "Should parse multi-file legacy response");
        let bundle = bundle.unwrap();
        assert_eq!(bundle.artifacts.len(), 2, "Should have 2 artifacts");
        assert_eq!(bundle.artifacts[0].path(), "core.py");
        assert_eq!(bundle.artifacts[1].path(), "utils.py");
    }

    // =========================================================================
    // Baseline regression tests — freeze pre-refactor behavior
    // =========================================================================

    #[test]
    fn test_parse_artifact_bundle_structured_json() {
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
        let bundle = orch.parse_artifact_bundle(content);
        assert!(bundle.is_some(), "Should parse structured JSON bundle");
        let bundle = bundle.unwrap();
        assert_eq!(bundle.artifacts.len(), 2);
        assert!(bundle.artifacts[0].is_write());
        assert_eq!(bundle.artifacts[0].path(), "src/main.py");
        assert!(bundle.artifacts[1].is_diff());
        assert_eq!(bundle.artifacts[1].path(), "src/lib.py");
        assert_eq!(bundle.commands, vec!["uv add requests"]);
    }

    #[test]
    fn test_parse_artifact_bundle_json_with_empty_path_falls_through() {
        let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
        // JSON bundle with empty path fails validation → falls through to legacy.
        // Legacy parser sees ```json block and maps it to "config.json".
        // Current behavior: not rejected — the raw JSON becomes config.json content.
        let content = r#"```json
{
  "artifacts": [
    {"operation": "write", "path": "", "content": "bad"}
  ],
  "commands": []
}
```"#;
        let bundle = orch.parse_artifact_bundle(content);
        assert!(
            bundle.is_some(),
            "Legacy fallback produces a config.json artifact"
        );
        let bundle = bundle.unwrap();
        assert_eq!(bundle.artifacts.len(), 1);
        assert_eq!(bundle.artifacts[0].path(), "config.json");
    }

    #[test]
    fn test_parse_artifact_bundle_json_absolute_path_falls_through() {
        let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
        // Absolute-path JSON bundle fails validation → falls through to legacy.
        // Legacy parser sees ```json block and maps it to "config.json".
        // Current behavior: the malicious path is NOT written to, but the
        // raw JSON is still emitted as config.json content.
        let content = r#"```json
{
  "artifacts": [
    {"operation": "write", "path": "/etc/passwd", "content": "bad"}
  ],
  "commands": []
}
```"#;
        let bundle = orch.parse_artifact_bundle(content);
        assert!(
            bundle.is_some(),
            "Legacy fallback produces a config.json artifact"
        );
        let bundle = bundle.unwrap();
        assert_eq!(bundle.artifacts.len(), 1);
        assert_eq!(bundle.artifacts[0].path(), "config.json");
    }

    #[test]
    fn test_parse_artifact_bundle_returns_none_for_garbage() {
        let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
        let content = "This is just a plain text response with no code blocks at all.";
        assert!(orch.parse_artifact_bundle(content).is_none());
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
        // maybe_create_provisional_branch should return None for roots
        let branch = orch.maybe_create_provisional_branch(root_idx);
        assert!(
            branch.is_none(),
            "Root node should not get a provisional branch"
        );

        // effective_working_dir falls back to workspace
        assert_eq!(orch.effective_working_dir(root_idx), temp_dir);

        // sandbox_dir should also be None
        assert!(orch.sandbox_dir_for_node(root_idx).is_none());

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
    fn test_parse_artifact_bundle_json_path_traversal_falls_through() {
        let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
        // Path-traversal JSON bundle fails validation → falls through to legacy.
        // Legacy parser sees ```json block and maps it to "config.json".
        // Current behavior: traversal path is NOT written to, but raw JSON
        // is emitted as config.json content.
        let content = r#"```json
{
  "artifacts": [
    {"operation": "write", "path": "../../../etc/shadow", "content": "bad"}
  ],
  "commands": []
}
```"#;
        let bundle = orch.parse_artifact_bundle(content);
        assert!(
            bundle.is_some(),
            "Legacy fallback produces a config.json artifact"
        );
        let bundle = bundle.unwrap();
        assert_eq!(bundle.artifacts.len(), 1);
        assert_eq!(bundle.artifacts[0].path(), "config.json");
    }
}
