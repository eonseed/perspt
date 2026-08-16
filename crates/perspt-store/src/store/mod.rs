//! Session Store Implementation
//!
//! Provides CRUD operations for SRBN sessions, node states, and energy history.

use anyhow::{Context, Result};
use duckdb::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::schema::init_schema;

mod branches;
mod evidence;
mod handle;
mod planning;
mod psp9_ledger;
mod reviews;
mod rows;
mod sessions;
mod steps;
mod telemetry;
#[cfg(test)]
mod tests;

pub use handle::*;
pub use psp9_ledger::{
    Psp9CalibrationEpochRow, Psp9ExternalEffectRow, Psp9LedgerRow, Psp9VerdictRow,
};
pub use rows::*;
