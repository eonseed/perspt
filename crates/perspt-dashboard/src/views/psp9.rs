//! Read-only projections over the PSP-9 ledger (system 14).
//!
//! Every dashboard page derives its data from `Psp9LedgerRow.event_json`;
//! the dashboard never mutates the ledger. `graph_revision` payloads carry a
//! full `perspt_sdk::WorkGraphRevision`; `tool_loop` payloads carry the
//! agent's `LoopEvent` JSON, tagged by an `"event"` field.

use std::collections::BTreeMap;

use perspt_sdk::workgraph::{EdgeKind, GraphRevisionReason, NodeClass, WorkNodeState};
use perspt_sdk::{LedgerEvent, WorkGraphRevision};
use perspt_store::Psp9LedgerRow;

/// One `candidate_measured` tool-loop event.
pub struct Measurement {
    pub sequence: i64,
    pub node_id: String,
    pub generation: u32,
    /// Candidate identity (PSP-10 Phase 2); empty on pre-PSP-10 rows.
    pub candidate_id: String,
    pub energy: f64,
    pub hard_pass: bool,
}

/// Search-forest counters folded from the search alphabet (PSP-10
/// system 28).
#[derive(Default)]
pub struct SearchCounters {
    pub forests_opened: usize,
    pub branches_forked: usize,
    pub branches_ineligible: usize,
    pub branches_not_selected: usize,
    pub branches_abandoned: usize,
    pub branches_selected: usize,
    pub branches_committed: usize,
    pub no_goods: usize,
    pub forests_closed: usize,
}

/// Typed projection of a session's ledger rows, in ledger order.
#[derive(Default)]
pub struct LedgerProjection {
    /// Every event count, including kinds not projected below.
    pub event_count: usize,
    /// Graph-revision lineage (each `graph_revision` event is a full snapshot).
    pub revisions: Vec<WorkGraphRevision>,
    /// Every `candidate_measured` event.
    pub measurements: Vec<Measurement>,
    /// Search-forest state counts (PSP-10).
    pub search: SearchCounters,
}

impl LedgerProjection {
    /// Parse ledger rows (already ordered by sequence by the store).
    pub fn from_rows(rows: &[Psp9LedgerRow]) -> Self {
        let mut projection = Self {
            event_count: rows.len(),
            ..Self::default()
        };
        for row in rows {
            let Ok(LedgerEvent::Custom { kind, payload }) = serde_json::from_str(&row.event_json)
            else {
                continue;
            };
            match kind.as_str() {
                "graph_revision" => {
                    if let Ok(revision) = serde_json::from_value(payload) {
                        projection.revisions.push(revision);
                    }
                }
                "tool_loop" => {
                    // PSP-10: unwrap the versioned envelope; an unknown
                    // version is display-only forensic data — the
                    // projection skips it rather than guessing.
                    let body = match perspt_sdk::ledger::tool_loop_body(&payload) {
                        Ok(
                            perspt_sdk::ledger::ToolLoopBody::Legacy(body)
                            | perspt_sdk::ledger::ToolLoopBody::V1(body),
                        ) => body,
                        Err(_) => continue,
                    };
                    if let Some(m) = parse_measurement(row.sequence, body) {
                        projection.measurements.push(m);
                    }
                    if let Some(event) = body.get("event").and_then(|event| event.as_str()) {
                        count_search_event(&mut projection.search, event);
                    }
                }
                _ => {}
            }
        }
        projection
    }

    /// The active (most recently ledgered) graph revision.
    pub fn latest_revision(&self) -> Option<&WorkGraphRevision> {
        self.revisions.last()
    }

    /// Latest measured energy per node id.
    pub fn latest_energy_by_node(&self) -> BTreeMap<&str, f64> {
        let mut latest = BTreeMap::new();
        for m in &self.measurements {
            latest.insert(m.node_id.as_str(), m.energy);
        }
        latest
    }
}

