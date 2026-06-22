//! Dependency-aware parallel ready-queue scheduler (PSP-8 System 4).
//!
//! The scheduler runs independent work in parallel and serializes only work that
//! conflicts through dependencies, leases, or non-commuting durable effects. Two
//! durable turns commute — and need no relative order — only when their semantic
//! read and write footprints are disjoint:
//!
//! ```text
//! writes(e) ∩ reads(e') = ∅  and  writes(e') ∩ reads(e) = ∅.
//! ```
//!
//! The conflict footprint includes durable platform state, not only workspace
//! files: the capability table, risk budgets, fresh-identifier allocation, and
//! the ledger root are modeled as read/write resources.

use std::collections::{BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::certificate::ResidualCertificate;
use crate::workgraph::{WorkGraphRevision, WorkNode, WorkNodeState};

/// A durable resource that turns read or write. Overlapping footprints force
/// serialization (PSP-8 System 4 / Theorem 8).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Resource {
    File(String),
    Interface(String),
    Manifest(String),
    Lockfile(String),
    Migration(String),
    TestFixture(String),
    Toolchain(String),
    /// A specific capability in the capability table `Γ`.
    Capability(String),
    /// A named risk budget.
    RiskBudget(String),
    /// The fresh-identifier allocator.
    FreshIdAllocator,
    /// The ledger root (Merkle head).
    LedgerRoot,
}

/// The read/write footprint of a turn.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Footprint {
    pub reads: BTreeSet<Resource>,
    pub writes: BTreeSet<Resource>,
}

impl Footprint {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read(mut self, r: Resource) -> Self {
        self.reads.insert(r);
        self
    }

    pub fn write(mut self, r: Resource) -> Self {
        self.writes.insert(r);
        self
    }

    /// Whether this turn commutes with `other`: write/read and write/write
    /// footprints must be disjoint in both directions.
    pub fn commutes_with(&self, other: &Footprint) -> bool {
        self.writes.is_disjoint(&other.reads)
            && other.writes.is_disjoint(&self.reads)
            && self.writes.is_disjoint(&other.writes)
    }

    pub fn conflicts_with(&self, other: &Footprint) -> bool {
        !self.commutes_with(other)
    }
}

/// What a lease protects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaseKind {
    /// Serializes all graph revisions against each other.
    GraphWrite,
    /// Package-manager / dependency mutation.
    Toolchain,
    /// Exclusive access to a workspace resource.
    Resource,
}

/// A held lease.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionLease {
    pub lease_id: String,
    pub holder_work_id: String,
    pub kind: LeaseKind,
    pub scope: Resource,
}

/// The lease table, tracking exclusive grants.
#[derive(Debug, Clone, Default)]
pub struct LeaseTable {
    held: Vec<ExecutionLease>,
}

impl LeaseTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a lease over `scope` is available (not already held by another).
    pub fn is_available(&self, scope: &Resource) -> bool {
        !self.held.iter().any(|l| &l.scope == scope)
    }

    /// Acquire a lease, returning its id, or `None` if it is unavailable.
    pub fn acquire(&mut self, holder: &str, kind: LeaseKind, scope: Resource) -> Option<String> {
        if !self.is_available(&scope) {
            return None;
        }
        let lease_id = uuid::Uuid::new_v4().to_string();
        self.held.push(ExecutionLease {
            lease_id: lease_id.clone(),
            holder_work_id: holder.to_string(),
            kind,
            scope,
        });
        Some(lease_id)
    }

    pub fn release(&mut self, lease_id: &str) {
        self.held.retain(|l| l.lease_id != lease_id);
    }

    pub fn held_count(&self) -> usize {
        self.held.len()
    }
}

/// A repair outcome that becomes durable scheduler work (PSP-8 System 4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum RepairAction {
    RetryNode {
        node_id: String,
        generation: u32,
    },
    ExpandScope {
        node_id: String,
        generation: u32,
        added_paths: Vec<String>,
    },
    SplitNode {
        node_id: String,
        generation: u32,
        child_goals: Vec<String>,
    },
    InsertInterfaceNode {
        boundary: String,
    },
    AddNode {
        goal: String,
        reason: String,
    },
    RetireNode {
        node_id: String,
        generation: u32,
        reason: String,
    },
    ReplanSubgraph {
        root: String,
        affected: Vec<String>,
    },
    StopNode {
        node_id: String,
        generation: u32,
        certificate_id: String,
    },
}

