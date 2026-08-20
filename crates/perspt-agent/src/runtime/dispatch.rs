//! Bounded multi-node dispatch (PSP-9 system 15, Gate P).
//!
//! Borrowed structured concurrency: per-node lifecycle futures run on a
//! `FuturesUnordered` against `&self`, each holding a cancellation token —
//! dropping the future is the cancellation. The dispatcher owns the
//! scheduler (footprint conflicts and the slot bound) and a single
//! non-replenishing recovery pool shared by every node (Paper III's session
//! budget). The ledger sequencer is `Psp9Recorder::append`, which holds the
//! ledger mutex across stage → durable write → commit, so concurrent
//! appends interleave atomically on one hash chain. Promotion runs only in
//! the completion arm — single-flight by construction. Provider rate-limit
//! waits stay inside the transport and are never footprint conflicts.

use futures::stream::{FuturesUnordered, StreamExt};
use tokio_util::sync::CancellationToken;

use super::*;

/// One non-replenishing recovery pool for the whole session.
pub(super) struct SharedRecoveryPool {
    remaining: std::sync::Mutex<u32>,
}

impl SharedRecoveryPool {
    fn new(budget: u32) -> Self {
        Self {
            remaining: std::sync::Mutex::new(budget),
        }
    }

    /// Claim up to `want` gate decisions for one attempt.
    fn claim(&self, want: u32) -> u32 {
        let mut remaining = self.remaining.lock().unwrap();
        let granted = (*remaining).min(want);
        *remaining -= granted;
        granted
    }

    /// Return the unspent part of a claim. The pool never grows past what
    /// was claimed from it.
    fn refund(&self, unspent: u32) {
        *self.remaining.lock().unwrap() += unspent;
    }
}

/// A finished (or cancelled) node lifecycle future.
struct NodeDone {
    node_id: String,
    generation: u32,
    claimed: u32,
    outcome: Option<Result<NodeAttempt>>,
}

/// Aggregate result of a dispatched graph.
pub(super) struct DispatchOutcome {
    pub outcome: NodeTerminalOutcome,
    pub status: &'static str,
    pub promoted_paths: Vec<String>,
    pub turns_used: u32,
}

impl Psp9AgentRuntime {
    /// Run every node of `graph` under the slot bound, folding completions
    /// sequentially. Gate P: conflicting footprints never run concurrently
    /// (the scheduler refuses them a slot), commuting ones do.
    pub(super) async fn run_dispatched(
        &self,
        recorder: &Psp9Recorder,
        session_id: &str,
        mut graph: WorkGraphRevision,
    ) -> Result<DispatchOutcome> {
        let scheduler = tokio::sync::Mutex::new(Scheduler::new(self.config.max_parallel_nodes));
        let mut leases = perspt_sdk::LeaseTable::default();
        let pool = SharedRecoveryPool::new(self.config.rejection_budget);
        // PSP-10 system 22: node winners stage here; only a hard-passing
        // integration root reaches the user workspace.
        let mut staging = super::integrate::StagingRoot::default();
        let mut running: FuturesUnordered<futures::future::BoxFuture<'_, NodeDone>> =
            FuturesUnordered::new();
        let mut tokens: std::collections::BTreeMap<String, CancellationToken> = Default::default();
        let mut aggregate = DispatchOutcome {
            outcome: NodeTerminalOutcome::HardPass,
            status: "COMPLETED_PSP9",
            promoted_paths: Vec::new(),
            turns_used: 0,
        };

        loop {
            graph = self
                .dispatch_ready(
                    recorder,
                    session_id,
                    &graph,
                    &scheduler,
                    &pool,
                    &mut running,
                    &mut tokens,
                    &staging,
                )
                .await?;
            let Some(done) = running.next().await else {
                break;
            };
            let done: NodeDone = done;
            scheduler
                .lock()
                .await
                .finish(&done.node_id, done.generation);
            tokens.remove(&done.node_id);
            let node_id = done.node_id.clone();
            let Some(attempt) = settle_done(recorder, &pool, done)? else {
                continue;
            };
            graph = self
                .fold_completion(
                    recorder,
                    session_id,
                    &scheduler,
                    &mut leases,
                    &pool,
                    graph,
                    node_id,
                    attempt,
                    &mut aggregate,
                    &mut staging,
                )
                .await?;
            cancel_stale(&graph, &tokens);
        }
        self.integrate_staged(recorder, session_id, &graph, &staging, &mut aggregate)
            .await?;
        Ok(aggregate)
    }

