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
