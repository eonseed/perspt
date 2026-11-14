//! Governed interactive handlers (`ask_user` and `run_repo_script`).

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use anyhow::{Context, Result};
use perspt_sdk::{ProviderToolCall, ToolEntry};
use serde::Deserialize;

use super::{CandidateHandlerRegistry, CandidateToolHandler};
use crate::candidate::CandidateWorkspace;
use crate::toolloop::EffectOutcome;

/// Host callback backing the `ask_user` tool.
#[async_trait::async_trait]
pub trait InteractionHandler: Send + Sync {
    async fn ask_user(&self, question: &str) -> Result<String>;
}

/// Default fail-safe handler used in non-interactive headless/CI runs.
#[derive(Debug, Clone, Copy, Default)]
pub struct NonInteractiveHandler;

#[async_trait::async_trait]
impl InteractionHandler for NonInteractiveHandler {
    async fn ask_user(&self, _question: &str) -> Result<String> {
        Ok(
            "interactive user input is unavailable in non-interactive mode; \
             proceed autonomously under declared constraints."
                .to_string(),
        )
    }
}

/// Closure-backed [`InteractionHandler`] for SDK callers and tests.
pub struct FnInteractionHandler<F>(pub F);

#[async_trait::async_trait]
impl<F> InteractionHandler for FnInteractionHandler<F>
where
    F: Fn(&str) -> Result<String> + Send + Sync + 'static,
{
    async fn ask_user(&self, question: &str) -> Result<String> {
        (self.0)(question)
    }
}

/// Async closure-backed [`InteractionHandler`] for SDK callers and tests.
pub struct AsyncFnInteractionHandler<F>(pub F);

#[async_trait::async_trait]
impl<F, Fut> InteractionHandler for AsyncFnInteractionHandler<F>
where
    F: Fn(String) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<String>> + Send + 'static,
{
    async fn ask_user(&self, question: &str) -> Result<String> {
        (self.0)(question.to_string()).await
    }
}

pub(crate) struct AskUserHandler;

#[async_trait::async_trait]
impl CandidateToolHandler for AskUserHandler {
    async fn apply(
        &self,
        workspace: &CandidateWorkspace,
        call: &ProviderToolCall,
        _entry: &ToolEntry,
    ) -> Result<EffectOutcome> {
        let question = call
            .arguments
            .get("question")
            .and_then(serde_json::Value::as_str)
            .context("ask_user requires a string 'question' argument")?;
        let output = workspace.interaction_handler().ask_user(question).await?;
        Ok(EffectOutcome {
            output,
            mutated: false,
            completed: true,
        })
    }
}

#[derive(Deserialize)]
struct PersptScriptsConfig {
    #[serde(default)]
    scripts: BTreeMap<String, toml::Value>,
}

pub(crate) struct RunRepoScriptHandler;

#[async_trait::async_trait]
impl CandidateToolHandler for RunRepoScriptHandler {
    async fn apply(
        &self,
        workspace: &CandidateWorkspace,
        call: &ProviderToolCall,
        _entry: &ToolEntry,
    ) -> Result<EffectOutcome> {
        let name = call
            .arguments
            .get("name")
            .and_then(serde_json::Value::as_str)
            .context("run_repo_script requires a string 'name' argument")?;
        let Some(raw_bytes) = workspace.overlay_bytes("perspt.toml")? else {
            return Ok(tool_failure("perspt.toml not found in candidate workspace"));
        };
        let raw_toml = match String::from_utf8(raw_bytes) {
            Ok(text) => text,
            Err(_) => return Ok(tool_failure("perspt.toml is not valid UTF-8")),
        };
        let config: PersptScriptsConfig = match toml::from_str(&raw_toml) {
            Ok(parsed) => parsed,
            Err(err) => return Ok(tool_failure(&format!("invalid perspt.toml: {err}"))),
        };
        let Some(spec) = config.scripts.get(name) else {
            return Ok(tool_failure(&format!(
                "script {name:?} is not declared under [scripts] in perspt.toml"
            )));
        };
        let Some(command) = script_command(spec) else {
            return Ok(tool_failure(&format!(
                "script {name:?} must be a command string or non-empty string array"
            )));
        };
        let exec = workspace.run_governed_verifier(&command, None).await?;
        if exec.success {
            Ok(EffectOutcome {
                output: exec.output,
                mutated: false,
                completed: true,
            })
        } else {
            Ok(tool_failure(&format!(
                "script {name:?} exited non-zero:\n{}",
                exec.output
            )))
        }
    }
}

fn script_command(value: &toml::Value) -> Option<String> {
    match value {
        toml::Value::String(cmd) => (!cmd.trim().is_empty()).then(|| cmd.clone()),
        toml::Value::Array(items) if !items.is_empty() => {
            let parts: Option<Vec<&str>> = items.iter().map(toml::Value::as_str).collect();
            parts.map(shell_words::join)
        }
        _ => None,
    }
}

fn tool_failure(message: &str) -> EffectOutcome {
    EffectOutcome {
        output: format!("tool failed: {message}"),
        mutated: false,
        completed: true,
    }
}

pub(crate) fn register(registry: &mut CandidateHandlerRegistry) {
    registry
        .register("ask_user", Arc::new(AskUserHandler))
        .expect("builtin ask_user registration must succeed");
    registry
        .register("run_repo_script", Arc::new(RunRepoScriptHandler))
        .expect("builtin run_repo_script registration must succeed");
}