    /// Gate AA: one global integration gate after every node settles. A
    /// sibling failure discards the staged winners whole — no partial
    /// promotion path exists.
    async fn integrate_staged(
        &self,
        recorder: &Psp9Recorder,
        session_id: &str,
        graph: &WorkGraphRevision,
        staging: &super::integrate::StagingRoot,
        aggregate: &mut DispatchOutcome,
    ) -> Result<()> {
        if staging.is_empty() {
            return Ok(());
        }
        if matches!(aggregate.outcome, NodeTerminalOutcome::HardPass) {
            match self
                .run_integration_gate(recorder, session_id, graph, staging)
                .await?
            {
                Some(paths) => aggregate.promoted_paths = paths,
                None => {
                    aggregate.outcome = NodeTerminalOutcome::Escalated {
                        certificate_id: uuid::Uuid::new_v4().to_string(),
                    };
                    aggregate.status = "ESCALATED_PSP9";
                }
            }
        } else {
            recorder.record_custom(
                "integration_failed",
                serde_json::json!({
                    "staging_root": staging.digest(),
                    "reason": "a sibling node failed; staged winners were discarded",
                }),
            )?;
        }
        Ok(())
    }

    /// Fill free slots with ready nodes; footprints and the slot bound come
    /// from the scheduler. Returns the revision with dispatched nodes
    /// marked running.
    #[allow(clippy::too_many_arguments)]
    async fn dispatch_ready<'a>(
        &'a self,
        recorder: &'a Psp9Recorder,
        session_id: &'a str,
        graph: &WorkGraphRevision,
        scheduler: &tokio::sync::Mutex<Scheduler>,
        pool: &SharedRecoveryPool,
        running: &mut FuturesUnordered<futures::future::BoxFuture<'a, NodeDone>>,
        tokens: &mut std::collections::BTreeMap<String, CancellationToken>,
        staging: &super::integrate::StagingRoot,
    ) -> Result<WorkGraphRevision> {
        let mut updated = graph.clone();
        loop {
            let Some(selected) = take_ready_slot(recorder, scheduler, &updated).await? else {
                break;
            };
            let claimed = self.claim_fair_share(recorder, pool, &selected)?;
            let token = CancellationToken::new();
            tokens.insert(selected.node_id.clone(), token.clone());
            updated = execution_revision(&updated, &selected.node_id, WorkNodeState::Running)?;
            recorder.record_custom("graph_revision", serde_json::to_value(&updated)?)?;
            let graph_snapshot = updated.clone();
            // PSP-10 system 22: downstream work builds on the latest
            // staging root, never on unstaged sibling state.
            let seed = self
                .staging_seed(&updated, staging, &selected.node_id)
                .await?;
            running.push(Box::pin(async move {
                let node_id = selected.node_id.clone();
                let generation = selected.generation;
                tokio::select! {
                    _ = token.cancelled() => NodeDone {
                        node_id,
                        generation,
                        claimed,
                        outcome: None,
                    },
                    attempt = self.attempt_node(
                        recorder,
                        session_id,
                        &selected.goal,
                        &selected.node_id,
                        selected.generation,
                        &self.model,
                        &graph_snapshot,
                        seed.as_ref(),
                        claimed,
                    ) => NodeDone {
                        node_id,
                        generation,
                        claimed,
                        outcome: Some(attempt),
                    },
                }
            }));
        }
        Ok(updated)
    }

    /// Sequential completion arm: recovery (if escalated), conclusion,
    /// promotion, and the terminal graph fold under a `GraphWrite` lease.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    async fn fold_completion(
        &self,
        recorder: &Psp9Recorder,
        session_id: &str,
        scheduler: &tokio::sync::Mutex<Scheduler>,
        leases: &mut perspt_sdk::LeaseTable,
        pool: &SharedRecoveryPool,
        graph: WorkGraphRevision,
        node_id: String,
        attempt: NodeAttempt,
        aggregate: &mut DispatchOutcome,
        staging: &mut super::integrate::StagingRoot,
    ) -> Result<WorkGraphRevision> {
        let (attempt, mut graph, _generation) = self
            .recover_if_escalated(
                recorder, session_id, scheduler, pool, graph, &node_id, attempt,
            )
            .await?;

        let task = graph
            .node(&node_id)
            .map(|node| node.goal.clone())
            .unwrap_or_default();
        // PSP-10 system 22: a hard-passing winner is STAGED, not promoted;
        // only the globally verified integration root reaches the user
        // workspace (Gate AA).
        let (final_outcome, status, promoted) = self
            .stage_or_conclude(recorder, &attempt, &node_id, &task, staging)
            .await?;

        // The fold itself is the first live GraphWrite lease site: a lost
        // lease would invalidate the turn's state witnesses.
        let lease = leases
            .acquire(
                "dispatcher",
                perspt_sdk::LeaseKind::GraphWrite,
                Resource::WorkGraph,
            )
            .context("graph-write lease unavailable")?;
        let terminal_state = if matches!(final_outcome, NodeTerminalOutcome::HardPass) {
            WorkNodeState::Stable
        } else {
            WorkNodeState::Stopped {
                certificate_id: uuid::Uuid::new_v4().to_string(),
            }
        };
        graph = execution_revision(&graph, &node_id, terminal_state)?;
        recorder.record_custom("graph_revision", serde_json::to_value(&graph)?)?;
        recorder.record_custom(
            "node_terminal",
            serde_json::json!({"node_id": node_id, "outcome": final_outcome}),
        )?;
        leases.release(&lease);

        aggregate.turns_used += attempt.outcome.turns_used;
        aggregate.promoted_paths.extend(promoted);
        if !matches!(final_outcome, NodeTerminalOutcome::HardPass) {
            aggregate.outcome = final_outcome;
            aggregate.status = status;
        }
        Ok(graph)
    }
}

