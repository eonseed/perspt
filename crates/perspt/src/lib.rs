//! # Perspt (`"Omni"` SDK & Facade)
//!
//! Unified Rust entry point into Perspt's governed agentic runtime, domain
//! packages, policy engine, sandbox, and durable session store.
//!
//! ## Quick Start
//!
//! ```no_run
//! use perspt::prelude::*;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let runtime = AgentBuilder::new(".")
//!     .research_domain()
//!     .approval_policy(ApprovalPolicy::Auto)
//!     .on_ask_user_async(|q| async move { Ok(format!("answered: {q}")) })
//!     .build()?;
//! let _summary = runtime.run("verify paper citations".into()).await?;
//! # Ok(())
//! # }
//! ```

mod builder;

pub use builder::AgentBuilder;

pub use perspt_agent as agent;
pub use perspt_coding as coding;
pub use perspt_core as core;
pub use perspt_policy as policy;
pub use perspt_research as research;
pub use perspt_sandbox as sandbox;
pub use perspt_sdk as sdk;
pub use perspt_store as store;

/// Convenient re-exports for building governed omni-domain agents.
pub mod prelude {
    pub use crate::builder::AgentBuilder;
    pub use perspt_agent::{
        AsyncFnInteractionHandler, CandidateToolHandler, FnInteractionHandler, Psp9AgentRuntime,
        Psp9RunConfig, Psp9RunSummary,
    };
    pub use perspt_coding::CodingDomain;
    pub use perspt_research::ResearchDomain;
    pub use perspt_sdk::{
        AgentDomainPackage, ApprovalPolicy, DomainEvaluation, EffectKind, ModelId, ModelTransport,
        NodeTerminalOutcome, RiskClass, ToolEntry, WorkspaceSnapshot,
    };
}
