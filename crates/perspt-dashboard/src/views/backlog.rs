//! Backlog view model: conditional-capacity diagnostics over the PSP-9
//! ledger. Every number here is a diagnostic projection, never a stability
//! claim — positive recurrence is only reported when every Theorem 9
//! hypothesis is evidenced, which this page does not attempt.

use std::collections::BTreeMap;

use super::psp9::{is_backlog_state, state_label, LedgerProjection};

/// Fallback potential for a backlog node with no measurement yet.
const UNMEASURED_POTENTIAL: f64 = 1.0;

/// Count of latest-revision nodes in one state.
pub struct StateCountRow {
    pub state: String,
    pub count: usize,
}

/// Last measured energy for one node.
pub struct NodeEnergyRow {
    pub node_id: String,
    pub state: String,
    pub last_v: String,
    pub in_backlog: bool,
}

/// View model for the backlog diagnostics page.
pub struct BacklogViewModel {
    pub session_id: String,
    pub state_counts: Vec<StateCountRow>,
    pub backlog_nodes: usize,
    pub unmeasured_backlog_nodes: usize,
    /// Arriving-potential gauge Φ(W), formatted.
    pub phi: String,
    /// Empirical drift (last measured V − first measured V), formatted.
    pub drift: String,
    pub measurement_count: usize,
    pub node_rows: Vec<NodeEnergyRow>,
    pub has_revision: bool,
}

impl BacklogViewModel {
    pub fn from_projection(session_id: String, projection: &LedgerProjection) -> Self {
        let latest_energy = projection.latest_energy_by_node();
        let latest = projection.latest_revision();
        let nodes: &[perspt_sdk::WorkNode] = latest.map(|r| r.nodes.as_slice()).unwrap_or(&[]);

        let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
        for node in nodes {
            *counts.entry(state_label(&node.state)).or_default() += 1;
        }
        let state_counts = counts
            .into_iter()
            .map(|(state, count)| StateCountRow {
                state: state.to_string(),
                count,
            })
            .collect();

        // Arriving-potential gauge Φ(W): sum over non-terminal
        // (pending/ready/running) nodes of the latest measured energy for
        // that node, falling back to 1.0 per unmeasured node. Computed
        // inline: `perspt_sdk::observability::backlog_gauge` sums the PSP-8
        // System 2 potential φ = 1 + V/ρ_gate + B, which needs the gate
        // threshold and per-workflow budget the ledger view does not carry.
        let mut phi = 0.0;
        let mut backlog_nodes = 0usize;
        let mut unmeasured_backlog_nodes = 0usize;
        for node in nodes.iter().filter(|n| is_backlog_state(&n.state)) {
            backlog_nodes += 1;
            match latest_energy.get(node.node_id.as_str()) {
                Some(v) => phi += v,
                None => {
                    unmeasured_backlog_nodes += 1;
                    phi += UNMEASURED_POTENTIAL;
                }
            }
        }

        // Empirical drift: last measured V minus first measured V across the
        // whole session, in ledger order. A diagnostic difference, not a
        // Foster–Lyapunov drift bound.
        let drift = match (
            projection.measurements.first(),
            projection.measurements.last(),
        ) {
            (Some(first), Some(last)) => format!("{:+.3}", last.energy - first.energy),
            _ => "—".into(),
        };

        let node_rows = nodes
            .iter()
            .map(|node| NodeEnergyRow {
                node_id: node.node_id.clone(),
                state: state_label(&node.state).to_string(),
                last_v: latest_energy
                    .get(node.node_id.as_str())
                    .map(|v| format!("{v:.3}"))
                    .unwrap_or_else(|| "unmeasured".into()),
                in_backlog: is_backlog_state(&node.state),
            })
            .collect();

        Self {
            session_id,
            state_counts,
            backlog_nodes,
            unmeasured_backlog_nodes,
            phi: format!("{phi:.3}"),
            drift,
            measurement_count: projection.measurements.len(),
            node_rows,
            has_revision: latest.is_some(),
        }
    }
}
