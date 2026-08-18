//! The bounded search plane (PSP-10 systems 19–21).
//!
//! Types and deterministic rules only: forests, branches, witnesses,
//! partial checkpoints, limits with the reservation discipline, and the
//! Proposition 5 selection order. Execution lives in
//! `perspt-agent::runtime::search`.

pub mod budget;
pub mod context;
pub mod domain_types;
pub mod forest;
pub mod select;

pub use budget::{ReservationRequest, ReservationTicket, SearchLimits, SearchUsage};
pub use context::{exploration_capability, is_read_only_capability, ProjectMap};
pub use domain_types::{
    BranchMeasurement, BranchSelection, DomainMeasurement, SearchContext, SearchStrategy,
};
pub use forest::{
    ObligationRef, PartialCheckpointRef, PromptProgramDigest, SearchBranch, SearchBranchState,
    SearchEvidenceDigest, SearchForest, WitnessRef,
};
pub use select::{frontier_order, select_branch, BranchCandidate, FrontierEntry};
