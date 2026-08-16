use super::*;

impl SRBNOrchestrator {
    /// Run the complete SRBN control loop
    pub async fn run(&mut self, task: String) -> Result<()> {
        log::info!("Starting SRBN execution for task: {}", task);
        self.emit_log(format!("🚀 Starting task: {}", task));

        // Step 0: Start session first
        let session_id = uuid::Uuid::new_v4().to_string();
        self.context.session_id = session_id.clone();
        self.ledger.start_session(
            &session_id,
            &task,
            &self.context.working_dir.to_string_lossy(),
        )?;

        // Run orchestration and always finalize the session
        let result = self.run_orchestration(task).await;
        self.finalize_session(&result);
        result.map(|_| ())
    }

    /// Inner orchestration logic — called by `run()` which handles session lifecycle.
    pub(crate) async fn run_orchestration(
        &mut self,
        task: String,
    ) -> Result<perspt_core::SessionOutcome> {
        if self.context.log_llm {
            self.emit_log("📝 LLM request logging enabled".to_string());
        }

        // PSP-5: Detect execution mode (Project is default, Solo only on explicit keywords)
        let execution_mode = self.detect_execution_mode(&task);
        self.context.execution_mode = execution_mode;
        self.emit_log(format!("🎯 Execution mode: {}", execution_mode));

        if execution_mode == perspt_core::types::ExecutionMode::Solo {
            // Solo Mode: Single-file execution without DAG
            log::info!("Using Solo Mode for explicit single-file task");
            self.emit_log("⚡ Solo Mode: Single-file execution".to_string());
            return self
                .run_solo_mode(task)
                .await
                .map(|()| perspt_core::SessionOutcome::Success);
        }

        // PSP-5: Classify workspace state before deciding plugin/init strategy
        let workspace_state = self.classify_workspace(&task);
        self.context.workspace_state = workspace_state.clone();
        self.emit_log(format!("📋 Workspace: {}", workspace_state));

        // For existing projects, detect plugins and probe verifier readiness now.
        // For greenfield/ambiguous, defer until after step_init_project().
        if let WorkspaceState::ExistingProject { ref plugins } = workspace_state {
            self.context.active_plugins = plugins.clone();
            self.emit_log(format!("🔌 Detected plugins: {}", plugins.join(", ")));
            self.emit_plugin_readiness();
        }

        // Team Mode: Full project initialization and DAG sheafification
        self.step_init_project(&task).await?;

        // PSP-5: For greenfield/ambiguous workspaces, re-detect plugins after init
        // and probe verifier readiness against the newly initialized project.
        if !matches!(workspace_state, WorkspaceState::ExistingProject { .. }) {
            self.redetect_plugins_after_init();
        }

        // Gate: verify at least one plugin has build capability before planning.
        // Without this, the architect may produce a plan whose verification is
        // fully degraded, leading to false stability.
        self.check_verifier_readiness_gate();

        // Start LSP for detected plugins (after classification + init so we
        // use the authoritative plugin set, not a provisional one).
        {
            let plugin_refs: Vec<String> = self.context.active_plugins.clone();
            let refs: Vec<&str> = plugin_refs.iter().map(|s| s.as_str()).collect();
            if !refs.is_empty() {
                self.emit_log("🔍 Starting language servers...".to_string());
                if let Err(e) = self.start_lsp_for_plugins(&refs).await {
                    log::warn!("Failed to start LSP: {}", e);
                    self.emit_log("⚠️ Continuing without LSP".to_string());
                } else {
                    self.emit_log("✅ Language servers ready".to_string());
                }
            }
        }

        // Select planning policy based on workspace state before architect runs.
        // Greenfield workspaces use GreenfieldBuild; existing projects
        // default to FeatureIncrement (callers may override via set_planning_policy).
        if self.planning_policy == perspt_core::PlanningPolicy::default() {
            self.planning_policy = match &self.context.workspace_state {
                WorkspaceState::Greenfield { .. } => perspt_core::PlanningPolicy::GreenfieldBuild,
                WorkspaceState::ExistingProject { .. } => {
                    perspt_core::PlanningPolicy::FeatureIncrement
                }
                WorkspaceState::Ambiguous => perspt_core::PlanningPolicy::FeatureIncrement,
            };
        }

        // PSP-5 Phase 12: Create a default FeatureCharter so the
        // file-budget gate in step_sheafify has bounds to enforce.
        // Derive sensible defaults from the planning policy.
        if self.ledger.get_feature_charter().ok().flatten().is_none() {
            let mut charter = perspt_core::FeatureCharter::new(&self.context.session_id, &task);
            match self.planning_policy {
                perspt_core::PlanningPolicy::LocalEdit => {
                    charter.max_modules = Some(1);
                    charter.max_files = Some(5);
                    charter.max_revisions = Some(3);
                }
                perspt_core::PlanningPolicy::FeatureIncrement => {
                    charter.max_modules = Some(10);
                    charter.max_files = Some(30);
                    charter.max_revisions = Some(5);
                }
                perspt_core::PlanningPolicy::LargeFeature
                | perspt_core::PlanningPolicy::GreenfieldBuild
                | perspt_core::PlanningPolicy::ArchitecturalRevision => {
                    charter.max_modules = Some(25);
                    charter.max_files = Some(80);
                    charter.max_revisions = Some(10);
                }
            }
            if let Some(ref lang) = self.context.active_plugins.first() {
                charter.language_constraint = Some(lang.to_string());
            }
            if let Err(e) = self.ledger.record_feature_charter(&charter) {
                log::warn!("Failed to persist default FeatureCharter: {}", e);
            } else {
                log::info!(
                    "Registered default FeatureCharter (max_modules={:?}, max_files={:?})",
                    charter.max_modules,
                    charter.max_files
                );
            }
        }

        // Gate architect planning on policy: LocalEdit skips the architect
        // and creates a single-node deterministic graph directly.
        if self.planning_policy.needs_architect() {
            self.step_sheafify(task.clone()).await?;
        } else {
            self.emit_log("📐 LocalEdit policy — skipping architect, single-node plan".to_string());
            self.create_deterministic_fallback_graph(&task)?;
        }

        // Planning policy is already resolved above; log it after sheafification.
        self.emit_log(format!("📐 Planning policy: {:?}", self.planning_policy));

        // PSP-5: Emit PlanReady event after sheafification
        let node_count = self.graph.node_count();
        self.emit_event(perspt_core::AgentEvent::PlanReady {
            nodes: node_count,
            plugins: self.context.active_plugins.clone(),
            execution_mode: execution_mode.to_string(),
        });

        // Emit task nodes to TUI after sheafification
        for node_id in self.node_indices.keys() {
            if let Some(idx) = self.node_indices.get(node_id) {
                if let Some(node) = self.graph.node_weight(*idx) {
                    self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                        node_id: node.node_id.clone(),
                        status: perspt_core::NodeStatus::Pending,
                    });
                }
            }
        }

        // Step 2-7: Closed-loop ("fly-by-wire") execution.
        //
        // Instead of walking a frozen topological snapshot once, re-evaluate the
        // mutable work graph each round and run the next *ready* node. This lets
        // repair/rewrite actions (Retry, split, interface, replan) and goal-driven
        // re-plan amendments actually take effect: a reworked node is re-picked,
        // and newly inserted nodes are executed. When the ready set empties the
        // loop "settles" and the goal-completion gate decides whether the user's
        // intent is met, the plan should be amended, or the loop should stop.
        let mut completed_count: usize = 0;
        let mut escalated_count: usize = 0;
        let mut goal_achieved = false;

        // Infinite-loop backstop: bound total rounds by plan size and the allowed
        // number of re-plan revisions. The per-step budget is the primary bound;
        // this is a hard ceiling so the controller can never spin forever.
        let max_revisions = self
            .ledger
            .get_feature_charter()
            .ok()
            .flatten()
            .and_then(|c| c.max_revisions)
            .unwrap_or(5) as usize;
        let max_rounds = (self.graph.node_count() + 1) * (max_revisions + 2) + 16;
        let mut replan_state = ReplanState::new(max_revisions);

        // Per-node rework guard: a node that keeps reworking without reaching a
        // terminal state is force-escalated so it cannot stall the loop.
        const MAX_REWORKS_PER_NODE: usize = 6;
        let mut rework_counts: HashMap<NodeIndex, usize> = HashMap::new();

        let mut round: usize = 0;
        loop {
            round += 1;
            if round > max_rounds {
                log::warn!("Control loop reached round cap ({max_rounds}) — stopping");
                self.emit_log(format!(
                    "⛔ Control loop reached round cap ({max_rounds}) — stopping"
                ));
                break;
            }

            // Abort gate.
            if self.is_abort_requested() {
                self.emit_log("⚠️ Session aborted — stopping execution".to_string());
                break;
            }
            // Budget gate (steps / cost / revisions).
            if self.budget.any_exhausted() {
                self.emit_log("⛔ Budget exhausted — stopping execution".to_string());
                break;
            }

            // Pick the next runnable node (deps satisfied, not seal-blocked).
            let idx = match self.next_ready_node() {
                Some(idx) => idx,
                None => {
                    // Settle: nothing runnable. Decide whether the goal is met,
                    // the plan should be amended, or we stop.
                    match self
                        .evaluate_goal_completion(&task, &mut replan_state)
                        .await
                    {
                        SettleDecision::Achieved => {
                            goal_achieved = true;
                            break;
                        }
                        SettleDecision::Replanned => {
                            // Amendment added new ready work — keep looping.
                            continue;
                        }
                        SettleDecision::Stop => break,
                    }
                }
            };

            // Emit selection events. Progress is over the *current* node count,
            // which can grow as the plan is amended.
            let total_nodes = self.graph.node_count();
            if let Some(node) = self.graph.node_weight(idx) {
                self.emit_log(format!(
                    "📝 [{}/{}] {}",
                    completed_count + 1,
                    total_nodes,
                    node.goal
                ));
                self.emit_event(perspt_core::AgentEvent::NodeSelected {
                    node_id: node.node_id.clone(),
                    goal: node.goal.clone(),
                    node_class: node.node_class.to_string(),
                });
                self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                    node_id: node.node_id.clone(),
                    status: perspt_core::NodeStatus::Running,
                });
            }

            let outcome = self.execute_node(idx).await;
            // Every dispatch consumes a step, so the per-step budget bounds the
            // closed loop regardless of how many reworks occur.
            self.budget.record_step();
            self.emit_event(perspt_core::AgentEvent::BudgetUpdated {
                steps_used: self.budget.steps_used,
                max_steps: self.budget.max_steps,
                cost_used_usd: self.budget.cost_used_usd,
                max_cost_usd: self.budget.max_cost_usd,
                revisions_used: self.budget.revisions_used,
                max_revisions: self.budget.max_revisions,
            });
            if let Err(e) = self.ledger.upsert_budget_envelope(&self.budget) {
                log::warn!("Failed to persist budget envelope: {}", e);
            }

            match outcome {
                Ok(NodeOutcome::Completed) => {
                    completed_count += 1;
                    if let Some(node) = self.graph.node_weight(idx) {
                        self.emit_event(perspt_core::AgentEvent::NodeCompleted {
                            node_id: node.node_id.clone(),
                            goal: node.goal.clone(),
                        });
                    }
                }
                Ok(NodeOutcome::Reworked) => {
                    // A repair was applied; the node is back in Retry (or replaced
                    // by inserted nodes). Bound per-node reworks so a node that
                    // never converges is force-escalated instead of stalling.
                    let count = rework_counts.entry(idx).or_insert(0);
                    *count += 1;
                    if *count > MAX_REWORKS_PER_NODE {
                        let node_id = self.graph[idx].node_id.clone();
                        log::warn!(
                            "Node {node_id} exceeded rework limit ({MAX_REWORKS_PER_NODE}) — escalating"
                        );
                        self.emit_log(format!(
                            "⛔ Node {node_id} exceeded rework limit — escalating"
                        ));
                        self.graph[idx].state = NodeState::Escalated;
                        escalated_count += 1;
                        self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                            node_id,
                            status: perspt_core::NodeStatus::Escalated,
                        });
                    }
                    // Otherwise leave the node in its repair-assigned state so the
                    // next round re-picks it (or runs the newly inserted nodes).
                }
                Ok(NodeOutcome::Escalated) => {
                    escalated_count += 1;
                    if let Some(node) = self.graph.node_weight(idx) {
                        self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                            node_id: node.node_id.clone(),
                            status: perspt_core::NodeStatus::Escalated,
                        });
                    }
                }
                Err(e) => {
                    escalated_count += 1;
                    let node_id = self.graph[idx].node_id.clone();
                    eprintln!("[SRBN-DIAG] Node {} failed: {:#}", node_id, e);
                    log::error!("Node {} failed: {}", node_id, e);
                    self.emit_log(format!("❌ Node {} failed: {}", node_id, e));

                    // Flush the node's provisional branch so sandbox files don't leak.
                    if let Some(bid) = self.graph[idx].provisional_branch_id.clone() {
                        self.flush_provisional_branch(&bid, &node_id);
                    }
                    self.flush_descendant_branches(idx);

                    self.graph[idx].state = NodeState::Escalated;
                    self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                        node_id: node_id.clone(),
                        status: perspt_core::NodeStatus::Escalated,
                    });
                }
            }
        }

        let total_nodes = self.graph.node_count();
        log::info!("SRBN execution completed");

        // PSP-5 Phase 6: Clean up all session sandboxes
        if let Err(e) = crate::tools::cleanup_session_sandboxes(
            &self.context.working_dir,
            &self.context.session_id,
        ) {
            log::warn!("Failed to clean up session sandboxes: {}", e);
        }

        // Derive session outcome. Success requires either an explicit goal
        // verdict (auto mode) or — when the goal gate did not run — every node
        // completing with no escalations.
        let all_completed_no_escalation =
            escalated_count == 0 && completed_count >= total_nodes && total_nodes > 0;
        let outcome = if goal_achieved || all_completed_no_escalation {
            perspt_core::SessionOutcome::Success
        } else if completed_count > 0 {
            perspt_core::SessionOutcome::PartialSuccess
        } else {
            perspt_core::SessionOutcome::Failed
        };
        self.emit_event(perspt_core::AgentEvent::Complete {
            success: outcome == perspt_core::SessionOutcome::Success,
            message: format!(
                "{}/{} nodes completed, {} escalated",
                completed_count, total_nodes, escalated_count
            ),
        });
        Ok(outcome)
    }

    /// Pick the next runnable node for the closed control loop.
    ///
    /// A node is *ready* when its state is `TaskQueued` or `Retry`, every
    /// dependency parent has `Completed`, and it is not blocked on an interface
    /// seal prerequisite. Candidates are returned in ascending `NodeIndex`
    /// order, which preserves the original topological order for the common
    /// linear DAG while also re-picking reworked nodes and newly inserted ones.
    pub(crate) fn next_ready_node(&mut self) -> Option<NodeIndex> {
        let mut candidates: Vec<NodeIndex> = self
            .graph
            .node_indices()
            .filter(|&idx| {
                matches!(
                    self.graph[idx].state,
                    NodeState::TaskQueued | NodeState::Retry
                )
            })
            .filter(|&idx| {
                self.graph
                    .neighbors_directed(idx, petgraph::Direction::Incoming)
                    .all(|parent| self.graph[parent].state == NodeState::Completed)
            })
            .collect();
        candidates.sort();

        for idx in candidates {
            // check_seal_prerequisites takes &mut self (it may emit events), so
            // evaluate candidates one at a time after the immutable scan.
            if self.check_seal_prerequisites(idx) {
                continue;
            }
            return Some(idx);
        }
        None
    }

    /// Decide what to do when the control loop settles (no runnable node).
    ///
    /// Phase A provides the deterministic settle: the goal is "achieved" when
    /// every node completed with no escalations. Phase B extends this into the
    /// full hybrid gate (verifier LLM verdict + architect re-plan amendment).
    /// Hybrid goal-completion gate (PSP-8 closed loop).
    ///
    /// Runs when the work graph settles. In auto mode: a cheap deterministic
    /// pre-gate (all nodes completed, none escalated) must pass before a
    /// verifier-tier LLM is asked for a structured `GoalVerdict`; an unmet goal
    /// triggers a bounded architect re-plan amendment. In interactive mode it
    /// never auto-amends — achieved iff the deterministic gate passes.
    pub(crate) async fn evaluate_goal_completion(
        &mut self,
        task: &str,
        state: &mut ReplanState,
    ) -> SettleDecision {
        let completed = self.count_in_state(NodeState::Completed);
        let escalated = self.count_in_state(NodeState::Escalated);
        let total = self.graph.node_count();
        let phi = self.workflow_phi();

        // Telemetry: Φ is the progress / "altitude" indicator for the loop.
        log::info!(
            target: "perspt::sdk_gate",
            "settle: completed={completed}/{total} escalated={escalated} Φ={phi:.2} replans={}/{}",
            state.count, state.max
        );
        self.emit_log(format!(
            "📐 settle: {completed}/{total} done, {escalated} escalated, Φ={phi:.2} (replans {}/{})",
            state.count, state.max
        ));

        let det = self.deterministic_goal_gate();

        // Interactive mode: do not auto-amend the plan.
        if !self.auto_approve {
            return if det {
                SettleDecision::Achieved
            } else {
                SettleDecision::Stop
            };
        }

        if det {
            // Cheap gate passed → confirm with the verifier LLM verdict.
            match self.goal_verdict(task).await {
                Some(v) if v.achieved => {
                    self.emit_log("✅ Goal verdict: achieved".to_string());
                    SettleDecision::Achieved
                }
                Some(v) => {
                    self.emit_log(format!(
                        "🛠️ Goal not yet met — missing: {}",
                        v.missing.join("; ")
                    ));
                    self.try_replan(task, &v.missing, completed, phi, state)
                        .await
                }
                None => {
                    // Verdict unavailable (LLM error): trust deterministic completion.
                    log::warn!("Goal verdict unavailable — accepting deterministic completion");
                    SettleDecision::Achieved
                }
            }
        } else {
            // Some nodes escalated/incomplete. Try to route around the gap.
            let missing = self.collect_unmet_summary();
            self.try_replan(task, &missing, completed, phi, state).await
        }
    }

    /// Attempt a bounded architect re-plan amendment toward the goal.
    pub(crate) async fn try_replan(
        &mut self,
        task: &str,
        missing: &[String],
        completed: usize,
        phi: f64,
        state: &mut ReplanState,
    ) -> SettleDecision {
        if state.count >= state.max {
            self.emit_log(format!(
                "⛔ Re-plan budget exhausted ({}/{}) — stopping",
                state.count, state.max
            ));
            return SettleDecision::Stop;
        }
        if self.budget.any_exhausted() {
            return SettleDecision::Stop;
        }
        // Progress non-regression: after the first amendment, require that the
        // previous one actually advanced (more completed nodes, or Φ decreased).
        if state.count > 0 {
            let progressed = completed > state.last_completed
                || state.last_phi.map(|p| phi < p - 1e-6).unwrap_or(true);
            if !progressed {
                self.emit_log(
                    "⛔ Re-plan made no progress (Φ/completed not improving) — stopping"
                        .to_string(),
                );
                return SettleDecision::Stop;
            }
        }

        match self.amend_plan_for_goal(task, missing).await {
            Ok(n) if n > 0 => {
                state.count += 1;
                state.last_completed = completed;
                state.last_phi = Some(phi);
                self.emit_log(format!(
                    "🗺️ Re-planned: +{n} task(s) toward the goal (revision {}/{})",
                    state.count, state.max
                ));
                SettleDecision::Replanned
            }
            Ok(_) => {
                self.emit_log("⚠️ Amendment produced no new tasks — stopping".to_string());
                SettleDecision::Stop
            }
            Err(e) => {
                log::warn!("Plan amendment failed: {e}");
                self.emit_log(format!("⚠️ Plan amendment failed: {e}"));
                SettleDecision::Stop
            }
        }
    }

    /// Deterministic pre-gate: every node is `Completed`, none escalated, and
    /// there is at least one node. The cheap condition that must hold before any
    /// LLM goal verdict.
    pub(crate) fn deterministic_goal_gate(&self) -> bool {
        let mut any = false;
        for idx in self.graph.node_indices() {
            any = true;
            if self.graph[idx].state != NodeState::Completed {
                return false;
            }
        }
        any
    }

    /// Count nodes currently in a given state.
    pub(crate) fn count_in_state(&self, state: NodeState) -> usize {
        self.graph
            .node_indices()
            .filter(|&i| self.graph[i].state == state)
            .count()
    }

    /// Workflow potential Φ (SDK `observability::phi`) over the live graph:
    /// total residual energy + remaining step budget. Lower energy = closer to
    /// the goal manifold; used as the loop's progress indicator.
    pub(crate) fn workflow_phi(&self) -> f64 {
        // Nodes that never recorded energy report `current_energy() == INFINITY`
        // ("unknown"); exclude those so Φ reflects the known residual backlog
        // rather than collapsing to infinity.
        let accepted_energy: f64 = self
            .graph
            .node_indices()
            .map(|i| self.graph[i].monitor.current_energy() as f64)
            .filter(|e| e.is_finite())
            .sum();
        let remaining = match self.budget.max_steps {
            Some(m) => m.saturating_sub(self.budget.steps_used),
            None => 0,
        };
        perspt_sdk::phi(accepted_energy, 0.5, remaining)
    }

    /// Goals of nodes that did not complete, used to seed a re-plan amendment.
    pub(crate) fn collect_unmet_summary(&self) -> Vec<String> {
        self.graph
            .node_indices()
            .filter(|&i| self.graph[i].state != NodeState::Completed)
            .map(|i| format!("{}: {}", self.graph[i].node_id, self.graph[i].goal))
            .collect()
    }

    /// Ask the verifier-tier model whether the user's overall goal is achieved.
    /// Returns `None` on any LLM/parse failure (caller decides the fallback).
    pub(crate) async fn goal_verdict(
        &mut self,
        task: &str,
    ) -> Option<perspt_core::types::GoalVerdict> {
        let tree = crate::tools::list_sandbox_files(&self.context.working_dir)
            .ok()
            .filter(|t| !t.is_empty())
            .map(|t| t.join("\n"));

        // Key file contents: the output targets the plan produced (capped).
        let mut files: Vec<(String, String)> = Vec::new();
        'outer: for idx in self.graph.node_indices() {
            for target in &self.graph[idx].output_targets {
                let path = self.context.working_dir.join(target);
                if let Ok(content) = std::fs::read_to_string(&path) {
                    files.push((target.to_string_lossy().to_string(), content));
                    if files.len() >= 12 {
                        break 'outer;
                    }
                }
            }
        }

        let ev = perspt_core::types::PromptEvidence {
            user_goal: Some(task.to_string()),
            project_file_tree: tree,
            existing_file_contents: files,
            build_test_output: self.context.last_test_output.clone(),
            ..Default::default()
        };
        let prompt = crate::prompt_compiler::compile(
            perspt_core::types::PromptIntent::GoalCompletionCheck,
            &ev,
        )
        .text;
        let model = self.verifier_model.clone();
        let resp = self
            .call_llm_with_logging(&model, &prompt, None)
            .await
            .ok()?;
        parse_goal_verdict(&resp)
    }

    /// Execute a single node through the control loop
    pub(crate) async fn execute_node(&mut self, idx: NodeIndex) -> Result<NodeOutcome> {
        let node = &self.graph[idx];
        log::info!("Executing node: {} ({})", node.node_id, node.goal);

        // PSP-5 Phase 6: Create provisional branch if node has graph parents
        let branch_id = self.maybe_create_provisional_branch(idx);

        // Step 2: Recursive Sub-graph Execution (already in topo order)
        self.graph[idx].state = NodeState::Coding;
        self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
            node_id: self.graph[idx].node_id.clone(),
            status: perspt_core::NodeStatus::Coding,
        });

        // Step 3: Speculative Generation
        let speculate_start = std::time::Instant::now();
        self.step_speculate(idx).await?;
        self.record_step_quietly(
            &self.graph[idx].node_id.clone(),
            "speculate",
            "ok",
            None,
            0,
            speculate_start.elapsed().as_millis() as i32,
        );

        // Step 4: Stability Verification
        let verify_start = std::time::Instant::now();
        let mut energy = self.step_verify(idx).await?;
        self.record_step_quietly(
            &self.graph[idx].node_id.clone(),
            "verify",
            "ok",
            Some(&energy),
            0,
            verify_start.elapsed().as_millis() as i32,
        );

        // PSP-7: Sheaf pre-check retry loop.
        // After convergence succeeds, a lightweight structural check verifies
        // output artifacts exist on disk before proceeding to full sheaf
        // validation. If pre-check fails, re-enter convergence with sheaf
        // evidence (max 1 retry to prevent infinite loops).
        let mut sheaf_pre_check_retries = 0u32;
        let mut converge_start;
        loop {
            // Step 5: Convergence & Self-Correction
            converge_start = std::time::Instant::now();
            if !self.step_converge(idx, energy.clone()).await? {
                self.record_step_quietly(
                    &self.graph[idx].node_id.clone(),
                    "converge",
                    "escalated",
                    Some(&energy),
                    self.graph[idx].monitor.attempt_count as i32,
                    converge_start.elapsed().as_millis() as i32,
                );
                // PSP-5 Phase 5: Classify non-convergence and choose repair action
                let category = self.classify_non_convergence(idx);
                let action = self.choose_repair_action(idx, &category);

                // Persist the escalation report
                let node = &self.graph[idx];
                let report = EscalationReport {
                    node_id: node.node_id.clone(),
                    session_id: self.context.session_id.clone(),
                    category,
                    action: action.clone(),
                    energy_snapshot: EnergyComponents {
                        v_syn: node.monitor.current_energy(),
                        ..Default::default()
                    },
                    stage_outcomes: self
                        .last_verification_result
                        .as_ref()
                        .map(|vr| vr.stage_outcomes.clone())
                        .unwrap_or_default(),
                    evidence: self.build_escalation_evidence(idx),
                    affected_node_ids: self.affected_dependents(idx),
                    timestamp: epoch_seconds(),
                };

                if let Err(e) = self.ledger.record_escalation_report(&report) {
                    log::warn!("Failed to persist escalation report: {}", e);
                }

                // PSP-5 Phase 9: Also persist artifact bundle on escalation path
                if let Some(bundle) = self.last_applied_bundle.take() {
                    if let Err(e) = self
                        .ledger
                        .record_artifact_bundle(&self.graph[idx].node_id, &bundle)
                    {
                        log::warn!(
                            "Failed to persist artifact bundle on escalation for {}: {}",
                            self.graph[idx].node_id,
                            e
                        );
                    }
                }

                self.emit_event(perspt_core::AgentEvent::EscalationClassified {
                    node_id: report.node_id.clone(),
                    category: report.category.to_string(),
                    action: report.action.to_string(),
                });

                // PSP-5 Phase 6: Flush this branch and all descendant branches
                let node_id_for_flush = self.graph[idx].node_id.clone();
                if let Some(ref bid) = branch_id {
                    self.flush_provisional_branch(bid, &node_id_for_flush);
                }
                self.flush_descendant_branches(idx);

                // Apply the chosen repair action or escalate to user
                let applied = self.apply_repair_action(idx, &action).await;

                if applied {
                    // The repair mutated the graph (node set to Retry, or new
                    // nodes inserted). Signal the control loop to re-evaluate and
                    // re-run the affected work rather than treating it as terminal.
                    log::info!(
                        "Node {} reworked via {}: {} — will re-evaluate",
                        self.graph[idx].node_id,
                        action,
                        category
                    );
                    return Ok(NodeOutcome::Reworked);
                }

                self.graph[idx].state = NodeState::Escalated;
                self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                    node_id: self.graph[idx].node_id.clone(),
                    status: perspt_core::NodeStatus::Escalated,
                });
                log::warn!(
                    "Node {} escalated to user: {} → {}",
                    self.graph[idx].node_id,
                    category,
                    action
                );

                return Ok(NodeOutcome::Escalated);
            }

            // PSP-7: Lightweight sheaf pre-check before full validation.
            // Verifies output artifacts exist and are non-empty on disk.
            if sheaf_pre_check_retries < 1 {
                if let Some(evidence) = self.sheaf_pre_check(idx) {
                    sheaf_pre_check_retries += 1;
                    log::warn!(
                        "Sheaf pre-check failed for {}, retrying convergence: {}",
                        self.graph[idx].node_id,
                        evidence
                    );
                    self.emit_log(format!("⚠️ Sheaf pre-check: {}", evidence));
                    // Inject sheaf evidence so the correction LLM sees it
                    self.context.last_test_output = Some(format!(
                    "Structural pre-check failure: {}\nEnsure all declared output files are generated \
                        correctly.",
                    evidence
                ));
                    // Re-verify and add sheaf penalty to force correction loop entry
                    energy = self.step_verify(idx).await?;
                    energy.v_sheaf += 2.0;
                    continue;
                }
            }
            break;
        } // end PSP-7 sheaf pre-check loop

        // Final sheaf pre-check guard: after the retry loop, verify once more.
        // If the retry still produced stub/missing artifacts, escalate the node
        // instead of proceeding to commit.
        if sheaf_pre_check_retries > 0 {
            if let Some(evidence) = self.sheaf_pre_check(idx) {
                log::warn!(
                    "Sheaf pre-check still failing for {} after retry, escalating: {}",
                    self.graph[idx].node_id,
                    evidence
                );
                self.emit_log(format!(
                    "❌ Sheaf pre-check failed after retry: {}",
                    evidence
                ));
                self.graph[idx].state = NodeState::Escalated;
                self.emit_event(perspt_core::AgentEvent::TaskStatusChanged {
                    node_id: self.graph[idx].node_id.clone(),
                    status: perspt_core::NodeStatus::Escalated,
                });
                // Flush provisional branch on escalation
                let node_id_for_flush = self.graph[idx].node_id.clone();
                if let Some(ref bid) = branch_id {
                    self.flush_provisional_branch(bid, &node_id_for_flush);
                }
                self.flush_descendant_branches(idx);
                return Ok(NodeOutcome::Escalated);
            }
        }

        // Record converge success (timing from last converge_start)
        self.record_step_quietly(
            &self.graph[idx].node_id.clone(),
            "converge",
            "ok",
            Some(&energy),
            self.graph[idx].monitor.attempt_count as i32,
            converge_start.elapsed().as_millis() as i32,
        );

        // Step 6: Sheaf Validation (Post-Subgraph Consistency)
        let sheaf_start = std::time::Instant::now();
        self.step_sheaf_validate(idx).await?;
        self.record_step_quietly(
            &self.graph[idx].node_id.clone(),
            "sheaf_validate",
            "ok",
            None,
            0,
            sheaf_start.elapsed().as_millis() as i32,
        );

        // Step 7: Merkle Ledger Commit
        let commit_start = std::time::Instant::now();
        self.step_commit(idx).await?;
        self.record_step_quietly(
            &self.graph[idx].node_id.clone(),
            "commit",
            "ok",
            None,
            0,
            commit_start.elapsed().as_millis() as i32,
        );

        // PSP-5 Phase 6: Merge provisional branch after successful commit
        if let Some(ref bid) = branch_id {
            self.merge_provisional_branch(bid, idx);
        }

        Ok(NodeOutcome::Completed)
    }
}
