//! Mutable, revisioned work graph (PSP-8 System 4).
//!
//! Each graph revision is acyclic, but the session as a whole may add, retire,
//! split, merge, update, or requeue nodes as verifier evidence changes. Updating
//! a node creates a new [`WorkNode`] generation rather than mutating it in place;
//! retired generations remain in the ledger but are no longer executable.

use std::collections::{BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::error::{Result, SdkError};
use crate::residual::ResidualEventRef;

/// The kind of work a node performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeClass {
    Explore,
    Plan,
    Implement,
    Verify,
    Test,
    Integrate,
    Repair,
    Interface,
}

/// Execution state of a node generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkNodeState {
    /// Waiting on dependencies, sensors, or leases.
    Pending,
    /// Eligible for dispatch.
    Ready,
    /// Currently executing.
    Running,
    /// Accepted (hard pass or descent).
    Stable,
    /// Stopped with a residual certificate.
    Stopped { certificate_id: String },
    /// Superseded by a newer generation.
    Retired { reason: String },
    /// Blocked on a missing or degraded required sensor.
    BlockedOnSensor { sensor: String },
}

/// Typed edge semantics (PSP-8 System 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// Destination reads a file produced by source.
    RequiresArtifact,
    /// Destination relies on a sealed signature/schema/symbol.
    RequiresInterface,
    /// Destination test node validates source implementation node.
    Tests,
    /// Destination reconciles a cross-node/domain/adapter boundary.
    Integrates,
    /// Nodes touch non-commuting durable state and must serialize.
    ConflictsWith,
    /// Graph-rewrite lineage edge for audit.
    DerivedFrom,
    /// Node cannot execute until a required sensor exists or is downgraded.
    BlocksOnSensor,
}

impl EdgeKind {
    /// Whether this edge imposes an execution-ordering dependency (the source
    /// must reach a stable state before the destination is ready). `DerivedFrom`
    /// is audit-only and `ConflictsWith` is handled by footprint serialization,
    /// not readiness.
    pub fn is_dependency(self) -> bool {
        matches!(
            self,
            EdgeKind::RequiresArtifact
                | EdgeKind::RequiresInterface
                | EdgeKind::Tests
                | EdgeKind::Integrates
        )
    }
}

/// A directed edge in a graph revision.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkEdge {
    pub src: String,
    pub dst: String,
    pub kind: EdgeKind,
}

impl WorkEdge {
    pub fn new(src: impl Into<String>, dst: impl Into<String>, kind: EdgeKind) -> Self {
        Self { src: src.into(), dst: dst.into(), kind }
    }
}

/// An immutable incarnation of a node (PSP-8 `WorkNode`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkNode {
    pub node_id: String,
    pub generation: u32,
    pub goal: String,
    pub node_class: NodeClass,
    pub owner_domains: Vec<String>,
    pub output_targets: Vec<String>,
    /// Required sensors that gate readiness (mapped from `BlocksOnSensor`).
    pub required_sensors: Vec<String>,
    pub state: WorkNodeState,
}

impl WorkNode {
    pub fn new(node_id: impl Into<String>, goal: impl Into<String>, node_class: NodeClass) -> Self {
        Self {
            node_id: node_id.into(),
            generation: 0,
            goal: goal.into(),
            node_class,
            owner_domains: Vec::new(),
            output_targets: Vec::new(),
            required_sensors: Vec::new(),
            state: WorkNodeState::Pending,
        }
    }

    pub fn with_outputs(mut self, outputs: Vec<String>) -> Self {
        self.output_targets = outputs;
        self
    }

    /// Create the next generation of this node, resetting it to pending.
    pub fn next_generation(&self) -> Self {
        Self {
            generation: self.generation + 1,
            state: WorkNodeState::Pending,
            ..self.clone()
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            WorkNodeState::Stable | WorkNodeState::Stopped { .. } | WorkNodeState::Retired { .. }
        )
    }

