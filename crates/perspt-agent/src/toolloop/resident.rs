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
    /// Live paging on (default). The evaluation ladder's paging arm turns
    /// it off to measure the feature's contribution.
    pub paging_enabled: bool,
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
            paging_enabled: true,
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

/// The conversation partitioned into atomic pages: an assistant tool-call
/// message and **all** of its result messages form one page (spec
/// :1431-1432); every other message is its own page. A page is the unit of
/// residency, eviction, and recall — a pair can never be half-evicted.
pub(super) struct PageGroups {
    /// Member message indices per page, in first-member order.
    pub(super) members: Vec<Vec<usize>>,
    /// Owning page id per message index (parallel to the conversation).
    pub(super) owners: Vec<String>,
}

/// Group the conversation's messages into atomic pages. A `ToolResponse`
/// joins the most recent tool-call message that issued its `call_id`; an
/// orphan response (none did) is its own page.
pub(super) fn page_groups(conversation: &Conversation) -> PageGroups {
    let messages = conversation.messages();
    let mut members: Vec<Vec<usize>> = Vec::new();
    let mut issued: BTreeMap<String, usize> = BTreeMap::new();
    for (index, message) in messages.iter().enumerate() {
        match message {
            Message::AssistantToolCalls { calls } => {
                let group = members.len();
                members.push(vec![index]);
                for call in calls {
                    issued.insert(call.call_id.clone(), group);
                }
            }
            Message::ToolResponse { call_id, .. } => match issued.get(call_id) {
                Some(&group) => members[group].push(index),
                None => members.push(vec![index]),
            },
            _ => members.push(vec![index]),
        }
    }
    let mut owners = vec![String::new(); messages.len()];
    for group in &members {
        let serialized: String = group
            .iter()
            .map(|&i| serialize_message(&messages[i]))
            .collect();
        let page_id = perspt_sdk::ledger::content_hash(serialized.as_bytes());
        for &index in group {
            owners[index] = page_id.clone();
        }
    }
    PageGroups { members, owners }
}

/// The single serialization rule page identity hashes over.
fn serialize_message(message: &Message) -> String {
    serde_json::to_string(message).unwrap_or_default()
}

/// The page kind of one group.
fn group_kind(messages: &[Message], group: &[usize]) -> &'static str {
    match &messages[group[0]] {
        Message::System { .. } => "instruction",
        Message::User { .. } => "task",
        Message::Assistant { .. } => "assistant",
        Message::AssistantToolCalls { .. } | Message::ToolResponse { .. } => "tool_pair",
    }
}

/// The model-facing content of one page, returned by `context_recall`. A
/// pair page returns the call JSON plus each result labeled by call id.
fn group_content(messages: &[Message], group: &[usize]) -> String {
    let mut parts = Vec::new();
    for &index in group {
        match &messages[index] {
            Message::System { content }
            | Message::User { content }
            | Message::Assistant { content } => parts.push(content.clone()),
            Message::AssistantToolCalls { calls } => parts.push(format!(
                "calls: {}",
                serde_json::to_string(calls).unwrap_or_default()
            )),
            Message::ToolResponse { call_id, content } => {
                parts.push(format!("result {call_id}:\n{content}"));
            }
        }
    }
    parts.join("\n")
}

/// Every page of the projection keyed by page id — the `context_recall`
/// store. The projection is the backing store (Definition 6): eviction
/// removes a page from the transport view, never from here.
pub(super) fn page_contents(conversation: &Conversation) -> BTreeMap<String, String> {
    let groups = page_groups(conversation);
    let messages = conversation.messages();
    groups
        .members
        .iter()
        .map(|group| {
            (
                groups.owners[group[0]].clone(),
                group_content(messages, group),
            )
        })
        .collect()
}

/// Tool-call arguments above this size are elided when their page is
/// evicted (whole-file writes dominate stale context cost).
const EVICTED_ARGS_BYTES: usize = 256;

fn tombstone(page_id: &str, kind: &str) -> String {
    format!(
        "[{kind} page {page_id} evicted from the resident context; call \
         context_recall with this page_id to restore it]"
    )
}

