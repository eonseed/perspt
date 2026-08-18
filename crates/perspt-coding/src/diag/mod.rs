//! The structured verifier plane (PSP-10 system 26).
//!
//! Language adapters preserve as much source structure as each tool
//! provides: cargo JSON messages, `ty` LSP/text diagnostics, Pyright's
//! JSON fallback (separately fingerprinted), pytest JUnit XML, and
//! normalized `tsc` text. Cascades cluster by root cause before scoring;
//! magnitudes are profile-normalized, replacing the historic unconditional
//! `score = 1.0`.

pub mod cargo;
pub mod cluster;
pub mod pyright;
pub mod pytest;
pub mod tsc;
pub mod ty;
pub mod types;

pub use cluster::{cluster, DiagnosticCluster, CLUSTER_PROFILE_V1};
pub use types::{cluster_residual, StructuredDiagnostic};
