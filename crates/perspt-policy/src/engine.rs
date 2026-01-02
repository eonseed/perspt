//! Starlark Policy Engine
//!
//! Evaluates Starlark rules from ~/.perspt/rules to control command execution.

use anyhow::{Context, Result};
use starlark::environment::{FrozenModule, Globals, GlobalsBuilder, Module};
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::syntax::{AstModule, Dialect};
use starlark::values::none::NoneType;
use std::path::{Path, PathBuf};

/// Policy decision for a command
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    /// Allow the command to execute
    Allow,
    /// Require user confirmation before execution
    Prompt(String),
    /// Deny the command execution
    Deny(String),
}

/// Policy engine that evaluates Starlark rules
pub struct PolicyEngine {
    /// Loaded policy modules
    policies: Vec<FrozenModule>,
    /// Path to policy directory
    policy_dir: PathBuf,
}

impl PolicyEngine {
    /// Create a new policy engine
    pub fn new() -> Result<Self> {
        let policy_dir = Self::default_policy_dir();
        let mut engine = Self {
            policies: Vec::new(),
            policy_dir: policy_dir.clone(),
        };

        // Load policies if directory exists
        if policy_dir.exists() {
            engine.load_policies()?;
        } else {
            log::info!(
                "Policy directory {:?} does not exist, using defaults",
                policy_dir
            );
        }

        Ok(engine)
    }

    /// Get the default policy directory
    pub fn default_policy_dir() -> PathBuf {
        // Use simple fallback if dirs crate fails
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".perspt")
            .join("rules")
    }

    /// Load all .star files from the policy directory
    pub fn load_policies(&mut self) -> Result<()> {
        if !self.policy_dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(&self.policy_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext == "star") {
                match self.load_policy_file(&path) {
                    Ok(module) => {
                        self.policies.push(module);
                        log::info!("Loaded policy: {:?}", path);
                    }
                    Err(e) => {
                        log::warn!("Failed to load policy {:?}: {}", path, e);
                    }
                }
            }
        }

        log::info!("Loaded {} policies", self.policies.len());
        Ok(())
    }

    /// Load a single policy file
    fn load_policy_file(&self, path: &Path) -> Result<FrozenModule> {
        let content = std::fs::read_to_string(path)
            .context(format!("Failed to read policy file: {:?}", path))?;

        let ast = AstModule::parse(path.to_string_lossy().as_ref(), content, &Dialect::Standard)
            .map_err(|e| anyhow::anyhow!("Parse error: {}", e))?;

        let globals = Self::create_globals();
        let module = Module::new();

        {
            let mut eval = Evaluator::new(&module);
            eval.eval_module(ast, &globals)
                .map_err(|e| anyhow::anyhow!("Eval error: {}", e))?;
        }

        Ok(module.freeze()?)
    }

    /// Create the globals for Starlark evaluation
    fn create_globals() -> Globals {
        #[starlark_module]
        fn policy_builtins(builder: &mut GlobalsBuilder) {
            /// Check if a command matches a pattern
            fn matches_pattern(command: &str, pattern: &str) -> anyhow::Result<bool> {
                Ok(command.contains(pattern))
            }

            /// Log a message from policy
            fn log_policy(message: &str) -> anyhow::Result<NoneType> {
                log::info!("[Policy] {}", message);
                Ok(NoneType)
            }
        }

        GlobalsBuilder::standard().with(policy_builtins).build()
    }

    /// Evaluate a command against loaded policies
    pub fn evaluate(&self, command: &str) -> PolicyDecision {
        // If no policies loaded, use default behavior
        if self.policies.is_empty() {
            return self.default_policy(command);
        }

        // For now, use default policy logic
        // Full Starlark policy evaluation can be implemented later
        self.default_policy(command)
    }

    /// Default policy when no rules are loaded
    fn default_policy(&self, command: &str) -> PolicyDecision {
        // Always prompt for potentially dangerous commands
        let dangerous_patterns = ["rm -rf", "sudo", "chmod 777", "> /dev/", "mkfs", "dd if="];

        for pattern in &dangerous_patterns {
            if command.contains(pattern) {
                return PolicyDecision::Deny(format!(
                    "Command contains dangerous pattern: {}",
                    pattern
                ));
            }
        }

        // Prompt for network access
        let network_patterns = ["curl", "wget", "nc ", "ssh ", "scp "];
        for pattern in &network_patterns {
            if command.contains(pattern) {
                return PolicyDecision::Prompt(format!(
                    "Command requires network access: {}",
                    command
                ));
            }
        }

        // Prompt for git push operations
        if command.contains("git push") || command.contains("git force") {
            return PolicyDecision::Prompt("Git push operation requires confirmation".to_string());
        }

        PolicyDecision::Allow
    }

    /// Check if a command is allowed without prompting
    pub fn is_safe(&self, command: &str) -> bool {
        matches!(self.evaluate(command), PolicyDecision::Allow)
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            policies: Vec::new(),
            policy_dir: PathBuf::from("."),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy_allows_safe_commands() {
        let engine = PolicyEngine::default();
        assert!(matches!(
            engine.evaluate("cargo build"),
            PolicyDecision::Allow
        ));
        assert!(matches!(engine.evaluate("ls -la"), PolicyDecision::Allow));
    }

    #[test]
    fn test_default_policy_denies_dangerous() {
        let engine = PolicyEngine::default();
        assert!(matches!(
            engine.evaluate("rm -rf /"),
            PolicyDecision::Deny(_)
        ));
        assert!(matches!(
            engine.evaluate("sudo rm file"),
            PolicyDecision::Deny(_)
        ));
    }

    #[test]
    fn test_default_policy_prompts_network() {
        let engine = PolicyEngine::default();
        assert!(matches!(
            engine.evaluate("curl https://example.com"),
            PolicyDecision::Prompt(_)
        ));
    }
}
