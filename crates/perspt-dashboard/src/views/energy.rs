//! Energy view model: the `candidate_measured` trajectory from the PSP-9
//! ledger (sequence → energy, hard_pass).

use super::psp9::LedgerProjection;

/// One measured candidate, in ledger order.
pub struct EnergyPoint {
    pub sequence: i64,
    pub node_id: String,
    pub generation: u32,
    pub energy: f64,
    pub hard_pass: bool,
    /// Percentage of energy relative to the session maximum (0-100).
    pub bar_pct: f64,
}

pub struct EnergySummary {
    pub count: usize,
    pub avg_energy: f64,
    pub min_energy: f64,
    pub max_energy: f64,
    pub hard_pass_count: usize,
}

/// View model for the energy trajectory page.
pub struct EnergyViewModel {
    pub session_id: String,
    pub records: Vec<EnergyPoint>,
    pub summary: EnergySummary,
}

impl EnergyViewModel {
    pub fn from_projection(session_id: String, projection: &LedgerProjection) -> Self {
        let measurements = &projection.measurements;
        let count = measurements.len();
        let hard_pass_count = measurements.iter().filter(|m| m.hard_pass).count();

        let (avg_energy, min_energy, max_energy) = if measurements.is_empty() {
            (0.0, 0.0, 0.0)
        } else {
            let sum: f64 = measurements.iter().map(|m| m.energy).sum();
            let min = measurements
                .iter()
                .map(|m| m.energy)
                .fold(f64::INFINITY, f64::min);
            let max = measurements
                .iter()
                .map(|m| m.energy)
                .fold(f64::NEG_INFINITY, f64::max);
            (sum / count as f64, min, max)
        };

        let max_for_bar = if max_energy > 0.0 { max_energy } else { 1.0 };
        let records = measurements
            .iter()
            .map(|m| EnergyPoint {
                sequence: m.sequence,
                node_id: m.node_id.clone(),
                generation: m.generation,
                energy: m.energy,
                hard_pass: m.hard_pass,
                bar_pct: (m.energy / max_for_bar * 100.0).clamp(0.0, 100.0),
            })
            .collect();

        Self {
            session_id,
            records,
            summary: EnergySummary {
                count,
                avg_energy,
                min_energy,
                max_energy,
                hard_pass_count,
            },
        }
    }
}
