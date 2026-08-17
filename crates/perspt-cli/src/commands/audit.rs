//! Audit command — delayed calibration labels and conformal activation
//! (PSP-9 system 16, Paper III Theorem 5).
//!
//! `perspt audit` lists promoted candidates waiting for their delayed audit
//! label. `perspt audit label <sample> --safe|--unsafe` ingests one label and
//! then recomputes the stratum's conformal threshold over *labeled* samples
//! only. When the finite sample floor `n >= 1/rho - 1` is met, a **new**
//! immutable epoch row is activated; prior epochs are never mutated, so
//! every historical readiness claim stays auditable.

use anyhow::{Context, Result};
use perspt_sdk::{CalibrationSample, ThresholdOutcome};

pub async fn run(sample: Option<String>, safe: bool, mark_unsafe: bool) -> Result<()> {
    let store = perspt_store::SessionStore::new().context("opening the session store")?;
    match sample {
        None => list_pending(&store),
        Some(sample_id) => {
            anyhow::ensure!(
                safe ^ mark_unsafe,
                "pass exactly one of --safe or --unsafe with a sample id"
            );
            ingest_label(&store, &sample_id, mark_unsafe)
        }
    }
}

fn list_pending(store: &perspt_store::SessionStore) -> Result<()> {
    let pending = store.pending_psp9_audit_samples(50)?;
    if pending.is_empty() {
        println!("No promoted candidates are waiting for a delayed audit label.");
        return Ok(());
    }
    println!("Pending delayed-audit samples ({}):", pending.len());
    for (epoch_id, sample_id, score) in &pending {
        println!(
            "  {}  score {:.3}  epoch {}",
            &sample_id[..16.min(sample_id.len())],
            score,
            &epoch_id[..8.min(epoch_id.len())]
        );
    }
    println!();
    println!("Label one with: perspt audit <sample_id> --safe | --unsafe");
    Ok(())
}

fn ingest_label(
    store: &perspt_store::SessionStore,
    sample_id: &str,
    is_unsafe: bool,
) -> Result<()> {
    // Prefix match so the truncated ids printed by `perspt audit` work.
    let resolved = resolve_sample(store, sample_id)?;
    let labeled = store.label_psp9_calibration_sample(&resolved.1, is_unsafe)?;
    anyhow::ensure!(
        labeled > 0,
        "sample {sample_id} is unknown or already labeled (labels are single-assignment)"
    );
    // The same delayed oracle labels validator verdicts (candidate id ==
    // sample id == realized state root), feeding the pairwise
    // independence estimator (system 8).
    let verdicts = store.label_psp9_verdicts(&resolved.1, is_unsafe)?;
    println!(
        "Labeled {} as {} ({verdicts} validator verdict(s) labeled).",
        &resolved.1[..16.min(resolved.1.len())],
        if is_unsafe { "UNSAFE" } else { "safe" }
    );
    recompute_stratum(store, &resolved.0)
}

/// (epoch_id, full sample_id) for a possibly-truncated sample id.
fn resolve_sample(store: &perspt_store::SessionStore, prefix: &str) -> Result<(String, String)> {
    let pending = store.pending_psp9_audit_samples(1000)?;
    let matches: Vec<_> = pending
        .iter()
        .filter(|(_, sample_id, _)| sample_id.starts_with(prefix))
        .collect();
    match matches.as_slice() {
        [(epoch_id, sample_id, _)] => Ok((epoch_id.clone(), sample_id.clone())),
        [] => anyhow::bail!("no pending sample matches {prefix:?}"),
        _ => anyhow::bail!("{prefix:?} is ambiguous; use more of the sample id"),
    }
}

/// Recompute the stratum threshold over labeled samples and, at the finite
/// sample floor, activate a new immutable epoch.
fn recompute_stratum(store: &perspt_store::SessionStore, epoch_id: &str) -> Result<()> {
    let stratum = store
        .psp9_epoch_stratum(epoch_id)?
        .context("labeled sample's epoch has no stratum")?;
    let current = store
        .latest_psp9_calibration_epoch(&stratum)?
        .context("stratum has no epoch")?;
    let labeled: Vec<CalibrationSample> = store
        .labeled_psp9_samples_for_stratum(&stratum)?
        .into_iter()
        .map(|(score, is_unsafe)| CalibrationSample::new(score, is_unsafe))
        .collect();
    match perspt_sdk::conformal_threshold_checked(&labeled, current.target_rho)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?
    {
        ThresholdOutcome::Feasible { theta_hat } => {
            let activated = perspt_store::Psp9CalibrationEpochRow {
                epoch_id: uuid::Uuid::new_v4().to_string(),
                stratum,
                target_rho: current.target_rho,
                threshold: Some(theta_hat),
                state: "active".into(),
                sample_count: labeled.len() as i64,
            };
            store.record_psp9_calibration_epoch(&activated)?;
            println!(
                "Activated epoch {} for the stratum: theta = {theta_hat:.3} over {} labeled \
                 sample(s) at rho = {}.",
                &activated.epoch_id[..8.min(activated.epoch_id.len())],
                labeled.len(),
                current.target_rho
            );
            println!("Hard-pass candidates above theta now commit without a prompt.");
        }
        ThresholdOutcome::InsufficientCalibration { have, need } => {
            println!(
                "Stratum remains uncertified: {have} labeled sample(s), floor is {need} \
                 (rho = {}). Promotion keeps routing to approval.",
                current.target_rho
            );
        }
    }
    Ok(())
}
