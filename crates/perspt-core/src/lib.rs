//! perspt-core: Core types and LLM provider abstraction

pub mod config;
pub mod events;
pub mod llm_provider;
pub mod local_command;
pub mod memory;
pub mod normalize;
pub mod path;
pub mod paths;
pub mod plugin;
pub mod portfolio;
pub mod prompts;
pub mod tools_driver;
pub mod types;

pub use config::{
    Config, ContextConfig, ExplorationConfig, ExternalOracleConfig, ExternalToolConfig,
    ExternalToolMode, ExternalToolPolicy, ExternalToolTransport, McpRootConfig, ModelsConfig,
    PromptsConfig, ProviderEntry, TestPolicy,
};
pub use events::{ActionType, AgentAction, AgentEvent, NodeStatus};
pub use llm_provider::{
    detect_provider_from_env, GenAIProvider, LlmResponse, ResolvedProvider, EOT_SIGNAL,
};
pub use memory::ProjectMemory;
pub use plugin::{
    InitOptions, JsPlugin, LanguagePlugin, LspConfig, PluginRegistry, PythonPlugin, RustPlugin,
};
pub use portfolio::{declared_caps, ModelPortfolio, ProviderCaps, ProviderHandle};
pub use tools_driver::{CoreMessage, CoreToolCall, CoreToolChoice, CoreToolSpec, CoreTurnOutput};

// Re-export commonly used types
pub use types::{
    CommandContract, EnergyComponents, ModelTier, PlannedContract, PlannedTask, PlannedTest,
    SensorStatus, StageOutcome, TaskPlan, TaskType,
};
