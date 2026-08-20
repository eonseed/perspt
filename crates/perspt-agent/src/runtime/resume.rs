//! Session resume (PSP-9 resolved decision 6; PSP-10 Gate AA).
//!
//! A single-node session continues its interrupted loop from the newest
//! durable candidate checkpoint with exactly the remaining budgets. A
//! multi-node session resumes through graph dispatch: the staging root is
//! reconstructed from the ledger and every winner reaches the user
//! workspace only through the global integration gate — never through
//! single-node direct promotion.

use super::*;

impl Psp9AgentRuntime {
    /// Continue an interrupted model loop from its newest durable candidate
    /// checkpoint (PSP-9 resolved decision 6). Nothing live is deserialized:
    /// the accepted candidate is rebuilt from content-addressed artifacts, a
    /// *fresh* capability is minted from the grant intersection at the
    /// current durable authority epoch, and the loop re-enters with exactly
    /// the checkpoint's remaining budgets. A bumped epoch refuses the resume.
    pub async fn resume_session(mut self, session_id: String) -> Result<Psp9RunSummary> {
        let recorder = Psp9Recorder::attach(
            &session_id,
            self.database_path.as_deref(),
            self.shared_store.clone(),
            self.event_sender.clone(),
        )?;
        let (control, seed) = load_candidate_checkpoint(&recorder, &session_id)?;

        // The checkpoint's real node identity (legacy checkpoints predate
        // the field and keep the historical single-node name).
        let node_id = if control.node_id.is_empty() {
            "implement-1".to_string()
        } else {
            control.node_id.clone()
        };
        self.seed_search_state(&recorder, &session_id, &control)?;
        let task = control.goal.clone();
        let running_graph = resumed_running_graph(
            &recorder,
            &control.graph_revision,
            &node_id,
            control.node_generation,
        )?;
        // Gate AA: an interrupted multi-node graph never resumes into
        // single-node direct promotion. Route state carries over; per-node
        // loop budgets stay the configured ones — a graph resume
        // re-dispatches nodes as fresh attempts (the ledger fold still
        // preserves search usage and no-goods).
        if running_graph.nodes.len() > 1 {
            self.adopt_route_state(&control)?;
            return self
                .resume_graph_session(
                    &recorder,
                    &session_id,
                    running_graph,
                    &node_id,
                    control.node_generation,
                    seed,
                )
                .await;
        }
        // Single-node continuation: adopt the checkpoint's exact remaining
        // budgets — a resumed loop never refills what it already spent.
        self.adopt_checkpoint(&recorder, &control, &seed)?;
        self.resume_single_node(
            &recorder,
            session_id,
            &task,
            &node_id,
            &control,
            seed,
            running_graph,
        )
        .await
    }

    /// The single-node continuation: re-enter the interrupted loop with
    /// the restored seed, conclude, and promote through the ordinary gate.
    #[allow(clippy::too_many_arguments)]
    async fn resume_single_node(
        &self,
        recorder: &Psp9Recorder,
        session_id: String,
        task: &str,
        node_id: &str,
        control: &perspt_sdk::ControlFrame,
        seed: CandidateSeed,
        running_graph: WorkGraphRevision,
    ) -> Result<Psp9RunSummary> {
        let attempt = match self
            .attempt_node(
                recorder,
                &session_id,
                task,
                node_id,
                control.node_generation,
                &self.model.clone(),
                &running_graph,
                Some(&seed),
                self.config.rejection_budget,
            )
            .await
        {
            Ok(attempt) => attempt,
            Err(error) => return Err(self.fail_session(recorder, error)),
        };
        let verdict = match self
            .conclude_attempt(recorder, &attempt, node_id, task)
            .await
        {
            Ok(verdict) => verdict,
            Err(error) => return Err(self.fail_session(recorder, error)),
        };
        let (final_outcome, status, promoted_paths) = verdict;
        if let Err(error) =
            self.finish_node(recorder, &running_graph, node_id, &final_outcome, status)
        {
            return Err(self.fail_session(recorder, error));
        }
        Ok(Psp9RunSummary {
            session_id,
            node_id: node_id.to_string(),
            outcome: final_outcome,
            turns_used: attempt.outcome.turns_used,
            ledger_head: recorder.head(),
            promoted_paths,
        })
    }

