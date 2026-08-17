//! Governed direct-program execution (`exec`) for read-only inspection
//! tools. One shell-free program, classified as inspection, run in the
//! deny-network process sandbox against the candidate overlay.

use std::path::{Component, Path};
use std::sync::Arc;

use anyhow::{Context, Result};
use perspt_sandbox::{ProcessPolicy, ProcessSandbox, SandboxedCommand};

use super::{CandidateHandlerRegistry, CandidateToolHandler};
use crate::candidate::CandidateWorkspace;
use crate::toolloop::EffectOutcome;

struct InspectionExec;

#[async_trait::async_trait]
impl CandidateToolHandler for InspectionExec {
    async fn apply(
        &self,
        workspace: &CandidateWorkspace,
        call: &perspt_sdk::ProviderToolCall,
        _entry: &perspt_sdk::ToolEntry,
    ) -> Result<EffectOutcome> {
        let raw = call
            .arguments
            .get("command")
            .and_then(serde_json::Value::as_str)
            .context("exec requires a command string")?;
        let invocation = perspt_sdk::canonicalize(raw, ".");
        anyhow::ensure!(
            perspt_sdk::classify_tier(&invocation) == perspt_sdk::CommandTier::Inspection,
            "exec only admits commands classified as inspection"
        );
        let perspt_sdk::CommandInvocation::Program { program, args, .. } = invocation else {
            anyhow::bail!("exec does not admit shell composition");
        };
        validate_inspection_args(&args)?;
        let overlay = workspace.overlay_root().to_path_buf();
        let policy = if workspace.unisolated_verifiers_allowed() {
            ProcessPolicy::inspection(&overlay).best_effort()
        } else {
            ProcessPolicy::inspection(&overlay)
        };
        let sandbox = ProcessSandbox::new(program, args, policy)?;
        let execution = tokio::task::spawn_blocking(move || sandbox.execute())
            .await
            .context("inspection process worker panicked")??;
        let output = format!("{}{}", execution.stdout, execution.stderr);
        Ok(EffectOutcome {
            output: if execution.success() {
                output
            } else {
                format!("tool failed (exit {:?}): {output}", execution.exit_code)
            },
            mutated: false,
        })
    }
}

fn validate_inspection_args(args: &[String]) -> Result<()> {
    const PROCESS_SPAWNING_OR_WRITING_FLAGS: &[&str] = &[
        "-exec",
        "-execdir",
        "-ok",
        "-okdir",
        "-delete",
        "-fls",
        "-fprint",
        "-fprint0",
        "-fprintf",
        "--pre",
        "--hostname-bin",
        "--ext-diff",
        "--textconv",
    ];
    for argument in args {
        let lower = argument.to_ascii_lowercase();
        if PROCESS_SPAWNING_OR_WRITING_FLAGS
            .iter()
            .any(|flag| lower == *flag || lower.starts_with(&format!("{flag}=")))
        {
            anyhow::bail!("inspection argument is not read-only: {argument:?}");
        }
        if argument.starts_with('-') || argument == "." {
            continue;
        }
        let path = Path::new(argument);
        if path.is_absolute() || path.components().any(|part| part == Component::ParentDir) {
            anyhow::bail!("inspection argument escapes the workspace: {argument:?}");
        }
    }
    Ok(())
}

pub(super) fn register(registry: &mut CandidateHandlerRegistry) {
    registry
        .register("exec", Arc::new(InspectionExec))
        .expect("builtin exec handler is registered once");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspection_arguments_cannot_escape_the_candidate() {
        assert!(validate_inspection_args(&["src".into(), "--hidden".into()]).is_ok());
        assert!(validate_inspection_args(&["../secret".into()]).is_err());
        assert!(validate_inspection_args(&["/etc/passwd".into()]).is_err());
        assert!(validate_inspection_args(&[".".into(), "-exec".into(), "sh".into()]).is_err());
        assert!(validate_inspection_args(&["--pre=sh".into()]).is_err());
    }
}
