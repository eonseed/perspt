//! The external (MCP) dispatcher — the handler registry's fallback.
//!
//! Namespaced external names (`mcp.<server>.<tool>`) are discovered at
//! runtime, so they dispatch through one fallback handler instead of exact
//! registrations. Admission already happened (`admit_external_tool` against
//! the session's derived grant surface); the loop brackets every call in
//! the external-effect log because admitted entries are `durable`; and the
//! result enters the conversation as an untrusted observation. Replay never
//! re-invokes the server: recorded outputs come back from the ledger.

use std::sync::Arc;

use anyhow::Result;

use crate::candidate::CandidateWorkspace;
use crate::external_tools::ExternalToolRuntime;
use crate::toolloop::EffectOutcome;

use super::CandidateToolHandler;

pub struct ExternalDispatcher {
    runtime: Arc<tokio::sync::Mutex<ExternalToolRuntime>>,
}

impl ExternalDispatcher {
    pub fn new(runtime: Arc<tokio::sync::Mutex<ExternalToolRuntime>>) -> Self {
        Self { runtime }
    }
}

#[async_trait::async_trait]
impl CandidateToolHandler for ExternalDispatcher {
    async fn apply(
        &self,
        _workspace: &CandidateWorkspace,
        call: &perspt_sdk::ProviderToolCall,
        _entry: &perspt_sdk::ToolEntry,
    ) -> Result<EffectOutcome> {
        let mut runtime = self.runtime.lock().await;
        match runtime.call(&call.name, call.arguments.clone()).await {
            Ok(result) => Ok(EffectOutcome {
                output: format!(
                    "[untrusted MCP result{}] {}",
                    if result.is_error { ", tool error" } else { "" },
                    result.content
                ),
                mutated: false,
            }),
            // Uncertain completion is evidence for the model, not a loop
            // abort: the bracket stays open in the ledger for reconciliation.
            Err(error) => Ok(EffectOutcome {
                output: format!("tool failed: {error:#}"),
                mutated: false,
            }),
        }
    }
}
