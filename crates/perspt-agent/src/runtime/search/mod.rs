//! The bounded search forest executor (PSP-10 systems 19–21, Phase 8).
//!
//! Sequential, at most three branch identities, one quantum (one branch
//! attempt) at a time. Every branch runs the ordinary governed loop in an
//! isolated eager-copy workspace against the same immutable accepted
//! root; internal states are private (the [`branch::BranchRecorder`]
//! rewrites trajectory events into the search alphabet); exactly one
//! selected candidate is committed through one `submit_with_floor` call,
//! and the committed decision must equal the preview or the forest fails
//! closed. Branches carry deliberate strategies with real behavior
//! (continuation, alternative approach, diagnostic probe, distinct
//! family), every consumed resource — turns, calls, mutations, verifier
//! runs, tokens, bytes, wall-clock seconds — is charged against the
//! declared limit vector, and partial checkpoints carry the correction
//! packet and the remaining obligations. A crash mid-forest resumes from
//! the node's last durable checkpoint and re-runs the forest
//! deterministically — branch workspaces are never resume points.

mod branch;
pub(crate) mod fold;
mod nogood;
mod strategy;

use anyhow::{Context, Result};
use perspt_sdk::search::{BranchCandidate, ReservationTicket};
use perspt_sdk::{
    evaluate_gate_with_floor, AcceptedTrajectory, BranchMeasurement, GateDecision, ModelId,
    ResidualClass, ResidualEvent, SearchLimits, WitnessRef,
};

use super::node::{seed_from_attempt, CandidateSeed};
use super::{NodeAttempt, Psp9AgentRuntime, Psp9Recorder};
use crate::candidate::CodingCandidateMeasurer;
use crate::toolloop::{LoopEvent, LoopRecorder, Measured};
use branch::{consumed_usage, measure_fork_cost, BranchRecorder};
use nogood::{NoGoodComponents, NoGoodStore, NoGoodSupport};
use strategy::{next_strategy, BranchStrategy, BranchSummary};

/// Prior search state folded from the ledger at resume: the recorded
/// no-goods (their exact repeated attempts stay suppressed), the
/// interrupted forest's consumption (limits are not silently refilled),
/// and its identity (the new forest links to it via `resumed_from`).
pub(crate) struct SearchSeed {
    pub no_goods: NoGoodStore,
    pub prior_usage: Option<perspt_sdk::SearchUsage>,
    pub resumed_from: Option<String>,
}

/// Runtime search settings, resolved from `[exploration]` at construction.
#[derive(Debug, Clone)]
pub(crate) struct SearchSettings {
    pub limits: SearchLimits,
    pub initial_branches: u8,
    pub max_branches: u8,
    pub distinct_family: bool,
}

impl Default for SearchSettings {
    fn default() -> Self {
        Self {
            limits: SearchLimits::release_default(),
            initial_branches: 1,
            max_branches: 3,
            distinct_family: true,
        }
    }
}

impl SearchSettings {
    pub fn from_config(config: Option<&perspt_core::ExplorationConfig>) -> Self {
        let mut settings = Self::default();
        if let Some(config) = config {
            settings.initial_branches = config.initial_branches.unwrap_or(1);
            settings.max_branches = config.max_branches.unwrap_or(3).min(3);
            settings.distinct_family = config.distinct_family.unwrap_or(true);
            if let Some(files) = config.max_workspace_files {
                settings.limits.workspace_files = files;
            }
            if let Some(bytes) = config.max_workspace_bytes {
                settings.limits.workspace_bytes = bytes;
            }
        }
        settings
    }
}

/// Everything one open forest tracks while its branches run.
struct ForestRun<'a> {
    recorder: &'a Psp9Recorder,
    forest_id: String,
    node_id: String,
    generation: u32,
    accepted_root: String,
    /// Best accepted energy before the forest opened; the preview and the
    /// single commit both measure against it.
    baseline_energy: f64,
    rho_gate: f64,
    /// The one shared budget every branch action reserves against before
    /// it runs (Gate AC).
    budget: perspt_sdk::search::SharedSearchBudget,
    no_goods: NoGoodStore,
    /// The live verifier-suite identity, derived from the detected plugin
    /// profiles and the cluster profile — never a hardcoded constant.
    sensor_fingerprint: String,
    started: std::time::Instant,
    epoch: u64,
}

impl ForestRun<'_> {
    fn emit(&self, event: LoopEvent) -> Result<()> {
        self.recorder.record(&event)
    }

    /// Open the next frontier epoch and snapshot the budget at its start.
    fn open_epoch(&mut self) -> Result<()> {
        self.epoch += 1;
        self.emit(LoopEvent::FrontierEpochStarted {
            forest_id: self.forest_id.clone(),
            epoch: self.epoch,
            forest_digest: self.forest_digest(),
        })?;
        self.snapshot_usage()
    }

    /// Snapshot the shared budget into the ledger, so an interrupted
    /// forest's consumption is reconstructible (Gate AC + resume).
    fn snapshot_usage(&self) -> Result<()> {
        self.emit(LoopEvent::SearchUsageSnapshot {
            forest_id: self.forest_id.clone(),
            epoch: self.epoch,
            usage: self.budget.snapshot(),
        })
    }

    /// The folded forest identity for this epoch; a deterministic resume
    /// re-run recomputes and compares it.
    fn forest_digest(&self) -> String {
        let mut encoder = perspt_sdk::canon::CanonicalEncoder::new(b"perspt-forest-v1");
        encoder
            .text(&self.node_id)
            .u64(u64::from(self.generation))
            .text(&self.accepted_root)
            .u64(self.epoch)
            .u64(u64::from(self.budget.snapshot().actions));
        encoder.digest()
    }
}

