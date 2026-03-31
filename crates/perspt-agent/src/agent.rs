//! Agent Trait and Implementations
//!
//! Defines the interface for all agent implementations and provides
//! LLM-integrated implementations for Architect, Actuator, and Verifier roles.

use crate::types::{AgentContext, AgentMessage, ModelTier, SRBNNode};
use anyhow::Result;
use async_trait::async_trait;
use perspt_core::llm_provider::GenAIProvider;
use std::fs;
use std::path::Path;
use std::sync::Arc;

/// The Agent trait defines the interface for SRBN agents.
///
/// Each agent role (Architect, Actuator, Verifier, Speculator) implements
/// this trait to provide specialized behavior.
#[async_trait]
pub trait Agent: Send + Sync {
    /// Process a task and return a message
    async fn process(&self, node: &SRBNNode, ctx: &AgentContext) -> Result<AgentMessage>;

    /// Get the agent's display name
    fn name(&self) -> &str;

    /// Check if this agent can handle the given node
    fn can_handle(&self, node: &SRBNNode) -> bool;

    /// Get the model name used by this agent (for logging)
    fn model(&self) -> &str;

    /// Build the prompt for this agent (for logging)
    fn build_prompt(&self, node: &SRBNNode, ctx: &AgentContext) -> String;
}

/// Architect agent - handles planning and DAG construction
pub struct ArchitectAgent {
    model: String,
    provider: Arc<GenAIProvider>,
}

impl ArchitectAgent {
    pub fn new(provider: Arc<GenAIProvider>, model: Option<String>) -> Self {
        Self {
            model: model.unwrap_or_else(|| ModelTier::Architect.default_model().to_string()),
            provider,
        }
    }

    pub fn build_planning_prompt(&self, node: &SRBNNode, ctx: &AgentContext) -> String {
        let project_context = format!(
            "Context Files: {:?}\nOutput Targets: {:?}",
            node.context_files, node.output_targets
        );
        crate::prompts::render_architect(
            crate::prompts::ARCHITECT_EXISTING,
            &node.goal,
            &ctx.working_dir,
            &project_context,
            "",
            "",
            &ctx.active_plugins,
        )
    }
}

#[async_trait]
impl Agent for ArchitectAgent {
    async fn process(&self, node: &SRBNNode, ctx: &AgentContext) -> Result<AgentMessage> {
        log::info!(
            "[Architect] Processing node: {} with model {}",
            node.node_id,
            self.model
        );

        let prompt = self.build_planning_prompt(node, ctx);

        let response = self
            .provider
            .generate_response_simple(&self.model, &prompt)
            .await?;

        Ok(AgentMessage::new(ModelTier::Architect, response))
    }

    fn name(&self) -> &str {
        "Architect"
    }

    fn can_handle(&self, node: &SRBNNode) -> bool {
        matches!(node.tier, ModelTier::Architect)
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn build_prompt(&self, node: &SRBNNode, ctx: &AgentContext) -> String {
        self.build_planning_prompt(node, ctx)
    }
}

/// Actuator agent - handles code generation
pub struct ActuatorAgent {
    model: String,
    provider: Arc<GenAIProvider>,
}

impl ActuatorAgent {
    pub fn new(provider: Arc<GenAIProvider>, model: Option<String>) -> Self {
        Self {
            model: model.unwrap_or_else(|| ModelTier::Actuator.default_model().to_string()),
            provider,
        }
    }

    pub fn build_coding_prompt(&self, node: &SRBNNode, ctx: &AgentContext) -> String {
        let contract = &node.contract;
        let allowed_output_paths: Vec<String> = node
            .output_targets
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect();
        let workspace_import_hints = Self::workspace_import_hints(&ctx.working_dir);

        // Determine target file from output_targets or generate default
        let target_file = node
            .output_targets
            .first()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "main.py".to_string());

        // PSP-5: Determine output format based on execution mode and plugin
        let is_project_mode = ctx.execution_mode == perspt_core::types::ExecutionMode::Project;
        let has_multiple_outputs = node.output_targets.len() > 1;

