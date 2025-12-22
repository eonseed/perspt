//! Agent Trait
//!
//! Defines the interface for all agent implementations.

use crate::types::{AgentContext, AgentMessage, SRBNNode};
use anyhow::Result;
use async_trait::async_trait;

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
}

impl ArchitectAgent {
    pub fn new(model: Option<String>) -> Self {
        Self {
            model: model.unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string()),
        }
    }
}

#[async_trait]
impl Agent for ArchitectAgent {
    async fn process(&self, node: &SRBNNode, _ctx: &AgentContext) -> Result<AgentMessage> {
        // Placeholder - actual implementation would call LLM
        log::info!("[Architect] Processing node: {}", node.node_id);
        Ok(AgentMessage::new(
            crate::types::ModelTier::Architect,
            format!("Planned: {}", node.goal),
        ))
    }

    fn name(&self) -> &str {
        "Architect"
    }

    fn can_handle(&self, node: &SRBNNode) -> bool {
        matches!(node.tier, crate::types::ModelTier::Architect)
    }
}

/// Actuator agent - handles code generation
pub struct ActuatorAgent {
    model: String,
}

impl ActuatorAgent {
    pub fn new(model: Option<String>) -> Self {
        Self {
            model: model.unwrap_or_else(|| "gpt-4o".to_string()),
        }
    }
}

#[async_trait]
impl Agent for ActuatorAgent {
    async fn process(&self, node: &SRBNNode, _ctx: &AgentContext) -> Result<AgentMessage> {
        log::info!("[Actuator] Processing node: {}", node.node_id);
        Ok(AgentMessage::new(
            crate::types::ModelTier::Actuator,
            format!("Implemented: {}", node.goal),
        ))
    }

    fn name(&self) -> &str {
        "Actuator"
    }

    fn can_handle(&self, node: &SRBNNode) -> bool {
        matches!(node.tier, crate::types::ModelTier::Actuator)
    }
}

/// Verifier agent - handles stability verification
pub struct VerifierAgent {
    model: String,
}

impl VerifierAgent {
    pub fn new(model: Option<String>) -> Self {
        Self {
            model: model.unwrap_or_else(|| "gpt-4o-mini".to_string()),
        }
    }
}

#[async_trait]
impl Agent for VerifierAgent {
    async fn process(&self, node: &SRBNNode, _ctx: &AgentContext) -> Result<AgentMessage> {
        log::info!("[Verifier] Processing node: {}", node.node_id);
        Ok(AgentMessage::new(
            crate::types::ModelTier::Verifier,
            format!("Verified: {}", node.goal),
        ))
    }

    fn name(&self) -> &str {
        "Verifier"
    }

    fn can_handle(&self, node: &SRBNNode) -> bool {
        matches!(node.tier, crate::types::ModelTier::Verifier)
    }
}