fn count_search_event(counters: &mut SearchCounters, event: &str) {
    match event {
        "search_opened" => counters.forests_opened += 1,
        "branch_forked" => counters.branches_forked += 1,
        "branch_ineligible" => counters.branches_ineligible += 1,
        "branch_not_selected" => counters.branches_not_selected += 1,
        "branch_abandoned" => counters.branches_abandoned += 1,
        "branch_selected" => counters.branches_selected += 1,
        "branch_committed" => counters.branches_committed += 1,
        "no_good_recorded" => counters.no_goods += 1,
        "search_closed" => counters.forests_closed += 1,
        _ => {}
    }
}

fn parse_measurement(sequence: i64, payload: &serde_json::Value) -> Option<Measurement> {
    if payload.get("event")?.as_str()? != "candidate_measured" {
        return None;
    }
    Some(Measurement {
        sequence,
        node_id: payload.get("node_id")?.as_str()?.to_string(),
        generation: u32::try_from(payload.get("generation")?.as_u64()?).ok()?,
        candidate_id: payload
            .get("candidate_id")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        energy: payload.get("energy")?.as_f64()?,
        hard_pass: payload.get("hard_pass")?.as_bool()?,
    })
}

/// Display label for a work-node execution state.
pub fn state_label(state: &WorkNodeState) -> &'static str {
    match state {
        WorkNodeState::Pending => "pending",
        WorkNodeState::Ready => "ready",
        WorkNodeState::Running => "running",
        WorkNodeState::Stable => "stable",
        WorkNodeState::Stopped { .. } => "stopped",
        WorkNodeState::Retired { .. } => "retired",
        WorkNodeState::BlockedOnSensor { .. } => "blocked",
    }
}

/// Whether a node still holds arriving work (non-terminal, dispatchable).
pub fn is_backlog_state(state: &WorkNodeState) -> bool {
    matches!(
        state,
        WorkNodeState::Pending | WorkNodeState::Ready | WorkNodeState::Running
    )
}

/// Display label for a graph-revision reason.
pub fn reason_label(reason: &GraphRevisionReason) -> &'static str {
    match reason {
        GraphRevisionReason::InitialPlan => "initial_plan",
        GraphRevisionReason::ExecutionUpdate => "execution_update",
        GraphRevisionReason::LocalRepair => "local_repair",
        GraphRevisionReason::ScopeExpansion => "scope_expansion",
        GraphRevisionReason::UserEdit => "user_edit",
        GraphRevisionReason::Replan => "replan",
    }
}

/// Display label for a node class.
pub fn class_label(class: NodeClass) -> &'static str {
    match class {
        NodeClass::Explore => "explore",
        NodeClass::Plan => "plan",
        NodeClass::Implement => "implement",
        NodeClass::Verify => "verify",
        NodeClass::Test => "test",
        NodeClass::Integrate => "integrate",
        NodeClass::Repair => "repair",
        NodeClass::Interface => "interface",
    }
}

/// Display label for a typed edge kind.
pub fn edge_kind_label(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::RequiresArtifact => "requires_artifact",
        EdgeKind::RequiresInterface => "requires_interface",
        EdgeKind::Tests => "tests",
        EdgeKind::Integrates => "integrates",
        EdgeKind::ConflictsWith => "conflicts_with",
        EdgeKind::DerivedFrom => "derived_from",
        EdgeKind::BlocksOnSensor => "blocks_on_sensor",
    }
}

/// First `len` characters of an id, safe on multibyte input.
pub fn short(value: &str, len: usize) -> String {
    value.chars().take(len).collect()
}

/// Truncate display text on a char boundary with an ellipsis.
pub fn truncate_chars(value: &str, len: usize) -> String {
    if value.chars().count() <= len {
        value.to_string()
    } else {
        let mut out: String = value.chars().take(len).collect();
        out.push('…');
        out
    }
}