        crate::prompts::render_actuator(
            &node.goal,
            &contract.interface_signature,
            &format!("{:?}", contract.invariants),
            &format!("{:?}", contract.forbidden_patterns),
            &format!("{:?}", ctx.working_dir),
            &format!("{:?}", node.context_files),
            &target_file,
            &format!("{:?}", allowed_output_paths),
            &format!("{:?}", workspace_import_hints),
            is_project_mode || has_multiple_outputs,
        )
    }

    fn workspace_import_hints(working_dir: &Path) -> Vec<String> {
        let mut hints = Vec::new();

        if let Some(crate_name) = Self::detect_rust_crate_name(working_dir) {
            hints.push(format!(
                "Rust crate name: {}. Integration tests and external modules must import via `{}`.",
                crate_name, crate_name
            ));
        }

        if let Some(package_name) = Self::detect_python_package_name(working_dir) {
            hints.push(format!(
                "Python package import root: {}. Tests and entry points must import `{}` and never `src.{}`.",
                package_name, package_name, package_name
            ));
        }

        hints
    }

    fn detect_rust_crate_name(working_dir: &Path) -> Option<String> {
        let cargo_toml = fs::read_to_string(working_dir.join("Cargo.toml")).ok()?;
        let mut in_package = false;

        for raw_line in cargo_toml.lines() {
            let line = raw_line.trim();
            if line.starts_with('[') {
                in_package = line == "[package]";
                continue;
            }

            if in_package && line.starts_with("name") {
                let (_, value) = line.split_once('=')?;
                return Some(value.trim().trim_matches('"').to_string());
            }
        }

        None
    }

    fn detect_python_package_name(working_dir: &Path) -> Option<String> {
        let src_dir = working_dir.join("src");
        if let Ok(entries) = fs::read_dir(&src_dir) {
            for entry in entries.flatten() {
                if entry.file_type().ok()?.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !name.starts_with('.') {
                        return Some(name);
                    }
                }
            }
        }

        let pyproject = fs::read_to_string(working_dir.join("pyproject.toml")).ok()?;
        let mut in_project = false;
        for raw_line in pyproject.lines() {
            let line = raw_line.trim();
            if line.starts_with('[') {
                in_project = line == "[project]";
                continue;
            }

            if in_project && line.starts_with("name") {
                let (_, value) = line.split_once('=')?;
                return Some(value.trim().trim_matches('"').replace('-', "_"));
            }
        }

        None
    }
}

#[async_trait]
impl Agent for ActuatorAgent {
    async fn process(&self, node: &SRBNNode, ctx: &AgentContext) -> Result<AgentMessage> {
        log::info!(
            "[Actuator] Processing node: {} with model {}",
            node.node_id,
            self.model
        );

        let prompt = self.build_coding_prompt(node, ctx);

        let response = self
            .provider
            .generate_response_simple(&self.model, &prompt)
            .await?;

        Ok(AgentMessage::new(ModelTier::Actuator, response))
    }

    fn name(&self) -> &str {
        "Actuator"
    }

    fn can_handle(&self, node: &SRBNNode) -> bool {
        matches!(node.tier, ModelTier::Actuator)
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn build_prompt(&self, node: &SRBNNode, ctx: &AgentContext) -> String {
        self.build_coding_prompt(node, ctx)
    }
}

/// Verifier agent - handles stability verification and contract checking
pub struct VerifierAgent {
    model: String,
    provider: Arc<GenAIProvider>,
}

impl VerifierAgent {
    pub fn new(provider: Arc<GenAIProvider>, model: Option<String>) -> Self {
        Self {
            model: model.unwrap_or_else(|| ModelTier::Verifier.default_model().to_string()),
            provider,
        }
    }

