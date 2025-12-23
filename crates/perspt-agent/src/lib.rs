//! perspt-agent: SRBN Orchestrator and Agent logic
//!
//! Implements the Stabilized Recursive Barrier Network for multi-agent coding.

pub mod agent;
pub mod context_retriever;
pub mod ledger;
pub mod lsp;
pub mod orchestrator;
pub mod test_runner;
pub mod tools;
pub mod types;

pub use agent::{ActuatorAgent, Agent, ArchitectAgent, SpeculatorAgent, VerifierAgent};
pub use context_retriever::{ContextRetriever, SearchHit};
pub use ledger::{MerkleCommit, MerkleLedger, SessionRecord};
pub use lsp::{DocumentSymbolInfo, LspClient};
pub use orchestrator::SRBNOrchestrator;
pub use test_runner::{PythonTestRunner, TestFailure, TestResults, TestRunner};
pub use tools::{AgentTools, ToolCall, ToolDefinition, ToolResult};
pub use types::{
    AgentContext, AgentMessage, BehavioralContract, Criticality, EnergyComponents, ErrorType,
    ModelTier, NodeState, PlannedContract, PlannedTask, PlannedTest, RetryPolicy, SRBNNode,
    StabilityMonitor, TaskPlan, TaskType, TokenBudget, WeightedTest,
};