/// The transport view of the conversation under the assembled resident
/// set: resident pages verbatim, evicted pages tombstoned **in place** so
/// message structure and tool pairing survive every provider's validation.
/// Membership is per owning page, so a tool pair is resident or evicted as
/// one unit and its tombstones all name the same recallable page id.
/// Returns the view and how many pages were tombstoned.
pub(super) fn resident_view(
    conversation: &Conversation,
    resident: &std::collections::BTreeSet<String>,
) -> (Conversation, usize) {
    let groups = page_groups(conversation);
    let mut view = Conversation::default();
    let mut evicted_pages: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for (index, message) in conversation.messages().iter().enumerate() {
        let page_id = &groups.owners[index];
        if resident.contains(page_id) {
            view.push(message.clone());
            continue;
        }
        evicted_pages.insert(page_id.as_str());
        view.push(evicted_message(message, page_id));
    }
    let evicted = evicted_pages.len();
    (view, evicted)
}

/// The in-place tombstone for one evicted page. Tool-call structure is
/// preserved; only oversized arguments are elided.
fn evicted_message(message: &Message, page_id: &str) -> Message {
    match message {
        Message::System { .. } => Message::System {
            content: tombstone(page_id, "instruction"),
        },
        Message::User { .. } => Message::User {
            content: tombstone(page_id, "task"),
        },
        Message::Assistant { .. } => Message::Assistant {
            content: tombstone(page_id, "assistant"),
        },
        Message::ToolResponse { call_id, .. } => Message::ToolResponse {
            call_id: call_id.clone(),
            content: tombstone(page_id, "tool_pair"),
        },
        Message::AssistantToolCalls { calls } => Message::AssistantToolCalls {
            calls: calls
                .iter()
                .map(|call| {
                    if call.arguments.to_string().len() <= EVICTED_ARGS_BYTES {
                        call.clone()
                    } else {
                        perspt_sdk::ProviderToolCall {
                            call_id: call.call_id.clone(),
                            name: call.name.clone(),
                            arguments: serde_json::json!({ "_evicted_page": page_id }),
                        }
                    }
                })
                .collect(),
        },
    }
}

