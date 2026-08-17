//! Shared energy-component breakdown used by events and the TUI.

use serde::{Deserialize, Serialize};

/// Energy components for Lyapunov calculation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnergyComponents {
    /// Syntactic energy (from LSP diagnostics)
    pub v_syn: f32,
    /// Structural energy (from contract verification)
    pub v_str: f32,
    /// Logic energy (from test results)
    pub v_log: f32,
    /// Bootstrapping energy (from command exit codes)
    pub v_boot: f32,
    /// Sheaf validation energy (cross-node consistency)
    pub v_sheaf: f32,
}

impl EnergyComponents {
    /// Total energy `V(x) = Σ_comp V_comp` (PSP-8 System 2).
    ///
    /// The five fields are the *derived component rollups* of the single
    /// canonical quadratic energy `V(x) = Σ_e w_e‖r_e(x)‖²`: each already carries
    /// its squared, weighted residual contribution (`V_comp = Σ_{e∈comp} w_e‖r_e‖²`),
    /// so the total is their plain sum. There is no second `α/β/γ` weighting pass —
    /// those per-component weights are folded into the residual weights `w_e` of
    /// the [`crate`]'s energy model before the rollups are formed.
    pub fn total(&self) -> f32 {
        self.v_syn + self.v_str + self.v_log + self.v_boot + self.v_sheaf
    }

    /// Total energy for Solo Mode. Identical to [`EnergyComponents::total`] now
    /// that aggregation carries no separate weights.
    pub fn total_simple(&self) -> f32 {
        self.total()
    }
}
