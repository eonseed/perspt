//! perspt-agent: SRBN Orchestrator and Agent logic
//!
//! Implements the Stabilized Recursive Barrier Network for multi-agent coding.

pub mod candidate;
pub mod exploration;
pub mod external_tools;
pub mod grant;
pub mod lsp;
pub mod measure;
pub mod probe;
pub mod promote;
pub mod realize;
pub mod runtime;
pub mod toolloop;
pub mod tools;
pub mod transport;
pub mod turn;
pub mod verifier;

pub use candidate::CandidateWorkspace;
pub use external_tools::{
    ExternalConnectionState, ExternalToolEvent, ExternalToolObserver, ExternalToolRejection,
    ExternalToolResult, ExternalToolRuntime, MCP_PROTOCOL_VERSION, McpClientServices,
    McpElicitationAction, McpElicitationBroker, McpElicitationProvider, McpPendingElicitation,
    McpSamplingProvider, McpServerEvent, ModelTransportSamplingProvider,
};
pub use lsp::{DocumentSymbolInfo, LspClient};
pub use measure::CodingCandidateMeasurer;
pub use probe::{ProbeReport, probe_route};
pub use realize::{ProjectionMismatch, SnapshotRealizer, WorkspaceState, snapshot_workspace};
pub use runtime::{Psp9AgentRuntime, Psp9ModelRoutes, Psp9Recorder, Psp9RunConfig, Psp9RunSummary};
pub use toolloop::{
    CandidateMeasurer, EffectExecutor, LoopBudgets, LoopOutcome, LoopRecorder, ToolLoop,
};
pub use tools::handlers::{
    AsyncFnInteractionHandler, CandidateHandlerRegistry, CandidateToolHandler,
    FnInteractionHandler, InteractionHandler, NonInteractiveHandler,
};
pub use tools::{AgentTools, ToolCall, ToolResult};
pub use transport::GenAiTransport;