impl Psp9AgentRuntime {
    /// Fair static split of the one shared pool: unspent claims are
    /// refunded at completion for later nodes. The claim is ledgered.
    fn claim_fair_share(
        &self,
        recorder: &Psp9Recorder,
        pool: &SharedRecoveryPool,
        selected: &WorkNode,
    ) -> Result<u32> {
        let fair_share =
            (self.config.rejection_budget / self.config.max_parallel_nodes.max(1) as u32).max(1);
        let claimed = pool.claim(fair_share);
        recorder.record_custom(
            "recovery_pool_claim",
            serde_json::json!({
                "node_id": selected.node_id,
                "claimed": claimed,
            }),
        )?;
        Ok(claimed)
    }

    /// Run the recovery ladder for an escalated node. It holds the
    /// scheduler lock: new dispatch pauses while the ladder revises;
    /// running nodes are unaffected.
    #[allow(clippy::too_many_arguments)]
    async fn recover_if_escalated(
        &self,
        recorder: &Psp9Recorder,
        session_id: &str,
        scheduler: &tokio::sync::Mutex<Scheduler>,
        pool: &SharedRecoveryPool,
        graph: WorkGraphRevision,
        node_id: &str,
        attempt: NodeAttempt,
    ) -> Result<(NodeAttempt, WorkGraphRevision, u32)> {
        if !matches!(
            attempt.outcome.outcome,
            NodeTerminalOutcome::Escalated { .. }
        ) {
            return Ok((attempt, graph, 0));
        }
        let claimed = pool.claim(self.config.rejection_budget);
        let mut guard = scheduler.lock().await;
        let session = LadderSession {
            recorder,
            session_id,
            node_id,
            scheduler: &mut guard,
        };
        let goal = graph
            .node(node_id)
            .map(|node| node.goal.clone())
            .unwrap_or_default();
        let result = self
            .recovery_ladder(session, claimed, graph, goal, attempt)
            .await?;
        pool.refund(claimed.saturating_sub(spent_of(&result.0)));
        Ok(result)
    }
}

