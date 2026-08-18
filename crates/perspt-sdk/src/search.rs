//! Search-plane data types (PSP-10 system 19; minimal Phase 6 subset).
//!
//! This module currently holds only the data types the domain-package
//! contract needs (`search_strategies`, `compare_branch_measurements`).
//! Phase 7 expands it into the full forest: `SearchForest`,
//! `SearchLimits`, budgets, selection, and frontier scheduling.

use serde::{Deserialize, Serialize};

use crate::residual::{ResidualClass, ResidualEvent};

/// What a domain sees when proposing search strategies.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SearchContext {
    pub task: String,
    /// Dominant residual classes of the failed attempt, most severe first.
    pub dominant_residuals: Vec<ResidualClass>,
    /// Strategy ids already tried this forest (duplicate suppression).
    pub tried_strategies: Vec<String>,
}

/// One domain-proposed branch strategy. A strategy is a search prior; it
/// carries no authority and no acceptance semantics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchStrategy {
    pub strategy_id: String,
    pub description: String,
    /// Optional preferred route label; the runtime may ignore it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_preference: Option<String>,
}

/// The accepted root's measurement, for branch comparison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DomainMeasurement {
    pub energy: f64,
    pub hard_pass: bool,
    pub residuals: Vec<ResidualEvent>,
}

/// One branch candidate's realized measurement (preview, not a decision).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BranchMeasurement {
    pub branch_id: String,
    pub candidate_id: String,
    pub energy: f64,
    pub hard_pass: bool,
    pub residuals: Vec<ResidualEvent>,
    /// The immutable sensor-profile identity these numbers were measured
    /// under; candidates with unequal profiles are never compared by
    /// energy (Proposition 5).
    pub sensor_profile: String,
    /// Recorded cost (selection key 4).
    pub cost: f64,
}

/// A domain's branch comparison verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchSelection {
    Selected { branch_id: String },
    NoneEligible,
}