/// A durable scheduler command consumed by the ready-queue loop (PSP-8
/// `SchedulerEffect`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case")]
pub enum SchedulerEffect {
    CommitNode {
        node_id: String,
        generation: u32,
    },
    RequeueNode {
        node_id: String,
        generation: u32,
        reason: String,
    },
    ApplyGraphRevision {
        revision_id: String,
    },
    SpawnWork {
        work_id: String,
    },
    CancelWork {
        work_id: String,
        reason: String,
    },
    RequestApproval {
        proposal_id: String,
    },
    StopWithCertificate {
        certificate_id: String,
    },
}

/// Convert a repair action into scheduler effects. A local repair that produces
/// new executable work SHALL NOT escalate; it returns effects the ready-queue
/// loop consumes (PSP-8 System 4).
pub fn repair_to_effects(action: &RepairAction) -> Vec<SchedulerEffect> {
    match action {
        RepairAction::RetryNode {
            node_id,
            generation,
        } => vec![SchedulerEffect::RequeueNode {
            node_id: node_id.clone(),
            generation: *generation,
            reason: "retry".into(),
        }],
        RepairAction::ExpandScope {
            node_id,
            generation,
            ..
        } => vec![SchedulerEffect::RequeueNode {
            node_id: node_id.clone(),
            generation: generation + 1,
            reason: "scope expanded".into(),
        }],
        RepairAction::SplitNode { child_goals, .. } => child_goals
            .iter()
            .map(|_| SchedulerEffect::SpawnWork {
                work_id: uuid::Uuid::new_v4().to_string(),
            })
            .chain(std::iter::once(SchedulerEffect::ApplyGraphRevision {
                revision_id: uuid::Uuid::new_v4().to_string(),
            }))
            .collect(),
        RepairAction::InsertInterfaceNode { .. } | RepairAction::AddNode { .. } => {
            vec![
                SchedulerEffect::SpawnWork {
                    work_id: uuid::Uuid::new_v4().to_string(),
                },
                SchedulerEffect::ApplyGraphRevision {
                    revision_id: uuid::Uuid::new_v4().to_string(),
                },
            ]
        }
        RepairAction::RetireNode { .. } | RepairAction::ReplanSubgraph { .. } => {
            vec![SchedulerEffect::ApplyGraphRevision {
                revision_id: uuid::Uuid::new_v4().to_string(),
            }]
        }
        RepairAction::StopNode { certificate_id, .. } => {
            vec![SchedulerEffect::StopWithCertificate {
                certificate_id: certificate_id.clone(),
            }]
        }
    }
}

/// A node currently executing, with the footprint it occupies.
#[derive(Debug, Clone)]
pub struct RunningTask {
    pub node_id: String,
    pub generation: u32,
    pub footprint: Footprint,
}

/// The mutable parallel scheduler.
#[derive(Debug)]
pub struct Scheduler {
    max_parallel: usize,
    running: Vec<RunningTask>,
    pub leases: LeaseTable,
}

impl Scheduler {
    pub fn new(max_parallel: usize) -> Self {
        Self {
            max_parallel: max_parallel.max(1),
            running: Vec::new(),
            leases: LeaseTable::new(),
        }
    }

    pub fn running_count(&self) -> usize {
        self.running.len()
    }

    /// Compute the ready set: pending nodes whose dependencies are all accepted,
    /// whose required sensors are resolved, and whose footprints do not conflict
    /// with currently-running tasks. `footprint_of` supplies each node's
    /// footprint (domain-derived from output targets and edges).
    pub fn ready_nodes<'a, F>(
        &self,
        revision: &'a WorkGraphRevision,
        footprint_of: F,
    ) -> Vec<&'a WorkNode>
    where
        F: Fn(&WorkNode) -> Footprint,
    {
        let accepted: HashSet<&str> = revision
            .nodes
            .iter()
            .filter(|n| n.is_accepted())
            .map(|n| n.node_id.as_str())
            .collect();

        let mut ready = Vec::new();
        // Footprints occupied by running tasks plus already-selected ready nodes,
        // so two conflicting ready nodes are not dispatched together.
        let mut occupied: Vec<Footprint> =
            self.running.iter().map(|t| t.footprint.clone()).collect();
        let slots = self.max_parallel.saturating_sub(self.running.len());

        for node in &revision.nodes {
            if ready.len() >= slots {
                break;
            }
            if !matches!(node.state, WorkNodeState::Pending | WorkNodeState::Ready) {
                continue;
            }
            // Required sensors resolved?
            if !node.required_sensors.is_empty() {
                continue;
            }
            // All dependencies accepted?
            let deps = revision.dependencies_of(&node.node_id);
            if !deps.iter().all(|d| accepted.contains(d)) {
                continue;
            }
            // Footprint free?
            let fp = footprint_of(node);
            if occupied.iter().any(|o| o.conflicts_with(&fp)) {
                continue;
            }
            occupied.push(fp);
            ready.push(node);
        }
        ready
    }

    /// Mark a node as running, occupying its footprint.
    pub fn start(&mut self, node: &WorkNode, footprint: Footprint) {
        self.running.push(RunningTask {
            node_id: node.node_id.clone(),
            generation: node.generation,
            footprint,
        });
    }

    /// Mark a running node as finished, freeing its footprint.
    pub fn finish(&mut self, node_id: &str, generation: u32) {
        self.running
            .retain(|t| !(t.node_id == node_id && t.generation == generation));
    }
}

