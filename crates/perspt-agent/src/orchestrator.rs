//! SRBN Orchestrator
//!
//! Manages the Task DAG and orchestrates agent execution following the 7-step control loop.

use crate::agent::{ActuatorAgent, Agent, ArchitectAgent, SpeculatorAgent, VerifierAgent};
use crate::context_retriever::ContextRetriever;
use crate::lsp::LspClient;
use crate::test_runner::{self, PythonTestRunner, TestRunnerTrait};
use crate::tools::{AgentTools, ToolCall};
use crate::types::{AgentContext, EnergyComponents, ModelTier, NodeState, SRBNNode, TaskPlan};
use anyhow::{Context, Result};
use perspt_core::types::{
    EscalationCategory, EscalationReport, NodeClass, ProvisionalBranch, ProvisionalBranchState,
    RewriteAction, RewriteRecord, SheafValidationResult, SheafValidatorClass,
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
    /// PSP-5 Phase 4: Last plugin-driven verification result (for convergence checks)
    last_verification_result: Option<perspt_core::types::VerificationResult>,
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
        Self::new_with_models(working_dir, auto_approve, None, None, None, None)
    }

    /// Create a new orchestrator with custom model configuration
    pub fn new_with_models(
        working_dir: PathBuf,
        auto_approve: bool,
        architect_model: Option<String>,
        actuator_model: Option<String>,
        verifier_model: Option<String>,
        speculator_model: Option<String>,
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
            event_sender: None,
            action_receiver: None,
            ledger: crate::ledger::MerkleLedger::new().expect("Failed to create ledger"),
            last_tool_failure: None,
            last_context_provenance: None,
            last_verification_result: None,
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
    /// Returns ApprovalResult with optional edited value
    async fn await_approval(
        &mut self,
        action_type: perspt_core::ActionType,
        description: String,
        diff: Option<String>,
    ) -> ApprovalResult {
        // If auto_approve is enabled, skip approval
        if self.auto_approve {
            return ApprovalResult::Approved;
        }

        // If no TUI connected, default to approve (headless with --yes)
        if self.action_receiver.is_none() {
            return ApprovalResult::Approved;
        }

        // Generate unique request ID
        let request_id = uuid::Uuid::new_v4().to_string();

        // Emit approval request
        self.emit_event(perspt_core::AgentEvent::ApprovalRequest {
            request_id: request_id.clone(),
            node_id: "current".to_string(),
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
                        return ApprovalResult::Approved;
                    }
                    perspt_core::AgentAction::ApproveWithEdit {
                        request_id: rid,
                        edited_value,
                    } if rid == request_id => {
                        self.emit_log(format!("✓ Approved with edit: {}", edited_value));
                        return ApprovalResult::ApprovedWithEdit(edited_value);
                    }
                    perspt_core::AgentAction::Reject {
                        request_id: rid,
                        reason,
                    } if rid == request_id => {
                        let msg = reason.unwrap_or_else(|| "User rejected".to_string());
                        self.emit_log(format!("✗ Rejected: {}", msg));
                        return ApprovalResult::Rejected;
                    }
                    perspt_core::AgentAction::Abort => {
                        self.emit_log("⚠️ Session aborted by user");
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

        // PSP-5: Detect active plugins before project init
        let registry = perspt_core::plugin::PluginRegistry::new();
        let active_plugins: Vec<String> = registry
            .detect_all(&self.context.working_dir)
            .iter()
            .map(|p| p.name().to_string())
            .collect();
        self.context.active_plugins = active_plugins.clone();

        if !active_plugins.is_empty() {
            self.emit_log(format!(
                "🔌 Detected plugins: {}",
                active_plugins.join(", ")
            ));
        }

        // Team Mode: Full project initialization and DAG sheafification
        self.step_init_project(&task).await?;
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

            self.execute_node(*idx).await?;

            // Emit completed status
            if let Some(node) = self.graph.node_weight(*idx) {
                self.emit_event(perspt_core::AgentEvent::NodeCompleted {
                    node_id: node.node_id.clone(),
                    goal: node.goal.clone(),
                });
            }
        }

        log::info!("SRBN execution completed");

        self.emit_event(perspt_core::AgentEvent::Complete {
            success: true,
            message: format!("Completed {} nodes", total_nodes),
        });
        Ok(())
    }

    /// Step 0: Project Initialization
    /// Checks for existing project or initializes new one based on task
    async fn step_init_project(&mut self, task: &str) -> Result<()> {
        let registry = perspt_core::plugin::PluginRegistry::new();

        // 1. Check if project already exists
        if let Some(plugin) = registry.detect(&self.context.working_dir) {
            self.emit_log(format!("📂 Detected existing {} project", plugin.name()));

            // For existing projects, check if tooling sync is needed
            match plugin.check_tooling_action(&self.context.working_dir) {
                perspt_core::plugin::ProjectAction::ExecCommand {
                    command,
                    description,
                } => {
                    self.emit_log(format!("🔧 Tooling action needed: {}", description));

                    // Request approval for tooling sync
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
                    self.emit_log("✓ Project tooling is up to date");
                }
            }

            return Ok(());
        }

        // 2. If no project detected, heuristically detect language from task
        let plugin_name = self.detect_language_from_task(task);

        if let Some(name) = plugin_name {
            // Check if this task actually requires a full project workspace/scaffold
            if !self.check_workspace_requirement(task).await {
                self.emit_log("ℹ️ Simple task detected, skipping project scaffolding.");
                return Ok(());
            }

            if let Some(plugin) = registry.get(name) {
                self.emit_log(format!("🌱 Initializing new {} project", name));

                // Check if working directory is empty
                let is_empty = std::fs::read_dir(&self.context.working_dir)
                    .map(|mut i| i.next().is_none())
                    .unwrap_or(true);

                let project_name = if is_empty {
                    ".".to_string() // Init in current directory
                } else {
                    // Suggest a meaningful project name from the task
                    self.suggest_project_name(task).await
                };

                let opts = perspt_core::plugin::InitOptions {
                    name: project_name.clone(),
                    is_empty_dir: is_empty,
                    ..Default::default()
                };

                // Use the new get_init_action method
                match plugin.get_init_action(&opts) {
                    perspt_core::plugin::ProjectAction::ExecCommand {
                        command,
                        description,
                    } => {
                        // Request approval for init with editable project name
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

                        // Determine final project name (may be edited by user)
                        let final_name = match &result {
                            ApprovalResult::ApprovedWithEdit(edited) => edited.clone(),
                            _ => project_name.clone(),
                        };

                        if matches!(
                            result,
                            ApprovalResult::Approved | ApprovalResult::ApprovedWithEdit(_)
                        ) {
                            // Regenerate command if name was edited
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

                            // Run command
                            let mut args = HashMap::new();
                            args.insert("command".to_string(), final_command.clone());
                            let call = ToolCall {
                                name: "run_command".to_string(),
                                arguments: args,
                            };
                            let result = self.tools.execute(&call).await;
                            if result.success {
                                self.emit_log(format!("✅ Project '{}' initialized", final_name));
                            } else {
                                self.emit_log(format!("❌ Init failed: {:?}", result.error));
                            }
                        }
                    }
                    perspt_core::plugin::ProjectAction::NoAction => {
                        self.emit_log("ℹ️ No initialization action needed");
                    }
                }
            }
        } else {
            self.emit_log("ℹ️ No language detected, skipping project init");
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

    /// Check if the task requires a full project workspace/scaffold
    /// Uses heuristics and LLM decision making to avoid unnecessary initialization
    async fn check_workspace_requirement(&self, task: &str) -> bool {
        let task_lower = task.to_lowercase();

        // 1. Simple heuristics: keywords that strongly suggest a single file
        let single_file_keywords = [
            "script",
            "single file",
            "snippet",
            "just a file",
            "one file",
            "standalone",
        ];
        if single_file_keywords.iter().any(|&k| task_lower.contains(k)) {
            return false;
        }

        // 2. Short tasks are often single files
        if task.len() < 50 && !task_lower.contains("project") && !task_lower.contains("app") {
            return false;
        }

        // 3. Fallback to LLM for a binary decision
        let prompt = format!(
            r#"Analyze this task and decide if it requires a full project workspace/scaffold (e.g., uv init, cargo init, npm init) or if it can be done as a simple standalone file.
- Full workspace is needed for multi-file projects, web servers, complex apps with dependencies.
- Simple standalone file is better for scripts, utility functions, or single-logic snippets.

Task: "{}"

Respond with ONLY 'WORKSPACE' or 'STANDALONE'."#,
            task
        );

        match self
            .call_llm_with_logging(&self.architect_model.clone(), &prompt, None)
            .await
        {
            Ok(response) => {
                let decision = response.trim().to_uppercase();
                decision.contains("WORKSPACE")
            }
            Err(e) => {
                log::warn!(
                    "Failed to get workspace decision from LLM: {}, defaulting to WORKSPACE",
                    e
                );
                true // Default to safety
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

            // Call the Architect
            let response = self
                .call_llm_with_logging(&self.get_architect_model(), &prompt, None)
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
    fn build_architect_prompt(&self, task: &str, last_error: Option<&str>) -> Result<String> {
        let error_feedback = if let Some(e) = last_error {
            format!(
                "\n## Previous Attempt Failed\nError: {}\nPlease fix the JSON format and try again.\n",
                e
            )
        } else {
            String::new()
        };

        // Gather existing project context
        let project_context = self.gather_project_context();

        let prompt = format!(
            r#"You are an Architect agent in a multi-agent coding system.

## Task
{task}

## Working Directory
{working_dir}

## Existing Project Structure
{project_context}
{error_feedback}
## Instructions
Analyze this task and produce a structured execution plan as JSON.

### MODULAR PROJECT STRUCTURE (CRITICAL)
Your plan MUST create a COMPLETE, RUNNABLE project with proper modularity:

1. **Entry Point First**: Create a main entry point file (e.g., `main.py`, `src/main.rs`, `index.js`)
2. **Logical Modules**: Split functionality into separate files/modules with clear responsibilities
3. **Proper Imports**: Ensure all cross-file imports will resolve correctly
4. **Package Structure**: For Python, include `__init__.py` files in subdirectories
5. **Test Isolation**: Put tests in a `tests/` directory or use `test_*.py` naming

### TASK ORDERING
1. Create foundational modules before dependent ones
2. Specify dependencies accurately between tasks
3. Entry point task should depend on all modules it imports

### COMPLETENESS CHECKLIST
- [ ] Every import in generated code must reference an existing or planned file
- [ ] The project must be immediately runnable after all tasks complete
- [ ] Include at least one test file for core functionality
- [ ] All functions must have type hints (Python) or type annotations (Rust/TS)

## CRITICAL CONSTRAINTS
- DO NOT create `pyproject.toml`, `requirements.txt`, `package.json`, `Cargo.toml`, or any project configuration files
- The system handles project initialization separately via CLI tools (uv, npm, cargo)
- Focus ONLY on source code files (.py, .js, .rs, etc.) and test files
- If you need to add dependencies, include them in the task goal description (e.g., "Add requests library for HTTP calls")

## Output Format
Respond with ONLY a JSON object in this exact format:
```json
{{
  "tasks": [
    {{
      "id": "task_1",
      "goal": "Description of what this task accomplishes",
      "context_files": ["existing_file.py"],
      "output_files": ["new_file.py"],
      "dependencies": [],
      "task_type": "code",
      "contract": {{
        "interface_signature": "def function_name(arg: Type) -> ReturnType",
        "invariants": ["Must handle edge cases"],
        "forbidden_patterns": ["no bare except"],
        "tests": [
          {{"name": "test_function_name", "criticality": "Critical"}}
        ]
      }}
    }},
    {{
      "id": "main_entry",
      "goal": "Create main.py entry point that imports and uses other modules",
      "context_files": ["module_a.py", "module_b.py"],
      "output_files": ["main.py"],
      "dependencies": ["task_1", "task_2"],
      "task_type": "code"
    }},
    {{
      "id": "test_1",
      "goal": "Unit tests for task_1",
      "context_files": ["new_file.py"],
      "output_files": ["tests/test_new_file.py"],
      "dependencies": ["task_1"],
      "task_type": "unit_test"
    }}
  ]
}}
```

Valid task_type values: "code", "unit_test", "integration_test", "refactor", "documentation"
Valid criticality values: "Critical", "High", "Low"

IMPORTANT: Output ONLY the JSON, no other text."#,
            task = task,
            working_dir = self.context.working_dir.display(),
            project_context = project_context,
            error_feedback = error_feedback
        );

        Ok(prompt)
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
    fn build_ownership_manifest_from_plan(&mut self, plan: &TaskPlan) {
        let registry = perspt_core::plugin::PluginRegistry::new();

        for task in &plan.tasks {
            // Detect the plugin for this task's output files
            let plugin_name = task
                .output_files
                .first()
                .and_then(|f| {
                    registry
                        .all()
                        .iter()
                        .find(|p| p.owns_file(f))
                        .map(|p| p.name().to_string())
                })
                .unwrap_or_else(|| "unknown".to_string());

            // Set owner_plugin on the node if we can find it
            if let Some(idx) = self.node_indices.get(&task.id) {
                self.graph[*idx].owner_plugin = plugin_name.clone();
            }

            // Register each output file in the manifest
            for file in &task.output_files {
                self.context.ownership_manifest.assign(
                    file.clone(),
                    task.id.clone(),
                    plugin_name.clone(),
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

        // Record provenance for later commit
        self.last_context_provenance = Some(context_package.provenance());

        let actuator = &self.agents[1];
        let node = &self.graph[idx];
        let node_id = node.node_id.clone();

        // Build prompt enriched with context package
        let base_prompt = actuator.build_prompt(node, &self.context);
        let prompt = if formatted_context.is_empty() {
            base_prompt
        } else {
            format!(
                "{}\n\n## Node Context (PSP-5 Restriction Map)\n\n{}",
                base_prompt, formatted_context
            )
        };
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
            let approval_result = self
                .await_approval(
                    perspt_core::ActionType::Command {
                        command: command.clone(),
                    },
                    format!("Execute shell command: {}", command),
                    None,
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
        // Then check for file code blocks (for TaskType::Code)
        else if let Some((filename, code, is_diff)) = self.extract_code_from_response(content) {
            log::info!("Extracted code for file: {} (diff={})", filename, is_diff);
            self.emit_log(format!(
                "📝 File proposed: {} (diff: {})",
                filename, is_diff
            ));

            // Build full path
            let full_path = self.context.working_dir.join(&filename);

            // Request approval before writing file
            let approval_result = self
                .await_approval(
                    perspt_core::ActionType::FileWrite {
                        path: filename.clone(),
                    },
                    format!("Write file: {}", filename),
                    Some(code.clone()),
                )
                .await;

            if !matches!(
                approval_result,
                ApprovalResult::Approved | ApprovalResult::ApprovedWithEdit(_)
            ) {
                self.emit_log("⏭️ File write skipped (not approved)");
                return Ok(());
            }

            let mut args = HashMap::new();
            args.insert("path".to_string(), filename.clone());

            let call = if is_diff {
                args.insert("diff".to_string(), code.clone());
                ToolCall {
                    name: "apply_diff".to_string(),
                    arguments: args,
                }
            } else {
                args.insert("content".to_string(), code.clone());
                ToolCall {
                    name: "write_file".to_string(), // Previously alias for apply_patch (fs::write)
                    arguments: args,
                }
            };

            let result = self.tools.execute(&call).await;
            if result.success {
                log::info!("✓ Applied changes to: {}", filename);
                self.emit_log(format!("✅ Applied changes to: {}", filename));
                self.last_tool_failure = None; // Reset error

                // Track the written file for LSP verification
                self.last_written_file = Some(full_path.clone());
                self.file_version += 1;

                // Notify LSP of file change (if LSP is running)
                let lsp_key = self.lsp_key_for_file(&full_path.to_string_lossy());
                if let Some(client) = lsp_key.and_then(|k| self.lsp_clients.get_mut(&k)) {
                    if self.file_version == 1 {
                        let _ = client.did_open(&full_path, &code).await;
                        if is_diff {
                            if let Ok(new_content) = std::fs::read_to_string(&full_path) {
                                let _ = client
                                    .did_change(&full_path, &new_content, self.file_version)
                                    .await;
                            }
                        } else {
                            let _ = client.did_open(&full_path, &code).await;
                        }
                    } else {
                        // For diff, read back file
                        if is_diff {
                            if let Ok(new_content) = std::fs::read_to_string(&full_path) {
                                let _ = client
                                    .did_change(&full_path, &new_content, self.file_version)
                                    .await;
                            }
                        } else {
                            let _ = client
                                .did_change(&full_path, &code, self.file_version)
                                .await;
                        }
                    }
                }
            } else {
                log::warn!("Failed to apply changes: {:?}", result.error);
                self.emit_log(format!("❌ Failed: {:?}", result.error));
                self.last_tool_failure = result.error.clone();
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
        // Look for patterns like:
        // File: hello_world.py
        // ```python
        // def hello():
        //     print("Hello World")
        // ```

        let lines: Vec<&str> = content.lines().collect();
        let mut file_path: Option<String> = None;
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
                // If we found code, return it
                if !code_lines.is_empty() {
                    let code = code_lines.join("\n");
                    // Generate filename from language if not found
                    let filename = file_path
                        .clone()
                        .unwrap_or_else(|| match code_lang.as_str() {
                            "python" | "py" => "main.py".to_string(),
                            "rust" | "rs" => "main.rs".to_string(),
                            _ => "output.txt".to_string(),
                        });
                    // Check if it's a diff
                    let is_diff = code_lang == "diff"
                        || code.starts_with("---")
                        || file_path
                            .as_ref()
                            .map(|_| content.contains("Diff:"))
                            .unwrap_or(false);
                    return Some((filename, code, is_diff));
                }
                continue;
            }

            if in_code_block {
                code_lines.push(line);
            }
        }

        None
    }

    /// Step 4: Stability Verification
    ///
    /// Computes Lyapunov Energy V(x) from LSP diagnostics, contracts, and tests
    async fn step_verify(&mut self, idx: NodeIndex) -> Result<EnergyComponents> {
        log::info!("Step 4: Verification - Computing stability energy");

        self.graph[idx].state = NodeState::Verifying;

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

            // V_log: Run tests and calculate logic energy
            // Skip tests if defer_tests is enabled (will run at sheaf validation)
            let node = &self.graph[idx];
            if self.context.defer_tests {
                self.emit_log("⏭️ Tests deferred (--defer-tests enabled)".to_string());
            } else if !node.contract.weighted_tests.is_empty() {
                // PSP-5 Phase 4: select runner by the node's owner plugin
                let plugin_name = &node.owner_plugin;
                let registry = perspt_core::plugin::PluginRegistry::new();
                let runner: Box<dyn TestRunnerTrait> = if let Some(plugin) =
                    registry.get(plugin_name)
                {
                    let profile = plugin.verifier_profile();
                    self.emit_log(format!("🧪 Running tests via {} plugin...", plugin.name()));
                    test_runner::test_runner_for_profile(profile, self.context.working_dir.clone())
                } else {
                    self.emit_log("🧪 Running tests (fallback Python runner)...".to_string());
                    Box::new(PythonTestRunner::new(self.context.working_dir.clone()))
                };

                match runner.run_tests().await {
                    Ok(results) => {
                        // V_log calculation still uses PythonTestRunner's method for
                        // weighted-test matching. This is language-agnostic since it
                        // only inspects failure names vs. contract weighted_tests.
                        let py_runner = PythonTestRunner::new(self.context.working_dir.clone());
                        energy.v_log = py_runner.calculate_v_log(&results, &node.contract);

                        if results.all_passed() {
                            self.emit_log(format!(
                                "✅ Tests passed: {}/{}",
                                results.passed, results.total
                            ));
                        } else {
                            self.emit_log(format!(
                                "❌ Tests failed: {} passed, {} failed",
                                results.passed, results.failed
                            ));

                            // Store test failures for correction prompt
                            for failure in &results.failures {
                                self.emit_log(format!(
                                    "   - {} in {:?}: {}",
                                    failure.name, failure.file, failure.message
                                ));
                            }

                            // Store test output in context for correction prompt
                            self.context.last_test_output = Some(results.output.clone());
                        }

                        log::info!(
                            "Test results: {}/{} passed, V_log={:.2}",
                            results.passed,
                            results.total,
                            energy.v_log
                        );
                    }
                    Err(e) => {
                        log::warn!("Failed to run tests: {}", e);
                        self.emit_log(format!("⚠️ Test execution failed: {}", e));
                        // Don't fail the verification, just log the error
                    }
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

        // Re-verify (recursive correction loop)
        let new_energy = self.step_verify(idx).await?;
        Box::pin(self.step_converge(idx, new_energy)).await
    }

    /// Build a correction prompt with diagnostic details
    fn build_correction_prompt(
        &self,
        _node_id: &str,
        goal: &str,
        energy: &EnergyComponents,
    ) -> Result<String> {
        let diagnostics = &self.context.last_diagnostics;

        // Read current code
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
            .unwrap_or_else(|| "main.py".to_string());

        let mut prompt = format!(
            r#"## Code Correction Required

The code you generated has {} error(s) detected by the Python type checker.
Your task is to fix ALL errors and return the complete corrected file.

### Original Goal
{}

### Current Code (with errors)
File: {}
```python
{}
```

### Detected Errors (V_syn = {:.2})
"#,
            diagnostics.len(),
            goal,
            file_path,
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

        prompt.push_str(
            r#"
### Fix Requirements
1. Fix ALL errors listed above - do not leave any unfixed
2. Maintain the original functionality and goal
3. Add proper type annotations if missing
4. Import any missing modules
5. Return the COMPLETE corrected file, not just snippets

### Output Format
Provide the complete corrected file:

File: [same filename]
```python
[complete corrected code]
```
"#,
        );

        Ok(prompt)
    }

    /// Map diagnostic message patterns to specific fix directions
    fn get_fix_direction(&self, diag: &lsp_types::Diagnostic) -> String {
        let msg = diag.message.to_lowercase();

        if msg.contains("undefined") || msg.contains("unresolved") || msg.contains("not defined") {
            "Define the missing variable/function, or import it from the correct module".into()
        } else if msg.contains("type") && (msg.contains("expected") || msg.contains("incompatible"))
        {
            "Change the value or add a type conversion to match the expected type".into()
        } else if msg.contains("import") || msg.contains("no module named") {
            "Add the correct import statement at the top of the file".into()
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

    /// Call LLM for code correction (uses stored provider with exponential backoff retry)
    async fn call_llm_for_correction(&self, prompt: &str) -> Result<String> {
        log::debug!(
            "Sending correction request to LLM model: {}",
            self.actuator_model
        );
        let response = self
            .call_llm_with_logging(&self.actuator_model.clone(), prompt, None)
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

    /// Step 6: Sheaf Validation
    async fn step_sheaf_validate(&mut self, idx: NodeIndex) -> Result<()> {
        log::info!("Step 6: Sheaf Validation - Cross-node consistency check");

        self.graph[idx].state = NodeState::SheafCheck;

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
            if !vr.tests_ok {
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

        // PSP-5 Phase 3: Record context provenance if available
        if let Some(provenance) = self.last_context_provenance.take() {
            if let Err(e) = self.ledger.record_context_provenance(&provenance) {
                log::warn!("Failed to record context provenance: {}", e);
            }
        }

        // PSP-5 Phase 6: Emit interface seals for Interface-class nodes
        self.emit_interface_seals(idx);

        self.graph[idx].state = NodeState::Completed;

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

        // Persist the rewrite record before applying
        let category = self.classify_non_convergence(idx);
        let rewrite = RewriteRecord {
            node_id: node_id.clone(),
            session_id: self.context.session_id.clone(),
            action: action.clone(),
            category,
            requeued_nodes: Vec::new(),
            inserted_nodes: Vec::new(),
            timestamp: epoch_seconds(),
        };
        if let Err(e) = self.ledger.record_rewrite(&rewrite) {
            log::warn!("Failed to persist rewrite record: {}", e);
        }

        match action {
            RewriteAction::DegradedValidationStop { reason } => {
                self.emit_log(format!("⛔ Degraded-validation stop: {}", reason));
                self.graph[idx].state = NodeState::Escalated;
                // This is a deliberate stop, not a silent false-stable — mark
                // it as escalated so the user can decide how to proceed.
                false
            }
            RewriteAction::UserEscalation { evidence } => {
                self.emit_log(format!("⚠️ User escalation required: {}", evidence));
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
                // Reset the node state so the caller's loop can re-execute
                self.graph[idx].state = NodeState::Retry;
                true
            }
            RewriteAction::ContractRepair { fields } => {
                log::info!("Contract repair for node {}: fields {:?}", node_id, fields);
                self.emit_log(format!(
                    "🔧 Contract repair for {}: {}",
                    node_id,
                    fields.join(", ")
                ));
                // Mark for retry — the next iteration will use the adjusted contract
                self.graph[idx].state = NodeState::Retry;
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
                true
            }
            RewriteAction::SensorRecovery { degraded_stages } => {
                log::info!(
                    "Sensor recovery for node {}: {:?}",
                    node_id,
                    degraded_stages
                );
                self.emit_log(format!("🔧 Attempting sensor recovery for {}", node_id));
                // For now, just mark for retry — actual recovery would
                // re-probe the plugin verifier profile.
                self.graph[idx].state = NodeState::Retry;
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
                let applied = self.split_node(idx, proposed_children);
                if applied {
                    self.emit_event(perspt_core::AgentEvent::GraphRewriteApplied {
                        trigger_node: node_id.clone(),
                        action: "node_split".to_string(),
                        nodes_affected: count,
                    });
                }
                applied
            }
            RewriteAction::InterfaceInsertion { boundary } => {
                log::info!("Interface insertion for {}: {}", node_id, boundary);
                self.emit_log(format!(
                    "📐 Inserting interface adapter at boundary: {}",
                    boundary
                ));
                let applied = self.insert_interface_node(idx, boundary);
                if applied {
                    self.emit_event(perspt_core::AgentEvent::GraphRewriteApplied {
                        trigger_node: node_id.clone(),
                        action: "interface_insertion".to_string(),
                        nodes_affected: 1,
                    });
                }
                applied
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
                    self.emit_event(perspt_core::AgentEvent::GraphRewriteApplied {
                        trigger_node: node_id.clone(),
                        action: "subgraph_replan".to_string(),
                        nodes_affected: count + 1, // trigger + affected
                    });
                }
                applied
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
    /// Returns `true` if the split was applied successfully.
    fn split_node(&mut self, idx: NodeIndex, proposed_children: &[String]) -> bool {
        if proposed_children.is_empty() {
            return false;
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
            self.node_indices.insert(child_id, c_idx);
            child_indices.push(c_idx);
        }

        // Wire incoming edges → first child, outgoing edges from last child.
        if let Some(&first) = child_indices.first() {
            for (src, dep) in &incoming {
                self.graph.add_edge(*src, first, dep.clone());
            }
        }
        if let Some(&last) = child_indices.last() {
            for (dst, dep) in &outgoing {
                self.graph.add_edge(last, *dst, dep.clone());
            }
        }

        // Chain children sequentially.
        for pair in child_indices.windows(2) {
            self.graph.add_edge(
                pair[0],
                pair[1],
                Dependency {
                    kind: "split_sequence".to_string(),
                },
            );
        }

        // Remove original node.
        self.node_indices.remove(&parent_id);
        self.graph.remove_node(idx);

        log::info!(
            "Split node {} into {} children",
            parent_id,
            proposed_children.len()
        );
        true
    }

    /// Insert an interface/adapter node on the edge between the given node
    /// and its dependents.  The boundary string describes the interface
    /// contract for the newly created adapter node.
    /// Returns `true` if the insertion succeeded.
    fn insert_interface_node(&mut self, idx: NodeIndex, boundary: &str) -> bool {
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
        // We remove edges by finding edge indices.
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

        // adapter → original dependents
        for (dst, dep) in outgoing {
            self.graph.add_edge(adapter_idx, dst, dep);
        }

        log::info!("Inserted interface node {} after {}", adapter_id, source_id);
        true
    }

    /// Reset the specified affected nodes back to `TaskQueued` so they get
    /// re-executed.  The triggering node itself is also reset.  Returns `true`
    /// if at least one node was replanned.
    fn replan_subgraph(&mut self, trigger_idx: NodeIndex, affected_nodes: &[String]) -> bool {
        let mut replanned = 0;

        // Reset the trigger node itself.
        self.graph[trigger_idx].state = NodeState::Retry;
        self.graph[trigger_idx].monitor.reset_for_replan();
        replanned += 1;

        // Reset each referenced affected node.
        for nid in affected_nodes {
            if let Some(&nidx) = self.node_indices.get(nid.as_str()) {
                self.graph[nidx].state = NodeState::TaskQueued;
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

        // Fall back to legacy File:/Diff: extraction
        if let Some((filename, code, is_diff)) = self.extract_code_from_response(content) {
            let op = if is_diff {
                perspt_core::types::ArtifactOperation::Diff {
                    path: filename,
                    patch: code,
                }
            } else {
                perspt_core::types::ArtifactOperation::Write {
                    path: filename,
                    content: code,
                }
            };
            let bundle = perspt_core::types::ArtifactBundle {
                artifacts: vec![op],
                commands: vec![],
            };
            log::info!("Constructed single-artifact bundle from legacy extraction");
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
        // Validate structural integrity first
        bundle.validate().map_err(|e| anyhow::anyhow!(e))?;

        // PSP-5 Phase 2: Validate ownership boundaries
        self.context
            .ownership_manifest
            .validate_bundle(bundle, node_id, node_class)
            .map_err(|e| anyhow::anyhow!("Ownership validation failed: {}", e))?;

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
            args.insert("path".to_string(), op.path().to_string());

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
                let full_path = self.context.working_dir.join(op.path());

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
            bundle,
            node_id,
            &owner_plugin,
            node_class,
        );

        // Emit BundleApplied event
        self.emit_event(perspt_core::AgentEvent::BundleApplied {
            node_id: node_id.to_string(),
            files_created,
            files_modified,
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

        let working_dir = self.context.working_dir.clone();
        let runner = test_runner::test_runner_for_profile(profile, working_dir);

        let mut result = perspt_core::types::VerificationResult::default();

        // Syntax check
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

        // Build check
        match runner.run_build_check().await {
            Ok(r) => {
                result.build_ok = r.passed > 0 && r.failed == 0;
                if result.build_ok {
                    self.emit_log("✅ Build passed".to_string());
                } else if r.run_succeeded {
                    self.emit_log("⚠️ Build failed".to_string());
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

        // Tests
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

        // Lint (only in Strict mode)
        if self.context.verifier_strictness == perspt_core::types::VerifierStrictness::Strict {
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
        } else {
            result.lint_ok = true; // Skip lint in non-strict mode
        }

        // Mark overall degraded if any stage used fallback or unavailable sensor
        if result.has_degraded_stages() {
            result.degraded = true;
            let reasons = result.degraded_stage_reasons();
            result.degraded_reason = Some(reasons.join("; "));

            // Emit per-stage SensorFallback events
            for outcome in &result.stage_outcomes {
                if let SensorStatus::Fallback { actual, reason } = &outcome.sensor_status {
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

        result
    }

    // =========================================================================
    // PSP-5 Phase 6: Provisional Branch Lifecycle
    // =========================================================================

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
            let depends_on_seal =
                self.graph[*pidx].node_class == NodeClass::Interface;
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
                seal_hash: seal_hash.iter().map(|b| format!("{:02x}", b)).collect::<String>(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_orchestrator_creation() {
        let orch = SRBNOrchestrator::new(PathBuf::from("."), false);
        assert_eq!(orch.node_count(), 0);
    }

    #[tokio::test]
    async fn test_add_nodes() {
        let mut orch = SRBNOrchestrator::new(PathBuf::from("."), false);

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
    async fn test_check_workspace_requirement() {
        let orch = SRBNOrchestrator::new(PathBuf::from("."), false);

        // Positive heuristics
        assert!(
            !orch
                .check_workspace_requirement("write a python script")
                .await
        );
        assert!(!orch.check_workspace_requirement("simple script").await);
        assert!(!orch.check_workspace_requirement("standalone file").await);

        // Negative heuristics (length or project keywords)
        // Note: For long strings without keywords, it would fall back to LLM which would fail in test
        // but the current implementation logs warning and returns true.
        // We'll test things that are definitely short and don't match or long but definitely project.

        assert!(!orch.check_workspace_requirement("calc sum").await); // Short, no project keywords -> STANDALONE
    }

    #[tokio::test]
    async fn test_lsp_key_for_file_resolves_by_plugin() {
        let mut orch = SRBNOrchestrator::new(PathBuf::from("."), false);
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
        let mut orch = SRBNOrchestrator::new(PathBuf::from("."), false);
        let mut node = SRBNNode::new("parent".into(), "Do everything".into(), ModelTier::Actuator);
        node.output_targets = vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")];
        orch.add_node(node);

        let idx = orch.node_indices["parent"];
        let applied = orch.split_node(idx, &["handle a.rs".into(), "handle b.rs".into()]);
        assert!(applied);
        // Parent should be gone
        assert!(!orch.node_indices.contains_key("parent"));
        // Two children should exist
        assert!(orch.node_indices.contains_key("parent__split_0"));
        assert!(orch.node_indices.contains_key("parent__split_1"));
    }

    #[tokio::test]
    async fn test_split_node_empty_children_is_noop() {
        let mut orch = SRBNOrchestrator::new(PathBuf::from("."), false);
        let node = SRBNNode::new("n".into(), "g".into(), ModelTier::Actuator);
        orch.add_node(node);
        let idx = orch.node_indices["n"];
        let applied = orch.split_node(idx, &[]);
        // Should not apply — return false but not panic
        assert!(!applied);
    }

    #[tokio::test]
    async fn test_insert_interface_node() {
        let mut orch = SRBNOrchestrator::new(PathBuf::from("."), false);
        let n1 = SRBNNode::new("a".into(), "source".into(), ModelTier::Actuator);
        let n2 = SRBNNode::new("b".into(), "dest".into(), ModelTier::Actuator);
        orch.add_node(n1);
        orch.add_node(n2);
        orch.add_dependency("a", "b", "data_flow").unwrap();

        let idx_a = orch.node_indices["a"];
        let applied = orch.insert_interface_node(idx_a, "API boundary");
        assert!(applied);
        assert!(orch.node_indices.contains_key("a__iface"));
        // Should now have 3 nodes
        assert_eq!(orch.node_count(), 3);
    }

    #[tokio::test]
    async fn test_replan_subgraph_resets_nodes() {
        let mut orch = SRBNOrchestrator::new(PathBuf::from("."), false);
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
        let mut orch = SRBNOrchestrator::new(PathBuf::from("."), false);
        let node = SRBNNode::new("n".into(), "g".into(), ModelTier::Actuator);
        orch.add_node(node);
        let idx = orch.node_indices["n"];

        let validators = orch.select_validators(idx);
        assert!(validators.contains(&SheafValidatorClass::DependencyGraphConsistency));
    }

    #[tokio::test]
    async fn test_select_validators_interface_node() {
        let mut orch = SRBNOrchestrator::new(PathBuf::from("."), false);
        let mut node = SRBNNode::new("iface".into(), "g".into(), ModelTier::Actuator);
        node.node_class = perspt_core::types::NodeClass::Interface;
        orch.add_node(node);
        let idx = orch.node_indices["iface"];

        let validators = orch.select_validators(idx);
        assert!(validators.contains(&SheafValidatorClass::ExportImportConsistency));
    }

    #[tokio::test]
    async fn test_run_sheaf_validator_dependency_graph_no_cycles() {
        let mut orch = SRBNOrchestrator::new(PathBuf::from("."), false);
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
        let mut orch = SRBNOrchestrator::new(PathBuf::from("."), false);
        let node = SRBNNode::new("n".into(), "g".into(), ModelTier::Actuator);
        orch.add_node(node);
        let idx = orch.node_indices["n"];

        // With no verification results or policy failures, should default to ImplementationError
        let category = orch.classify_non_convergence(idx);
        assert_eq!(category, EscalationCategory::ImplementationError);
    }

    #[tokio::test]
    async fn test_affected_dependents() {
        let mut orch = SRBNOrchestrator::new(PathBuf::from("."), false);
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
}
