//! Glob-like scope patterns shared by capabilities and grant policies
//! (split from `capability.rs` under the file-length rules).

use serde::{Deserialize, Serialize};

/// A glob-like path pattern. `matches` uses a simple prefix/suffix/`*` rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathPattern(pub String);

impl PathPattern {
    pub fn matches(&self, path: &str) -> bool {
        glob_match(&self.0, path)
    }
}

/// A command pattern matched against the canonical program name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandPattern(pub String);

impl CommandPattern {
    pub fn matches(&self, program: &str) -> bool {
        glob_match(&self.0, program)
    }
}

/// A network host/URL pattern.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkPattern(pub String);

impl NetworkPattern {
    pub fn matches(&self, target: &str) -> bool {
        glob_match(&self.0, target)
    }
}

/// Minimal glob: supports a single trailing `*`, leading `*`, or exact match.
fn glob_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return value.starts_with(prefix);
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return value.ends_with(suffix);
    }
    pattern == value
}
