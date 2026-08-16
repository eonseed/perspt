use super::*;

/// PSP-5 Phase 7: Aggregated review summary for a single node.
///
/// Consumed by both TUI review modal and CLI status/resume commands.
#[derive(Debug, Clone)]
pub struct NodeReviewSummary {
    pub node_id: String,
    pub latest_energy: Option<perspt_store::EnergyRecord>,
    pub energy_history: Vec<perspt_store::EnergyRecord>,
    pub attempt_count: u32,
    pub escalation_reports: Vec<perspt_store::EscalationReportRecord>,
    pub sheaf_validations: Vec<perspt_store::SheafValidationRow>,
    pub interface_seals: Vec<perspt_store::InterfaceSealRow>,
    pub context_provenance: Vec<perspt_store::ContextProvenanceRecord>,
    pub branches: Vec<perspt_store::ProvisionalBranchRow>,
}

/// PSP-5 Phase 7: Aggregated session-level review summary.
///
/// Consumed by both TUI dashboard and CLI status/resume commands.
#[derive(Debug, Clone)]
pub struct SessionReviewSummary {
    pub session_id: String,
    pub total_nodes: usize,
    pub completed: usize,
    pub failed: usize,
    pub escalated: usize,
    pub total_energy: f32,
    pub node_energies: Vec<(String, perspt_store::EnergyRecord)>,
    pub escalation_reports: Vec<perspt_store::EscalationReportRecord>,
    pub branches_total: usize,
    pub active_branches: usize,
    pub sealed_branches: usize,
    pub merged_branches: usize,
    pub flushed_branches: usize,
    pub flush_decisions: Vec<perspt_store::BranchFlushRow>,
    /// Review audit: total decisions and breakdown
    pub review_total: usize,
    pub reviews_approved: usize,
    pub reviews_rejected: usize,
    pub reviews_corrected: usize,
}

/// Ledger statistics (Legacy)
#[derive(Debug, Clone)]
pub struct LedgerStats {
    pub total_sessions: usize,
    pub total_commits: usize,
    pub db_size_bytes: u64,
}

/// PSP-5 Phase 8: Per-node evidence bundle for session rehydration.
#[derive(Debug, Clone)]
pub struct NodeSnapshotDetail {
    pub record: NodeStateRecord,
    pub energy_history: Vec<perspt_store::EnergyRecord>,
    pub verification: Option<perspt_store::VerificationResultRow>,
    pub artifact_bundle: Option<perspt_store::ArtifactBundleRow>,
    pub sheaf_validations: Vec<perspt_store::SheafValidationRow>,
    pub interface_seals: Vec<perspt_store::InterfaceSealRow>,
    pub context_provenance: Option<perspt_store::ContextProvenanceRecord>,
}

/// PSP-5 Phase 8: Complete session snapshot for resume/rehydration.
///
/// Aggregates all persisted state needed to reconstruct the orchestrator
/// DAG, restore node states, and resume execution from the last durable
/// boundary.
#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    pub session_id: String,
    pub node_details: Vec<NodeSnapshotDetail>,
    pub graph_edges: Vec<perspt_store::TaskGraphEdgeRow>,
    pub branches: Vec<perspt_store::ProvisionalBranchRow>,
    pub escalation_reports: Vec<perspt_store::EscalationReportRecord>,
    pub flushes: Vec<perspt_store::BranchFlushRow>,
}

/// Generate a unique commit ID
pub(crate) fn generate_commit_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{:x}", now)
}

/// Get current timestamp
pub(crate) fn chrono_timestamp() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// ISO-8601 timestamp for committed_at fields
pub(crate) fn chrono_iso_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    // Simple UTC timestamp — YYYY-MM-DDTHH:MM:SSZ
    let days = secs / 86400;
    let time = secs % 86400;
    let h = time / 3600;
    let m = (time % 3600) / 60;
    let s = time % 60;
    // Days since 1970-01-01 to y/m/d (civil calendar)
    let (y, mo, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, m, s)
}

/// Convert days since Unix epoch to (year, month, day)
pub(crate) fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Algorithm from Howard Hinnant's date library
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
