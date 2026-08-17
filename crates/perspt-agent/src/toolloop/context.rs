//! Write-ahead model context (Gate O): the loop's conversation is a pure
//! fold over recorded conversation deltas.
//!
//! Every context change — seed, message, tool activation, compaction — is
//! prepared as a digest-chained [`ConversationDeltaRecord`], emitted to the
//! ledger, and only then applied through
//! [`ConversationProjection::apply`], the sole writer. "Conversation is
//! rebuilt for each model turn from the immutable ledger" is therefore
//! structurally true: a from-scratch refold over the recorded deltas is the
//! mechanism check, and resume refuses a checkpoint whose conversation is
//! not derivable from the ledger.

use std::collections::BTreeSet;

use anyhow::{Context as _, Result};
use perspt_sdk::{
    Conversation, ConversationDelta, ConversationProjection, ConversationSeeded, LedgerEvent,
    Message, ProviderToolCall,
};

use super::{emit, EventLog, LoopEvent, LoopRecorder};

/// The loop's sole owner of model context and deferred tool activation.
pub(crate) struct LoopContext {
    projection: ConversationProjection,
}

impl LoopContext {
    /// Seed a fresh context (system prompt + goal), recording the seed.
    pub(crate) fn seed(
        system: &str,
        goal: &str,
        recorder: Option<&dyn LoopRecorder>,
        log: &mut EventLog,
    ) -> Result<Self> {
        let mut conversation = Conversation::with_system(system);
        conversation.push_user(goal.to_string());
        Self::from_conversation(conversation, Vec::new(), recorder, log)
    }

    /// Re-enter from a durable checkpoint's conversation clone. The clone
    /// was verified against the ledger fold before this call; re-seeding
    /// records the resumed context as the new fold base.
    pub(crate) fn resume(
        conversation: Conversation,
        restored_activated_tools: Vec<String>,
        recorder: Option<&dyn LoopRecorder>,
        log: &mut EventLog,
    ) -> Result<Self> {
        Self::from_conversation(conversation, restored_activated_tools, recorder, log)
    }

    fn from_conversation(
        conversation: Conversation,
        activated: Vec<String>,
        recorder: Option<&dyn LoopRecorder>,
        log: &mut EventLog,
    ) -> Result<Self> {
        let seed = ConversationSeeded::new(conversation).map_err(|e| anyhow::anyhow!("{e}"))?;
        emit(
            recorder,
            log,
            LoopEvent::ConversationSeeded { seed: seed.clone() },
        )?;
        let projection =
            ConversationProjection::from_seed(&seed).map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut context = Self { projection };
        for name in activated {
            context.activate_tool(&name, recorder, log)?;
        }
        Ok(context)
    }

