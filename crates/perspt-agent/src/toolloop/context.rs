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
    /// The birth dependency of each message (parallel to the conversation):
    /// the state the page was created under. Resident assembly stamps pages
    /// from this log instead of relabelling them with the live root, so a
    /// state change invalidates stale optional pages rather than silently
    /// rewriting their provenance (Gate AF, spec :1438-1444).
    birth_deps: Vec<perspt_sdk::prompt::StateDependency>,
    current_dep: perspt_sdk::prompt::StateDependency,
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
        // Seed messages (instructions, the task) are session-invariant;
        // the loop switches the current dependency to the live root once
        // the baseline checkpoint exists.
        let initial = perspt_sdk::prompt::StateDependency::SessionInvariant;
        let birth_deps = vec![initial.clone(); projection.conversation().messages().len()];
        let mut context = Self {
            projection,
            birth_deps,
            current_dep: initial,
        };
        for name in activated {
            context.activate_tool(&name, recorder, log)?;
        }
        Ok(context)
    }

    /// Set the dependency stamped onto messages appended from now on —
    /// called at the baseline and after every accepted checkpoint.
    pub(crate) fn set_state_dependency(&mut self, dep: perspt_sdk::prompt::StateDependency) {
        self.current_dep = dep;
    }

    /// Stamp the context after the loop opens it: pages appended from here
    /// depend on the live state (the partial seed root under a continuing
    /// branch, the accepted root otherwise). A resumed projection restores
    /// each page's **recorded birth dependency** when the checkpoint
    /// carried one — never a wholesale relabel with the live root, which
    /// would launder stale pages as fresh (Gate AF) and turn the
    /// session-invariant head into a page the next acceptance kills.
    pub(crate) fn stamp_after_open(
        &mut self,
        was_resumed: bool,
        partial_seed_root: Option<&str>,
        accepted_root: &str,
        restored_birth_deps: &[perspt_sdk::prompt::StateDependency],
    ) {
        let dep = match partial_seed_root {
            Some(root) => perspt_sdk::prompt::StateDependency::PartialCheckpointRoot(root.into()),
            None => perspt_sdk::prompt::StateDependency::AcceptedRoot(accepted_root.into()),
        };
        if !was_resumed {
            self.set_state_dependency(dep);
            return;
        }
        if restored_birth_deps.len() == self.birth_deps.len() {
            // Exact provenance from the durable checkpoint: pages keep the
            // roots they were born under; new pages depend on the live
            // state.
            self.birth_deps = restored_birth_deps.to_vec();
            self.current_dep = dep;
        } else {
            self.restamp_legacy(dep);
        }
    }

    /// Legacy fallback for pre-provenance checkpoints: the instruction and
    /// task head is session-invariant (a later acceptance must never stale
    /// the mandatory head); every remaining restored page is deliberately
    /// stale because its true birth root is unknowable. Relabelling those
    /// pages with the current root would launder stale evidence as fresh.
    /// The one exception is an unresolved tool-call pair: it is pinned
    /// mandatory unconditionally, and a mandatory page whose dependency no
    /// longer holds refuses assembly outright (Gate AF) — so its messages
    /// carry the live dependency instead of an unresumable marker.
    pub(crate) fn restamp_legacy(&mut self, dep: perspt_sdk::prompt::StateDependency) {
        let open_pair = {
            let unresolved: BTreeSet<String> = self
                .projection
                .conversation()
                .unresolved_call_ids()
                .into_iter()
                .collect();
            let messages = self.projection.conversation().messages();
            let mut open_pair = vec![false; messages.len()];
            let mut open_calls: BTreeSet<String> = BTreeSet::new();
            for (index, message) in messages.iter().enumerate() {
                match message {
                    Message::AssistantToolCalls { calls }
                        if calls.iter().any(|call| unresolved.contains(&call.call_id)) =>
                    {
                        open_pair[index] = true;
                        open_calls.extend(calls.iter().map(|call| call.call_id.clone()));
                    }
                    Message::ToolResponse { call_id, .. } if open_calls.contains(call_id) => {
                        open_pair[index] = true;
                    }
                    _ => {}
                }
            }
            open_pair
        };
        for (index, entry) in self.birth_deps.iter_mut().enumerate() {
            *entry = if index < 2 {
                perspt_sdk::prompt::StateDependency::SessionInvariant
            } else if open_pair.get(index).copied().unwrap_or(false) {
                dep.clone()
            } else {
                perspt_sdk::prompt::StateDependency::AcceptedRoot(
                    "legacy-unavailable-provenance".into(),
                )
            };
        }
        self.current_dep = dep;
    }

    /// The birth dependency per message, parallel to `conversation()`.
    pub(crate) fn birth_deps(&self) -> &[perspt_sdk::prompt::StateDependency] {
        &self.birth_deps
    }

    /// Keep the birth log parallel to the projection after a delta (a
    /// compaction may replace messages; replacements carry the current
    /// dependency).
    fn sync_birth_deps(&mut self) {
        let len = self.projection.conversation().messages().len();
        while self.birth_deps.len() < len {
            self.birth_deps.push(self.current_dep.clone());
        }
        self.birth_deps.truncate(len);
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
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        self.sync_birth_deps();
        Ok(())
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
    // Concurrent node loops (and search branches) interleave their
    // conversation events on the one session ledger, so the fold routes
    // by **digest chain** instead of holding a single live projection:
    // collect every seed and delta record, walk the target's parent chain
    // back to its seed, and replay that one chain forward through the
    // sole writer (`apply`, which re-verifies digest continuity). A
    // parallel graph's ledger is then exactly as refoldable as a serial
    // one.
    let mut seeds: std::collections::BTreeMap<String, perspt_sdk::ConversationSeeded> =
        Default::default();
    let mut records: std::collections::BTreeMap<String, perspt_sdk::ConversationDeltaRecord> =
        Default::default();
    for row in rows {
        let Ok(LedgerEvent::Custom { kind, payload }) = serde_json::from_str(&row.event_json)
        else {
            continue;
        };
        if kind != "tool_loop" {
            continue;
        }
        // Authoritative refold: unknown envelope versions fail closed
        // (Gate AD), so a checkpoint can never be validated against a
        // stream this build cannot fully read.
        let decoded = super::envelope::decode_tool_loop(&payload)?;
        match decoded.event {
            LoopEvent::ConversationSeeded { seed } => {
                seeds.insert(seed.digest.clone(), seed);
            }
            LoopEvent::ConversationDelta { record } => {
                records.insert(record.digest.clone(), record);
            }
            _ => {}
        }
    }
    if !seeds.contains_key(target_digest) && !records.contains_key(target_digest) {
        // The session predates conversation deltas, or the checkpoint is
        // not derivable — the caller refuses resume either way.
        return Ok(None);
    }
    let mut chain: Vec<&perspt_sdk::ConversationDeltaRecord> = Vec::new();
    let mut cursor = target_digest.to_string();
    while !seeds.contains_key(&cursor) {
        let record = records.get(&cursor).with_context(|| {
            format!("conversation chain to {target_digest} is broken at {cursor}")
        })?;
        chain.push(record);
        cursor = record.prior_digest.clone();
        anyhow::ensure!(
            chain.len() <= records.len(),
            "conversation delta chain contains a cycle"
        );
    }
    let mut projection =
        ConversationProjection::from_seed(&seeds[&cursor]).map_err(|e| anyhow::anyhow!("{e}"))?;
    for record in chain.iter().rev() {
        projection
            .apply(record)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    anyhow::ensure!(
        projection.digest() == target_digest,
        "refolded conversation does not reproduce the checkpoint digest"
    );
    Ok(Some(projection))
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

    /// Concurrent node loops interleave conversation events on one
    /// ledger (A seed → B seed → A delta → B delta → A delta). The
    /// digest-chain fold reaches BOTH checkpoints; a single-live-projection
    /// fold would apply A's delta to B's projection and refuse the whole
    /// session.
    #[test]
    fn interleaved_parallel_conversations_refold_by_digest_chain() {
        let mut log_a = EventLog::new(true);
        let mut a = LoopContext::seed("system", "goal A", None, &mut log_a).unwrap();
        let mut log_b = EventLog::new(true);
        let mut b = LoopContext::seed("system", "goal B", None, &mut log_b).unwrap();
        a.push_user("a-one", None, &mut log_a).unwrap();
        b.push_user("b-one", None, &mut log_b).unwrap();
        a.push_user("a-two", None, &mut log_a).unwrap();
        let digest_a = a.digest().to_string();
        let digest_b = b.digest().to_string();

        let events_a = log_a.into_events();
        let events_b = log_b.into_events();
        // Ledger order: A seed, B seed, A delta, B delta, A delta.
        let interleaved = vec![
            events_a[0].clone(),
            events_b[0].clone(),
            events_a[1].clone(),
            events_b[1].clone(),
            events_a[2].clone(),
        ];
        let rows = rows_from(interleaved);

        let folded_a = refold_session_context(&rows, &digest_a)
            .unwrap()
            .expect("A's chain folds");
        assert_eq!(folded_a.conversation(), a.conversation());
        let folded_b = refold_session_context(&rows, &digest_b)
            .unwrap()
            .expect("B's chain folds");
        assert_eq!(folded_b.conversation(), b.conversation());
    }

    #[test]
    fn legacy_resume_never_launders_unknown_pages_as_current() {
        let mut log = EventLog::new(true);
        let mut context = LoopContext::seed("system", "goal", None, &mut log).unwrap();
        context
            .push_user("old observation", None, &mut log)
            .unwrap();
        context.stamp_after_open(true, None, "live-root", &[]);

        assert_eq!(
            &context.birth_deps()[..2],
            &[
                perspt_sdk::prompt::StateDependency::SessionInvariant,
                perspt_sdk::prompt::StateDependency::SessionInvariant,
            ]
        );
        assert_eq!(
            context.birth_deps()[2],
            perspt_sdk::prompt::StateDependency::AcceptedRoot(
                "legacy-unavailable-provenance".into()
            )
        );
        assert_ne!(
            context.birth_deps()[2],
            perspt_sdk::prompt::StateDependency::AcceptedRoot("live-root".into())
        );
    }

    /// A legacy checkpoint with an unresolved tool call must stay
    /// resumable: the unresolved pair is pinned mandatory unconditionally,
    /// and a mandatory page whose dependency never holds would refuse
    /// every assembly (Gate AF) — so exactly those messages carry the live
    /// dependency while other unknown-provenance pages stay demoted.
    #[test]
    fn legacy_resume_keeps_the_unresolved_pair_assemblable() {
        let mut log = EventLog::new(true);
        let mut context = LoopContext::seed("system", "goal", None, &mut log).unwrap();
        context
            .push_user("old observation", None, &mut log)
            .unwrap();
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
        context.stamp_after_open(true, None, "live-root", &[]);

        let deps = context.birth_deps();
        assert_eq!(
            deps[2],
            perspt_sdk::prompt::StateDependency::AcceptedRoot(
                "legacy-unavailable-provenance".into()
            )
        );
        assert_eq!(
            deps[3],
            perspt_sdk::prompt::StateDependency::AcceptedRoot("live-root".into()),
            "the unresolved tool-call page must depend on a state that holds"
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
