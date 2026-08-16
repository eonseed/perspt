//! Stability verification, plugin-driven checks, and dependency auto-installation.

use super::*;
use perspt_sdk::{IndependenceRoute, ResidualClass, ResidualEvent, ResidualSeverity, SensorRef};

/// Push a typed PSP-8 residual onto the verification residual vector.
///
/// `score` is the raw residual *magnitude* `r_e ≥ 0` (a count or unit penalty);
/// the energy model squares and weights it later (`V = Σ_e w_e‖r_e‖²`). A
/// construction failure (negative/non-finite score) is dropped with a debug log
mod deps;
mod energy;
mod finalize;
mod plugin;

pub(crate) use finalize::{is_code_source_file, severity_to_str, verification_stages_for_node};
