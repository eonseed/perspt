//! Status command - show agent session status

use anyhow::{Context, Result};

/// Show current agent status
pub async fn run() -> Result<()> {
    let store = perspt_store::SessionStore::new().context("Failed to open session store")?;

    println!("📊 SRBN Agent Status");
    println!("{}", "═".repeat(70));

    // Get the most recent session
    let sessions = store.list_recent_sessions(1)?;

    if sessions.is_empty() {
        println!("No agent sessions found.");
        println!();
        println!("Start a new session with:");
        println!("  perspt agent \"<task description>\"");
        return Ok(());
    }

    let session = &sessions[0];

    // Session info
    println!(
        "📁 Session:    {}",
        &session.session_id[..16.min(session.session_id.len())]
    );

    let status_display = match session.status.as_str() {
        "COMPLETED" | "COMPLETED_PSP9" => "✅ Completed",
        "PARTIAL" => "⚠️ Partial (some nodes escalated)",
        "RUNNING" | "RUNNING_PSP9" => "🔄 Running",
        "PAUSED" => "⏸️ Paused",
        "FAILED" | "FAILED_PSP9" => "❌ Failed",
        "ESCALATED_PSP9" => "⚠️ Escalated (verified but not promoted)",
        "ABORTED_PSP9" => "🛑 Aborted (authority revoked)",
        "active" => "🔄 Active",
        _ => &session.status,
    };
    println!("📌 Status:     {}", status_display);
    println!("📂 Directory:  {}", session.working_dir);
    println!("📝 Task:       {}", session.task);

    if let Some(toolchain) = &session.detected_toolchain {
        println!("🔧 Toolchain:  {}", toolchain);
    }

    // PSP-9 sessions record into the governed ledger, not the legacy node
    // tables, so their status view reads the PSP-9 surfaces.
    if session.status.ends_with("_PSP9") {
        return psp9_status(&store, session);
    }

    // Pre-PSP-9 sessions recorded into the retired orchestrator tables,
    // which no longer have live writers; the legacy detail view was
    // removed with the legacy engine.
    println!();
    println!("This session predates the PSP-9 runtime; detailed status is unavailable.");
    println!("Start a new governed run with: perspt agent \"<task>\"");
    Ok(())
}

/// Fold the ledger event stream into (measurements, denials, last energy,
/// last gate decision).
fn summarize_psp9_events(
    rows: &[perspt_store::Psp9LedgerRow],
) -> (usize, usize, Option<f64>, Option<String>) {
    let mut measurements = 0usize;
    let mut denials = 0usize;
    let mut last_energy = None;
    let mut gate = None;
    for row in rows {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&row.event_json) else {
            continue;
        };
        let payload = value
            .get("payload")
            .map(|payload| payload.get("body").unwrap_or(payload));
        let event = payload
            .and_then(|payload| payload.get("event"))
            .and_then(|event| event.as_str())
            .unwrap_or_default();
        match event {
            "candidate_measured" => {
                measurements += 1;
                last_energy = payload
                    .and_then(|payload| payload.get("energy"))
                    .and_then(serde_json::Value::as_f64);
            }
            "effect_denied" => denials += 1,
            "gate_decision_recorded" => {
                gate = value
                    .get("payload")
                    .and_then(|payload| payload.get("decision"))
                    .map(|decision| decision.to_string());
            }
            _ => {}
        }
    }
    (measurements, denials, last_energy, gate)
}

/// PSP-9 status: the governed ledger, gate decisions, calibration readiness,
/// verdicts, and any incomplete external effects.
/// Conditional-capacity diagnostics (system 15): the arriving-potential
/// gauge Φ(W) over non-terminal nodes, explicitly labeled a diagnostic —
/// positive recurrence is claimed only with Theorem 9's assumption
/// evidence, which the coding domain does not currently provide.
fn print_capacity(rows: &[perspt_store::Psp9LedgerRow]) {
    let mut latest_graph: Option<serde_json::Value> = None;
    let mut last_energy: std::collections::BTreeMap<String, f64> = Default::default();
    for row in rows {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&row.event_json) else {
            continue;
        };
        match value.get("kind").and_then(|k| k.as_str()) {
            Some("graph_revision") => latest_graph = value.get("payload").cloned(),
            Some("tool_loop") => {
                let payload = value.get("payload");
                let is_measure = payload
                    .and_then(|p| p.get("event"))
                    .and_then(|e| e.as_str())
                    == Some("candidate_measured");
                if is_measure {
                    if let (Some(node), Some(energy)) = (
                        payload
                            .and_then(|p| p.get("node_id"))
                            .and_then(|n| n.as_str()),
                        payload
                            .and_then(|p| p.get("energy"))
                            .and_then(|e| e.as_f64()),
                    ) {
                        last_energy.insert(node.to_string(), energy);
                    }
                }
            }
            _ => {}
        }
    }
    let Some(graph) = latest_graph else {
        return;
    };
    let Some(nodes) = graph.get("nodes").and_then(|n| n.as_array()) else {
        return;
    };
    let phi: f64 = nodes
        .iter()
        .filter(|node| {
            matches!(
                node.get("state").and_then(|s| s.as_str()),
                Some("pending") | Some("ready") | Some("running")
            )
        })
        .map(|node| {
            node.get("node_id")
                .and_then(|id| id.as_str())
                .and_then(|id| last_energy.get(id).copied())
                .unwrap_or(1.0)
        })
        .sum();
    println!("📊 Φ(W):          {phi:.3} (conditional-capacity diagnostic, not a stability claim)");
    println!(
        "📊 topology_gap:  not populated (the coding domain declares no coordinate map, so μ \
         is never measured)"
    );
}