    pub fn is_accepted(&self) -> bool {
        matches!(self.state, WorkNodeState::Stable)
    }
}

/// Why a graph revision was produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphRevisionReason {
    InitialPlan,
    LocalRepair,
    ScopeExpansion,
    UserEdit,
    Replan,
}

/// Result of validating a revision before activation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphValidationReport {
    pub acyclic: bool,
    /// A topological order over dependency edges, if acyclic.
    pub topo_order: Vec<String>,
    pub dangling_edges: Vec<WorkEdge>,
}

/// A durable version of the work graph (PSP-8 `WorkGraphRevision`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkGraphRevision {
    pub revision_id: String,
    pub sequence: u32,
    pub parent_revision_id: Option<String>,
    pub reason: GraphRevisionReason,
    pub nodes: Vec<WorkNode>,
    pub edges: Vec<WorkEdge>,
    pub validation: GraphValidationReport,
    pub evidence: Vec<ResidualEventRef>,
}

impl WorkGraphRevision {
    /// Build and validate a revision. Returns an error if it is not acyclic over
    /// dependency edges; every revision SHALL validate acyclicity before
    /// activation (PSP-8 System 4).
    pub fn build(
        sequence: u32,
        parent_revision_id: Option<String>,
        reason: GraphRevisionReason,
        nodes: Vec<WorkNode>,
        edges: Vec<WorkEdge>,
    ) -> Result<Self> {
        let validation = validate(&nodes, &edges)?;
        if !validation.acyclic {
            return Err(SdkError::Domain("graph revision is not acyclic".into()));
        }
        if !validation.dangling_edges.is_empty() {
            return Err(SdkError::Domain(format!(
                "graph revision has {} dangling edge(s)",
                validation.dangling_edges.len()
            )));
        }
        Ok(Self {
            revision_id: uuid::Uuid::new_v4().to_string(),
            sequence,
            parent_revision_id,
            reason,
            nodes,
            edges,
            validation,
            evidence: Vec::new(),
        })
    }

    pub fn node(&self, node_id: &str) -> Option<&WorkNode> {
        self.nodes.iter().find(|n| n.node_id == node_id)
    }

    /// Dependency predecessors of a node (sources of dependency edges into it).
    pub fn dependencies_of(&self, node_id: &str) -> Vec<&str> {
        self.edges
            .iter()
            .filter(|e| e.dst == node_id && e.kind.is_dependency())
            .map(|e| e.src.as_str())
            .collect()
    }

    /// Nodes that conflict with the given node via an explicit `ConflictsWith`
    /// edge (in addition to footprint-derived conflicts handled by the
    /// scheduler).
    pub fn explicit_conflicts_of(&self, node_id: &str) -> Vec<&str> {
        self.edges
            .iter()
            .filter(|e| e.kind == EdgeKind::ConflictsWith && (e.src == node_id || e.dst == node_id))
            .map(|e| if e.src == node_id { e.dst.as_str() } else { e.src.as_str() })
            .collect()
    }
}

