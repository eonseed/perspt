//! perspt-store: DuckDB-based persistence layer for SRBN sessions
//!
//! Provides session persistence, node state tracking, and energy history
//! with Merkle tree support for state verification and rollback.

mod schema;
mod store;

pub use schema::init_schema;
pub use store::{
    ArtifactBundleRow, BranchFlushRow, BranchLineageRow, BudgetEnvelopeRow,
    ContextProvenanceRecord, CorrectionAttemptRow, EnergyRecord, EscalationReportRecord,
    FeatureCharterRow, InterfaceSealRow, LlmRequestRecord, NodeStateRecord, PlanRevisionRow,
    ProvisionalBranchRow, RepairFootprintRow, ReviewOutcomeRow, RewriteRecordRow, SessionRecord,
    SessionStore, SheafValidationRow, SrbnStepRecord, StructuralDigestRecord, TaskGraphEdgeRow,
    VerificationResultRow,
};