    pub fn build_verification_prompt(&self, node: &SRBNNode, implementation: &str) -> String {
        let contract = &node.contract;
        crate::prompts::render_verifier(
            &contract.interface_signature,
            &format!("{:?}", contract.invariants),
            &format!("{:?}", contract.forbidden_patterns),
            &format!("{:?}", contract.weighted_tests),
            implementation,
        )
    }
}

#[async_trait]
impl Agent for VerifierAgent {
    async fn process(&self, node: &SRBNNode, ctx: &AgentContext) -> Result<AgentMessage> {
        log::info!(
            "[Verifier] Processing node: {} with model {}",
            node.node_id,
            self.model
        );

        // In a real implementation, we would get the actual implementation from the context
        let implementation = ctx
            .history
            .last()
            .map(|m| m.content.as_str())
            .unwrap_or("No implementation provided");

        let prompt = self.build_verification_prompt(node, implementation);

        let response = self
            .provider
            .generate_response_simple(&self.model, &prompt)
            .await?;

        Ok(AgentMessage::new(ModelTier::Verifier, response))
    }

    fn name(&self) -> &str {
        "Verifier"
    }

    fn can_handle(&self, node: &SRBNNode) -> bool {
        matches!(node.tier, ModelTier::Verifier)
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn build_prompt(&self, node: &SRBNNode, _ctx: &AgentContext) -> String {
        // Verifier needs implementation context, use a placeholder
        self.build_verification_prompt(node, "<implementation>")
    }
}

/// Speculator agent - handles fast lookahead for exploration
pub struct SpeculatorAgent {
    model: String,
    provider: Arc<GenAIProvider>,
}

impl SpeculatorAgent {
    pub fn new(provider: Arc<GenAIProvider>, model: Option<String>) -> Self {
        Self {
            model: model.unwrap_or_else(|| ModelTier::Speculator.default_model().to_string()),
            provider,
        }
    }
}

#[async_trait]
impl Agent for SpeculatorAgent {
    async fn process(&self, node: &SRBNNode, ctx: &AgentContext) -> Result<AgentMessage> {
        log::info!(
            "[Speculator] Processing node: {} with model {}",
            node.node_id,
            self.model
        );

        let prompt = self.build_prompt(node, ctx);

        let response = self
            .provider
            .generate_response_simple(&self.model, &prompt)
            .await?;

        Ok(AgentMessage::new(ModelTier::Speculator, response))
    }

    fn name(&self) -> &str {
        "Speculator"
    }

    fn can_handle(&self, node: &SRBNNode) -> bool {
        matches!(node.tier, ModelTier::Speculator)
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn build_prompt(&self, node: &SRBNNode, _ctx: &AgentContext) -> String {
        crate::prompts::SPECULATOR_BASIC.replace("{goal}", &node.goal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn build_coding_prompt_includes_rust_crate_hint() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"validator_lib\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let provider = Arc::new(GenAIProvider::new().unwrap());
        let agent = ActuatorAgent::new(provider, Some("test-model".into()));
        let mut node = SRBNNode::new("n1".into(), "goal".into(), ModelTier::Actuator);
        node.output_targets.push("tests/integration.rs".into());
        let ctx = AgentContext {
            working_dir: dir.path().to_path_buf(),
            ..Default::default()
        };

        let prompt = agent.build_coding_prompt(&node, &ctx);
        assert!(
            prompt.contains("Rust crate name: validator_lib"),
            "{prompt}"
        );
    }

    #[test]
    fn build_coding_prompt_includes_python_package_hint() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/psp5_python_verify")).unwrap();
        fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nname = \"psp5-python-verify\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let provider = Arc::new(GenAIProvider::new().unwrap());
        let agent = ActuatorAgent::new(provider, Some("test-model".into()));
        let mut node = SRBNNode::new("n1".into(), "goal".into(), ModelTier::Actuator);
        node.output_targets.push("tests/test_main.py".into());
        let ctx = AgentContext {
            working_dir: dir.path().to_path_buf(),
            ..Default::default()
        };

        let prompt = agent.build_coding_prompt(&node, &ctx);
        assert!(
            prompt.contains("Python package import root: psp5_python_verify"),
            "{prompt}"
        );
    }
}
