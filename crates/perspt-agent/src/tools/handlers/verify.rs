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
        let command = workspace.command_for(&call.name)?;
        let execution = workspace.run_governed_verifier(&command).await?;
        Ok(EffectOutcome {
            output: if execution.success {
                execution.output
            } else {
                format!("tool failed: {}", execution.output)
            },
            mutated: false,
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
