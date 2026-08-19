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
    /// Definition 6 context reserves and working-set bounds (`[context]`).
    pub resident: crate::toolloop::ResidentReserves,
    /// Seven-arm evaluation ablation: drop typed correction packets and
    /// fall back to the legacy first-direction text. Never set in
    /// production configurations.
    pub ablate_correction_packets: bool,
    /// Seven-arm evaluation ablation: skip live resident-context paging.
    /// Never set in production configurations.
    pub ablate_context_paging: bool,
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
            resident: crate::toolloop::ResidentReserves::default(),
            ablate_correction_packets: false,
            ablate_context_paging: false,
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

/// Fold the file configuration into the run settings: `[models]`
/// turn deadline, `[verification]` format gating, and `[context]` reserves.
pub(super) fn apply_config_overrides(
    mut run_config: Psp9RunConfig,
    config: &perspt_core::Config,
) -> Psp9RunConfig {
    if let Some(secs) = config.models.as_ref().and_then(|m| m.turn_timeout_secs) {
        run_config.turn_deadline_secs = secs.max(1);
    }
    if let Some(verification) = &config.verification {
        run_config.require_format = verification.require_format.unwrap_or(false);
    }
    if let Some(context) = &config.context {
        let defaults = crate::toolloop::ResidentReserves::default();
        run_config.resident = crate::toolloop::ResidentReserves {
            paging_enabled: defaults.paging_enabled,
            output_reserve_tokens: context
                .output_reserve_tokens
                .unwrap_or(defaults.output_reserve_tokens),
            guard_reserve_tokens: context
                .guard_reserve_tokens
                .unwrap_or(defaults.guard_reserve_tokens),
            frame_tokens: context
                .synopsis_frame_tokens
                .unwrap_or(defaults.frame_tokens),
            pinned_tail: context
                .working_set_turns
                .map(|turns| turns as usize)
                .unwrap_or(defaults.pinned_tail),
        };
    }
    run_config
}
