use perspt_store::{NodeStateRecord, TaskGraphEdgeRow};

/// View model for the DAG topology page
pub struct DagViewModel {
    pub session_id: String,
    pub nodes: Vec<DagNode>,
    pub edges: Vec<DagEdge>,
}

pub struct DagNode {
    pub node_id: String,
    pub state: String,
    pub v_total: f32,
    pub node_class: String,
    pub goal: String,
    pub attempt_count: i32,
}

pub struct DagEdge {
    pub parent_id: String,
    pub child_id: String,
    pub edge_type: String,
}

impl DagViewModel {
    pub fn from_store(
        session_id: String,
        nodes: Vec<NodeStateRecord>,
        edges: Vec<TaskGraphEdgeRow>,
    ) -> Self {
        let dag_nodes = nodes
            .into_iter()
            .map(|n| DagNode {
                node_id: n.node_id,
                state: n.state,
                v_total: n.v_total,
                node_class: n.node_class.unwrap_or_default(),
                goal: n.goal.unwrap_or_default(),
                attempt_count: n.attempt_count,
            })
            .collect();

        let dag_edges = edges
            .into_iter()
            .map(|e| DagEdge {
                parent_id: e.parent_node_id,
                child_id: e.child_node_id,
                edge_type: e.edge_type,
            })
            .collect();

        Self {
            session_id,
            nodes: dag_nodes,
            edges: dag_edges,
        }
    }
}
