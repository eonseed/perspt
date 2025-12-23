//! Agent Trait and Implementations
//!
//! Defines the interface for all agent implementations and provides
//! LLM-integrated implementations for Architect, Actuator, and Verifier roles.

use crate::types::{AgentContext, AgentMessage, ModelTier, SRBNNode};
use anyhow::Result;
use async_trait::async_trait;
use perspt_core::llm_provider::GenAIProvider;
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

    fn build_planning_prompt(&self, node: &SRBNNode, ctx: &AgentContext) -> String {
        format!(
            r#"You are an Architect agent in a multi-agent coding system.

## Task
Goal: {}

## Context
Working Directory: {:?}
Context Files: {:?}
Output Targets: {:?}

## Requirements
1. Break down this task into subtasks if needed
2. Define behavioral contracts for each subtask
3. Identify dependencies between subtasks
4. Specify required interfaces and invariants

## Output Format
Provide a structured plan with:
- Subtask list with goals
- File dependencies
- Interface signatures
- Test criteria"#,
            node.goal, ctx.working_dir, node.context_files, node.output_targets,
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

    fn build_coding_prompt(&self, node: &SRBNNode, ctx: &AgentContext) -> String {
        let contract = &node.contract;

        format!(
            r#"You are an Actuator agent responsible for implementing code.

## Task
Goal: {}

## Behavioral Contract
Interface Signature: {}
Invariants: {:?}
Forbidden Patterns: {:?}

## Context
Working Directory: {:?}
Files to Read: {:?}
Files to Modify: {:?}

## Instructions
1. Implement the required functionality
2. Follow the interface signature exactly
3. Maintain all specified invariants
4. Avoid all forbidden patterns
5. Write clean, documented code

## Output Format
Provide the complete implementation with:
- File path
- Code content
- Brief explanation of changes"#,
            node.goal,
            contract.interface_signature,
            contract.invariants,
            contract.forbidden_patterns,
            ctx.working_dir,
            node.context_files,
            node.output_targets,
        )
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

    fn build_verification_prompt(&self, node: &SRBNNode, implementation: &str) -> String {
        let contract = &node.contract;

        format!(
            r#"You are a Verifier agent responsible for checking code correctness.

## Task
Verify the implementation satisfies the behavioral contract.

## Behavioral Contract
Interface Signature: {}
Invariants: {:?}
Forbidden Patterns: {:?}
Weighted Tests: {:?}

## Implementation
{}

## Verification Criteria
1. Does the interface match the signature?
2. Are all invariants satisfied?
3. Are any forbidden patterns present?
4. Would the weighted tests pass?

## Output Format
Provide:
- PASS or FAIL status
- Energy score (0.0 = perfect, 1.0 = total failure)
- List of violations if any
- Suggested fixes for each violation"#,
            contract.interface_signature,
            contract.invariants,
            contract.forbidden_patterns,
            contract.weighted_tests,
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
    async fn process(&self, node: &SRBNNode, _ctx: &AgentContext) -> Result<AgentMessage> {
        log::info!(
            "[Speculator] Processing node: {} with model {}",
            node.node_id,
            self.model
        );

        let prompt = format!(
            "Quickly evaluate if this approach is viable: {}\nProvide a brief YES/NO with one sentence justification.",
            node.goal
        );

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
}

#[cfg(test)]
mod tests {
    // Note: Integration tests would require actual API keys
    // These are unit tests for the prompt building logic

    #[test]
    fn test_architect_prompt_building() {
        // Would need provider mock for full test
    }
}