impl Psp9AgentRuntime {
    /// Stage a validated hard-pass winner into the content-addressed
    /// staging root; every other outcome keeps the ordinary conclusion.
    async fn stage_or_conclude(
        &self,
        recorder: &Psp9Recorder,
        attempt: &NodeAttempt,
        node_id: &str,
        task: &str,
        staging: &mut super::integrate::StagingRoot,
    ) -> Result<(NodeTerminalOutcome, &'static str, Vec<String>)> {
        let hard = matches!(attempt.outcome.outcome, NodeTerminalOutcome::HardPass);
        if !hard {
            return self
                .conclude_attempt(recorder, attempt, node_id, task)
                .await;
        }
        let approved = self
            .validate_and_approve_attempt(recorder, attempt, node_id, task)
            .await?;
        if !approved {
            return Ok((
                NodeTerminalOutcome::Escalated {
                    certificate_id: uuid::Uuid::new_v4().to_string(),
                },
                "ESCALATED_PSP9",
                Vec::new(),
            ));
        }
        let files = attempt.candidate.export_accepted().await?;
        let state_root = attempt.candidate.checkpoint(&[]).await?.witness.state_root;
        let paths: Vec<String> = files.iter().map(|file| file.path.clone()).collect();
        staging.contributions.insert(
            node_id.to_string(),
            super::integrate::StagedWinner { state_root, files },
        );
        recorder.record_custom(
            "staging_root_updated",
            serde_json::json!({
                "node_id": node_id,
                "staging_root": staging.digest(),
                "contributions": staging.contributions.len(),
                "paths": paths,
            }),
        )?;
        Ok((NodeTerminalOutcome::HardPass, "COMPLETED_PSP9", Vec::new()))
    }
}

/// Settle one finished lifecycle future: refund a cancelled node's claim,
/// surface attempt errors, and refund the unspent share of a completed
/// attempt. `None` means there is nothing to fold.
fn settle_done(
    recorder: &Psp9Recorder,
    pool: &SharedRecoveryPool,
    done: NodeDone,
) -> Result<Option<NodeAttempt>> {
    let Some(outcome) = done.outcome else {
        recorder.record_custom(
            "node_revalidation_cancelled",
            serde_json::json!({
                "node_id": done.node_id,
                "generation": done.generation,
            }),
        )?;
        pool.refund(done.claimed);
        return Ok(None);
    };
    let attempt = outcome?;
    pool.refund(done.claimed.saturating_sub(spent_of(&attempt)));
    Ok(Some(attempt))
}

/// Take one ready node under the slot bound, ledger the dispatch, and mark
/// it running in the scheduler.
async fn take_ready_slot(
    recorder: &Psp9Recorder,
    scheduler: &tokio::sync::Mutex<Scheduler>,
    graph: &WorkGraphRevision,
) -> Result<Option<WorkNode>> {
    let mut guard = scheduler.lock().await;
    let next = guard
        .ready_nodes(graph, node_footprint)
        .first()
        .map(|node| (*node).clone());
    let Some(node) = next else {
        return Ok(None);
    };
    guard.start(&node, node_footprint(&node));
    let slot = guard.running_count().saturating_sub(1);
    recorder.record_custom(
        "scheduler_dispatch",
        serde_json::json!({
            "node_id": node.node_id,
            "generation": node.generation,
            "parallel_slot": slot,
        }),
    )?;
    Ok(Some(node))
}

/// Cancel any running node whose generation no longer matches the graph —
/// a superseding revision invalidates its state witnesses.
fn cancel_stale(
    graph: &WorkGraphRevision,
    tokens: &std::collections::BTreeMap<String, CancellationToken>,
) {
    for (node_id, token) in tokens {
        let current = graph.node(node_id);
        let stale = current.is_none()
            || current.is_some_and(|node| !matches!(node.state, WorkNodeState::Running));
        if stale {
            token.cancel();
        }
    }
}
