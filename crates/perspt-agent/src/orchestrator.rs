//! SRBN Orchestrator
//!
//! Manages the Task DAG and orchestrates agent execution following the 7-step control loop.

use crate::agent::{ActuatorAgent, Agent, ArchitectAgent, VerifierAgent};
use crate::lsp::LspClient;
use crate::tools::{AgentTools, ToolCall};
use crate::types::{AgentContext, EnergyComponents, ModelTier, NodeState, SRBNNode};
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
}

impl SRBNOrchestrator {
    /// Create a new orchestrator
    pub fn new(working_dir: PathBuf, auto_approve: bool) -> Self {
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

        Self {
            graph: DiGraph::new(),
            node_indices: HashMap::new(),
            context,
            auto_approve,
            lsp_clients: HashMap::new(),
            agents: vec![
                Box::new(ArchitectAgent::new(provider.clone(), None)),
                Box::new(ActuatorAgent::new(provider.clone(), None)),
                Box::new(VerifierAgent::new(provider, None)),
            ],
            tools,
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
    /// The Architect analyzes the task and produces a Task DAG
    async fn step_sheafify(&mut self, task: String) -> Result<()> {
        log::info!("Step 1: Sheafification - Planning task decomposition");

        // Create the root planning node
        let root_node = SRBNNode::new("root".to_string(), task.clone(), ModelTier::Architect);
        let root_idx = self.add_node(root_node);

        // Get the architect agent
        let architect = &self.agents[0];

        // Process with architect (would normally call LLM)
        let root = &self.graph[root_idx];
        let message = architect.process(root, &self.context).await?;

        // Record message in history
        self.context.history.push(message);

        // Mark root as planned
        self.graph[root_idx].state = NodeState::Planning;

        Ok(())
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
        // In a real implementation, this would query the LSP client
        energy.v_syn = 0.0;

        // V_str: From structural contract verification
        energy.v_str = 0.0;

        // V_log: From test execution
        energy.v_log = 0.0;

        let node = &self.graph[idx];
        let total = energy.total(&node.contract);
        log::info!(
            "Energy for {}: V_syn={}, V_str={}, V_log={}, Total={}",
            node.node_id,
            energy.v_syn,
            energy.v_str,
            energy.v_log,
            total
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
        let attempt_count = node.monitor.attempt_count;
        let stable = node.monitor.stable;
        let should_escalate = node.monitor.should_escalate();

        if stable {
            log::info!("Node {} is stable (V(x) < ε)", node_id);
            return Ok(true);
        }

        if should_escalate {
            log::warn!(
                "Node {} failed to converge after {} attempts",
                node_id,
                attempt_count
            );
            return Ok(false);
        }

        // Retry with restorative feedback
        self.graph[idx].state = NodeState::Retry;
        log::info!("Retrying node {} (attempt {})", node_id, attempt_count);

        // In a real implementation, we would loop back to speculation
        // For now, assume success
        Ok(true)
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
