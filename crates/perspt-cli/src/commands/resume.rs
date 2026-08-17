//! Resume command - resume a paused or crashed session

use anyhow::{Context, Result};
use std::path::PathBuf;

/// Resume a paused or crashed session
pub async fn run(session_id: Option<String>, last: bool, db_path: Option<PathBuf>) -> Result<()> {
    let store = std::sync::Arc::new(super::psp9_chain::open_store(db_path.as_deref(), false)?);

    let target = if last {
        let sessions = store.list_recent_sessions(1)?;
        anyhow::ensure!(!sessions.is_empty(), "no sessions found to resume");
        Some(sessions[0].session_id.clone())
    } else {
        session_id
    };
    match target {
        Some(id) => resume_session(store, &id).await,
        None => list_sessions(&store).await,
    }
}

/// List recent sessions for the user to choose from
async fn list_sessions(store: &perspt_store::SessionStore) -> Result<()> {
    let sessions = store.list_recent_sessions(10)?;

    if sessions.is_empty() {
        println!("No sessions found.");
        println!();
        println!("Start a new session with: perspt agent \"<task>\"");
        return Ok(());
    }

    println!("Recent Sessions:");
    println!("{}", "─".repeat(80));
    println!("{:<12} {:<12} {:<50}", "SESSION ID", "STATUS", "TASK");
    println!("{}", "─".repeat(80));

    for session in &sessions {
        // Truncate task to 48 chars for display
        let task_display = if session.task.len() > 48 {
            format!("{}...", &session.task[..45])
        } else {
            session.task.clone()
        };

        // Shorten session ID for display
        let id_short = if session.session_id.len() > 10 {
            format!("{}...", &session.session_id[..8])
        } else {
            session.session_id.clone()
        };

        let status_emoji = match session.status.as_str() {
            "COMPLETED" => "✅",
            "RUNNING" => "🔄",
            "PAUSED" => "⏸️",
            "FAILED" => "❌",
            _ => "❓",
        };

        println!(
            "{:<12} {} {:<10} {:<50}",
            id_short, status_emoji, session.status, task_display
        );
    }

    println!("{}", "─".repeat(80));
    println!();
    println!("Resume with: perspt resume <session_id>");
    println!("Resume last: perspt resume --last");

    Ok(())
}

