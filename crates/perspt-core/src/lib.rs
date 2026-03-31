//! perspt-core: Core types and LLM provider abstraction

pub mod config;
pub mod events;
pub mod llm_provider;
pub mod memory;
pub mod normalize;
pub mod plugin;
pub mod types;

pub use config::Config;
pub use events::{ActionType, AgentAction, AgentEvent, NodeStatus};
pub use llm_provider::{GenAIProvider, EOT_SIGNAL};
pub use memory::ProjectMemory;
pub use plugin::{
    InitOptions, JsPlugin, LanguagePlugin, LspConfig, PluginRegistry, PythonPlugin, RustPlugin,
};

// Re-export commonly used types
pub use types::{
    AgentContext, AgentMessage, ArtifactKind, BehavioralContract, BlockedDependency,
    BranchFlushRecord, BranchLineage, BudgetEnvelope, CommandContract, ContextBudget,
    ContextPackage, ContextProvenance, Criticality, DependencyExpectation, EnergyComponents,
    ErrorType, EscalationCategory, EscalationReport, FeatureCharter, InterfaceSealRecord,
    ModelTier, NodeState, OwnershipManifest, PlanRevision, PlanRevisionStatus, PlannedContract,
    PlannedTask, PlannedTest, ProvisionalBranch, ProvisionalBranchState, RepairFootprint,
    RestrictionMap, RetryPolicy, RewriteAction, RewriteRecord, SRBNNode, SensorStatus,
    SheafValidationResult, SheafValidatorClass, StabilityMonitor, StageOutcome, StructuralDigest,
    SummaryDigest, SummaryKind, TargetedRequeue, TaskPlan, TaskType, TokenBudget,
    VerificationResult, WeightedTest, WorkspaceState,
};
