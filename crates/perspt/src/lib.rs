//! # Perspt: Your Terminal's Window to the AI World
//!
//! A high-performance command-line interface (CLI) application that gives you a peek
//! into the mind of Large Language Models (LLMs). Built with Rust for speed and reliability,
//! it allows you to chat with various AI models from multiple providers directly in your terminal.
//!
//! This crate is a meta-package that re-exports the core libraries of the Perspt workspace.

pub use perspt_agent as agent;
pub use perspt_core as core;
pub use perspt_policy as policy;
pub use perspt_sandbox as sandbox;
pub use perspt_store as store;