/// A terminal classification for a node generation, ensuring recovery is total:
/// every node ends accepted, certified-stopped, or escalated (PSP-8 System 4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum NodeOutcome {
    Committed { node_id: String, generation: u32 },
    Stopped { certificate: ResidualCertificate },
    Escalated { node_id: String, reason: String },
}

/// Validate that a set of node outcomes covers every node exactly once with a
/// terminal classification (recovery totality check).
pub fn recovery_is_total(revision: &WorkGraphRevision, outcomes: &[NodeOutcome]) -> bool {
    let classified: HashMap<&str, usize> = {
        let mut m: HashMap<&str, usize> = HashMap::new();
        for o in outcomes {
            let id = match o {
                NodeOutcome::Committed { node_id, .. } => node_id.as_str(),
                NodeOutcome::Stopped { certificate } => certificate.node_id.as_str(),
                NodeOutcome::Escalated { node_id, .. } => node_id.as_str(),
            };
            *m.entry(id).or_default() += 1;
        }
        m
    };
    revision
        .nodes
        .iter()
        .all(|n| classified.get(n.node_id.as_str()) == Some(&1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workgraph::{EdgeKind, GraphRevisionReason, NodeClass, WorkEdge};

    fn node(id: &str) -> WorkNode {
        WorkNode::new(id, format!("goal {id}"), NodeClass::Implement)
    }

    fn rev(nodes: Vec<WorkNode>, edges: Vec<WorkEdge>) -> WorkGraphRevision {
        WorkGraphRevision::build(0, None, GraphRevisionReason::InitialPlan, nodes, edges).unwrap()
    }

    #[test]
    fn disjoint_footprints_commute() {
        let a = Footprint::new().write(Resource::File("a.rs".into()));
        let b = Footprint::new().write(Resource::File("b.rs".into()));
        assert!(a.commutes_with(&b));
    }

    #[test]
    fn write_read_overlap_does_not_commute() {
        let a = Footprint::new().write(Resource::File("shared.rs".into()));
        let b = Footprint::new().read(Resource::File("shared.rs".into()));
        assert!(a.conflicts_with(&b));
    }

    #[test]
    fn manifest_writes_serialize() {
        let a = Footprint::new().write(Resource::Manifest("Cargo.toml".into()));
        let b = Footprint::new().write(Resource::Manifest("Cargo.toml".into()));
        assert!(a.conflicts_with(&b));
    }

    #[test]
    fn capability_table_is_a_conflict_resource() {
        // A capability grant conflicts with a commit that reads that capability.
        let grant = Footprint::new().write(Resource::Capability("write-src".into()));
        let commit = Footprint::new().read(Resource::Capability("write-src".into()));
        assert!(grant.conflicts_with(&commit));
    }

    #[test]
    fn ledger_root_and_fresh_id_serialize() {
        let a = Footprint::new().write(Resource::LedgerRoot);
        let b = Footprint::new().write(Resource::LedgerRoot);
        assert!(a.conflicts_with(&b));
        let c = Footprint::new().write(Resource::FreshIdAllocator);
        let d = Footprint::new().write(Resource::FreshIdAllocator);
        assert!(c.conflicts_with(&d));
    }

    #[test]
    fn independent_nodes_are_ready_in_parallel() {
        let nodes = vec![node("a"), node("b")];
        let revision = rev(nodes, vec![]);
        let sched = Scheduler::new(4);
        let fp = |n: &WorkNode| Footprint::new().write(Resource::File(format!("{}.rs", n.node_id)));
        let ready = sched.ready_nodes(&revision, fp);
        assert_eq!(ready.len(), 2);
    }

    #[test]
    fn dependent_node_waits_for_its_predecessor() {
        let nodes = vec![node("a"), node("b")];
        let edges = vec![WorkEdge::new("a", "b", EdgeKind::RequiresArtifact)];
        let revision = rev(nodes, edges);
        let sched = Scheduler::new(4);
        let fp = |n: &WorkNode| Footprint::new().write(Resource::File(format!("{}.rs", n.node_id)));
        let ready = sched.ready_nodes(&revision, fp);
        // Only `a` is ready; `b` depends on (not-yet-accepted) `a`.
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].node_id, "a");
    }

    #[test]
    fn conflicting_footprints_do_not_dispatch_together() {
        let nodes = vec![node("a"), node("b")];
        let revision = rev(nodes, vec![]);
        let sched = Scheduler::new(4);
        // Both touch the same manifest -> only one is ready at a time.
        let fp = |_n: &WorkNode| Footprint::new().write(Resource::Manifest("Cargo.toml".into()));
        let ready = sched.ready_nodes(&revision, fp);
        assert_eq!(ready.len(), 1);
    }

    #[test]
    fn inserted_node_becomes_ready_after_revision() {
        // The static-snapshot bug: a node inserted by a repair must be executed.
        let mut nodes = vec![node("a")];
        nodes[0].state = WorkNodeState::Stable;
        let revision = rev(nodes, vec![]);
        // Repair inserts node "b".
        let mut nodes2 = revision.nodes.clone();
        nodes2.push(node("b"));
        let revision2 = WorkGraphRevision::build(
            1,
            Some(revision.revision_id.clone()),
            GraphRevisionReason::LocalRepair,
            nodes2,
            vec![],
        )
        .unwrap();
        let sched = Scheduler::new(4);
        let fp = |n: &WorkNode| Footprint::new().write(Resource::File(format!("{}.rs", n.node_id)));
        let ready = sched.ready_nodes(&revision2, fp);
        assert!(
            ready.iter().any(|n| n.node_id == "b"),
            "inserted node must be ready"
        );
    }

    #[test]
    fn leases_are_exclusive() {
        let mut table = LeaseTable::new();
        let scope = Resource::Toolchain("cargo".into());
        let l1 = table.acquire("w1", LeaseKind::Toolchain, scope.clone());
        assert!(l1.is_some());
        assert!(table
            .acquire("w2", LeaseKind::Toolchain, scope.clone())
            .is_none());
        table.release(&l1.unwrap());
        assert!(table.acquire("w2", LeaseKind::Toolchain, scope).is_some());
    }

    #[test]
    fn repair_retry_becomes_requeue_effect() {
        let effects = repair_to_effects(&RepairAction::RetryNode {
            node_id: "a".into(),
            generation: 0,
        });
        assert_eq!(
            effects,
            vec![SchedulerEffect::RequeueNode {
                node_id: "a".into(),
                generation: 0,
                reason: "retry".into()
            }]
        );
    }

    #[test]
    fn split_produces_spawn_and_revision_effects() {
        let effects = repair_to_effects(&RepairAction::SplitNode {
            node_id: "a".into(),
            generation: 0,
            child_goals: vec!["x".into(), "y".into()],
        });
        let spawns = effects
            .iter()
            .filter(|e| matches!(e, SchedulerEffect::SpawnWork { .. }))
            .count();
        let revs = effects
            .iter()
            .filter(|e| matches!(e, SchedulerEffect::ApplyGraphRevision { .. }))
            .count();
        assert_eq!(spawns, 2);
        assert_eq!(revs, 1);
    }

    #[test]
    fn recovery_totality_requires_every_node_classified() {
        let nodes = vec![node("a"), node("b")];
        let revision = rev(nodes, vec![]);
        let outcomes = vec![NodeOutcome::Committed {
            node_id: "a".into(),
            generation: 0,
        }];
        assert!(!recovery_is_total(&revision, &outcomes));
        let outcomes = vec![
            NodeOutcome::Committed {
                node_id: "a".into(),
                generation: 0,
            },
            NodeOutcome::Escalated {
                node_id: "b".into(),
                reason: "blocked".into(),
            },
        ];
        assert!(recovery_is_total(&revision, &outcomes));
    }
}
