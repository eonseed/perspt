//! perspt-core: Core types and LLM provider abstraction

pub mod config;
pub mod events;
pub mod llm_provider;
pub mod memory;
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
    AgentContext, AgentMessage, BehavioralContract, CommandContract, Criticality, EnergyComponents,
    ErrorType, ModelTier, NodeState, PlannedContract, PlannedTask, PlannedTest, RetryPolicy,
    SRBNNode, StabilityMonitor, TaskPlan, TaskType, TokenBudget, WeightedTest,
};
