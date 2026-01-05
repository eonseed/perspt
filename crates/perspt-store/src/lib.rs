//! perspt-store: DuckDB-based persistence layer for SRBN sessions
//!
//! Provides session persistence, node state tracking, and energy history
//! with Merkle tree support for state verification and rollback.

mod schema;
mod store;

pub use schema::init_schema;
pub use store::{EnergyRecord, NodeStateRecord, SessionRecord, SessionStore};
