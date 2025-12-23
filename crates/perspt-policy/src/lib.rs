//! perspt-policy: Starlark execution policy engine

pub mod engine;
pub mod sanitize;

pub use engine::{PolicyDecision, PolicyEngine};
pub use sanitize::{canonicalize, is_safe_for_auto_exec, sanitize_command, SanitizeResult};
