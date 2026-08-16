//! perspt-agent: SRBN Orchestrator and Agent logic
//!
//! Implements the Stabilized Recursive Barrier Network for multi-agent coding.

pub mod agent;
pub mod candidate;
pub mod context_retriever;
pub mod exploration;
pub mod grant;
pub mod ledger;
pub mod lsp;
pub mod orchestrator;
pub mod prompt_compiler;
pub mod prompts;
pub mod realize;
pub mod runtime;
pub mod test_runner;
pub mod toolloop;
pub mod tools;
pub mod transport;
pub mod types;

pub use agent::{ActuatorAgent, Agent, ArchitectAgent, SpeculatorAgent, VerifierAgent};
pub use candidate::{CandidateWorkspace, CodingCandidateMeasurer};
pub use context_retriever::{ContextRetriever, SearchHit};
pub use ledger::{
    MerkleCommit, MerkleLedger, NodeCommitPayload, NodeReviewSummary, NodeSnapshotDetail,
    SessionRecord, SessionReviewSummary, SessionSnapshot,
};
pub use lsp::{DocumentSymbolInfo, LspClient};
pub use orchestrator::SRBNOrchestrator;
pub use realize::{snapshot_workspace, ProjectionMismatch, SnapshotRealizer, WorkspaceState};
pub use runtime::{Psp9AgentRuntime, Psp9ModelRoutes, Psp9Recorder, Psp9RunConfig, Psp9RunSummary};
pub use test_runner::{PythonTestRunner, TestFailure, TestResults, TestRunner};
pub use toolloop::{
    CandidateMeasurer, EffectExecutor, LoopBudgets, LoopOutcome, LoopRecorder, ToolLoop,
};
pub use tools::{AgentTools, ToolCall, ToolDefinition, ToolResult};
pub use transport::GenAiTransport;
pub use types::{
    AgentContext, AgentMessage, BehavioralContract, Criticality, EnergyComponents, ErrorType,
    ModelTier, NodeState, PlannedContract, PlannedTask, PlannedTest, RetryPolicy, SRBNNode,
    StabilityMonitor, TaskPlan, TaskType, TokenBudget, WeightedTest,
};
