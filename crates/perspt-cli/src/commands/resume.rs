//! Resume command - resume a paused or crashed session

use anyhow::{Context, Result};
use std::path::PathBuf;

/// Resume a paused or crashed session
pub async fn run(
    session_id: Option<String>,
    last: bool,
    db_path: Option<PathBuf>,
    config_override: Option<PathBuf>,
) -> Result<()> {
    let store = std::sync::Arc::new(super::psp9_chain::open_store(db_path.as_deref(), false)?);

    let target = if last {
        let sessions = store.list_recent_sessions(1)?;
        anyhow::ensure!(!sessions.is_empty(), "no sessions found to resume");
        Some(sessions[0].session_id.clone())
    } else {
        session_id
    };
    match target {
        Some(id) => resume_session(store, &id, config_override).await,
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
    config_override: Option<PathBuf>,
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

    anyhow::ensure!(
        session.status.ends_with("_PSP9"),
        "session {} belongs to the retired PSP-5 runtime; its tables remain \
         available as inert forensic data, but it cannot be resumed",
        session.session_id
    );
    resume_psp9(store, &session, config_override).await
}

/// PSP-9 resume verifies the ledger and completes bracketed promotion intents
/// from content-addressed artifacts. Mid-loop continuation reconstructs the
/// accepted candidate and provider-neutral conversation, then mints fresh
/// capabilities because live authority is intentionally never serialized.
async fn resume_psp9(
    store: std::sync::Arc<perspt_store::SessionStore>,
    session: &perspt_store::SessionRecord,
    config_override: Option<PathBuf>,
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
            return resume_mid_loop(store, session, config_override).await;
        }
        println!("This session is terminal; start a new agent session for additional work.");
        return Ok(());
    }
    let current_epoch = store.authority_epoch(&session.session_id)?;
    verify_persistent_grant(&store, &session.session_id, current_epoch)?;
    for effect in pending {
        // Only promotion intents are replayable file operations. An open
        // `tool:` bracket (e.g. an interrupted dependency mutation) is
        // evidence, not a recipe: report it and leave it open.
        if !effect.idempotency_key.starts_with("promote:") {
            println!(
                "Open external-effect bracket left unresolved: {} (inspect the ledger)",
                effect.idempotency_key
            );
            continue;
        }
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
    for row in store.get_psp9_events(session_id)? {
        let Ok(perspt_sdk::LedgerEvent::Custom { kind, payload }) =
            serde_json::from_str(&row.event_json)
        else {
            continue;
        };
        if kind != "tool_loop" {
            continue;
        }
        let body = payload.get("body").unwrap_or(&payload);
        if body.get("event").and_then(serde_json::Value::as_str)
            == Some("durable_candidate_checkpoint")
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Continue an interrupted model loop from its durable candidate checkpoint:
/// fresh transport from the effective config, fresh epoch-bound capability,
/// exactly the remaining budgets.
async fn resume_mid_loop(
    store: std::sync::Arc<perspt_store::SessionStore>,
    session: &perspt_store::SessionRecord,
    config_override: Option<PathBuf>,
) -> Result<()> {
    let config_path = config_override.or_else(perspt_core::paths::resolve_config_file);
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
    target: perspt_agent::promote::TargetDir,
    relative: String,
    before_hash: Option<String>,
    after_hash: Option<String>,
    after_bytes: Option<Vec<u8>>,
}

/// Resolve one manifest entry to a held parent-directory descriptor via the
/// shared hardened promotion engine (`perspt_agent::promote`), which refuses
/// workspace escapes and symlinked ancestors structurally.
fn prepare_promoted_file(
    store: &perspt_store::SessionStore,
    workspace: &std::path::Path,
    manifest: &serde_json::Value,
) -> Result<PromotionRecovery> {
    let relative = manifest
        .get("path")
        .and_then(serde_json::Value::as_str)
        .context("promotion file has no path")?;
    let root = perspt_agent::promote::WorkspaceRoot::open(workspace)?;
    let target = root.target_dir(relative, true)?;
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
        target,
        relative: relative.to_string(),
        before_hash: before,
        after_hash: after,
        after_bytes,
    })
}

fn recover_promoted_file(recovery: &PromotionRecovery) -> Result<()> {
    let current_bytes = recovery.target.read_optional()?;
    let current = current_bytes.as_deref().map(perspt_sdk::content_hash);
    if current.as_deref() == recovery.after_hash.as_deref() {
        return Ok(());
    }
    anyhow::ensure!(
        current.as_deref() == recovery.before_hash.as_deref(),
        "workspace file changed after promotion intent: {}",
        recovery.relative
    );
    if let Some(bytes) = recovery.after_bytes.as_deref() {
        anyhow::ensure!(
            recovery.after_hash.as_deref() == Some(&perspt_sdk::content_hash(bytes)),
            "promotion artifact changed after preparation"
        );
    }
    recovery.target.apply(recovery.after_bytes.as_deref())
}