/// One atomic page per group, plus the mandatory page-id set: the leading
/// instruction/task pages, every unresolved tool pair, and the pinned
/// recency tail. A pair page's cost, labels, and freshness are member
/// aggregates, so residency decisions always account the whole pair.
pub(super) fn conversation_pages(
    conversation: &Conversation,
    birth_deps: &[StateDependency],
    pinned_tail: usize,
) -> (Vec<ContextPage>, Vec<String>) {
    let accountant = TokenAccountantRef::approx_bytes_v1();
    let messages = conversation.messages();
    let unresolved = conversation.unresolved_call_ids();
    let total = messages.len();
    let groups = page_groups(conversation);
    let mut pages = Vec::with_capacity(groups.members.len());
    let mut mandatory = Vec::new();
    for group in &groups.members {
        let page_id = groups.owners[group[0]].clone();
        let mut tokens = 0u64;
        let mut bytes = 0u64;
        let mut obligations = Vec::new();
        let mut pinned = false;
        for &index in group {
            let message = &messages[index];
            let serialized = serialize_message(message);
            tokens += accountant.count_message(&serialized);
            bytes += serialized.len() as u64;
            obligations.extend(labels(index, message));
            pinned = pinned
                || index < 2
                || index + pinned_tail >= total
                || match message {
                    Message::AssistantToolCalls { calls } => {
                        calls.iter().any(|call| unresolved.contains(&call.call_id))
                    }
                    _ => false,
                };
        }
        if pinned {
            mandatory.push(page_id.clone());
        }
        pages.push(ContextPage {
            page_id,
            kind: group_kind(messages, group).into(),
            source_hashes: Vec::new(),
            // The page's birth dependency (of its first member), never the
            // live root — a state change invalidates, it does not rewrite.
            dependency: birth_deps
                .get(group[0])
                .cloned()
                .unwrap_or(StateDependency::SessionInvariant),
            bytes,
            tokens,
            obligations,
            freshness_turn: group.iter().copied().max().unwrap_or(0) as u64,
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

/// Conservative reserve for the transport-only page-index note appended
/// when pages are evicted (C4); charged inside the allowance like the
/// tombstones, and bounded by the note's own size cap.
const INDEX_NOTE_RESERVE: u64 = 768;

/// The exact token cost the page contributes to the wire **when evicted**:
/// every member message rendered in tombstone form. Wire cost of a request
/// is `Σ_all evicted_cost + Σ_resident (tokens − evicted_cost)`, so passing
/// the first sum as instruction cost and charging marginals per page makes
/// Definition 6's allowance bound the real composed request.
fn evicted_cost(
    accountant: &TokenAccountantRef,
    messages: &[Message],
    group: &[usize],
    page_id: &str,
) -> u64 {
    group
        .iter()
        .map(|&index| {
            let rendered = evicted_message(&messages[index], page_id);
            accountant.count_message(&serialize_message(&rendered))
        })
        .sum()
}

/// Assemble the worker's resident set for one model call, budgeting the
/// composed request: tombstones for every page plus the index-note reserve
/// form the base cost, and each page is charged its marginal cost over its
/// own tombstone (`saturating_sub` keeps the bound conservative).
pub(super) fn assemble_worker_resident(
    conversation: &Conversation,
    birth_deps: &[StateDependency],
    accepted_root: &str,
    partial_root: Option<&str>,
    window_tokens: u64,
    tool_reserve: u64,
    reserves: &ResidentReserves,
) -> perspt_sdk::Result<ResidentOutcome> {
    let accountant = TokenAccountantRef::approx_bytes_v1();
    let (mut pages, mandatory) = conversation_pages(conversation, birth_deps, reserves.pinned_tail);
    let groups = page_groups(conversation);
    let messages = conversation.messages();
    let mut tombstone_base = INDEX_NOTE_RESERVE;
    for (page, group) in pages.iter_mut().zip(&groups.members) {
        let evicted = evicted_cost(&accountant, messages, group, &page.page_id);
        tombstone_base += evicted;
        page.tokens = page.tokens.saturating_sub(evicted);
    }
    let env = DependencyEnv {
        accepted_root: accepted_root.into(),
        partial_root: partial_root.map(Into::into),
        path_blobs: BTreeMap::new(),
    };
    let budget = ContextBudget {
        window_tokens: window_tokens.max(1),
        output_reserve: reserves.output_reserve_tokens,
        tool_reserve,
        guard_reserve: reserves.guard_reserve_tokens,
    };
    let Ok(allowance) = budget.input_allowance() else {
        // Reserves alone exceed the window: the same refusal as an
        // unfittable mandatory closure — no model call.
        return Ok(ResidentOutcome::Infeasible {
            required: reserves.output_reserve_tokens + tool_reserve + reserves.guard_reserve_tokens,
            allowance: 0,
        });
    };
    let weights = weights(&pages);
    let outcome = assemble_resident(
        &pages,
        &mandatory,
        &env,
        &budget,
        tombstone_base,
        reserves.frame_tokens.max(1),
        &weights,
    )?;
    Ok(match outcome {
        ResidentOutcome::Assembled(assembled) => ResidentOutcome::Assembled(admit_large_pages(
            assembled,
            &pages,
            &env,
            allowance,
            tombstone_base,
            reserves.frame_tokens.max(1),
        )),
        infeasible => infeasible,
    })
}

/// Large-page admission: a page whose marginal cost exceeds the synopsis
/// frame bound `q` is never an optional candidate for greedy selection, so
/// a big fresh tool result would otherwise always be evicted. Admit such
/// pages newest-first while the composed request still fits.
fn admit_large_pages(
    assembled: perspt_sdk::prompt::ResidentContext,
    pages: &[ContextPage],
    env: &DependencyEnv,
    allowance: u64,
    tombstone_base: u64,
    frame_tokens: u64,
) -> perspt_sdk::prompt::ResidentContext {
    let mut resident = assembled;
    let mut used: u64 = tombstone_base + resident.pages.iter().map(|p| p.tokens).sum::<u64>();
    let resident_ids: std::collections::BTreeSet<&str> = resident
        .pages
        .iter()
        .map(|page| page.page_id.as_str())
        .collect();
    let mut large: Vec<&ContextPage> = pages
        .iter()
        .filter(|page| {
            !resident_ids.contains(page.page_id.as_str())
                && page.tokens > frame_tokens
                && page.dependency_holds(env)
        })
        .collect();
    large.sort_by(|a, b| {
        b.freshness_turn
            .cmp(&a.freshness_turn)
            .then_with(|| a.page_id.cmp(&b.page_id))
    });
    let mut admitted = false;
    for page in large {
        if used + page.tokens > allowance {
            continue;
        }
        used += page.tokens;
        resident.pages.push(page.clone());
        admitted = true;
    }
    if admitted {
        resident.resident_digest = perspt_sdk::prompt::resident_page_digest(
            resident.pages.iter().map(|page| page.page_id.as_str()),
        );
    }
    resident
}
