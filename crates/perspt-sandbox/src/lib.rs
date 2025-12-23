//! perspt-sandbox: Sandboxed command execution
//!
//! Provides the SandboxedCommand trait for executing commands in a controlled environment.

pub mod command;

pub use command::{BasicSandbox, CommandResult, SandboxedCommand};