/// Validate a node/edge set: check that all edge endpoints exist and that the
/// dependency subgraph is acyclic (Kahn's algorithm).
pub fn validate(nodes: &[WorkNode], edges: &[WorkEdge]) -> Result<GraphValidationReport> {
    let ids: HashSet<&str> = nodes.iter().map(|n| n.node_id.as_str()).collect();
    let dangling: Vec<WorkEdge> = edges
        .iter()
        .filter(|e| !ids.contains(e.src.as_str()) || !ids.contains(e.dst.as_str()))
        .cloned()
        .collect();

    // Kahn's algorithm over dependency edges only.
    let mut indegree: HashMap<&str, usize> = nodes.iter().map(|n| (n.node_id.as_str(), 0)).collect();
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in edges.iter().filter(|e| e.kind.is_dependency()) {
        if !ids.contains(e.src.as_str()) || !ids.contains(e.dst.as_str()) {
            continue;
        }
        adj.entry(e.src.as_str()).or_default().push(e.dst.as_str());
        *indegree.get_mut(e.dst.as_str()).unwrap() += 1;
    }

    let mut queue: BTreeSet<&str> =
        indegree.iter().filter(|(_, &d)| d == 0).map(|(&n, _)| n).collect();
    let mut topo_order = Vec::new();
    while let Some(&n) = queue.iter().next() {
        queue.remove(n);
        topo_order.push(n.to_string());
        if let Some(succs) = adj.get(n) {
            for &s in succs {
                let d = indegree.get_mut(s).unwrap();
                *d -= 1;
                if *d == 0 {
                    queue.insert(s);
                }
            }
        }
    }

    let acyclic = topo_order.len() == nodes.len();
    Ok(GraphValidationReport {
        acyclic,
        topo_order: if acyclic { topo_order } else { Vec::new() },
        dangling_edges: dangling,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str) -> WorkNode {
        WorkNode::new(id, format!("goal {id}"), NodeClass::Implement)
    }

    #[test]
    fn acyclic_graph_validates_with_topo_order() {
        let nodes = vec![node("a"), node("b"), node("c")];
        let edges = vec![
            WorkEdge::new("a", "b", EdgeKind::RequiresArtifact),
            WorkEdge::new("b", "c", EdgeKind::RequiresInterface),
        ];
        let rev = WorkGraphRevision::build(0, None, GraphRevisionReason::InitialPlan, nodes, edges).unwrap();
        assert!(rev.validation.acyclic);
        assert_eq!(rev.validation.topo_order, vec!["a", "b", "c"]);
    }

    #[test]
    fn cyclic_graph_is_rejected() {
        let nodes = vec![node("a"), node("b")];
        let edges = vec![
            WorkEdge::new("a", "b", EdgeKind::RequiresArtifact),
            WorkEdge::new("b", "a", EdgeKind::RequiresArtifact),
        ];
        assert!(WorkGraphRevision::build(0, None, GraphRevisionReason::InitialPlan, nodes, edges).is_err());
    }

    #[test]
    fn dangling_edge_is_rejected() {
        let nodes = vec![node("a")];
        let edges = vec![WorkEdge::new("a", "ghost", EdgeKind::RequiresArtifact)];
        assert!(WorkGraphRevision::build(0, None, GraphRevisionReason::InitialPlan, nodes, edges).is_err());
    }

    #[test]
    fn conflicts_with_does_not_create_dependency_cycle() {
        // ConflictsWith is bidirectional but is not a dependency edge, so a pair
        // of conflicting nodes is still a valid acyclic revision.
        let nodes = vec![node("a"), node("b")];
        let edges = vec![WorkEdge::new("a", "b", EdgeKind::ConflictsWith)];
        let rev = WorkGraphRevision::build(0, None, GraphRevisionReason::InitialPlan, nodes, edges).unwrap();
        assert!(rev.validation.acyclic);
        assert_eq!(rev.explicit_conflicts_of("a"), vec!["b"]);
    }

    #[test]
    fn next_generation_resets_state() {
        let mut n = node("a");
        n.state = WorkNodeState::Stable;
        let g1 = n.next_generation();
        assert_eq!(g1.generation, 1);
        assert_eq!(g1.state, WorkNodeState::Pending);
    }

    #[test]
    fn dependencies_exclude_audit_and_conflict_edges() {
        let nodes = vec![node("a"), node("b")];
        let edges = vec![
            WorkEdge::new("a", "b", EdgeKind::DerivedFrom),
            WorkEdge::new("a", "b", EdgeKind::ConflictsWith),
        ];
        let rev = WorkGraphRevision::build(0, None, GraphRevisionReason::LocalRepair, nodes, edges).unwrap();
        assert!(rev.dependencies_of("b").is_empty());
    }
}
