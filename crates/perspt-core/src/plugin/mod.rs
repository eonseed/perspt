//! Language Plugin Architecture
//!
//! Provides a trait-based plugin system for polyglot support.
//! Each language (Rust, Python, JS, etc.) implements this trait.
//!
//! PSP-000005 expands plugins from init-only to full runtime verification contracts.

use serde::{Deserialize, Serialize};
use std::path::Path;

mod contract;
mod js;
mod python;
mod registry;
mod rust;
#[cfg(test)]
mod tests;

pub use contract::*;
pub use js::*;
pub use python::*;
pub use registry::*;
pub use rust::*;
