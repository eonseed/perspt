//! Session detail view model: session record plus a summary projected from
//! the PSP-9 ledger (latest graph revision and measured-energy trajectory).

use super::normalize_status;
use super::psp9::{class_label, state_label, truncate_chars, LedgerProjection};

/// One node of the latest graph revision.
pub struct NodeSummaryRow {
    pub node_id: String,
    pub generation: u32,
    pub state: String,
    pub node_class: String,
    pub goal: String,
}

/// View model for the session detail summary page.
pub struct SessionDetailViewModel {
    pub session_id: String,
    pub task: String,
    pub working_dir: String,
    pub status: String,
    pub toolchain: String,
    pub total_nodes: usize,
    pub stable_nodes: usize,
    pub running_nodes: usize,
    pub event_count: usize,
    pub measurement_count: usize,
    pub last_energy: String,
    pub avg_energy: String,
    pub nodes: Vec<NodeSummaryRow>,
}

impl SessionDetailViewModel {
    pub fn from_store(
        session_id: String,
        task: String,
        working_dir: String,
        status: String,
        toolchain: Option<String>,
        projection: &LedgerProjection,
    ) -> Self {
        let nodes: Vec<NodeSummaryRow> = projection
            .latest_revision()
            .map(|r| r.nodes.iter().map(node_row).collect())
            .unwrap_or_default();
        let stable_nodes = nodes.iter().filter(|n| n.state == "stable").count();
        let running_nodes = nodes.iter().filter(|n| n.state == "running").count();

        let measurements = &projection.measurements;
        let last_energy = measurements
            .last()
            .map(|m| format!("{:.2}", m.energy))
            .unwrap_or_else(|| "—".into());
        let avg_energy = if measurements.is_empty() {
            "—".into()
        } else {
            let sum: f64 = measurements.iter().map(|m| m.energy).sum();
            format!("{:.2}", sum / measurements.len() as f64)
        };

        Self {
            session_id,
            task,
            working_dir,
            status: normalize_status(&status),
            toolchain: toolchain.unwrap_or_default(),
            total_nodes: nodes.len(),
            stable_nodes,
            running_nodes,
            event_count: projection.event_count,
            measurement_count: measurements.len(),
            last_energy,
            avg_energy,
            nodes,
        }
    }
}

fn node_row(node: &perspt_sdk::WorkNode) -> NodeSummaryRow {
    NodeSummaryRow {
        node_id: node.node_id.clone(),
        generation: node.generation,
        state: state_label(&node.state).to_string(),
        node_class: class_label(node.node_class).to_string(),
        goal: truncate_chars(&node.goal, 96),
    }
}
