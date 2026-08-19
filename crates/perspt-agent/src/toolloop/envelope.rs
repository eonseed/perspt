//! Typed decoding of `tool_loop` ledger payloads (PSP-10 Gate AD).
//!
//! One fact, one layer, one decoder per version: rows without
//! `schema_version` use the strict legacy decoder, which refuses any
//! post-PSP-10 variant appearing unversioned; version 1 rows decode as
//! [`LoopEventEnvelopeV1`]; unknown versions fail authoritative replay and
//! resume closed (a forensic display may still show raw bytes).

use anyhow::Result;
use perspt_sdk::ledger::{tool_loop_body, ToolLoopBody};

use super::contract::{LoopEvent, LoopEventEnvelopeV1};

/// One decoded runtime event with its envelope version.
#[derive(Debug, Clone)]
pub struct DecodedLoopEvent {
    /// 0 marks a legacy (pre-PSP-10) row.
    pub version: u16,
    pub event: LoopEvent,
}

/// Whether an event variant existed before the PSP-10 envelope. The match
/// is exhaustive, so adding a variant forces this classification.
pub(crate) fn is_legacy_variant(event: &LoopEvent) -> bool {
    match event {
        LoopEvent::ConversationSeeded { .. }
        | LoopEvent::ConversationDelta { .. }
        | LoopEvent::TurnObserved { .. }
        | LoopEvent::ToolCallObserved { .. }
        | LoopEvent::ProposalObserved { .. }
        | LoopEvent::ProposalChecked { .. }
        | LoopEvent::EffectApplied { .. }
        | LoopEvent::ToolBatchConflict { .. }
        | LoopEvent::EffectDenied { .. }
        | LoopEvent::CandidateMeasured { .. }
        | LoopEvent::GateDecisionRecorded { .. }
        | LoopEvent::DecisionBoundRefused { .. }
        | LoopEvent::CandidateRestored { .. }
        | LoopEvent::EffectBoundaryMeasured { .. }
        | LoopEvent::ContextCheckpointCreated { .. }
        | LoopEvent::DurableCandidateCheckpoint { .. }
        | LoopEvent::RecoveryControlGranted { .. }
        | LoopEvent::RouteFailover { .. }
        | LoopEvent::RecoveryContained { .. } => true,
        LoopEvent::SearchOpened { .. }
        | LoopEvent::BranchForked { .. }
        | LoopEvent::BranchStrategySelected { .. }
        | LoopEvent::BranchObservation { .. }
        | LoopEvent::BranchCandidateMeasured { .. }
        | LoopEvent::PartialCheckpointed { .. }
        | LoopEvent::FrontierEpochStarted { .. }
        | LoopEvent::FrontierEntryServed { .. }
        | LoopEvent::BranchIneligible { .. }
        | LoopEvent::BranchNotSelected { .. }
        | LoopEvent::BranchAbandoned { .. }
        | LoopEvent::BranchSelected { .. }
        | LoopEvent::BranchCommitted { .. }
        | LoopEvent::NoGoodRecorded { .. }
        | LoopEvent::SearchClosed { .. }
        | LoopEvent::ContextWorkingSet { .. }
        | LoopEvent::ContextPagesSelected { .. }
        | LoopEvent::ContextMiss { .. }
        | LoopEvent::ContextPageRecalled { .. }
        | LoopEvent::ContextInfeasible { .. }
        | LoopEvent::PromptProgramCompiled { .. }
        | LoopEvent::PromptProgramInvoked { .. }
        | LoopEvent::ContextCompacted { .. } => false,
    }
}

/// Decode one `tool_loop` payload for authoritative consumption (replay,
/// resume, refold). Fails closed on unknown versions and on a legacy row
/// carrying a post-PSP-10 variant.
pub fn decode_tool_loop(payload: &serde_json::Value) -> Result<DecodedLoopEvent> {
    match tool_loop_body(payload).map_err(|e| anyhow::anyhow!("{e}"))? {
        ToolLoopBody::Legacy(body) => {
            let event: LoopEvent = serde_json::from_value(body.clone())?;
            anyhow::ensure!(
                is_legacy_variant(&event),
                "a PSP-10 event appeared in an unversioned legacy row; refusing"
            );
            Ok(DecodedLoopEvent { version: 0, event })
        }
        ToolLoopBody::V1(_) => {
            let envelope: LoopEventEnvelopeV1 = serde_json::from_value(payload.clone())?;
            anyhow::ensure!(
                envelope.schema_version == 1,
                "envelope version {} is not 1",
                envelope.schema_version
            );
            Ok(DecodedLoopEvent {
                version: 1,
                event: envelope.body,
            })
        }
    }
}
