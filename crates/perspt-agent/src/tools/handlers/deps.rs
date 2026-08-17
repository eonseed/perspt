//! Governed dependency mutation (`mutate_dependencies`, Gate J).
//!
//! The action resolves through the detected language plugin's
//! `dependency_commands` and is re-checked against its
//! `dependency_command_policy` (fail closed). Resolution needs the network,
//! so the command runs outside the deny-network verifier profile — but
//! shell-free, in the candidate overlay, with the plugin's declared
//! manifest/lockfile footprint journaled before and marked promotable after.
//! The loop brackets the whole call in the external-effect log because the
//! catalog entry is `durable`.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use perspt_core::plugin::DependencyAction;

use super::{CandidateHandlerRegistry, CandidateToolHandler};
use crate::candidate::CandidateWorkspace;
use crate::toolloop::EffectOutcome;

const OUTPUT_CAP: usize = 32 * 1024;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(600);

struct MutateDependencies;

#[async_trait::async_trait]
impl CandidateToolHandler for MutateDependencies {
    async fn apply(
        &self,
        workspace: &CandidateWorkspace,
        call: &perspt_sdk::ProviderToolCall,
        _entry: &perspt_sdk::ToolEntry,
    ) -> Result<EffectOutcome> {
        let (action, packages, dev) = parse_arguments(call)?;
        let registry = perspt_core::PluginRegistry::new();
        let overlay = workspace.overlay_root().to_path_buf();
        let plugin = registry
            .detect_all(&overlay)
            .into_iter()
            .find(|plugin| {
                !plugin
                    .dependency_commands(action, &packages, dev)
                    .is_empty()
            })
            .context("no detected language plugin supports governed dependency mutation")?;
        let commands = plugin.dependency_commands(action, &packages, dev);
        for command in &commands {
            anyhow::ensure!(
                plugin.dependency_command_policy(command)
                    != perspt_core::types::CommandPolicyDecision::Deny,
                "plugin {} denies dependency command {command:?}",
                plugin.name()
            );
        }

        let footprint = plugin.dependency_files();
        workspace.admit_external_paths(&footprint)?;
        let before: Vec<Option<Vec<u8>>> = footprint
            .iter()
            .map(|rel| workspace.overlay_bytes(rel))
            .collect::<Result<_>>()?;

        let mut transcript = String::new();
        for command in &commands {
            let (success, output) = run_dependency_command(command, &overlay).await?;
            transcript.push_str(&output);
            if !success {
                return Ok(EffectOutcome {
                    output: format!("tool failed: {command}: {transcript}"),
                    mutated: false,
                });
            }
        }

        let mut changed = Vec::new();
        for (rel, prior) in footprint.iter().zip(before) {
            if workspace.overlay_bytes(rel)? != prior {
                changed.push(rel.clone());
            }
        }
        workspace.note_mutated_paths(&changed)?;
        Ok(EffectOutcome {
            output: format!(
                "{}\nchanged: {}",
                transcript.trim_end(),
                if changed.is_empty() {
                    "(nothing)".to_string()
                } else {
                    changed.join(", ")
                }
            ),
            mutated: !changed.is_empty(),
        })
    }
}

fn parse_arguments(
    call: &perspt_sdk::ProviderToolCall,
) -> Result<(DependencyAction, Vec<String>, bool)> {
    let action = call
        .arguments
        .get("action")
        .and_then(|v| v.as_str())
        .and_then(DependencyAction::parse)
        .context("action must be add, remove, or update")?;
    let packages: Vec<String> = call
        .arguments
        .get("packages")
        .and_then(|v| v.as_array())
        .context("packages must be an array of package names")?
        .iter()
        .filter_map(|v| v.as_str())
        .map(str::to_string)
        .collect();
    anyhow::ensure!(!packages.is_empty(), "packages must not be empty");
    for package in &packages {
        validate_package_name(package)?;
    }
    let dev = call
        .arguments
        .get("dev")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    Ok((action, packages, dev))
}

/// Package names are interpolated into a command line, so anything that
/// could smuggle a flag or a second argument is refused.
fn validate_package_name(package: &str) -> Result<()> {
    anyhow::ensure!(
        !package.is_empty() && !package.starts_with('-'),
        "invalid package name {package:?}"
    );
    anyhow::ensure!(
        package
            .chars()
            .all(|c| { c.is_ascii_alphanumeric() || "_.@/:^~<>=,+[]".contains(c) || c == '-' }),
        "package name contains forbidden characters: {package:?}"
    );
    Ok(())
}

/// Run one shell-free dependency command in the overlay with the network
/// available, bounded output, and a hard timeout.
async fn run_dependency_command(
    command: &str,
    overlay: &std::path::Path,
) -> Result<(bool, String)> {
    let parts = shell_words::split(command).context("splitting dependency command")?;
    let (program, args) = parts.split_first().context("dependency command is empty")?;
    let child = tokio::process::Command::new(program)
        .args(args)
        .current_dir(overlay)
        .kill_on_drop(true)
        .output();
    let output = tokio::time::timeout(COMMAND_TIMEOUT, child)
        .await
        .with_context(|| format!("dependency command timed out: {command}"))?
        .with_context(|| format!("spawning {command}"))?;
    let mut text = format!(
        "$ {command}\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if text.len() > OUTPUT_CAP {
        text.truncate(OUTPUT_CAP);
        text.push_str("\n… (truncated)");
    }
    Ok((output.status.success(), text))
}

pub(super) fn register(registry: &mut CandidateHandlerRegistry) {
    registry
        .register("mutate_dependencies", Arc::new(MutateDependencies))
        .expect("builtin dependency handler is registered once");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_names_cannot_smuggle_flags() {
        assert!(validate_package_name("serde").is_ok());
        assert!(validate_package_name("serde_json@1.0").is_ok());
        assert!(validate_package_name("python-dateutil>=2.8").is_ok());
        assert!(validate_package_name("--allow-dirty").is_err());
        assert!(validate_package_name("a b").is_err());
        assert!(validate_package_name("$(rm -rf /)").is_err());
        assert!(validate_package_name("").is_err());
    }

    #[tokio::test]
    async fn unsupported_workspace_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = CandidateWorkspace::create(dir.path(), "n1", 0, "r1").unwrap();
        let call = perspt_sdk::ProviderToolCall {
            call_id: "c1".into(),
            name: "mutate_dependencies".into(),
            arguments: serde_json::json!({"action": "add", "packages": ["serde"]}),
        };
        let entry = perspt_sdk::base_entries()
            .into_iter()
            .find(|entry| entry.name == "mutate_dependencies")
            .unwrap();
        let result = MutateDependencies.apply(&workspace, &call, &entry).await;
        assert!(result.is_err(), "no plugin detects an empty workspace");
    }
}
