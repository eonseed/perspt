//! perspt-core: Core types and LLM provider abstraction

pub mod config;
pub mod llm_provider;
pub mod memory;

pub use config::Config;
pub use llm_provider::{GenAIProvider, EOT_SIGNAL};
pub use memory::ProjectMemory;