/// What one finished branch leaves for the next branch's triggers and
/// goal program — measurements only, never its private workspace.
struct PreviousBranch {
    support_key: String,
    residual_count: usize,
    energy: f64,
    residual_summaries: Vec<String>,
    /// Typed obligation refs (`class:hash`) — the only correction state
    /// that may enter the next attempt's no-good key (Gate AB).
    obligation_refs: Vec<String>,
}

/// The branch's goal program inputs: the strategy-conditioned text the
/// model sees, and the typed parts that enter the no-good proposal key.
struct BranchGoal {
    text: String,
    base: String,
    previous_obligations: Vec<String>,
}

/// Resolve one branch's goal program from the base goal and the history.
fn goal_program(goal: &str, strategy: BranchStrategy, history: &[PreviousBranch]) -> BranchGoal {
    BranchGoal {
        text: branch_goal(goal, strategy, history.last()),
        base: goal.to_string(),
        previous_obligations: history
            .last()
            .map(|previous| previous.obligation_refs.clone())
            .unwrap_or_default(),
    }
}

/// The next branch's resolved plan.
struct BranchPlan {
    strategy: BranchStrategy,
    route: ModelId,
}

impl Psp9AgentRuntime {
    /// Open a forest at the recovery ladder's refine rung (system 20): the
    /// first `initial_branches` open immediately with the default strategy;
    /// further branches open only on measured triggers; one candidate
    /// commits through the ordinary gate.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn run_search_forest(
        &self,
        recorder: &Psp9Recorder,
        session_id: &str,
        goal: &str,
        node_id: &str,
        generation: u32,
        model: &ModelId,
        graph: &perspt_sdk::WorkGraphRevision,
        seed: Option<&CandidateSeed>,
        remaining_budget: u32,
        baseline_energy: f64,
        rho_gate: f64,
    ) -> Result<NodeAttempt> {
        let forest_id = uuid::Uuid::new_v4().to_string();
        let accepted_root = match seed {
            Some(seed) => seed.expected_state_root.clone(),
            // An unseeded forest still measures against the real workspace
            // content root, never a synthetic marker.
            None => crate::realize::snapshot_workspace(&self.working_dir, &[])?.root_hash(),
        };
        let (resumed_from, no_goods, budget) = self.opening_state();
        let mut forest = ForestRun {
            recorder,
            forest_id: forest_id.clone(),
            node_id: node_id.to_string(),
            generation,
            accepted_root: accepted_root.clone(),
            baseline_energy,
            rho_gate,
            budget,
            no_goods,
            sensor_fingerprint: self.live_sensor_fingerprint(),
            started: std::time::Instant::now(),
            epoch: 0,
        };
        forest.emit(LoopEvent::SearchOpened {
            forest_id: forest_id.clone(),
            node_id: node_id.to_string(),
            generation,
            accepted_root: accepted_root.clone(),
            limits: forest.budget.limits().clone(),
            resumed_from,
        })?;
        let attempts = self
            .run_branches(
                &mut forest,
                session_id,
                goal,
                graph,
                seed,
                model,
                remaining_budget,
            )
            .await?;
        let chosen = self.select_and_commit(&mut forest, attempts)?;
        let usage = forest.budget.close();
        forest.emit(LoopEvent::SearchClosed { forest_id, usage })?;
        Ok(chosen)
    }

    /// The forest's opening state. A resume seed (the ledger fold's
    /// no-goods and the interrupted forest's consumption) is consumed by
    /// the first forest opened afterwards — a fresh identity, honest state.
    fn opening_state(
        &self,
    ) -> (
        Option<String>,
        NoGoodStore,
        perspt_sdk::search::SharedSearchBudget,
    ) {
        let seed_state = self.search_seed.lock().unwrap().take();
        let resumed_from = seed_state.as_ref().and_then(|s| s.resumed_from.clone());
        match seed_state {
            Some(seeded) => {
                let budget = match seeded.prior_usage {
                    Some(usage) => perspt_sdk::search::SharedSearchBudget::with_usage(
                        self.search.limits.clone(),
                        usage,
                    ),
                    None => perspt_sdk::search::SharedSearchBudget::new(self.search.limits.clone()),
                };
                (resumed_from, seeded.no_goods, budget)
            }
            None => (
                None,
                NoGoodStore::default(),
                perspt_sdk::search::SharedSearchBudget::new(self.search.limits.clone()),
            ),
        }
    }

    /// The live sensor-profile identity: detected plugin verifier commands
    /// plus the clustering profile, canonically digested (Proposition 5:
    /// candidates compared by energy must share one immutable profile).
    fn live_sensor_fingerprint(&self) -> String {
        let mut encoder = perspt_sdk::canon::CanonicalEncoder::new(b"perspt-sensor-v1");
        let registry = perspt_core::PluginRegistry::new();
        for plugin in registry.detect_all(&self.working_dir) {
            for capability in plugin.verifier_profile().capabilities {
                encoder
                    .text(plugin.name())
                    .text(capability.stage.policy_name())
                    .text(capability.effective_command().unwrap_or_default());
            }
        }
        encoder.text("perspt-cluster-v1:log-damped");
        encoder.digest()
    }

    /// The sequential branch loop. Continuation and expansion are trigger-
    /// driven (system 20): measured progress continues the witnessed
    /// partial, a repeated signature forces an alternative approach, and
    /// stagnation expands to a distinct family.
    #[allow(clippy::too_many_arguments)]
    /// The evidence summary a later branch's goal text draws on.
    fn history_entry(
        support_key: String,
        measured: &Measured,
        candidate: &BranchCandidate,
    ) -> PreviousBranch {
        PreviousBranch {
            support_key,
            residual_count: measured.residuals.len(),
            energy: candidate.measurement.energy,
            residual_summaries: measured
                .residuals
                .iter()
                .take(6)
                .map(|residual| residual.evidence.summary.clone())
                .collect(),
            obligation_refs: obligation_refs(measured),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_branches(
        &self,
        forest: &mut ForestRun<'_>,
        session_id: &str,
        goal: &str,
        graph: &perspt_sdk::WorkGraphRevision,
        seed: Option<&CandidateSeed>,
        model: &ModelId,
        remaining_budget: u32,
    ) -> Result<Vec<(BranchCandidate, NodeAttempt, bool)>> {
        let routes = self.branch_routes(model);
        let mut attempts: Vec<(BranchCandidate, NodeAttempt, bool)> = Vec::new();
        let mut partial: Option<(CandidateSeed, WitnessRef, String)> = None;
        let mut history: Vec<PreviousBranch> = Vec::new();
        for index in 0..usize::from(self.search.max_branches) {
            let plan = self.plan_branch(index, &routes, &history, forest.baseline_energy);
            let (branch_seed, witness, parent_branch) = Self::branch_lineage(
                &partial,
                plan.strategy.continues_partial(),
                seed,
                &forest.accepted_root,
            );
            let goal_program = goal_program(goal, plan.strategy, &history);
            forest.open_epoch()?;
            let outcome = self
                .run_one_branch(
                    forest,
                    session_id,
                    &goal_program,
                    graph,
                    branch_seed.as_ref(),
                    &witness,
                    &plan,
                    index,
                    parent_branch,
                    remaining_budget,
                )
                .await?;
            let Some((candidate, attempt, eligible, measured, support_key)) = outcome else {
                break; // budget refused the fork: the forest must close.
            };
            let accepted = eligible && measured.hard_pass;
            let over_budget = self.charge_attempt(forest, &attempt, &candidate).is_err();
            forest.snapshot_usage()?;
            if !accepted && !over_budget && index + 1 < usize::from(self.search.max_branches) {
                partial = self
                    .partial_checkpoint(
                        forest,
                        &attempt,
                        &measured,
                        &witness,
                        &candidate.measurement.branch_id,
                    )
                    .await?;
            }
            history.push(Self::history_entry(support_key, &measured, &candidate));
            // A transport-contained branch means the route itself is
            // refusing (rate limit, outage): opening more branches against
            // it multiplies the retry storm for nothing. Close the forest.
            let transport_dead = attempt.outcome.contained_by_transport;
            let budget_denied =
                Self::note_denied_reservation(forest, &candidate.measurement.branch_id)?;
            attempts.push((candidate, attempt, eligible));
            if accepted || over_budget || transport_dead || budget_denied {
                break;
            }
        }
        Ok(attempts)
    }

    /// A refused in-loop reservation aborted the branch before the action
    /// ran: ledger the abandonment so the forest closes gracefully.
    fn note_denied_reservation(forest: &ForestRun<'_>, branch_id: &str) -> Result<bool> {
        if !forest.budget.take_denied() {
            return Ok(false);
        }
        forest.emit(LoopEvent::BranchAbandoned {
            forest_id: forest.forest_id.clone(),
            branch_id: branch_id.to_string(),
            reason: "the search budget refused a reservation; branch aborted".into(),
        })?;
        Ok(true)
    }

    /// Real seed lineage for the next branch: a continuing strategy
    /// inherits the pending partial (its producer becomes the parent);
    /// every other strategy restarts from the accepted root with no
    /// parent branch — `parent_branch` and `seed_witness` always agree.
    #[allow(clippy::type_complexity)]
    fn branch_lineage(
        partial: &Option<(CandidateSeed, WitnessRef, String)>,
        continues: bool,
        seed: Option<&CandidateSeed>,
        accepted_root: &str,
    ) -> (Option<CandidateSeed>, WitnessRef, Option<String>) {
        match (partial, continues) {
            (Some((seed, witness, producer)), true) => {
                (Some(seed.clone()), witness.clone(), Some(producer.clone()))
            }
            _ => (seed.cloned(), WitnessRef::root(accepted_root), None),
        }
    }

    /// Resolve the next branch's strategy and route. The first
    /// `initial_branches` open with the default strategy; later branches
    /// follow the measured triggers; the distinct-family strategy walks the
    /// diverse route plan.
    fn plan_branch(
        &self,
        index: usize,
        routes: &[ModelId],
        history: &[PreviousBranch],
        baseline_energy: f64,
    ) -> BranchPlan {
        let strategy = if index < usize::from(self.search.initial_branches.max(1)) {
            BranchStrategy::LocalRepair
        } else {
            next_strategy(summarize(history, baseline_energy))
        };
        let route_index = if strategy == BranchStrategy::DistinctFamily {
            index.min(routes.len() - 1)
        } else {
            0
        };
        BranchPlan {
            strategy,
            route: routes[route_index].clone(),
        }
    }

    /// Charge the branch's wall-clock interval. Turns, calls, mutations,
    /// verifier runs, tokens, and result bytes were already reserved and
    /// settled **inside** the loop as each action ran (Gate AC); a debug
    /// cross-check confirms the settled totals cover the observed outcome.
    fn charge_attempt(
        &self,
        forest: &mut ForestRun<'_>,
        attempt: &NodeAttempt,
        candidate: &BranchCandidate,
    ) -> Result<()> {
        if cfg!(debug_assertions) {
            let observed = consumed_usage(&attempt.outcome);
            let settled = forest.budget.snapshot();
            debug_assert!(
                settled.model_turns >= observed.model_turns
                    && settled.tool_calls >= observed.tool_calls,
                "in-loop reservations must cover the observed outcome"
            );
        }
        let elapsed = forest.started.elapsed().as_secs();
        let interval = elapsed.saturating_sub(forest.budget.snapshot().elapsed_secs);
        forest.budget.charge_elapsed(interval).map_err(|error| {
            let _ = forest.emit(LoopEvent::BranchObservation {
                forest_id: forest.forest_id.clone(),
                branch_id: candidate.measurement.branch_id.clone(),
                observation: format!("budget exhausted after branch: {error}"),
            });
            anyhow::anyhow!("{error}")
        })
    }

    /// The deliberately diverse route plan: primary first, then fallbacks
    /// and the handoff route (a new family is an expansion prior, never a
    /// certificate).
    fn branch_routes(&self, primary: &ModelId) -> Vec<ModelId> {
        let mut routes = vec![primary.clone()];
        for route in self
            .fallback_models
            .iter()
            .chain(self.handoff_model.as_ref())
        {
            if routes.contains(route) {
                continue;
            }
            if self.search.distinct_family {
                let family = self.transport.family_of(route);
                let seen = routes
                    .iter()
                    .any(|existing| self.transport.family_of(existing) == family);
                if seen && routes.len() > 1 {
                    continue;
                }
            }
            routes.push(route.clone());
        }
        routes
    }

    /// Reserve, fork, and run one branch; measure and preview its realized
    /// candidate. `None` when the budget refuses the fork.
    #[allow(clippy::too_many_arguments)]
    async fn run_one_branch(
        &self,
        forest: &mut ForestRun<'_>,
        session_id: &str,
        goal: &BranchGoal,
        graph: &perspt_sdk::WorkGraphRevision,
        seed: Option<&CandidateSeed>,
        witness: &WitnessRef,
        plan: &BranchPlan,
        index: usize,
        parent_branch: Option<String>,
        remaining_budget: u32,
    ) -> Result<Option<(BranchCandidate, NodeAttempt, bool, Measured, String)>> {
        let branch_id = format!("{}/b{}", forest.forest_id, index + 1);
        // Assemble first (no workspace is touched): the exact no-good
        // components need the real compiled prompt, grant, and catalog
        // identities before the fork is even admitted (Gate AB).
        let prepared = self
            .prepare_attempt(
                forest.recorder,
                session_id,
                &forest.node_id,
                forest.generation,
                &plan.route,
                graph,
            )
            .await?;
        let no_good = self.no_good_components(
            forest,
            &goal.base,
            plan.strategy.id(),
            &prepared,
            &goal.previous_obligations,
        );
        if !self.admit_fork(forest, &branch_id, remaining_budget, &no_good)? {
            return Ok(None);
        }
        emit_branch_opened(forest, &branch_id, parent_branch, witness, plan)?;
        let branch_recorder = BranchRecorder {
            inner: forest.recorder,
            forest_id: forest.forest_id.clone(),
            branch_id: branch_id.clone(),
        };
        // A witness chain longer than [root] means this branch continues a
        // partial checkpoint; its root enters the loop's dependency env.
        let partial_root = (witness.chain.len() > 1)
            .then(|| seed.map(|s| s.expected_state_root.clone()))
            .flatten();
        let attempt = self
            .attempt_prepared(
                prepared,
                &goal.text,
                &forest.node_id,
                forest.generation,
                &plan.route,
                graph,
                seed,
                remaining_budget,
                &branch_recorder,
                partial_root,
                Some(forest.budget.clone()),
            )
            .await?;
        let previewed = self
            .preview_branch(forest, &branch_id, &attempt, &no_good)
            .await?;
        let (candidate, eligible, measured, support_key) = previewed;
        Ok(Some((candidate, attempt, eligible, measured, support_key)))
    }

    /// Reserve the fork's eager-copy cost — **held for the forest's
    /// lifetime**, so the reservation precedes the copy and stands while
    /// the branch exists — check turn headroom without consuming it (the
    /// in-loop reservation takes each turn as it happens), and check the
    /// exact no-good store. A refused reservation or a suppressed
    /// duplicate creates no branch (Gates AC and AB).
    fn admit_fork(
        &self,
        forest: &mut ForestRun<'_>,
        branch_id: &str,
        remaining_budget: u32,
        no_good: &NoGoodComponents,
    ) -> Result<bool> {
        let request = measure_fork_cost(&self.working_dir);
        let _ticket: ReservationTicket = match forest.budget.reserve(request) {
            Ok(ticket) => ticket,
            Err(error) => {
                forest.emit(LoopEvent::BranchObservation {
                    forest_id: forest.forest_id.clone(),
                    branch_id: branch_id.to_string(),
                    observation: format!("fork refused: {error}; no branch created"),
                })?;
                return Ok(false);
            }
        };
        // The ticket is deliberately never released: the eager copy's
        // file/byte cost stays reserved for the forest's lifetime.
        let turn_probe = perspt_sdk::search::ReservationRequest {
            model_turns: remaining_budget.min(self.config.max_turns).max(1),
            ..Default::default()
        };
        if !forest.budget.headroom(&turn_probe) {
            let _ = forest.budget.take_denied();
            forest.emit(LoopEvent::BranchObservation {
                forest_id: forest.forest_id.clone(),
                branch_id: branch_id.to_string(),
                observation: "fork refused: no model-turn headroom remains".into(),
            })?;
            return Ok(false);
        }
        if forest.no_goods.suppresses(no_good) {
            forest.emit(LoopEvent::BranchObservation {
                forest_id: forest.forest_id.clone(),
                branch_id: branch_id.to_string(),
                observation: "exact no-good suppressed a duplicate attempt".into(),
            })?;
            return Ok(false);
        }
        Ok(true)
    }

    /// Realize the branch workspace, run the required sensors once more,
    /// and evaluate the pure gate preview (Definition 3: no decision is
    /// appended).
    async fn preview_branch(
        &self,
        forest: &mut ForestRun<'_>,
        branch_id: &str,
        attempt: &NodeAttempt,
        no_good: &NoGoodComponents,
    ) -> Result<(BranchCandidate, bool, Measured, String)> {
        // The preview sweep is a verifier action: reserved before it runs.
        let sweep = forest
            .budget
            .reserve(perspt_sdk::search::ReservationRequest {
                verifier_runs: 1,
                ..Default::default()
            });
        if let Err(error) = sweep {
            forest.emit(LoopEvent::BranchIneligible {
                forest_id: forest.forest_id.clone(),
                branch_id: branch_id.to_string(),
                reason: format!("preview refused: {error}"),
            })?;
            anyhow::bail!("the budget refused the preview sweep: {error}");
        }
        let measurer =
            CodingCandidateMeasurer::new(&attempt.candidate, &forest.node_id, forest.generation)
                .with_domain(self.domain.clone())
                .with_max_parallel(self.config.max_parallel_verifiers)
                .with_require_format(self.config.require_format)
                .with_correction_packets(!self.config.ablate_correction_packets);
        let measured = crate::toolloop::CandidateMeasurer::measure(&measurer).await?;
        let candidate_id = format!("{}/{}/{branch_id}", forest.node_id, forest.generation);
        let measurement = BranchMeasurement {
            branch_id: branch_id.to_string(),
            candidate_id: candidate_id.clone(),
            energy: measured.energy,
            hard_pass: measured.hard_pass,
            residuals: measured.residuals.clone(),
            sensor_profile: forest.sensor_fingerprint.clone(),
            cost: f64::from(attempt.outcome.turns_used),
        };
        forest.emit(LoopEvent::BranchCandidateMeasured {
            forest_id: forest.forest_id.clone(),
            branch_id: branch_id.to_string(),
            candidate_id,
            measurement: measurement.clone(),
        })?;
        let mut support_key = String::new();
        if !candidate_preview_accepted(forest, &measurement) {
            forest.emit(LoopEvent::BranchIneligible {
                forest_id: forest.forest_id.clone(),
                branch_id: branch_id.to_string(),
                reason: "preview did not accept (no hard pass or measured descent)".into(),
            })?;
            support_key = self
                .record_no_good(forest, branch_id, attempt, &measured, no_good)
                .await?;
        }
        let eligible = candidate_eligible(forest, &measurement);
        let candidate = BranchCandidate {
            targeted_improvement: (forest.baseline_energy - measurement.energy).max(0.0),
            measurement,
        };
        Ok((candidate, eligible, measured, support_key))
    }

    /// Record a failed preview's exact no-good when Gate AB admits its
    /// evidence; an environmental failure records only an observation, so a
    /// retry stays legal and the repeated-signature trigger never fires on
    /// it. Returns the support key (empty when nothing was recorded).
    async fn record_no_good(
        &self,
        forest: &mut ForestRun<'_>,
        branch_id: &str,
        attempt: &NodeAttempt,
        measured: &Measured,
        no_good: &NoGoodComponents,
    ) -> Result<String> {
        let state_root = crate::toolloop::EffectExecutor::checkpoint(&attempt.candidate, &[])
            .await?
            .witness
            .state_root;
        match no_good_support(measured, &state_root) {
            Some(support) => {
                let key = forest.no_goods.record(no_good, &support);
                forest.emit(LoopEvent::NoGoodRecorded {
                    forest_id: forest.forest_id.clone(),
                    branch_id: branch_id.to_string(),
                    key,
                    evidence_hash: support.evidence_hash(),
                    support_kind: support.kind().to_string(),
                })?;
                Ok(support.evidence_hash())
            }
            None => {
                forest.emit(LoopEvent::BranchObservation {
                    forest_id: forest.forest_id.clone(),
                    branch_id: branch_id.to_string(),
                    observation: "no deterministic support; no-good not recorded".into(),
                })?;
                Ok(String::new())
            }
        }
    }

    /// Persist an ineligible-but-actionable branch as a private partial
    /// checkpoint the next quantum may continue (system 19). The checkpoint
    /// carries the correction packet and the remaining obligations, so a
    /// continuation knows exactly what is left. The witness chain extends
    /// the **producing branch's** seed witness, so a partial-of-partial
    /// keeps its complete ancestry back to the accepted root (spec
    /// :2342-2343), and the event names the real producing branch.
    async fn partial_checkpoint(
        &self,
        forest: &mut ForestRun<'_>,
        attempt: &NodeAttempt,
        measured: &Measured,
        seed_witness: &WitnessRef,
        branch_id: &str,
    ) -> Result<Option<(CandidateSeed, WitnessRef, String)>> {
        let Some(seed) = seed_from_attempt(forest.recorder, &forest.node_id, attempt).await? else {
            return Ok(None);
        };
        let witness = seed_witness.extend(&seed.expected_state_root);
        let correction = measured.packet.as_ref().map(|packet| {
            perspt_sdk::CorrectionPacketRef(perspt_sdk::ledger::content_hash(
                serde_json::to_string(packet).unwrap_or_default().as_bytes(),
            ))
        });
        let remaining_obligations: Vec<perspt_sdk::search::ObligationRef> =
            obligation_refs(measured)
                .into_iter()
                .map(perspt_sdk::search::ObligationRef)
                .collect();
        let checkpoint = perspt_sdk::PartialCheckpointRef {
            state_root: seed.expected_state_root.clone(),
            accepted_ancestor: forest.accepted_root.clone(),
            parent_witness: seed_witness.clone(),
            correction,
            remaining_obligations,
            evidence_digest: seed.expected_state_root.clone(),
        };
        forest.emit(LoopEvent::PartialCheckpointed {
            forest_id: forest.forest_id.clone(),
            branch_id: branch_id.to_string(),
            checkpoint,
        })?;
        Ok(Some((seed, witness, branch_id.to_string())))
    }

    /// Proposition 5 selection, then exactly one authoritative commit whose
    /// decision must equal the preview (Definition 3: fail closed on
    /// divergence).
    fn select_and_commit(
        &self,
        forest: &mut ForestRun<'_>,
        attempts: Vec<(BranchCandidate, NodeAttempt, bool)>,
    ) -> Result<NodeAttempt> {
        anyhow::ensure!(!attempts.is_empty(), "the forest opened no branch");
        let eligible: Vec<perspt_sdk::search::BranchCandidate> = attempts
            .iter()
            .filter(|(_, _, eligible)| *eligible)
            .map(|(candidate, _, _)| candidate.clone())
            .collect();
        let selected_id = perspt_sdk::search::select_branch(&eligible)
            .map(|candidate| candidate.measurement.branch_id.clone());
        let mut chosen: Option<NodeAttempt> = None;
        let mut fallback_best: Option<(f64, NodeAttempt)> = None;
        for (candidate, attempt, _) in attempts {
            if Some(&candidate.measurement.branch_id) == selected_id.as_ref() {
                self.commit_selected(forest, &candidate)?;
                chosen = Some(attempt);
            } else {
                forest.emit(LoopEvent::BranchNotSelected {
                    forest_id: forest.forest_id.clone(),
                    branch_id: candidate.measurement.branch_id.clone(),
                })?;
                let energy = candidate.measurement.energy;
                if fallback_best
                    .as_ref()
                    .is_none_or(|(best, _)| energy < *best)
                {
                    fallback_best = Some((energy, attempt));
                }
                // Its private workspace drops with the attempt.
            }
        }
        match chosen {
            Some(attempt) => Ok(attempt),
            // No eligible candidate: the ladder continues from the best
            // observation, exactly like a failed single attempt.
            None => Ok(fallback_best
                .map(|(_, attempt)| attempt)
                .expect("nonempty attempts")),
        }
    }

    /// The single authoritative gate commit for the forest (Gate X: one
    /// decision, and the committed decision equals the preview).
    fn commit_selected(
        &self,
        forest: &mut ForestRun<'_>,
        candidate: &BranchCandidate,
    ) -> Result<()> {
        let measurement = &candidate.measurement;
        let preview = evaluate_gate_with_floor(
            measurement.hard_pass,
            measurement.energy,
            forest.baseline_energy,
            forest.rho_gate,
            None,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut trajectory = AcceptedTrajectory::new(
            forest.node_id.clone(),
            forest.generation,
            forest.baseline_energy,
            forest.rho_gate,
            1,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        let committed = trajectory
            .submit_with_floor(measurement.hard_pass, measurement.energy, None)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        anyhow::ensure!(
            committed == preview,
            "committed decision {committed:?} diverged from the preview {preview:?}; \
             stale root — failing closed"
        );
        let recorded = trajectory.gate_decisions.last().context("one decision")?;
        forest.emit(LoopEvent::CandidateMeasured {
            node_id: forest.node_id.clone(),
            generation: forest.generation,
            candidate_id: measurement.candidate_id.clone(),
            energy: measurement.energy,
            hard_pass: measurement.hard_pass,
            residuals: measurement.residuals.clone(),
        })?;
        forest.emit(LoopEvent::GateDecisionRecorded {
            node_id: forest.node_id.clone(),
            generation: forest.generation,
            candidate_id: measurement.candidate_id.clone(),
            decision: committed.clone(),
            observed_energy: Some(recorded.observed_energy),
            best_accepted_before: Some(recorded.best_accepted_before),
        })?;
        forest.emit(LoopEvent::BranchSelected {
            forest_id: forest.forest_id.clone(),
            branch_id: measurement.branch_id.clone(),
            candidate_id: measurement.candidate_id.clone(),
        })?;
        forest.emit(LoopEvent::BranchCommitted {
            forest_id: forest.forest_id.clone(),
            branch_id: measurement.branch_id.clone(),
            candidate_id: measurement.candidate_id.clone(),
            decision: committed,
        })?;
        Ok(())
    }

    /// The exact no-good components for one attempt configuration (Gate
    /// AB): `q` is the compiled prompt invocation's real digest, `t` the
    /// effective tool-surface hash, `c` the typed grant/capability scopes,
    /// and `p` the base goal + strategy + the previous branch's **typed**
    /// obligation refs — natural-language residual prose never enters the
    /// key. The sensor fingerprint is the forest's live verifier identity.
    fn no_good_components(
        &self,
        forest: &ForestRun<'_>,
        base_goal: &str,
        strategy_id: &str,
        prepared: &super::node::PreparedAttempt,
        previous_obligations: &[String],
    ) -> NoGoodComponents {
        let digest = component_digest;
        let (prompt_digest, catalog_digest) = match &prepared.envelope.invocation {
            Some(invocation) => (
                invocation.invocation_digest.clone(),
                digest("catalog", &invocation.platform.tool_spec_hash),
            ),
            // Scripted transports without a compiled program still get an
            // exact (if coarser) identity from the envelope text.
            None => (
                digest("prompt-text", &prepared.envelope.text),
                digest("catalog", "uncompiled"),
            ),
        };
        let mut proposal = perspt_sdk::canon::CanonicalEncoder::new(b"perspt.no-good.v1");
        proposal.text("proposal").text(base_goal).text(strategy_id);
        for obligation in previous_obligations {
            proposal.text(obligation);
        }
        NoGoodComponents {
            accepted_root: forest.accepted_root.clone(),
            domain_digest: digest("domain", &self.domain.domain_id().0),
            strategy_digest: digest("strategy", strategy_id),
            prompt_digest,
            proposal_digest: proposal.digest(),
            grant_digest: grant_capability_digest(&prepared.assembly),
            catalog_digest,
            sensor_fingerprint: forest.sensor_fingerprint.clone(),
            build_digest: digest("build", &runtime_build_id()),
        }
    }
}

/// The three fork-opening events, in the alphabet's order. `parent_branch`
/// is the real seed lineage, mutually consistent with `seed_witness`:
/// `Some(producer)` only when this branch continues its partial.
fn emit_branch_opened(
    forest: &ForestRun<'_>,
    branch_id: &str,
    parent_branch: Option<String>,
    witness: &WitnessRef,
    plan: &BranchPlan,
) -> Result<()> {
    forest.emit(LoopEvent::BranchForked {
        forest_id: forest.forest_id.clone(),
        branch_id: branch_id.to_string(),
        parent_branch,
        seed_checkpoint: witness.chain.first().cloned().unwrap_or_default(),
        seed_witness: witness.clone(),
    })?;
    forest.emit(LoopEvent::BranchStrategySelected {
        forest_id: forest.forest_id.clone(),
        branch_id: branch_id.to_string(),
        strategy_id: plan.strategy.id().into(),
    })?;
    forest.emit(LoopEvent::FrontierEntryServed {
        forest_id: forest.forest_id.clone(),
        branch_id: branch_id.to_string(),
        epoch: forest.epoch,
    })
}

fn component_digest(label: &str, value: &str) -> String {
    let mut encoder = perspt_sdk::canon::CanonicalEncoder::new(b"perspt.no-good.v1");
    encoder.text(label).text(value);
    encoder.digest()
}

/// `b`: the runtime build identity — crate version plus the git commit
/// when the build exported one (`PERSPT_GIT_SHA`).
fn runtime_build_id() -> String {
    format!(
        "{}+{}",
        env!("CARGO_PKG_VERSION"),
        option_env!("PERSPT_GIT_SHA").unwrap_or("dev")
    )
}

/// `c`: the typed ceilings and scopes governing what the branch may do.
/// Per-mint identifiers (policy id, capability id, session id, integrity
/// binding) are excluded — per-mint noise would defeat exact matching.
fn grant_capability_digest(assembly: &super::node::NodeAssembly) -> String {
    let policy = &assembly.grant_policy;
    let capability = &assembly.capability;
    let surface = serde_json::json!({
        "effect_ceiling": policy.effect_ceiling,
        "path_ceiling": policy.path_ceiling,
        "command_ceiling": policy.command_ceiling,
        "network_ceiling": policy.network_ceiling,
        "approval_ceiling": policy.approval_ceiling,
        "authority_epoch": policy.authority_epoch,
        "persistent": policy.persistent,
        "effects": capability.effects,
        "path_scope": capability.path_scope,
        "command_scope": capability.command_scope,
        "network_scope": capability.network_scope,
        "role": capability.role,
        "approval_policy": capability.approval_policy,
        "max_calls": capability.max_calls,
    });
    component_digest("grant", &surface.to_string())
}

/// Typed obligation refs from a measurement: `{class:?}:{hash(summary)}` —
/// the shared shape for partial checkpoints and no-good proposal keys.
fn obligation_refs(measured: &Measured) -> Vec<String> {
    measured
        .residuals
        .iter()
        .map(|residual| {
            format!(
                "{:?}:{}",
                residual.class,
                perspt_sdk::ledger::content_hash(residual.evidence.summary.as_bytes())
            )
        })
        .collect()
}

/// Fold the branch history into the next branch's trigger inputs: the
/// latest branch is compared against its predecessor (or the baseline).
fn summarize(history: &[PreviousBranch], baseline_energy: f64) -> BranchSummary {
    let Some(last) = history.last() else {
        return BranchSummary::default();
    };
    match history.len() {
        0 => BranchSummary::default(),
        1 => BranchSummary {
            repeated_signature: false,
            obligations_decreased: false,
            energy_improved: last.energy < baseline_energy,
        },
        _ => {
            let earlier = &history[history.len() - 2];
            BranchSummary {
                repeated_signature: !last.support_key.is_empty()
                    && last.support_key == earlier.support_key,
                obligations_decreased: last.residual_count < earlier.residual_count,
                energy_improved: last.energy < earlier.energy,
            }
        }
    }
}

/// The strategy-conditioned branch goal: the base goal, the strategy's
/// fragment, and the previous branch's typed obligation summary.
fn branch_goal(goal: &str, strategy: BranchStrategy, previous: Option<&PreviousBranch>) -> String {
    let mut text = goal.to_string();
    text.push_str("\n\n[search strategy: ");
    text.push_str(strategy.id());
    text.push_str("] ");
    text.push_str(strategy.goal_fragment());
    if let Some(previous) = previous {
        if !previous.residual_summaries.is_empty() {
            text.push_str("\nRemaining diagnostics from the previous attempt:\n");
            for summary in &previous.residual_summaries {
                text.push_str("- ");
                text.push_str(summary);
                text.push('\n');
            }
        }
    }
    text
}

/// Definition 3 eligibility: required sensors ran (no required-stage
/// `SensorUnavailable`), the witness chain holds by construction, and the
/// pure gate preview accepts.
fn candidate_eligible(forest: &ForestRun<'_>, measurement: &BranchMeasurement) -> bool {
    let sensors_ok = !measurement.residuals.iter().any(|residual| {
        residual.class == ResidualClass::SensorUnavailable
            && residual.evidence.summary.contains("required-stage:")
    });
    sensors_ok && candidate_preview_accepted(forest, measurement)
}

fn candidate_preview_accepted(forest: &ForestRun<'_>, measurement: &BranchMeasurement) -> bool {
    matches!(
        evaluate_gate_with_floor(
            measurement.hard_pass,
            measurement.energy,
            forest.baseline_energy,
            forest.rho_gate,
            None,
        ),
        Ok(GateDecision::HardPass | GateDecision::AcceptedByDescent { .. })
    )
}

/// The typed evidence value of one residual: the structured tool code
/// (`E0432`, a test id) when the sensor preserved one, else a class-tagged
/// content hash of the summary — never free prose.
fn evidence_value(residual: &ResidualEvent) -> String {
    let code = residual
        .evidence
        .structured
        .as_ref()
        .and_then(|value| value.get("code"))
        .and_then(|code| code.as_str());
    match code {
        Some(code) => code.to_string(),
        None => format!(
            "{:?}:{}",
            residual.class,
            perspt_sdk::ledger::content_hash(residual.evidence.summary.as_bytes())
        ),
    }
}

/// Deterministic support for a failed attempt's no-good, or `None` when no
/// residual class Gate AB admits is present (environmental gaps and model
/// interpretation cannot support a no-good — recording one would wrongly
/// suppress a valid retry). An empty residual set supports the unchanged
/// realized-state hash — the actual candidate root, never an energy string.
fn no_good_support(measured: &Measured, state_root: &str) -> Option<NoGoodSupport> {
    use ResidualClass as C;
    if measured.residuals.is_empty() {
        return Some(NoGoodSupport::UnchangedStateHash(state_root.to_string()));
    }
    let mut best: Option<(u8, NoGoodSupport)> = None;
    for residual in &measured.residuals {
        let ranked = match residual.class {
            C::TestFailure | C::Regression | C::Runtime => {
                Some((0, NoGoodSupport::FailedTest(evidence_value(residual))))
            }
            C::Syntax
            | C::Type
            | C::Build
            | C::Lint
            | C::Format
            | C::ImportGraph
            | C::SymbolMismatch
            | C::Manifest
            | C::Dependency => Some((1, NoGoodSupport::CompilerCode(evidence_value(residual)))),
            C::InterfaceMismatch
            | C::OwnershipViolation
            | C::SheafInconsistency
            | C::ContextDrift
            | C::WitnessStale
            | C::ToolArgumentInvalid => Some((
                2,
                NoGoodSupport::ContractDiagnostic(evidence_value(residual)),
            )),
            C::Policy | C::CapabilityDenied => {
                Some((3, NoGoodSupport::DeniedCapability(evidence_value(residual))))
            }
            // Environmental / budget / model-plane classes: no support.
            _ => None,
        };
        if let Some((rank, support)) = ranked {
            if best.as_ref().is_none_or(|(current, _)| rank < *current) {
                best = Some((rank, support));
            }
        }
    }
    best.map(|(_, support)| support)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn residual(class: ResidualClass) -> ResidualEvent {
        let mut residual = ResidualEvent::new(
            "n1",
            0,
            class,
            perspt_sdk::ResidualSeverity::Error,
            1.0,
            perspt_sdk::SensorRef::new(
                "governed-verifier",
                perspt_sdk::IndependenceRoute::DeterministicTool,
            ),
        )
        .unwrap();
        residual.evidence.summary = format!("{class:?} happened");
        residual
    }

    fn measured(residuals: Vec<ResidualEvent>) -> Measured {
        Measured {
            hard_pass: false,
            energy: 4.0,
            residuals,
            correction: None,
            packet: None,
        }
    }

    /// Gate AB's admitted evidence classes, and only those, support a
    /// no-good; environmental and model-plane classes never do.
    #[test]
    fn support_classification_follows_gate_ab() {
        use ResidualClass as C;
        let cases = [
            (C::TestFailure, Some("failed-test")),
            (C::Runtime, Some("failed-test")),
            (C::Build, Some("compiler-code")),
            (C::Type, Some("compiler-code")),
            (C::Lint, Some("compiler-code")),
            (C::WitnessStale, Some("contract-diagnostic")),
            (C::ToolArgumentInvalid, Some("contract-diagnostic")),
            (C::Policy, Some("denied-capability")),
            (C::CapabilityDenied, Some("denied-capability")),
            (C::SensorUnavailable, None),
            (C::ToolFailure, None),
            (C::ProviderUnavailable, None),
            (C::BudgetExhausted, None),
            (C::AdjudicatorObjection, None),
        ];
        for (class, expected) in cases {
            let support = no_good_support(&measured(vec![residual(class)]), "root-1");
            assert_eq!(
                support.as_ref().map(NoGoodSupport::kind),
                expected,
                "class {class:?}"
            );
        }
    }

    /// A failed test outranks a compiler code; an empty residual set
    /// supports the real state root, never an energy string.
    #[test]
    fn support_precedence_and_state_root_fallback() {
        let mixed = measured(vec![
            residual(ResidualClass::Build),
            residual(ResidualClass::TestFailure),
        ]);
        assert_eq!(
            no_good_support(&mixed, "root-1").unwrap().kind(),
            "failed-test"
        );
        let empty = no_good_support(&measured(vec![]), "root-xyz").unwrap();
        assert!(matches!(
            empty,
            NoGoodSupport::UnchangedStateHash(ref root) if root == "root-xyz"
        ));
    }

    /// A structured tool code is the evidence value when the sensor
    /// preserved one; prose only ever enters as a class-tagged hash.
    #[test]
    fn evidence_value_prefers_structured_codes() {
        let mut with_code = residual(ResidualClass::Build);
        with_code.evidence.structured = Some(serde_json::json!({"code": "E0432"}));
        assert_eq!(evidence_value(&with_code), "E0432");
        let without = residual(ResidualClass::Build);
        let value = evidence_value(&without);
        assert!(value.starts_with("Build:"), "{value}");
        assert!(!value.contains("happened"), "prose must not enter");
    }

    /// The build identity carries the crate version (plus the git sha when
    /// exported), so every dev build shares one digest only within one
    /// version+commit.
    #[test]
    fn build_id_carries_version() {
        assert!(runtime_build_id().starts_with(env!("CARGO_PKG_VERSION")));
    }
}
