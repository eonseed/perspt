use perspt_store::EnergyRecord;

/// View model for the energy convergence page
pub struct EnergyViewModel {
    pub session_id: String,
    pub records: Vec<EnergyPoint>,
    pub summary: EnergySummary,
}

pub struct EnergySummary {
    pub count: usize,
    pub avg_total: f32,
    pub min_total: f32,
    pub max_total: f32,
    pub avg_syn: f32,
    pub avg_str: f32,
    pub avg_log: f32,
    pub avg_boot: f32,
    pub avg_sheaf: f32,
}

pub struct EnergyPoint {
    pub node_id: String,
    pub v_syn: f32,
    pub v_str: f32,
    pub v_log: f32,
    pub v_boot: f32,
    pub v_sheaf: f32,
    pub v_total: f32,
    /// Percentage of v_total relative to max v_total in session (0-100)
    pub bar_pct: f32,
}

impl EnergyViewModel {
    pub fn from_records(session_id: String, records: Vec<EnergyRecord>) -> Self {
        let count = records.len();

        let (avg_total, min_total, max_total, avg_syn, avg_str, avg_log, avg_boot, avg_sheaf) =
            if records.is_empty() {
                (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
            } else {
                let n = count as f32;
                let sum_total: f32 = records.iter().map(|r| r.v_total).sum();
                let min_total = records
                    .iter()
                    .map(|r| r.v_total)
                    .fold(f32::INFINITY, f32::min);
                let max_total = records
                    .iter()
                    .map(|r| r.v_total)
                    .fold(f32::NEG_INFINITY, f32::max);
                let avg_syn: f32 = records.iter().map(|r| r.v_syn).sum::<f32>() / n;
                let avg_str: f32 = records.iter().map(|r| r.v_str).sum::<f32>() / n;
                let avg_log: f32 = records.iter().map(|r| r.v_log).sum::<f32>() / n;
                let avg_boot: f32 = records.iter().map(|r| r.v_boot).sum::<f32>() / n;
                let avg_sheaf: f32 = records.iter().map(|r| r.v_sheaf).sum::<f32>() / n;
                (
                    sum_total / n,
                    min_total,
                    max_total,
                    avg_syn,
                    avg_str,
                    avg_log,
                    avg_boot,
                    avg_sheaf,
                )
            };

        let max_for_bar = if max_total > 0.0 { max_total } else { 1.0 };

        let points = records
            .into_iter()
            .map(|r| {
                let bar_pct = (r.v_total / max_for_bar * 100.0).min(100.0);
                EnergyPoint {
                    node_id: r.node_id,
                    v_syn: r.v_syn,
                    v_str: r.v_str,
                    v_log: r.v_log,
                    v_boot: r.v_boot,
                    v_sheaf: r.v_sheaf,
                    v_total: r.v_total,
                    bar_pct,
                }
            })
            .collect();

        Self {
            session_id,
            records: points,
            summary: EnergySummary {
                count,
                avg_total,
                min_total,
                max_total,
                avg_syn,
                avg_str,
                avg_log,
                avg_boot,
                avg_sheaf,
            },
        }
    }
}