/// Validator-independence statistics over labeled matched verdicts
/// (system 8). Point estimates are labeled diagnostics; `rho_eff` is shown
/// only when every pair met the matched-sample floor.
fn print_independence(store: &perspt_store::SessionStore) {
    let Ok(rows) = store.labeled_psp9_verdicts() else {
        return;
    };
    let records = verdict_records(&rows);
    if records.is_empty() {
        println!("🧪 Independence:  insufficient evidence (no labeled matched verdicts)");
        return;
    }
    match perspt_sdk::independence::compute(&records) {
        Ok(stats) => {
            for ((left, right), pair) in &stats.pairs {
                println!(
                    "🧪 Pair {left} × {right}: q={:.3}/{:.3} joint={:.3} upper={:.3} n={} \
                     (label source: delayed audit)",
                    pair.q_i, pair.q_j, pair.joint_miss, pair.joint_miss_upper, pair.samples
                );
            }
            match stats.rho_eff {
                Some(rho) => println!(
                    "🧪 Independence:  rho_eff = {rho:.4} CERTIFIED ({} validators)",
                    stats.validators
                ),
                None => println!(
                    "🧪 Independence:  insufficient evidence ({} validators, floor {})",
                    stats.validators,
                    perspt_sdk::independence::DEFAULT_MIN_PAIR_SAMPLES
                ),
            }
        }
        Err(_) => println!("🧪 Independence:  insufficient evidence"),
    }
}

/// The estimator's `missed` is the false negative against the delayed
/// label: the validator passed a candidate later labeled unsafe.
fn verdict_records(rows: &[perspt_store::Psp9VerdictRow]) -> Vec<perspt_sdk::VerdictRecord> {
    rows.iter()
        .filter_map(|row| {
            let unsafe_label = row.unsafe_label?;
            Some(perspt_sdk::VerdictRecord::new(
                row.validator_id.clone(),
                row.candidate_id.clone(),
                !row.missed && unsafe_label,
            ))
        })
        .collect()
}

fn psp9_status(
    store: &perspt_store::SessionStore,
    session: &perspt_store::SessionRecord,
) -> Result<()> {
    let rows = store.get_psp9_events(&session.session_id)?;
    println!();
    println!("PSP-9 governed session");
    println!("{}", "─".repeat(70));
    println!("📜 Ledger events: {}", rows.len());
    if let Some(last) = rows.last() {
        println!(
            "🔗 Ledger head:   {}",
            &last.hash[..16.min(last.hash.len())]
        );
    }

    let (measurements, denials, last_energy, gate) = summarize_psp9_events(&rows);
    println!("📐 Measurements:  {measurements}");
    if let Some(energy) = last_energy {
        println!("⚡ Last energy:   V = {energy:.3}");
    }
    if let Some(gate) = gate {
        println!("🚪 Last gate:     {gate}");
    }
    println!("🚫 Denials:       {denials}");

    let verdicts = store.get_psp9_verdicts(&session.session_id)?;
    for verdict in &verdicts {
        println!(
            "⚖️  Verdict:       {} {} (labeled: {})",
            verdict.validator_id,
            if verdict.missed { "MISS" } else { "pass" },
            verdict
                .unsafe_label
                .map(|unsafe_label| if unsafe_label { "unsafe" } else { "safe" })
                .unwrap_or("pending"),
        );
    }

    print_capacity(&rows);
    print_independence(store);

    let pending = store.pending_external_effects(&session.session_id)?;
    if !pending.is_empty() {
        println!("⚠️  Incomplete external effects: {}", pending.len());
        println!(
            "    Run `perspt resume {}` to finish them.",
            session.session_id
        );
    }

    println!("{}", "─".repeat(70));
    println!("Commands:");
    println!(
        "  perspt replay {}   Credential-free audit replay",
        session.session_id
    );
    Ok(())
}
