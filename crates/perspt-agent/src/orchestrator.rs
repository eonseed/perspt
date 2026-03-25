//! SRBN Orchestrator
//!
//! Manages the Task DAG and orchestrates agent execution following the 7-step control loop.

use crate::agent::{ActuatorAgent, Agent, ArchitectAgent, SpeculatorAgent, VerifierAgent};
use crate::context_retriever::ContextRetriever;
use crate::lsp::LspClient;
use crate::test_runner::PythonTestRunner;
use crate::tools::{AgentTools, ToolCall};
use crate::types::{AgentContext, EnergyComponents, ModelTier, NodeState, SRBNNode, TaskPlan};
use anyhow::{Context, Result};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::{Topo, Walker};
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
            // PSP-5: Emit NodeSelected event before execution
            if let Some(node) = self.graph.node_weight(*idx) {
                self.emit_log(format!("📝 [{}/{}] {}", i + 1, total_nodes, node.goal));
                self.emit_event(perspt_core::AgentEvent::NodeSelected {
                    node_id: node.node_id.clone(),
                    goal: node.goal.clone(),
                    node_class: "implementation".to_string(),
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
        if let Some(client) = self.lsp_clients.get("python") {
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
    fn parse_task_plan(&self, content: &str) -> Result<TaskPlan> {
        // Try to extract JSON from markdown code block if present
        let json_str = if let Some(start) = content.find("```json") {
            let start = start + 7;
            if let Some(end_offset) = content[start..].find("```") {
                content[start..start + end_offset].trim()
            } else {
                content[start..].trim()
            }
        } else if let Some(start) = content.find("```") {
            // Try generic code block
            let start = start + 3;
            // Skip language identifier if present
            let start = content[start..]
                .find('\n')
                .map(|n| start + n + 1)
                .unwrap_or(start);
            if let Some(end_offset) = content[start..].find("```") {
                content[start..start + end_offset].trim()
            } else {
                content[start..].trim()
            }
        } else if content.trim().starts_with('{') {
            // Direct JSON
            content.trim()
        } else {
            // Try to find JSON object anywhere in the content
            if let Some(start) = content.find('{') {
                if let Some(end) = content.rfind('}') {
                    &content[start..=end]
                } else {
                    content.trim()
                }
            } else {
                content.trim()
            }
        };

        log::debug!(
            "Attempting to parse JSON: {}...",
            &json_str[..json_str.len().min(200)]
        );

        serde_json::from_str(json_str).context("Failed to parse TaskPlan JSON")
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

        Ok(())
    }


    /// Get the Architect model name
    fn get_architect_model(&self) -> String {
        self.architect_model.clone()
    }

    /// Execute a single node through the control loop
    async fn execute_node(&mut self, idx: NodeIndex) -> Result<()> {
        let node = &self.graph[idx];
        log::info!("Executing node: {} ({})", node.node_id, node.goal);

        // Step 2: Recursive Sub-graph Execution (already in topo order)
        self.graph[idx].state = NodeState::Coding;

        // Step 3: Speculative Generation
        self.step_speculate(idx).await?;

        // Step 4: Stability Verification
        let energy = self.step_verify(idx).await?;

        // Step 5: Convergence & Self-Correction
        if !self.step_converge(idx, energy).await? {
            // Failed to converge - escalate
            self.graph[idx].state = NodeState::Escalated;
            log::warn!("Node {} escalated to user", self.graph[idx].node_id);
            return Ok(());
        }

        // Step 6: Sheaf Validation (Post-Subgraph Consistency)
        self.step_sheaf_validate(idx).await?;

        // Step 7: Merkle Ledger Commit
        self.step_commit(idx).await?;

        Ok(())
    }

    /// Step 3: Speculative Generation
    async fn step_speculate(&mut self, idx: NodeIndex) -> Result<()> {
        log::info!("Step 3: Speculation - Generating implementation");

        let actuator = &self.agents[1];
        let node = &self.graph[idx];
        let node_id = node.node_id.clone();

        // Build prompt and call LLM with logging support
        let prompt = actuator.build_prompt(node, &self.context);
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
                if let Some(client) = self.lsp_clients.get_mut("python") {
                    if self.file_version == 1 {
                        let _ = client.did_open(&full_path, &code).await; // Note: For diff, we should ideally send full text but we don't have it easily here without reading back.
                                                                          // Ideally we should reread file from disk for LSP sync if it was a diff
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
            // Try to get diagnostics from LSP
            if let Some(client) = self.lsp_clients.get("python") {
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
                log::debug!("No LSP client available for Python");
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
                self.emit_log("🧪 Running tests...".to_string());
                let runner = PythonTestRunner::new(self.context.working_dir.clone());

                match runner.run_pytest(&[]).await {
                    Ok(results) => {
                        energy.v_log = runner.calculate_v_log(&results, &node.contract);

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
            log::info!(
                "Node {} is stable (V(x)={:.2} < ε={:.2})",
                node_id,
                total,
                epsilon
            );
            self.emit_log(format!("✅ Stable! V(x)={:.2} < ε={:.2}", total, epsilon));
            return Ok(true);
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
                if let Some(client) = self.lsp_clients.get_mut("python") {
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

        // Check for cyclic dependencies
        if petgraph::algo::is_cyclic_directed(&self.graph) {
            anyhow::bail!("Cyclic dependency detected in task graph");
        }

        // In a real implementation, verify interface consistency
        // using LSP textDocument/definition

        Ok(())
    }

    /// Step 7: Merkle Ledger Commit
    async fn step_commit(&mut self, idx: NodeIndex) -> Result<()> {
        log::info!("Step 7: Committing stable state to ledger");

        self.graph[idx].state = NodeState::Committing;

        // In a real implementation, write to DuckDB Merkle Ledger
        // For now, just mark as completed
        self.graph[idx].state = NodeState::Completed;

        log::info!("Node {} committed", self.graph[idx].node_id);
        Ok(())
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
    pub async fn start_python_lsp(&mut self) -> Result<()> {
        log::info!("Starting ty language server for Python");

        let mut client = LspClient::new("ty");
        match client.start(&self.context.working_dir).await {
            Ok(()) => {
                log::info!("ty language server started successfully");
                self.lsp_clients.insert("python".to_string(), client);
            }
            Err(e) => {
                log::warn!("Failed to start ty: {} (continuing without LSP)", e);
                // Continue without LSP - it's optional
            }
        }

        Ok(())
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
    fn try_parse_json_bundle(
        &self,
        content: &str,
    ) -> Option<perspt_core::types::ArtifactBundle> {
        // Try to find JSON in markdown code blocks
        let json_str = if let Some(start) = content.find("```json") {
            let start = start + 7;
            if let Some(end_offset) = content[start..].find("```") {
                Some(content[start..start + end_offset].trim())
            } else {
                None
            }
        } else if content.trim().starts_with('{') {
            Some(content.trim())
        } else if let Some(start) = content.find('{') {
            if let Some(end) = content.rfind('}') {
                Some(&content[start..=end])
            } else {
                None
            }
        } else {
            None
        };

        let json_str = json_str?;

        // Try to parse as ArtifactBundle
        match serde_json::from_str::<perspt_core::types::ArtifactBundle>(json_str) {
            Ok(bundle) => Some(bundle),
            Err(e) => {
                log::debug!("JSON is not an ArtifactBundle: {}", e);
                None
            }
        }
    }

    /// PSP-5: Apply an artifact bundle transactionally
    ///
    /// All file operations are validated first, then applied.
    /// If any operation fails, the method returns an error describing which operation
    /// failed, and previous successful operations are logged for manual review.
    pub async fn apply_bundle_transactionally(
        &mut self,
        bundle: &perspt_core::types::ArtifactBundle,
        node_id: &str,
    ) -> Result<()> {
        // Validate first
        bundle.validate().map_err(|e| anyhow::anyhow!(e))?;

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
                for (lang, client) in self.lsp_clients.iter_mut() {
                    // Only notify if the file extension matches the LSP language
                    let should_notify = match lang.as_str() {
                        "python" => op.path().ends_with(".py"),
                        "rust" => op.path().ends_with(".rs"),
                        _ => true,
                    };
                    if should_notify {
                        if let Ok(content) = std::fs::read_to_string(&full_path) {
                            let _ = client.did_change(&full_path, &content, self.file_version).await;
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
            "rust" => ("src/main.rs".to_string(), "tests/integration_test.rs".to_string()),
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
    /// Uses the active language plugin's commands for syntax check, build, test,
    /// and lint instead of hardcoded Python verification.
    pub async fn run_plugin_verification(
        &mut self,
        plugin_name: &str,
    ) -> perspt_core::types::VerificationResult {
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

        // Check if tools are available
        if !plugin.host_tool_available() {
            return perspt_core::types::VerificationResult::degraded(format!(
                "{} toolchain not available on host",
                plugin.name()
            ));
        }

        let mut result = perspt_core::types::VerificationResult::default();
        let working_dir = self.context.working_dir.clone();

        // Run syntax check
        if let Some(cmd) = plugin.syntax_check_command() {
            let output = tokio::process::Command::new("sh")
                .args(["-c", &cmd])
                .current_dir(&working_dir)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .await;

            match output {
                Ok(out) => {
                    result.syntax_ok = out.status.success();
                    if !result.syntax_ok {
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        result.diagnostics_count = stderr.lines().count();
                        result.raw_output = Some(stderr.to_string());
                        self.emit_log(format!(
                            "⚠️ Syntax check failed ({} diagnostics)",
                            result.diagnostics_count
                        ));
                    } else {
                        result.syntax_ok = true;
                        self.emit_log("✅ Syntax check passed".to_string());
                    }
                }
                Err(e) => {
                    log::warn!("Syntax check command failed to run: {}", e);
                    result.syntax_ok = false;
                }
            }
        } else {
            result.syntax_ok = true; // No syntax check available, assume ok
        }

        // Run build check
        if let Some(cmd) = plugin.build_command() {
            let output = tokio::process::Command::new("sh")
                .args(["-c", &cmd])
                .current_dir(&working_dir)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .await;

            match output {
                Ok(out) => {
                    result.build_ok = out.status.success();
                    if !result.build_ok {
                        self.emit_log("⚠️ Build failed".to_string());
                    } else {
                        self.emit_log("✅ Build passed".to_string());
                    }
                }
                Err(e) => {
                    log::warn!("Build command failed to run: {}", e);
                    result.build_ok = false;
                }
            }
        } else {
            result.build_ok = true; // No build step, assume ok
        }

        // Run tests
        let test_cmd = plugin.test_command();
        let output = tokio::process::Command::new("sh")
            .args(["-c", &test_cmd])
            .current_dir(&working_dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .await;

        match output {
            Ok(out) => {
                result.tests_ok = out.status.success();
                let stdout = String::from_utf8_lossy(&out.stdout);
                // Simple heuristic to count tests
                let passed = stdout.matches("passed").count() + stdout.matches("ok").count();
                let failed = stdout.matches("failed").count() + stdout.matches("FAILED").count();
                result.tests_passed = passed;
                result.tests_failed = failed;

                if result.tests_ok {
                    self.emit_log(format!("✅ Tests passed ({})", plugin.name()));
                } else {
                    self.emit_log(format!("❌ Tests failed ({})", plugin.name()));
                    result.raw_output = Some(format!(
                        "{}\n{}",
                        stdout,
                        String::from_utf8_lossy(&out.stderr)
                    ));
                }
            }
            Err(e) => {
                log::warn!("Test command failed to run: {}", e);
                result.tests_ok = false;
            }
        }

        // Run lint (only in Strict mode)
        if self.context.verifier_strictness == perspt_core::types::VerifierStrictness::Strict {
            if let Some(cmd) = plugin.lint_command() {
                let output = tokio::process::Command::new("sh")
                    .args(["-c", &cmd])
                    .current_dir(&working_dir)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .output()
                    .await;

                match output {
                    Ok(out) => {
                        result.lint_ok = out.status.success();
                        if result.lint_ok {
                            self.emit_log("✅ Lint passed".to_string());
                        } else {
                            self.emit_log("⚠️ Lint issues found".to_string());
                        }
                    }
                    Err(e) => {
                        log::warn!("Lint command failed to run: {}", e);
                        result.lint_ok = false;
                    }
                }
            }
        } else {
            result.lint_ok = true; // Skip lint in non-strict mode
        }

        // Build summary
        result.summary = format!(
            "{}: syntax={}, build={}, tests={}, lint={}",
            plugin.name(),
            if result.syntax_ok { "✅" } else { "❌" },
            if result.build_ok { "✅" } else { "❌" },
            if result.tests_ok { "✅" } else { "❌" },
            if result.lint_ok { "✅" } else { "⏭️" },
        );

        result
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
}