    /// Adopt only the checkpoint's route state (graph resume): the model
    /// and failover chain carry over, per-node loop budgets do not.
    fn adopt_route_state(&mut self, control: &perspt_sdk::ControlFrame) -> Result<()> {
        self.model = control.active_model.clone();
        self.fallback_models = control.remaining_fallback_models.clone();
        anyhow::ensure!(
            self.transport.capabilities(&self.model).tool_calling,
            "checkpoint model route {} no longer supports native tool calling",
            self.model
        );
        Ok(())
    }

    /// The resumed graph revision: staged nodes are stable, everything
    /// else resets to pending at its recorded generation.
    fn resumed_revision(
        graph: &WorkGraphRevision,
        staging: &integrate::StagingRoot,
    ) -> Result<WorkGraphRevision> {
        let mut nodes = graph.nodes.clone();
        for node in &mut nodes {
            node.state = if staging.contributions.contains_key(&node.node_id) {
                WorkNodeState::Stable
            } else {
                WorkNodeState::Pending
            };
        }
        WorkGraphRevision::build(
            graph.sequence + 1,
            Some(graph.revision_id.clone()),
            perspt_sdk::GraphRevisionReason::ExecutionUpdate,
            nodes,
            graph.edges.clone(),
        )
        .map_err(|error| anyhow::anyhow!(error.to_string()))
    }

    /// The newest recorded state of the graph: the last `graph_revision`
    /// event (in ledger order) inside the descendant closure of `base`.
    /// The checkpoint binds the resumed node to its own revision, but a
    /// sibling's terminal fold — recorded in a *later* revision — must
    /// still be seen: judging terminality against the checkpoint-era
    /// snapshot would silently retry a node erratum 12 requires refusing.
    fn latest_graph_state(
        recorder: &Psp9Recorder,
        session_id: &str,
        base: &WorkGraphRevision,
    ) -> Result<WorkGraphRevision> {
        let mut closure: std::collections::BTreeSet<String> =
            [base.revision_id.clone()].into_iter().collect();
        let mut latest = base.clone();
        for row in recorder.store.get_psp9_events(session_id)? {
            let Ok(perspt_sdk::LedgerEvent::Custom { kind, payload }) =
                serde_json::from_str(&row.event_json)
            else {
                continue;
            };
            if kind != "graph_revision" {
                continue;
            }
            let graph: WorkGraphRevision = serde_json::from_value(payload)?;
            let descends = graph
                .parent_revision_id
                .as_deref()
                .is_some_and(|parent| closure.contains(parent));
            if descends && closure.insert(graph.revision_id.clone()) {
                latest = graph;
            }
        }
        Ok(latest)
    }

