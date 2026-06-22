//! # Perspt SRBN Agent SDK
//!
//! `perspt-sdk` is the domain-neutral control plane for Perspt's SRBN agent
//! platform (PSP-8). It owns the SRBN stability contract — residual evidence,
//! the canonical quadratic energy, the measured acceptance gate, the spectral
//! energy-slope diagnostic, verifier independence, analytic stability claims,
//! residual certificates, and the [`srbn`] kernel adapter — while domain
//! packages such as `perspt-coding` provide task-specific residual construction,
//! weights, and correction directions.
//!
//! ## SRBN contract at a glance
//!
//! * Residuals carry a raw non-negative magnitude `r_e >= 0`
//!   ([`residual::ResidualEvent`]).
//! * The single gating energy is `V(x) = sum_e w_e ||r_e||^2`
//!   ([`energy::score_candidate`]); the component rollups `V_syn..V_sheaf` are
//!   derived projections, not independently weighted sums.
//! * Acceptance is the measured discrete gate
//!   `accept(y) <=> hard(y) OR V(y) <= V(x_best) - rho_gate`
//!   ([`gate::evaluate_gate`]), with the finite-decision bound
//!   `floor(V_0 / rho_gate) + B + 1` ([`gate::finite_decision_bound`]).
//! * The spectral constant `mu = 2 lambda_min+(A)` is computed off the critical
//!   path from the verification graph ([`spectral::VerificationGraph::mu`]).
//! * Continuous constants `alpha, beta, delta, L, eta` are optional and remain
//!   `NotClaimed` for the coding domain ([`stability::StabilityClaim`]).
//! * Exhaustion yields a [`certificate::ResidualCertificate`], an honest stop.
//!
//! This is the Phase-0/1 foundation (plus the spectral and verifier-independence
//! pieces of Phase 5). Later phases — the mutable scheduler, capability kernel,
//! replay ledger, exploration/model routing, calibrated risk budgets, and
//! dashboards — build on these contracts.

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

pub mod benchmark;
pub mod capability;
pub mod certificate;
pub mod command;
pub mod conformal;
pub mod domain;
pub mod energy;
pub mod error;
pub mod exploration;
pub mod gate;
pub mod goal;
pub mod independence;
pub mod kernel;
pub mod ledger;
pub mod observability;
pub mod residual;
pub mod routing;
pub mod scheduler;
pub mod spectral;
pub mod stability;
pub mod workgraph;

// Re-export the published kernel crates so consumers depend on one SRBN source.
pub use srbn;
pub use srbn_serde;

// Convenience re-exports of the most-used SDK types.
pub use benchmark::{BenchmarkCase, BenchmarkOutcome, BenchmarkReport, BenchmarkResult};
pub use capability::{
    check_admissibility, ActorId, AdmissibilityDecision, AdmissibilityWitness, ApprovalPolicy,
    Capability, DenyReason, EffectKind, EffectProposal, KernelState, RecoveryClass, RiskBudget,
    RiskClass, StateWitness,
};
pub use certificate::{BudgetRef, ResidualCertificate};
pub use command::{canonicalize, classify_tier, CommandInvocation, CommandTier};
pub use conformal::{
    conformal_threshold, decide as conformal_decide, is_drifted, ks_statistic, AcceptOutcome,
    CalibrationSample, CalibrationState,
};
pub use domain::{
    AgentDomainPackage, DomainDetection, DomainId, DomainRegistry, DomainScope, ResidualSchema,
    WorkspaceSnapshot,
};
pub use energy::{score_candidate, EnergyComponents, EnergyModel, EnergyScore, ResidualWeight};
pub use error::{Result, SdkError};
pub use exploration::{
    exploration_capability, is_read_only_capability, ExplorationBudget, ExplorationReport,
    ExplorationUsage, GraphHint, ProjectMap,
};
pub use gate::{
    evaluate_gate, finite_decision_bound, AcceptedTrajectory, GateDecision, GateDecisionRef,
};
pub use goal::{goal_presence_residual, goal_presence_sensor, missing_symbols, GoalSpec};
pub use independence::{compute as compute_independence, IndependenceStats, VerdictRecord};
pub use kernel::{AgentBarrierResult, AgentStabilizationStatus, CorrectionDirectionSet, Evidence};
pub use ledger::{
    content_hash, replay_accepted_trajectory, ExternalEffectLog, IdempotencyLog, Ledger,
    LedgerEvent, LedgerRecord,
};
pub use observability::{
    backlog_gauge, phi, residual_heatmap, CapabilityAudit, ResidualHeatmap, TrajectoryProjection,
    WorkflowPotential,
};
pub use residual::{
    CorrectionDirection, EnergyComponent, EvidencePayload, IndependenceRoute, ResidualClass,
    ResidualEvent, ResidualEventRef, ResidualSeverity, SensorRef, SymbolRef,
};
pub use routing::{resolve_route, AgentPhase, ModelBudget, ModelRoute, ModelTier, ModelTierConfig};
pub use scheduler::{
    recovery_is_total, repair_to_effects, ExecutionLease, Footprint, LeaseKind, LeaseTable,
    NodeOutcome, RepairAction, Resource, Scheduler, SchedulerEffect,
};
pub use spectral::{VerificationEdge, VerificationGraph};
pub use stability::{StabilityClaim, StabilityParameters};
pub use workgraph::{
    EdgeKind, GraphRevisionReason, GraphValidationReport, NodeClass, WorkEdge, WorkGraphRevision,
    WorkNode, WorkNodeState,
};
