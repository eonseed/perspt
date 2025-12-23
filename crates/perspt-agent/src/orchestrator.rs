//! SRBN Orchestrator
//!
//! Manages the Task DAG and orchestrates agent execution following the 7-step control loop.

use crate::agent::{ActuatorAgent, Agent, ArchitectAgent, SpeculatorAgent, VerifierAgent};
use crate::lsp::LspClient;
use crate::tools::{AgentTools, ToolCall};
use crate::types::{AgentContext, EnergyComponents, ModelTier, NodeState, SRBNNode, TaskPlan};
use anyhow::{Context, Result};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::{Topo, Walker};
use std::collections::HashMap;
use std::path::PathBuf;

/// Dependency edge type
#[derive(Debug, Clone)]
pub struct Dependency {
    /// Dependency type description
    pub kind: String,
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
        let mut context = AgentContext::default();
        context.working_dir = working_dir.clone();
        context.auto_approve = auto_approve;

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
        }
    }

    /// Add a node to the task DAG
    pub fn add_node(&mut self, node: SRBNNode) -> NodeIndex {
        let node_id = node.node_id.clone();
        let idx = self.graph.add_node(node);
        self.node_indices.insert(node_id, idx);
        idx
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

        // Step 1: Architecture Sheafification
        self.step_sheafify(task).await?;

        // Step 2-7: Execute nodes in topological order
        let topo = Topo::new(&self.graph);
        let indices: Vec<_> = topo.iter(&self.graph).collect();

        for idx in indices {
            self.execute_node(idx).await?;
        }

        log::info!("SRBN execution completed");
        Ok(())
    }

    /// Step 1: Architecture Sheafification
    ///
    /// The Architect analyzes the task and produces a structured Task DAG.
    /// This step retries until a valid JSON plan is produced or max attempts reached.
    async fn step_sheafify(&mut self, task: String) -> Result<()> {
        log::info!("Step 1: Sheafification - Planning task decomposition");
        println!("   🏗️ Architect is analyzing the task...");

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
                .provider
                .generate_response_simple(&self.get_architect_model(), &prompt)
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
                            println!(
                                "   ❌ Failed to get valid plan after {} attempts",
                                MAX_ATTEMPTS
                            );
                            // Fall back to single-task execution
                            return self.create_fallback_task(&task);
                        }
                        continue;
                    }

                    // Check complexity gating
                    if plan.len() > self.context.complexity_k && !self.auto_approve {
                        println!(
                            "   ⚠️ Plan has {} tasks (exceeds K={}). Approve? [y/n]",
                            plan.len(),
                            self.context.complexity_k
                        );
                        // TODO: Implement interactive approval
                        // For now, auto-approve in headless mode
                    }

                    println!("   ✅ Architect produced plan with {} task(s)", plan.len());

                    // Create nodes from the plan
                    self.create_nodes_from_plan(&plan)?;
                    return Ok(());
                }
                Err(e) => {
                    log::warn!("Plan parsing failed (attempt {}): {}", attempt, e);
                    last_error = Some(format!("JSON parse error: {}", e));

                    if attempt >= MAX_ATTEMPTS {
                        println!("   ⚠️ Could not parse structured plan, using single task");
                        return self.create_fallback_task(&task);
                    }
                }
            }
        }

        // Should not reach here
        self.create_fallback_task(&task)
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

        let prompt = format!(
            r#"You are an Architect agent in a multi-agent coding system.

## Task
{task}

## Working Directory
{working_dir}
{error_feedback}
## Instructions
Analyze this task and produce a structured execution plan as JSON.


1. Break down the task into atomic subtasks
2. Each subtask should produce one or more files
3. Include unit tests where appropriate
4. Specify dependencies between tasks
5. Output ONLY valid JSON (no markdown, no explanation)

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
      "id": "test_1",
      "goal": "Unit tests for task_1",
      "context_files": ["new_file.py"],
      "output_files": ["test_new_file.py"],
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
            error_feedback = error_feedback
        );

        Ok(prompt)
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

    /// Create a fallback single-task execution when plan parsing fails
    fn create_fallback_task(&mut self, task: &str) -> Result<()> {
        log::warn!("Using fallback single-task execution");
        println!("   📝 Using simplified single-task execution");

        let root_node = SRBNNode::new("root".to_string(), task.to_string(), ModelTier::Actuator);
        self.add_node(root_node);

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
        let message = actuator.process(node, &self.context).await?;

        // Extract code blocks from the LLM response and write to files
        let content = &message.content;

        // Parse code blocks in format: ```python\n...``` or ```\n...```
        // Also look for file path patterns like "File: path/to/file.py"
        if let Some(code_info) = self.extract_code_from_response(content) {
            log::info!("Extracted code for file: {}", code_info.0);

            // Build full path
            let full_path = self.context.working_dir.join(&code_info.0);

            // Create a tool call to write the file
            let mut args = HashMap::new();
            args.insert("path".to_string(), code_info.0.clone());
            args.insert("content".to_string(), code_info.1.clone());

            let call = ToolCall {
                name: "apply_patch".to_string(),
                arguments: args,
            };

            let result = self.tools.execute(&call).await;
            if result.success {
                log::info!("✓ Wrote file: {}", code_info.0);
                println!("   📝 Wrote file: {}", code_info.0);

                // Track the written file for LSP verification
                self.last_written_file = Some(full_path.clone());
                self.file_version += 1;

                // Notify LSP of file change (if LSP is running)
                if let Some(client) = self.lsp_clients.get_mut("python") {
                    if self.file_version == 1 {
                        let _ = client.did_open(&full_path, &code_info.1).await;
                    } else {
                        let _ = client
                            .did_change(&full_path, &code_info.1, self.file_version)
                            .await;
                    }
                }
            } else {
                log::warn!("Failed to write file: {:?}", result.error);
            }
        } else {
            log::debug!(
                "No code block found in response, response length: {}",
                content.len()
            );
            println!("   ℹ No file changes detected in response");
        }

        self.context.history.push(message);
        Ok(())
    }

    /// Extract code from LLM response
    /// Returns (filename, code_content) if found
    fn extract_code_from_response(&self, content: &str) -> Option<(String, String)> {
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
                            "javascript" | "js" => "main.js".to_string(),
                            "typescript" | "ts" => "main.ts".to_string(),
                            _ => "output.txt".to_string(),
                        });
                    return Some((filename, code));
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
    /// Computes Lyapunov Energy V(x) from LSP diagnostics and tests
    async fn step_verify(&mut self, idx: NodeIndex) -> Result<EnergyComponents> {
        log::info!("Step 4: Verification - Computing stability energy");

        self.graph[idx].state = NodeState::Verifying;

        // Calculate energy components
        let mut energy = EnergyComponents::default();

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
                    println!("   🔍 LSP found {} diagnostics:", diagnostics.len());
                    for d in &diagnostics {
                        println!("      - [{:?}] {}", d.severity, d.message);
                    }

                    // Store diagnostics for correction prompt
                    self.context.last_diagnostics = diagnostics;
                } else {
                    log::info!("LSP reports no errors (diagnostics vector is empty)");
                }
            } else {
                log::debug!("No LSP client available for Python");
            }
        }

        let node = &self.graph[idx];
        log::info!(
            "Energy for {}: V_syn={:.2}, V_str={:.2}, V_log={:.2}, Total={:.2}",
            node.node_id,
            energy.v_syn,
            energy.v_str,
            energy.v_log,
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
            println!("   ✅ Stable! V(x)={:.2} < ε={:.2}", total, epsilon);
            return Ok(true);
        }

        if should_escalate {
            log::warn!(
                "Node {} failed to converge after {} attempts (V(x)={:.2})",
                node_id,
                attempt_count,
                total
            );
            println!(
                "   ⚠️ Escalating: failed to converge after {} attempts",
                attempt_count
            );
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
        println!(
            "   🔄 V(x)={:.2} > ε={:.2}, sending errors to LLM (attempt {})",
            total, epsilon, attempt_count
        );

        // Build correction prompt with diagnostics
        let correction_prompt = self.build_correction_prompt(&node_id, &goal, &energy)?;

        log::info!(
            "--- CORRECTION PROMPT ---\n{}\n-------------------------",
            correction_prompt
        );
        println!(
            "--- CORRECTION PROMPT ---\n{}\n-------------------------",
            correction_prompt
        );

        // Call LLM for corrected code
        let corrected = self.call_llm_for_correction(&correction_prompt).await?;

        // Extract and apply diff
        if let Some((filename, new_code)) = self.extract_code_from_response(&corrected) {
            let full_path = self.context.working_dir.join(&filename);

            // Write corrected file
            let mut args = HashMap::new();
            args.insert("path".to_string(), filename.clone());
            args.insert("content".to_string(), new_code.clone());

            let call = ToolCall {
                name: "apply_patch".to_string(),
                arguments: args,
            };

            let result = self.tools.execute(&call).await;
            if result.success {
                log::info!("✓ Applied correction to: {}", filename);
                println!("   📝 Applied correction to: {}", filename);

                // Update tracking
                self.last_written_file = Some(full_path.clone());
                self.file_version += 1;

                // Notify LSP of file change
                if let Some(client) = self.lsp_clients.get_mut("python") {
                    let _ = client
                        .did_change(&full_path, &new_code, self.file_version)
                        .await;
                }
            }
        }

        // Re-verify (recursive correction loop)
        let new_energy = self.step_verify(idx).await?;
        Box::pin(self.step_converge(idx, new_energy)).await
    }

    /// Build a correction prompt with diagnostic details
    fn build_correction_prompt(
        &self,
        node_id: &str,
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
            .provider
            .generate_response_simple(&self.actuator_model, prompt)
            .await?;
        log::debug!("Received correction response with {} chars", response.len());

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
}
