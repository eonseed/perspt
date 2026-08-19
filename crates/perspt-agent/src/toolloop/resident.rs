//! Live resident-context assembly for the worker (PSP-10 Definition 6,
//! system 24).
//!
//! The conversation projection is the backing store; every message is one
//! immutable content-addressed page with typed obligation labels. Before
//! each model call the worker assembles the resident set: the mandatory
//! closure (instructions, the task, and the unresolved tool-pair tail) is
//! pinned, optional older pages are chosen by deterministic recency-
//! weighted greedy coverage, and an infeasible mandatory closure makes no
//! transport call at all.

use std::collections::BTreeMap;

use perspt_sdk::prompt::{
    assemble_resident, ContextBudget, ContextPage, DependencyEnv, ResidentOutcome, StateDependency,
    TokenAccountantRef,
};
use perspt_sdk::{Conversation, Message};

/// The worker's per-call context reserves, from `[context]`.
#[derive(Debug, Clone, Copy)]
pub struct ResidentReserves {
    pub output_reserve_tokens: u64,
    pub guard_reserve_tokens: u64,
    /// The synopsis frame bound `q`.
    pub frame_tokens: u64,
    /// Messages pinned at the tail of the conversation (the live working
    /// window), unresolved tool pairs included on top.
    pub pinned_tail: usize,
}

impl Default for ResidentReserves {
    fn default() -> Self {
        Self {
            output_reserve_tokens: 1_024,
            guard_reserve_tokens: 256,
            frame_tokens: 512,
            pinned_tail: 8,
        }
    }
}

/// Typed obligation labels for one message — derived from typed fields
/// (roles, tool names, call ids), never from model prose.
fn labels(index: usize, message: &Message) -> Vec<String> {
    let mut labels = vec![format!("turn:{index}")];
    match message {
        Message::System { .. } => labels.push("instruction".into()),
        Message::User { .. } => labels.push("task".into()),
        Message::Assistant { .. } => labels.push("assistant".into()),
        Message::AssistantToolCalls { calls } => {
            labels.extend(calls.iter().map(|call| format!("tool:{}", call.name)));
        }
        Message::ToolResponse { call_id, .. } => labels.push(format!("result:{call_id}")),
    }
    labels
}

/// One page per message, plus the mandatory page-id set: the leading
/// instruction/task pages, every unresolved tool pair, and the pinned
/// recency tail.
pub(super) fn conversation_pages(
    conversation: &Conversation,
    accepted_root: &str,
    pinned_tail: usize,
) -> (Vec<ContextPage>, Vec<String>) {
    let accountant = TokenAccountantRef::approx_bytes_v1();
    let messages = conversation.messages();
    let unresolved = conversation.unresolved_call_ids();
    let total = messages.len();
    let mut pages = Vec::with_capacity(total);
    let mut mandatory = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        let serialized = serde_json::to_string(message).unwrap_or_default();
        let page_id = perspt_sdk::ledger::content_hash(serialized.as_bytes());
        let tokens = accountant.count_message(&serialized);
        let pinned = index < 2
            || index + pinned_tail >= total
            || match message {
                Message::AssistantToolCalls { calls } => {
                    calls.iter().any(|call| unresolved.contains(&call.call_id))
                }
                _ => false,
            };
        if pinned {
            mandatory.push(page_id.clone());
        }
        pages.push(ContextPage {
            page_id,
            kind: match message {
                Message::System { .. } => "instruction",
                Message::User { .. } => "task",
                Message::Assistant { .. } => "assistant",
                Message::AssistantToolCalls { .. } => "tool_call",
                Message::ToolResponse { .. } => "tool_result",
            }
            .into(),
            source_hashes: Vec::new(),
            dependency: StateDependency::AcceptedRoot(accepted_root.into()),
            bytes: serialized.len() as u64,
            tokens,
            obligations: labels(index, message),
            freshness_turn: index as u64,
            depends_on: Vec::new(),
        });
    }
    (pages, mandatory)
}

/// Recency-weighted coverage: newer turns carry more weight, the task
/// itself the most.
fn weights(pages: &[ContextPage]) -> BTreeMap<String, f64> {
    let total = pages.len().max(1) as f64;
    let mut weights = BTreeMap::new();
    weights.insert("task".to_string(), 4.0);
    weights.insert("instruction".to_string(), 4.0);
    for (index, page) in pages.iter().enumerate() {
        let recency = 1.0 + (index as f64) / total;
        for label in &page.obligations {
            let entry = weights.entry(label.clone()).or_insert(0.0);
            if *entry < recency {
                *entry = recency;
            }
        }
    }
    weights
}

/// Assemble the worker's resident set for one model call.
pub(super) fn assemble_worker_resident(
    conversation: &Conversation,
    accepted_root: &str,
    window_tokens: u64,
    tool_reserve: u64,
    reserves: &ResidentReserves,
) -> perspt_sdk::Result<ResidentOutcome> {
    let (pages, mandatory) = conversation_pages(conversation, accepted_root, reserves.pinned_tail);
    let env = DependencyEnv {
        accepted_root: accepted_root.into(),
        partial_root: None,
        path_blobs: BTreeMap::new(),
    };
    let budget = ContextBudget {
        window_tokens: window_tokens.max(1),
        output_reserve: reserves.output_reserve_tokens,
        tool_reserve,
        guard_reserve: reserves.guard_reserve_tokens,
    };
    if budget.input_allowance().is_err() {
        // Reserves alone exceed the window: the same refusal as an
        // unfittable mandatory closure — no model call.
        return Ok(ResidentOutcome::Infeasible {
            required: reserves.output_reserve_tokens + tool_reserve + reserves.guard_reserve_tokens,
            allowance: 0,
        });
    }
    let weights = weights(&pages);
    assemble_resident(
        &pages,
        &mandatory,
        &env,
        &budget,
        0,
        reserves.frame_tokens.max(1),
        &weights,
    )
}
