use perspt_store::EnergyRecord;

/// View model for the energy convergence page
pub struct EnergyViewModel {
    pub session_id: String,
    pub records: Vec<EnergyPoint>,
}

pub struct EnergyPoint {
    pub node_id: String,
    pub v_syn: f32,
    pub v_str: f32,
    pub v_log: f32,
    pub v_boot: f32,
    pub v_sheaf: f32,
    pub v_total: f32,
}

impl EnergyViewModel {
    pub fn from_records(session_id: String, records: Vec<EnergyRecord>) -> Self {
        let points = records
            .into_iter()
            .map(|r| EnergyPoint {
                node_id: r.node_id,
                v_syn: r.v_syn,
                v_str: r.v_str,
                v_log: r.v_log,
                v_boot: r.v_boot,
                v_sheaf: r.v_sheaf,
                v_total: r.v_total,
            })
            .collect();
        Self {
            session_id,
            records: points,
        }
    }
}
