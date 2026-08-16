//! Starlark Policy Engine
//!
//! Evaluates Starlark rules from ~/.perspt/rules to control command execution.

use anyhow::{Context, Result};
use starlark::environment::{Globals, GlobalsBuilder, Module};
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
    policies: Vec<String>,
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
        // Use centralized path resolution with legacy fallback
        perspt_core::paths::resolve_policy_dir()
            .or_else(perspt_core::paths::policy_dir)
            .unwrap_or_else(|| PathBuf::from(".").join(".perspt").join("rules"))
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
    fn load_policy_file(&self, path: &Path) -> Result<String> {
        let content = std::fs::read_to_string(path)
            .context(format!("Failed to read policy file: {:?}", path))?;

        let executable = format!("{content}\nevaluate\n");
        let ast = AstModule::parse(
            path.to_string_lossy().as_ref(),
            executable,
            &Dialect::Standard,
        )
        .map_err(|e| anyhow::anyhow!("Parse error: {}", e))?;

        let globals = Self::create_globals();

        Module::with_temp_heap(|module| {
            {
                let mut eval = Evaluator::new(&module);
                eval.eval_module(ast, &globals)
                    .map_err(|e| anyhow::anyhow!("Eval error: {}", e))?;
            }

            Ok(content)
        })
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
        let mut decision = self.default_policy(command);
        for policy in &self.policies {
            let evaluated: anyhow::Result<PolicyDecision> = Module::with_temp_heap(|module| {
                let mut evaluator = Evaluator::new(&module);
                evaluator.set_max_callstack_size(32)?;
                evaluator.set_max_heap_size(8 * 1024 * 1024)?;
                evaluator.set_max_tick_count(25_000)?;
                let ast = AstModule::parse(
                    "policy.star",
                    format!("{policy}\nevaluate\n"),
                    &Dialect::Standard,
                )
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                let function = evaluator
                    .eval_module(ast, &Self::create_globals())
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                let argument = module.heap().alloc(command);
                let value = evaluator
                    .eval_function(function, &[argument], &[])
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                match value.unpack_str() {
                    Some("allow") => Ok(PolicyDecision::Allow),
                    Some("prompt") => Ok(PolicyDecision::Prompt(
                        "Starlark policy requires approval".into(),
                    )),
                    Some("deny") => Ok(PolicyDecision::Deny(
                        "Starlark policy denied command".into(),
                    )),
                    Some(other) => {
                        anyhow::bail!("Starlark policy returned unknown decision {other:?}")
                    }
                    None => match value.unpack_bool() {
                        Some(true) => Ok(PolicyDecision::Allow),
                        Some(false) => Ok(PolicyDecision::Deny(
                            "Starlark policy denied command".into(),
                        )),
                        None => anyhow::bail!(
                            "Starlark policy must return allow, prompt, deny, or bool"
                        ),
                    },
                }
            });
            let policy_decision = match evaluated {
                Ok(decision) => decision,
                Err(error) => {
                    return PolicyDecision::Deny(format!(
                        "Starlark policy evaluation failed: {error}"
                    ));
                }
            };
            decision = stricter_decision(decision, policy_decision);
        }
        decision
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

fn stricter_decision(left: PolicyDecision, right: PolicyDecision) -> PolicyDecision {
    match (left, right) {
        (decision @ PolicyDecision::Deny(_), _) | (_, decision @ PolicyDecision::Deny(_)) => {
            decision
        }
        (decision @ PolicyDecision::Prompt(_), _) | (_, decision @ PolicyDecision::Prompt(_)) => {
            decision
        }
        _ => PolicyDecision::Allow,
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

    #[test]
    fn loaded_starlark_policy_is_actually_evaluated() {
        let directory = std::env::temp_dir().join(format!("perspt-policy-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("deny.star");
        std::fs::write(
            &path,
            "def evaluate(command):\n    return 'deny' if 'forbidden' in command else 'allow'\n",
        )
        .unwrap();
        let mut engine = PolicyEngine {
            policies: Vec::new(),
            policy_dir: directory.clone(),
        };
        engine.load_policies().unwrap();
        assert_eq!(engine.evaluate("ls"), PolicyDecision::Allow);
        assert!(matches!(
            engine.evaluate("forbidden operation"),
            PolicyDecision::Deny(_)
        ));
        let _ = std::fs::remove_dir_all(directory);
    }
}
