//! Read-only exploration (PSP-8 System 3).
//!
//! Before durable mutations, the agent needs a cheap, bounded, read-only
//! understanding of the repository. Exploration runs with read/search/list/LSP
//! capabilities only; if it discovers that a mutation may be required it emits a
//! residual, graph hint, or capability request rather than performing the
//! mutation. Exploration evidence is advisory unless backed by deterministic
//! tool output, and a cheap model summarizing it never becomes a correctness
//! barrier.

use serde::{Deserialize, Serialize};

use crate::capability::{ActorId, Capability, EffectKind};
use crate::routing::ModelRoute;

/// Independent budgets for exploration (PSP-8 System 3).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ExplorationBudget {
    pub max_files: u32,
    pub max_tokens: u64,
    pub max_tool_calls: u32,
    pub max_wall_clock_secs: u64,
    pub max_parallel_workers: u32,
}

impl Default for ExplorationBudget {
    fn default() -> Self {
        Self {
            max_files: 500,
            max_tokens: 50_000,
            max_tool_calls: 200,
            max_wall_clock_secs: 120,
            max_parallel_workers: 8,
        }
    }
}

/// Running usage measured against an [`ExplorationBudget`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ExplorationUsage {
    pub files: u32,
    pub tokens: u64,
    pub tool_calls: u32,
    pub wall_clock_secs: u64,
}

impl ExplorationBudget {
    /// Whether current usage is still within budget.
    pub fn admits(&self, usage: &ExplorationUsage) -> bool {
        usage.files <= self.max_files
            && usage.tokens <= self.max_tokens
            && usage.tool_calls <= self.max_tool_calls
            && usage.wall_clock_secs <= self.max_wall_clock_secs
    }
}

/// A structured map of the repository (PSP-8 System 3).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectMap {
    pub languages: Vec<String>,
    pub package_roots: Vec<String>,
    pub build_systems: Vec<String>,
    pub entry_points: Vec<String>,
    pub risk_hotspots: Vec<String>,
}

/// A seed node / edge hint for the planner (PSP-8 System 3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphHint {
    pub goal: String,
    pub suggested_outputs: Vec<String>,
    pub rationale: String,
}

/// A read-only exploration report (PSP-8 `ExplorationReport`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExplorationReport {
    pub report_id: String,
    pub model_route: Option<ModelRoute>,
    pub project_map: ProjectMap,
    pub graph_hints: Vec<GraphHint>,
    pub verifier_recommendations: Vec<String>,
    /// Content hashes of the inputs this report observed, for provenance.
    pub input_witnesses: Vec<String>,
    /// Whether the report is backed by deterministic tool output (not just a
    /// model summary). Advisory-only reports cannot act as a correctness barrier.
    pub deterministically_backed: bool,
}

impl ExplorationReport {
    pub fn new(project_map: ProjectMap) -> Self {
        Self {
            report_id: uuid::Uuid::new_v4().to_string(),
            model_route: None,
            project_map,
            graph_hints: Vec::new(),
            verifier_recommendations: Vec::new(),
            input_witnesses: Vec::new(),
            deterministically_backed: false,
        }
    }

    /// Whether this report may be relied upon as a correctness barrier. A model
    /// summary alone may not; only deterministically-backed evidence may.
    pub fn is_barrier_eligible(&self) -> bool {
        self.deterministically_backed
    }
}

/// Build a read-only exploration capability for an actor. Exploration SHALL NOT
/// write files, mutate dependencies, change graph policy, or apply patches.
pub fn exploration_capability(actor: ActorId) -> Capability {
    Capability::new(
        actor,
        vec![
            EffectKind::ReadFile,
            EffectKind::Search,
            EffectKind::List,
            EffectKind::LspQuery,
            EffectKind::GitRead,
        ],
    )
    .with_paths(vec!["*"])
}

/// Whether a capability is strictly read-only (the exploration invariant).
pub fn is_read_only_capability(cap: &Capability) -> bool {
    cap.effects.iter().all(|e| e.is_read_only())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exploration_capability_is_read_only() {
        let cap = exploration_capability(ActorId::new("explorer"));
        assert!(is_read_only_capability(&cap));
        assert!(!cap.grants(EffectKind::WriteArtifact));
        assert!(!cap.grants(EffectKind::ApplyPatch));
        assert!(!cap.grants(EffectKind::MutateDependencies));
    }

    #[test]
    fn budget_admits_within_limits_and_rejects_overflow() {
        let budget = ExplorationBudget::default();
        let ok = ExplorationUsage {
            files: 10,
            tokens: 1000,
            tool_calls: 5,
            wall_clock_secs: 10,
        };
        assert!(budget.admits(&ok));
        let over = ExplorationUsage { files: 9999, ..ok };
        assert!(!budget.admits(&over));
    }

    #[test]
    fn model_summary_alone_is_not_a_barrier() {
        let report = ExplorationReport::new(ProjectMap::default());
        assert!(!report.is_barrier_eligible());
    }

    #[test]
    fn deterministically_backed_report_is_barrier_eligible() {
        let mut report = ExplorationReport::new(ProjectMap::default());
        report.deterministically_backed = true;
        assert!(report.is_barrier_eligible());
    }
}
