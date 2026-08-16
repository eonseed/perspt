//! DuckDB Merkle Ledger
//!
//! Persistent storage for session history, commits, and Merkle proofs.

use anyhow::{Context, Result};
pub use perspt_store::{LlmRequestRecord, NodeStateRecord, SessionRecord, SessionStore};
use std::path::{Path, PathBuf};

/// Full commit payload collected by the orchestrator at commit time.
///
/// Bundles graph-structural fields, retry/error metadata, and merkle
/// material so that `commit_node_snapshot()` can persist a complete
/// node record in a single call.
mod chain;
mod planning;
mod records;
mod summaries;

pub use chain::*;
pub use summaries::*;
pub(crate) use summaries::{chrono_iso_now, chrono_timestamp, generate_commit_id};
