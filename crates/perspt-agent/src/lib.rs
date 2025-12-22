//! perspt-agent: SRBN Orchestrator and Agent logic
//!
//! Implements the Stabilized Recursive Barrier Network for multi-agent coding.

pub mod agent;
pub mod lsp;
pub mod orchestrator;
pub mod types;

pub use agent::{ActuatorAgent, Agent, ArchitectAgent, VerifierAgent};
pub use lsp::LspClient;
pub use orchestrator::SRBNOrchestrator;
pub use types::{
    AgentContext, AgentMessage, BehavioralContract, Criticality, EnergyComponents, ModelTier,
    NodeState, SRBNNode, StabilityMonitor, WeightedTest,
};
