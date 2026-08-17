//! Topology view model: the PSP-9 work-graph lineage projected from
//! `graph_revision` ledger events. Each revision is a full acyclic snapshot;
//! the latest one is the active graph.

use super::psp9::{
    class_label, edge_kind_label, reason_label, short, state_label, truncate_chars,
    LedgerProjection,
};

/// One graph revision in the session lineage.
pub struct RevisionRow {
    pub revision_short: String,
    pub sequence: u32,
    pub reason: String,
    pub parent_short: String,
    pub node_count: usize,
    pub edge_count: usize,
}

/// One node of the latest revision.
pub struct TopoNodeRow {
    pub node_id: String,
    pub generation: u32,
    pub state: String,
    pub node_class: String,
    pub goal: String,
    pub output_targets: String,
}

/// One typed edge of the latest revision.
pub struct TopoEdgeRow {
    pub src: String,
    pub dst: String,
    pub kind: String,
}

/// View model for the topology page.
pub struct TopologyViewModel {
    pub session_id: String,
    pub revisions: Vec<RevisionRow>,
    pub latest_revision_short: String,
    pub nodes: Vec<TopoNodeRow>,
    pub edges: Vec<TopoEdgeRow>,
    pub stable_nodes: usize,
    pub running_nodes: usize,
    pub stopped_nodes: usize,
}

impl TopologyViewModel {
    pub fn from_projection(session_id: String, projection: &LedgerProjection) -> Self {
        let revisions = projection
            .revisions
            .iter()
            .map(|r| RevisionRow {
                revision_short: short(&r.revision_id, 8),
                sequence: r.sequence,
                reason: reason_label(&r.reason).to_string(),
                parent_short: r
                    .parent_revision_id
                    .as_deref()
                    .map(|p| short(p, 8))
                    .unwrap_or_else(|| "—".into()),
                node_count: r.nodes.len(),
                edge_count: r.edges.len(),
            })
            .collect();

        let latest = projection.latest_revision();
        let latest_revision_short = latest
            .map(|r| short(&r.revision_id, 8))
            .unwrap_or_else(|| "—".into());
        let nodes: Vec<TopoNodeRow> = latest
            .map(|r| r.nodes.iter().map(node_row).collect())
            .unwrap_or_default();
        let edges = latest
            .map(|r| {
                r.edges
                    .iter()
                    .map(|e| TopoEdgeRow {
                        src: e.src.clone(),
                        dst: e.dst.clone(),
                        kind: edge_kind_label(e.kind).to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let stable_nodes = nodes.iter().filter(|n| n.state == "stable").count();
        let running_nodes = nodes.iter().filter(|n| n.state == "running").count();
        let stopped_nodes = nodes.iter().filter(|n| n.state == "stopped").count();

        Self {
            session_id,
            revisions,
            latest_revision_short,
            nodes,
            edges,
            stable_nodes,
            running_nodes,
            stopped_nodes,
        }
    }
}

fn node_row(node: &perspt_sdk::WorkNode) -> TopoNodeRow {
    TopoNodeRow {
        node_id: node.node_id.clone(),
        generation: node.generation,
        state: state_label(&node.state).to_string(),
        node_class: class_label(node.node_class).to_string(),
        goal: truncate_chars(&node.goal, 96),
        output_targets: node.output_targets.join(", "),
    }
}
