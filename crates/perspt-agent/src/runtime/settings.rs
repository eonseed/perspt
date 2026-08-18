//! Run settings, model routes, and the run summary for one PSP-9 session.

use perspt_sdk::{ApprovalPolicy, NodeTerminalOutcome};

/// Finite settings for one PSP-9 run.
#[derive(Debug, Clone)]
pub struct Psp9RunConfig {
    pub max_turns: u32,
    pub max_calls_per_turn: u32,
    pub rejection_budget: u32,
    pub rho_gate: f64,
    pub approval_policy: ApprovalPolicy,
    /// Embedders that already isolate the entire process may opt out of the
    /// nested verifier sandbox. The CLI never enables this.
    pub allow_unisolated_verifiers: bool,
    pub max_parallel_verifiers: usize,
    /// Persist signed grant intent across sessions. Disabled by default;
    /// resume still re-mints fresh, epoch-bound capabilities.
    pub persistent_grants: bool,
    /// Explicit opt-in for governed dependency mutation (Gate J). Off by
    /// default: `MutateDependencies` stays withheld from every grant.
    pub allow_dependency_mutation: bool,
    /// Concurrent work-graph nodes (Gate P). 1 keeps the single-node path
    /// verbatim; above 1 the multi-node dispatcher runs and a governed
    /// architect planning turn may decompose the task.
    pub max_parallel_nodes: usize,
    /// Per-model-call wall-clock deadline (seconds). Exceeding it is a
    /// transport failure that consumes sticky failover.
    pub turn_deadline_secs: u64,
    /// Declare the plugin `format` verifier stage as an acceptance sensor
    /// (`[verification] require_format`). Off by default.
    pub require_format: bool,
}

impl Default for Psp9RunConfig {
    fn default() -> Self {
        Self {
            max_turns: 12,
            max_calls_per_turn: 8,
            rejection_budget: 4,
            rho_gate: 0.5,
            approval_policy: ApprovalPolicy::Ask,
            allow_unisolated_verifiers: false,
            max_parallel_verifiers: 4,
            persistent_grants: false,
            allow_dependency_mutation: false,
            max_parallel_nodes: 1,
            turn_deadline_secs: crate::turn::DEFAULT_TURN_DEADLINE_SECS,
            require_format: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Psp9RunSummary {
    pub session_id: String,
    pub node_id: String,
    pub outcome: NodeTerminalOutcome,
    pub turns_used: u32,
    pub ledger_head: String,
    pub promoted_paths: Vec<String>,
}

/// Explicit model-plane routes for one PSP-9 session. Role routes are not
/// interchangeable: only `fallbacks` may replace the actuator after a recorded
/// transport failure.
#[derive(Debug, Clone, Default)]
pub struct Psp9ModelRoutes {
    pub primary: Option<String>,
    pub actuator: Option<String>,
    pub explorer: Option<String>,
    pub adjudicator: Option<String>,
    pub fallbacks: Vec<String>,
}