/// Resume a specific session
async fn resume_session(
    store: std::sync::Arc<perspt_store::SessionStore>,
    session_id: &str,
) -> Result<()> {
    let actual_id = session_id.to_string();

    // Get the session
    let session = store
        .get_session(&actual_id)?
        .context(format!("Session not found: {}", actual_id))?;

    println!("📂 Resuming session: {}", session.session_id);
    println!("📝 Task: {}", session.task);
    println!("📁 Working dir: {}", session.working_dir);
    println!("🔖 Status: {}", session.status);

    if session.status.ends_with("_PSP9") {
        return resume_psp9(store, &session).await;
    }
    let store = &*store;

    // Get completed nodes
    let node_states = store.get_node_states(&actual_id)?;
    let completed_count = node_states
        .iter()
        .filter(|n| perspt_core::types::NodeState::from_display_str(&n.state).is_success())
        .count();

    println!(
        "✅ Completed nodes: {}/{}",
        completed_count,
        node_states.len()
    );

    // PSP-5 Phase 6: Show provisional branch state
    let branches = store.get_provisional_branches(&actual_id)?;
    if !branches.is_empty() {
        let active = branches.iter().filter(|b| b.state == "active").count();
        let flushed = branches.iter().filter(|b| b.state == "flushed").count();
        if active > 0 || flushed > 0 {
            println!(
                "🌿 Provisional: {} active, {} flushed (of {} total)",
                active,
                flushed,
                branches.len()
            );
        }
    }

    // PSP-5 Phase 7: Show trust context before resuming
    let escalations = store.get_escalation_reports(&actual_id)?;
    if !escalations.is_empty() {
        println!("⚠️  Escalations: {} recorded", escalations.len());
    }
    // Show last energy state
    if let Some(latest) = node_states.last() {
        if let Ok(energy_history) = store.get_energy_history(&actual_id, &latest.node_id) {
            if let Some(last_energy) = energy_history.last() {
                println!(
                    "⚡ Last energy: V(x)={:.3} (syn={:.2} str={:.2} log={:.2})",
                    last_energy.v_total, last_energy.v_syn, last_energy.v_str, last_energy.v_log
                );
            }
        }
    }
    let total_retries: i32 = node_states.iter().map(|n| n.attempt_count.max(0)).sum();
    if total_retries > 0 {
        println!("↻  Total retries: {}", total_retries);
    }

    // PSP-5 Phase 8: Show budget position before resuming
    if let Ok(Some(budget)) = store.get_budget_envelope(&actual_id) {
        let steps_str = budget
            .max_steps
            .map(|m| format!("{}/{}", budget.steps_used, m))
            .unwrap_or_else(|| format!("{}", budget.steps_used));
        let cost_str = budget
            .max_cost_usd
            .map(|m| format!("${:.2}/${:.2}", budget.cost_used_usd, m))
            .unwrap_or_else(|| format!("${:.2}", budget.cost_used_usd));
        println!("💰 Budget: steps={} cost={}", steps_str, cost_str);
    }

    // PSP-7: Show step timeline and correction summary before resuming
    if let Ok(steps) = store.get_session_steps(&actual_id) {
        if !steps.is_empty() {
            let corrections: Vec<_> = steps
                .iter()
                .filter(|s| s.step == "converge" && s.attempt_count > 0)
                .collect();
            if !corrections.is_empty() {
                let total_attempts: i32 = corrections.iter().map(|s| s.attempt_count).sum();
                println!(
                    "🔄 Corrections: {} node(s), {} attempts",
                    corrections.len(),
                    total_attempts
                );
            }
        }
    }

    // Check if session is already completed
    if session.status == "COMPLETED" {
        println!();
        println!("ℹ️  This session is already completed.");
        println!("   Start a new session with: perspt agent \"<task>\"");
        return Ok(());
    }

    // Update session status to RUNNING
    store.update_session_status(&actual_id, "RUNNING")?;

    // Create orchestrator and resume
    let working_dir = PathBuf::from(&session.working_dir);

    if !working_dir.exists() {
        anyhow::bail!(
            "Working directory no longer exists: {}",
            session.working_dir
        );
    }

    println!();
    println!("🚀 Resuming orchestration...");
    println!();

    // PSP-5 Phase 8: Rehydrate from persisted session state instead of
    // creating a fresh orchestrator that would re-plan from scratch.
    let mut orchestrator = perspt_agent::SRBNOrchestrator::new(
        working_dir.clone(),
        false, // Don't auto-approve on resume
    );

    // Attempt ledger-backed rehydration; fall back to fresh run if the
    // session has no persisted node data (pre-Phase-8 session or empty DAG).
    let rehydrated = match orchestrator.rehydrate_session(&actual_id) {
        Ok(snapshot) => {
            let total = snapshot.node_details.len();
            let terminal = snapshot
                .node_details
                .iter()
                .filter(|d| {
                    matches!(
                        d.record.state.as_str(),
                        "Completed"
                            | "COMPLETED"
                            | "STABLE"
                            | "Failed"
                            | "FAILED"
                            | "Aborted"
                            | "ABORTED"
                    )
                })
                .count();
            println!(
                "📦 Rehydrated {} nodes ({} terminal, {} to resume)",
                total,
                terminal,
                total - terminal
            );

            // Show degraded conditions
            let missing_goals = snapshot
                .node_details
                .iter()
                .filter(|d| d.record.goal.is_none())
                .count();
            if missing_goals > 0 {
                println!(
                    "⚠️  Degraded: {} nodes missing goal metadata (older session)",
                    missing_goals
                );
            }

            true
        }
        Err(e) => {
            println!(
                "⚠️  Cannot rehydrate session ({}), falling back to fresh run",
                e
            );
            false
        }
    };

    let result = if rehydrated {
        orchestrator.run_resumed().await
    } else {
        orchestrator.run(session.task.clone()).await
    };

    match result {
        Ok(()) => {
            println!("✅ Session completed successfully!");
        }
        Err(e) => {
            println!("❌ Session failed: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

/// PSP-9 resume verifies the ledger and completes bracketed promotion intents
/// from content-addressed artifacts. Mid-loop continuation reconstructs the
/// accepted candidate and provider-neutral conversation, then mints fresh
/// capabilities because live authority is intentionally never serialized.
async fn resume_psp9(
    store: std::sync::Arc<perspt_store::SessionStore>,
    session: &perspt_store::SessionRecord,
) -> Result<()> {
    verify_psp9_chain(&store, &session.session_id)?;
    let pending = store.pending_external_effects(&session.session_id)?;
    if pending.is_empty() {
        println!("PSP-9 ledger is valid and has no incomplete external effects.");
        if session.status == "RUNNING_PSP9" {
            // Mid-loop continuation: only from a durable candidate checkpoint;
            // Perspt never silently restarts an interrupted task.
            anyhow::ensure!(
                has_candidate_checkpoint(&store, &session.session_id)?,
                "the process stopped inside a model/tool loop without a durable candidate \
                 checkpoint; exact continuation is unavailable, and Perspt will not silently \
                 restart the task"
            );
            return resume_mid_loop(store, session).await;
        }
        println!("This session is terminal; start a new agent session for additional work.");
        return Ok(());
    }
    let current_epoch = store.authority_epoch(&session.session_id)?;
    verify_persistent_grant(&store, &session.session_id, current_epoch)?;
    for effect in pending {
        let intent: serde_json::Value = serde_json::from_str(&effect.intent_json)?;
        let epoch = intent
            .get("authority_epoch")
            .and_then(serde_json::Value::as_u64)
            .context("promotion intent has no authority epoch")?;
        anyhow::ensure!(
            epoch == current_epoch,
            "refusing stale promotion intent at epoch {epoch}; current epoch is {current_epoch}"
        );
        let workspace = PathBuf::from(
            intent
                .get("workspace_root")
                .and_then(serde_json::Value::as_str)
                .context("promotion intent has no workspace binding")?,
        )
        .canonicalize()?;
        anyhow::ensure!(
            workspace == PathBuf::from(&session.working_dir).canonicalize()?,
            "promotion workspace binding differs from the session workspace"
        );
        let files = intent
            .get("files")
            .and_then(serde_json::Value::as_array)
            .context("promotion intent has no file manifest")?;
        for file in files {
            let recovery = prepare_promoted_file(&store, &workspace, file)?;
            store.with_authority_epoch(&session.session_id, epoch, || {
                recover_promoted_file(&recovery)
            })?;
        }
        store.complete_external_effect(
            &session.session_id,
            &effect.idempotency_key,
            &serde_json::json!({"recovered": true, "files": files.len()}).to_string(),
        )?;
        println!(
            "Completed interrupted promotion {} ({} file(s)).",
            effect.idempotency_key,
            files.len()
        );
    }
    // Finishing an interrupted promotion completes only sessions that were
    // still running; a FAILED/ESCALATED/ABORTED terminal status is evidence
    // and must never be relabelled as success.
    if session.status == "RUNNING_PSP9" {
        store.update_session_status(&session.session_id, "COMPLETED_PSP9")?;
        println!("Session marked COMPLETED_PSP9.");
    } else {
        println!("Preserved terminal status {}.", session.status);
    }
    Ok(())
}

fn has_candidate_checkpoint(store: &perspt_store::SessionStore, session_id: &str) -> Result<bool> {
    Ok(store
        .latest_psp9_checkpoint(session_id)?
        .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
        .is_some_and(|value| value.get("kind").and_then(|kind| kind.as_str()) == Some("candidate")))
}

/// Continue an interrupted model loop from its durable candidate checkpoint:
/// fresh transport from the effective config, fresh epoch-bound capability,
/// exactly the remaining budgets.
async fn resume_mid_loop(
    store: std::sync::Arc<perspt_store::SessionStore>,
    session: &perspt_store::SessionRecord,
) -> Result<()> {
    let config_path = perspt_core::paths::resolve_config_file();
    let config = match config_path {
        Some(path) => perspt_core::Config::load_from_path(&path)?,
        None => perspt_core::Config::default(),
    };
    let working_dir = PathBuf::from(&session.working_dir);
    anyhow::ensure!(
        working_dir.exists(),
        "working directory no longer exists: {}",
        session.working_dir
    );
    let runtime = perspt_agent::Psp9AgentRuntime::from_config(
        working_dir,
        &config,
        perspt_agent::Psp9ModelRoutes::default(),
        perspt_agent::Psp9RunConfig {
            approval_policy: perspt_sdk::ApprovalPolicy::Ask,
            ..perspt_agent::Psp9RunConfig::default()
        },
    )?
    .with_session_store(store);
    println!("Continuing the interrupted loop from its durable candidate checkpoint...");
    let summary = runtime.resume_session(session.session_id.clone()).await?;
    println!("Outcome: {:?}", summary.outcome);
    println!("Ledger head: {}", summary.ledger_head);
    if !summary.promoted_paths.is_empty() {
        println!("Promoted paths: {}", summary.promoted_paths.join(", "));
    }
    Ok(())
}

/// PSP-9 resolved decision 6: a persisted grant is *intent*, never a live
/// capability. Before resume completes any durable effect it verifies the
/// stored signature against the locally resolved trust anchor and checks the
/// grant's authority epoch against the durable epoch, so a revoked or
/// tampered grant invalidates the resume.
fn verify_persistent_grant(
    store: &perspt_store::SessionStore,
    session_id: &str,
    current_epoch: u64,
) -> Result<()> {
    let Some(policy_json) = store.get_grant_policy(session_id)? else {
        return Ok(());
    };
    let Ok(signed) = serde_json::from_str::<perspt_sdk::SignedGrantPolicy>(&policy_json) else {
        // Session-only (unsigned) grant intent: nothing durable to verify.
        return Ok(());
    };
    let key = perspt_agent::grant::GrantSigningKey::resolve()
        .context("resolving the grant signing key for resume verification")?;
    signed
        .verify_against(&key.public_key())
        .map_err(|error| anyhow::anyhow!("persisted grant failed verification: {error}"))?;
    anyhow::ensure!(
        signed.policy.authority_epoch == current_epoch,
        "persisted grant is bound to authority epoch {} but the durable epoch is {}; \
         the grant was revoked",
        signed.policy.authority_epoch,
        current_epoch
    );
    println!("Verified persisted grant intent (signed, epoch {current_epoch}).");
    Ok(())
}

fn verify_psp9_chain(store: &perspt_store::SessionStore, session_id: &str) -> Result<()> {
    let (ledger, tampered) = super::psp9_chain::rebuild_and_verify(store, session_id)?;
    anyhow::ensure!(
        !ledger.is_empty(),
        "PSP-9 session has no durable ledger events"
    );
    anyhow::ensure!(
        tampered.is_empty(),
        "PSP-9 ledger diverges from its stored hashes at sequences {tampered:?}"
    );
    Ok(())
}

struct PromotionRecovery {
    workspace: PathBuf,
    target: PathBuf,
    relative_path: PathBuf,
    relative: String,
    before_hash: Option<String>,
    after_hash: Option<String>,
    after_bytes: Option<Vec<u8>>,
}

fn prepare_promoted_file(
    store: &perspt_store::SessionStore,
    workspace: &std::path::Path,
    manifest: &serde_json::Value,
) -> Result<PromotionRecovery> {
    let relative = manifest
        .get("path")
        .and_then(serde_json::Value::as_str)
        .context("promotion file has no path")?;
    let relative_path = std::path::Path::new(relative);
    anyhow::ensure!(
        !relative_path.is_absolute()
            && !relative_path
                .components()
                .any(|component| component == std::path::Component::ParentDir),
        "promotion path escapes workspace: {relative:?}"
    );
    reject_symlink_ancestor(workspace, relative_path)?;
    let target = workspace.join(relative_path);
    let before = manifest
        .get("before_hash")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let after = manifest
        .get("after_hash")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let after_bytes = match after.as_deref() {
        Some(hash) => Some(
            store
                .get_psp9_artifact(hash)?
                .with_context(|| format!("missing promotion artifact {hash}"))?,
        ),
        None => None,
    };
    if let (Some(hash), Some(bytes)) = (after.as_deref(), after_bytes.as_deref()) {
        anyhow::ensure!(
            perspt_sdk::content_hash(bytes) == hash,
            "promotion artifact hash mismatch"
        );
    }
    Ok(PromotionRecovery {
        workspace: workspace.to_path_buf(),
        target,
        relative_path: relative_path.to_path_buf(),
        relative: relative.to_string(),
        before_hash: before,
        after_hash: after,
        after_bytes,
    })
}

fn recover_promoted_file(recovery: &PromotionRecovery) -> Result<()> {
    reject_symlink_ancestor(&recovery.workspace, &recovery.relative_path)?;
    let parent = recovery
        .target
        .parent()
        .context("promotion target has no parent")?;
    let current_bytes = match std::fs::read(&recovery.target) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(error).with_context(|| format!("reading {}", recovery.target.display()))
        }
    };
    let current = current_bytes.as_deref().map(perspt_sdk::content_hash);
    if current.as_deref() == recovery.after_hash.as_deref() {
        return Ok(());
    }
    anyhow::ensure!(
        current.as_deref() == recovery.before_hash.as_deref(),
        "workspace file changed after promotion intent: {}",
        recovery.relative
    );
    match recovery.after_bytes.as_deref() {
        Some(bytes) => {
            anyhow::ensure!(
                recovery.after_hash.as_deref() == Some(&perspt_sdk::content_hash(bytes)),
                "promotion artifact changed after preparation"
            );
            std::fs::create_dir_all(parent)?;
            let hash = recovery
                .after_hash
                .as_deref()
                .context("missing after hash")?;
            let staged = parent.join(format!(".perspt-resume-{}", &hash[..12.min(hash.len())]));
            std::fs::write(&staged, bytes)?;
            std::fs::rename(staged, &recovery.target)?;
        }
        None if recovery.target.is_file() => std::fs::remove_file(&recovery.target)?,
        None => {}
    }
    Ok(())
}

fn reject_symlink_ancestor(root: &std::path::Path, relative: &std::path::Path) -> Result<()> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        if let std::path::Component::Normal(part) = component {
            current.push(part);
            match std::fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    anyhow::bail!("promotion through a workspace symlink is forbidden")
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
    Ok(())
}
