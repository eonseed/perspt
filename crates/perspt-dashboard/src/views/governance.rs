//! Governance view model: calibration readiness, authority epochs, verdicts,
//! and pending delayed audits (PSP-9 systems 12 and 16).

/// One calibration epoch row for display.
pub struct EpochRow {
    pub epoch_id_short: String,
    pub state: String,
    pub target_rho: String,
    pub threshold: String,
    pub sample_count: i64,
    pub model_route: String,
}

/// One validator verdict row for display.
pub struct VerdictRow {
    pub validator_id: String,
    pub candidate_short: String,
    pub missed: bool,
    pub label: String,
}

/// One pending audit sample for display.
pub struct PendingAuditRow {
    pub sample_short: String,
    pub score: String,
    pub epoch_short: String,
}

/// One validator pair's matched-stratum independence statistics.
pub struct IndependencePairRow {
    pub validator_i: String,
    pub validator_j: String,
    pub q_i: String,
    pub q_j: String,
    pub joint_miss: String,
    pub joint_miss_upper: String,
    pub samples: usize,
}

/// Validator-independence table (PSP-9 system 8). Labels come from the
/// delayed audit oracle; `rho_eff` is only shown when certified.
pub struct IndependenceView {
    pub pairs: Vec<IndependencePairRow>,
    pub validators: usize,
    /// `rho_eff` when certified, else the literal "insufficient evidence".
    pub certification: String,
    pub labeled_records: usize,
}

impl IndependenceView {
    /// Build from labeled verdict rows. `compute` errors on empty input, so
    /// the no-evidence state is rendered without calling it.
    pub fn from_rows(labeled: &[perspt_store::Psp9VerdictRow]) -> Self {
        let records: Vec<perspt_sdk::independence::VerdictRecord> = labeled
            .iter()
            .map(|row| {
                // A miss vs the delayed label: the validator passed the
                // candidate (`!missed`) and the audit later labeled it unsafe.
                let missed_vs_label = !row.missed && row.unsafe_label == Some(true);
                perspt_sdk::independence::VerdictRecord::new(
                    row.validator_id.clone(),
                    row.candidate_id.clone(),
                    missed_vs_label,
                )
            })
            .collect();
        let stats = if records.is_empty() {
            None
        } else {
            perspt_sdk::independence::compute(&records).ok()
        };
        let pairs = stats
            .as_ref()
            .map(|s| {
                s.pairs
                    .iter()
                    .map(|((vi, vj), pair)| IndependencePairRow {
                        validator_i: vi.clone(),
                        validator_j: vj.clone(),
                        q_i: format!("{:.3}", pair.q_i),
                        q_j: format!("{:.3}", pair.q_j),
                        joint_miss: format!("{:.3}", pair.joint_miss),
                        joint_miss_upper: format!("{:.3}", pair.joint_miss_upper),
                        samples: pair.samples,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let certification = stats
            .as_ref()
            .and_then(|s| s.rho_eff)
            .map(|rho| format!("ρ_eff = {rho:.4}"))
            .unwrap_or_else(|| "insufficient evidence".into());
        Self {
            pairs,
            validators: stats.map(|s| s.validators).unwrap_or(0),
            certification,
            labeled_records: labeled.len(),
        }
    }
}

pub struct GovernanceViewModel {
    pub session_id: String,
    pub authority_epoch: u64,
    pub grant_signed: bool,
    pub epochs: Vec<EpochRow>,
    pub verdicts: Vec<VerdictRow>,
    pub pending_audits: Vec<PendingAuditRow>,
}

fn short(value: &str, len: usize) -> String {
    value.chars().take(len).collect()
}

impl GovernanceViewModel {
    pub fn from_store(
        session_id: String,
        authority_epoch: u64,
        grant_policy_json: Option<String>,
        epochs: Vec<perspt_store::Psp9CalibrationEpochRow>,
        verdicts: Vec<perspt_store::Psp9VerdictRow>,
        pending: Vec<(String, String, f64)>,
    ) -> Self {
        let grant_signed = grant_policy_json
            .as_deref()
            .is_some_and(|json| json.contains("\"signature\""));
        let epochs = epochs
            .into_iter()
            .map(|epoch| {
                // The stratum is serialized JSON; surface the model route,
                // which is what an operator scans for.
                let model_route = serde_json::from_str::<serde_json::Value>(&epoch.stratum)
                    .ok()
                    .and_then(|value| {
                        value
                            .get("model_route")
                            .and_then(|route| route.as_str())
                            .map(str::to_string)
                    })
                    .unwrap_or_else(|| short(&epoch.stratum, 24));
                EpochRow {
                    epoch_id_short: short(&epoch.epoch_id, 8),
                    state: epoch.state,
                    target_rho: format!("{:.3}", epoch.target_rho),
                    threshold: epoch
                        .threshold
                        .map(|theta| format!("{theta:.3}"))
                        .unwrap_or_else(|| "—".into()),
                    sample_count: epoch.sample_count,
                    model_route,
                }
            })
            .collect();
        let verdicts = verdicts
            .into_iter()
            .map(|verdict| VerdictRow {
                validator_id: verdict.validator_id,
                candidate_short: short(&verdict.candidate_id, 12),
                missed: verdict.missed,
                label: match verdict.unsafe_label {
                    Some(true) => "unsafe".into(),
                    Some(false) => "safe".into(),
                    None => "pending".into(),
                },
            })
            .collect();
        let pending_audits = pending
            .into_iter()
            .map(|(epoch_id, sample_id, score)| PendingAuditRow {
                sample_short: short(&sample_id, 16),
                score: format!("{score:.3}"),
                epoch_short: short(&epoch_id, 8),
            })
            .collect();
        Self {
            session_id,
            authority_epoch,
            grant_signed,
            epochs,
            verdicts,
            pending_audits,
        }
    }
}
