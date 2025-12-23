//! perspt-agent: SRBN Orchestrator and Agent logic
//!
//! Implements the Stabilized Recursive Barrier Network for multi-agent coding.

pub mod agent;
pub mod ledger;
pub mod lsp;
pub mod orchestrator;
pub mod tools;
pub mod types;

pub use agent::{ActuatorAgent, Agent, ArchitectAgent, SpeculatorAgent, VerifierAgent};
pub use ledger::{MerkleCommit, MerkleLedger, SessionRecord};
pub use lsp::LspClient;
pub use orchestrator::SRBNOrchestrator;
pub use tools::{AgentTools, ToolCall, ToolDefinition, ToolResult};
pub use types::{
    AgentContext, AgentMessage, BehavioralContract, Criticality, EnergyComponents, ModelTier,
    NodeState, PlannedContract, PlannedTask, PlannedTest, SRBNNode, StabilityMonitor, TaskPlan,
    TaskType, WeightedTest,
};
