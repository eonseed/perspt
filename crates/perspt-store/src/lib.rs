//! perspt-store: DuckDB-based persistence layer for SRBN sessions
//!
//! Provides session persistence, node state tracking, and energy history
//! with Merkle tree support for state verification and rollback.

mod repair;
mod schema;
mod store;

pub use repair::{RepairReport, repair_database};
pub use schema::init_schema;
pub use store::{
    Psp9CalibrationEpochRow, Psp9ExternalEffectRow, Psp9LedgerRow, Psp9VerdictRow, SessionRecord,
    SessionStore,
};