    /// Write-ahead by construction: the delta record is ledgered before the
    /// projection applies it.
    fn record(
        &mut self,
        delta: ConversationDelta,
        recorder: Option<&dyn LoopRecorder>,
        log: &mut EventLog,
    ) -> Result<()> {
        let record = self
            .projection
            .prepare(delta)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        emit(
            recorder,
            log,
            LoopEvent::ConversationDelta {
                record: record.clone(),
            },
        )?;
        self.projection
            .apply(&record)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    pub(crate) fn push_message(
        &mut self,
        message: Message,
        recorder: Option<&dyn LoopRecorder>,
        log: &mut EventLog,
    ) -> Result<()> {
        self.record(ConversationDelta::Message { message }, recorder, log)
    }

    pub(crate) fn push_user(
        &mut self,
        content: impl Into<String>,
        recorder: Option<&dyn LoopRecorder>,
        log: &mut EventLog,
    ) -> Result<()> {
        self.push_message(
            Message::User {
                content: content.into(),
            },
            recorder,
            log,
        )
    }

    pub(crate) fn push_tool_calls(
        &mut self,
        calls: Vec<ProviderToolCall>,
        recorder: Option<&dyn LoopRecorder>,
        log: &mut EventLog,
    ) -> Result<()> {
        self.push_message(Message::AssistantToolCalls { calls }, recorder, log)
    }

    pub(crate) fn push_tool_response(
        &mut self,
        call_id: impl Into<String>,
        content: impl Into<String>,
        recorder: Option<&dyn LoopRecorder>,
        log: &mut EventLog,
    ) -> Result<()> {
        self.push_message(
            Message::ToolResponse {
                call_id: call_id.into(),
                content: content.into(),
            },
            recorder,
            log,
        )
    }

    pub(crate) fn activate_tool(
        &mut self,
        name: &str,
        recorder: Option<&dyn LoopRecorder>,
        log: &mut EventLog,
    ) -> Result<()> {
        if self.projection.activated_tools().contains(name) {
            return Ok(());
        }
        self.record(
            ConversationDelta::ToolActivated { name: name.into() },
            recorder,
            log,
        )
    }

    pub(crate) fn compact(
        &mut self,
        control: String,
        recorder: Option<&dyn LoopRecorder>,
        log: &mut EventLog,
    ) -> Result<()> {
        self.record(ConversationDelta::Compacted { control }, recorder, log)
    }

    pub(crate) fn conversation(&self) -> &Conversation {
        self.projection.conversation()
    }

    pub(crate) fn activated_tools(&self) -> &BTreeSet<String> {
        self.projection.activated_tools()
    }

    pub(crate) fn digest(&self) -> &str {
        self.projection.digest()
    }
}

/// Refold the model context recorded in a session's ledger rows, returning
/// the fold state whose rolling digest matches `target_digest` (deltas may
/// continue past a checkpoint before an interruption). `None` when no fold
/// state reaches the target — either the session predates conversation
/// deltas or the checkpoint is not derivable from the ledger.
pub(crate) fn refold_session_context(
    rows: &[perspt_store::Psp9LedgerRow],
    target_digest: &str,
) -> Result<Option<ConversationProjection>> {
    let mut live: Option<ConversationProjection> = None;
    let mut matched: Option<ConversationProjection> = None;
    for row in rows {
        let Ok(LedgerEvent::Custom { kind, payload }) = serde_json::from_str(&row.event_json)
        else {
            continue;
        };
        if kind != "tool_loop" {
            continue;
        }
        match serde_json::from_value::<LoopEvent>(payload) {
            Ok(LoopEvent::ConversationSeeded { seed }) => {
                let projection =
                    ConversationProjection::from_seed(&seed).map_err(|e| anyhow::anyhow!("{e}"))?;
                if projection.digest() == target_digest {
                    matched = Some(projection.clone());
                }
                live = Some(projection);
            }
            Ok(LoopEvent::ConversationDelta { record }) => {
                let projection = live
                    .as_mut()
                    .context("conversation delta recorded before any seed")?;
                projection
                    .apply(&record)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                if projection.digest() == target_digest {
                    matched = Some(projection.clone());
                }
            }
            _ => {}
        }
    }
    Ok(matched)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows_from(events: Vec<LoopEvent>) -> Vec<perspt_store::Psp9LedgerRow> {
        events
            .into_iter()
            .enumerate()
            .map(|(sequence, event)| perspt_store::Psp9LedgerRow {
                session_id: "s".into(),
                sequence: sequence as i64,
                event_json: serde_json::to_string(&LedgerEvent::Custom {
                    kind: "tool_loop".into(),
                    payload: serde_json::to_value(&event).unwrap(),
                })
                .unwrap(),
                prev_hash: String::new(),
                hash: String::new(),
            })
            .collect()
    }

    #[test]
    fn ledger_fold_reproduces_the_live_context_exactly() {
        let mut log = EventLog::new(true);
        let mut context = LoopContext::seed("system", "goal", None, &mut log).unwrap();
        context
            .push_tool_calls(
                vec![ProviderToolCall {
                    call_id: "c1".into(),
                    name: "read_file".into(),
                    arguments: serde_json::json!({"path": "a.rs"}),
                }],
                None,
                &mut log,
            )
            .unwrap();
        context
            .push_tool_response("c1", "contents", None, &mut log)
            .unwrap();
        context.activate_tool("lsp_query", None, &mut log).unwrap();
        let digest = context.digest().to_string();

        let rows = rows_from(log.into_events());
        let folded = refold_session_context(&rows, &digest).unwrap().unwrap();
        assert_eq!(folded.conversation(), context.conversation());
        assert_eq!(folded.activated_tools(), context.activated_tools());
    }

    #[test]
    fn a_removed_delta_makes_the_checkpoint_unreachable() {
        let mut log = EventLog::new(true);
        let mut context = LoopContext::seed("system", "goal", None, &mut log).unwrap();
        context.push_user("first", None, &mut log).unwrap();
        context.push_user("second", None, &mut log).unwrap();
        let digest = context.digest().to_string();

        let mut rows = rows_from(log.into_events());
        rows.remove(1);
        assert!(
            refold_session_context(&rows, &digest).is_err(),
            "a delta with a missing parent must fail closed, not skip"
        );
    }

    #[test]
    fn compaction_folds_to_the_exact_compacted_context() {
        let mut log = EventLog::new(true);
        let mut context = LoopContext::seed("system", "goal", None, &mut log).unwrap();
        context.push_user("progress", None, &mut log).unwrap();
        context
            .compact("PERSPECTIVE_CONTROL_FRAME_V1\n{}".into(), None, &mut log)
            .unwrap();
        let digest = context.digest().to_string();

        let rows = rows_from(log.into_events());
        let folded = refold_session_context(&rows, &digest).unwrap().unwrap();
        assert_eq!(folded.conversation(), context.conversation());
    }
}
