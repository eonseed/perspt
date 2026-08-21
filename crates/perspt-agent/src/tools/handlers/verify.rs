//! Domain verifier commands (`run_test`, `run_build`, `run_formatter`),
//! executed inside the governed verifier sandbox the candidate provides.

use std::sync::Arc;

use anyhow::Result;

use super::{CandidateHandlerRegistry, CandidateToolHandler};
use crate::candidate::CandidateWorkspace;
use crate::toolloop::EffectOutcome;

struct VerifierCommand;

#[async_trait::async_trait]
impl CandidateToolHandler for VerifierCommand {
    async fn apply(
        &self,
        workspace: &CandidateWorkspace,
        call: &perspt_sdk::ProviderToolCall,
        _entry: &perspt_sdk::ToolEntry,
    ) -> Result<EffectOutcome> {
        let commands = workspace.commands_for(&call.name, &call.arguments)?;
        let multi = commands.len() > 1;
        let mut success = true;
        let mut output = String::new();
        for (plugin, command, stage) in commands {
            let execution = workspace
                .run_governed_verifier(&command, Some(stage))
                .await?;
            if multi {
                output.push_str(&format!("== {plugin} ==\n"));
            }
            output.push_str(&execution.output);
            output.push('\n');
            success &= execution.success;
        }
        Ok(EffectOutcome {
            output: if success {
                output
            } else {
                format!("tool failed: {output}")
            },
            mutated: false,
            completed: true,
        })
    }
}

pub(super) fn register(registry: &mut CandidateHandlerRegistry) {
    for name in ["run_test", "run_build", "run_formatter"] {
        registry
            .register(name, Arc::new(VerifierCommand))
            .expect("builtin verifier commands are registered once");
    }
}
