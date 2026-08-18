//! The universal actor turn runner (PSP-10 system 27, Phase 9).
//!
//! One turn discipline for every stochastic actor — worker, explorer,
//! architect, adjudicator, evidence summarizer, and capability probe:
//! resolve the route, check the resident-context budget before any
//! transport call (Definition 6: infeasible means no call), send with the
//! shared sticky failover chain, and record the raw observation as an
//! actor-tagged `turn_observed` before anything inspects it. The worker's
//! tool loop keeps its loop policy (budgets, batching, cadence) and shares
//! the same failure classification.

mod runner;

pub use runner::{
    chat_turn_with_deadline, transport_failure_kind, ActorKind, ActorTurnRunner,
    DEFAULT_TURN_DEADLINE_SECS,
};
