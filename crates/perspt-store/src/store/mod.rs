//! Session Store Implementation
//!
//! CRUD operations for sessions and the PSP-9 governed ledger surfaces.

use anyhow::{Context, Result};
use duckdb::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::schema::init_schema;

mod handle;
mod psp9_ledger;
mod rows;
mod sessions;
#[cfg(test)]
mod tests;

pub use handle::*;
pub use psp9_ledger::{
    Psp9CalibrationEpochRow, Psp9ExternalEffectRow, Psp9LedgerRow, Psp9VerdictRow,
};
pub use rows::*;
