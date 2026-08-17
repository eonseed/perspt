//! perspt-agent: SRBN Orchestrator and Agent logic
//!
//! Implements the Stabilized Recursive Barrier Network for multi-agent coding.

pub mod candidate;
pub mod exploration;
pub mod external_tools;
pub mod grant;
pub mod lsp;
pub mod probe;
pub mod promote;
pub mod realize;
pub mod runtime;
pub mod toolloop;
pub mod tools;
pub mod transport;

pub use candidate::{CandidateWorkspace, CodingCandidateMeasurer};
pub use external_tools::{
    ExternalConnectionState, ExternalToolEvent, ExternalToolObserver, ExternalToolResult,
    ExternalToolRuntime, McpTransport, MCP_PROTOCOL_VERSION,
};
pub use lsp::{DocumentSymbolInfo, LspClient};
pub use probe::{probe_route, ProbeReport};
pub use realize::{snapshot_workspace, ProjectionMismatch, SnapshotRealizer, WorkspaceState};
pub use runtime::{Psp9AgentRuntime, Psp9ModelRoutes, Psp9Recorder, Psp9RunConfig, Psp9RunSummary};
pub use toolloop::{
    CandidateMeasurer, EffectExecutor, LoopBudgets, LoopOutcome, LoopRecorder, ToolLoop,
};
pub use tools::{AgentTools, ToolCall, ToolResult};
pub use transport::GenAiTransport;