    /// Resume an interrupted multi-node graph (Gate AA): rebuild the
    /// staging root from the durable ledger, mark staged nodes stable,
    /// reset the rest to pending, and re-enter ordinary dispatch — the
    /// interrupted node continues from its candidate checkpoint when it
    /// inherits nothing, and every winner reaches the user workspace only
    /// through the global integration gate. Sibling terminality and
    /// staging are judged against the **latest** recorded descendant of
    /// the checkpoint's revision: a graph with a terminally failed node
    /// refuses resume — its staged winners were already discarded by the
    /// sibling-failure rule.
    async fn resume_graph_session(
        &self,
        recorder: &Psp9Recorder,
        session_id: &str,
        graph: WorkGraphRevision,
        resumed_node: &str,
        resumed_generation: u32,
        seed: node::CandidateSeed,
    ) -> Result<Psp9RunSummary> {
        let latest = Self::latest_graph_state(recorder, session_id, &graph)?;
        if let Some(failed) = latest.nodes.iter().find(|node| {
            !matches!(
                node.state,
                WorkNodeState::Pending
                    | WorkNodeState::Ready
                    | WorkNodeState::Running
                    | WorkNodeState::Stable
            )
        }) {
            recorder.finish("FAILED_PSP9").ok();
            anyhow::bail!(
                "node {} is terminal ({:?}); an interrupted graph with a failed \
                 node cannot be resumed — its staged winners were discarded",
                failed.node_id,
                failed.state
            );
        }
        let staging = integrate::fold_staging(recorder, session_id, &latest)?;
        let resumed = Self::resumed_revision(&latest, &staging)?;
        recorder.record_custom("graph_revision", serde_json::to_value(&resumed)?)?;
        recorder.record_custom(
            "graph_resumed",
            serde_json::json!({
                "resumed_node": resumed_node,
                "staged": staging.contributions.keys().collect::<Vec<_>>(),
                "staging_root": staging.digest(),
                "latest_revision": latest.revision_id,
            }),
        )?;
        let mut resume_seeds = std::collections::BTreeMap::new();
        // The mid-loop checkpoint continues only a node the latest state
        // still shows unchanged (same generation, not yet staged): a node
        // refined or folded after the checkpoint re-runs fresh.
        let unchanged = latest
            .node(resumed_node)
            .is_some_and(|node| node.generation == resumed_generation);
        if unchanged && !staging.contributions.contains_key(resumed_node) {
            resume_seeds.insert(resumed_node.to_string(), seed);
        }
        let dispatched = match self
            .run_dispatched_with(recorder, session_id, resumed, staging, resume_seeds)
            .await
        {
            Ok(dispatched) => dispatched,
            Err(error) => {
                recorder.finish("FAILED_PSP9").ok();
                return Err(error);
            }
        };
        recorder.finish(dispatched.status)?;
        self.emit(perspt_core::AgentEvent::Complete {
            success: matches!(dispatched.outcome, NodeTerminalOutcome::HardPass),
            message: format!("PSP-9 outcome: {:?}", dispatched.outcome),
        });
        Ok(Psp9RunSummary {
            session_id: session_id.to_string(),
            node_id: "graph".into(),
            outcome: dispatched.outcome,
            turns_used: dispatched.turns_used,
            ledger_head: recorder.head(),
            promoted_paths: dispatched.promoted_paths,
        })
    }

    /// Adopt a durable checkpoint's exact remaining budgets and route state.
    /// A resume never refills what the interrupted loop already spent.
    fn adopt_checkpoint(
        &mut self,
        recorder: &Psp9Recorder,
        control: &perspt_sdk::ControlFrame,
        seed: &CandidateSeed,
    ) -> Result<()> {
        anyhow::ensure!(
            control.remaining_turns > 0,
            "the durable checkpoint has no model-turn budget remaining; resume cannot refill it"
        );
        self.config.max_turns = control.remaining_turns;
        self.config.rejection_budget = control.remaining_rejection_budget;
        self.model = control.active_model.clone();
        self.fallback_models = control.remaining_fallback_models.clone();
        anyhow::ensure!(
            self.transport.capabilities(&self.model).tool_calling,
            "checkpoint model route {} no longer supports native tool calling",
            self.model
        );
        recorder.record_custom(
            "session_resumed",
            serde_json::json!({
                "accepted_state_root": control.accepted_state_root,
                "remaining_turns": self.config.max_turns,
                "remaining_rejection_budget": self.config.rejection_budget,
                "seed_files": seed.files.len(),
                "model": self.model,
                "remaining_fallback_models": self.fallback_models,
                "activated_tools": control.activated_tools,
            }),
        )?;
        Ok(())
    }

    /// Fold the recorded search state: exact no-goods stay suppressed and
    /// an interrupted forest's consumption is not silently refilled
    /// (spec :2340-2341, by ledger fold).
    fn seed_search_state(
        &self,
        recorder: &Psp9Recorder,
        session_id: &str,
        control: &perspt_sdk::ControlFrame,
    ) -> Result<()> {
        let fold = search::fold::fold_search_ledger(&recorder.store.get_psp9_events(session_id)?)?;
        let prior = fold
            .interrupted
            .filter(|forest| forest.accepted_root == control.accepted_state_root);
        *self.search_seed.lock().unwrap() = Some(search::SearchSeed {
            no_goods: fold.no_goods,
            prior_usage: prior.as_ref().map(|forest| forest.last_usage.clone()),
            resumed_from: prior.map(|forest| forest.forest_id),
        });
        Ok(())
    }
}
