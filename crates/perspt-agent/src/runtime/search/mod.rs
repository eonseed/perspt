//! The bounded search forest executor (PSP-10 systems 19–21, Phase 8).
//!
//! Sequential, at most three branch identities, one quantum (one branch
//! attempt) at a time. Every branch runs the ordinary governed loop in an
//! isolated eager-copy workspace against the same immutable accepted
//! root; internal states are private (the [`branch::BranchRecorder`]
//! rewrites trajectory events into the search alphabet); exactly one
//! selected candidate is committed through one `submit_with_floor` call,
//! and the committed decision must equal the preview or the forest fails
//! closed. A crash mid-forest resumes from the node's last durable
//! checkpoint and re-runs the forest deterministically — branch
//! workspaces are never resume points.

mod branch;
mod nogood;

use anyhow::{Context, Result};
use perspt_sdk::search::{BranchCandidate, ReservationTicket};
use perspt_sdk::{
    evaluate_gate_with_floor, AcceptedTrajectory, BranchMeasurement, GateDecision, ModelId,
    ResidualClass, SearchLimits, SearchUsage, WitnessRef,
};

use super::node::{seed_from_attempt, CandidateSeed};
use super::{NodeAttempt, Psp9AgentRuntime, Psp9Recorder};
use crate::candidate::CodingCandidateMeasurer;
use crate::toolloop::{LoopEvent, LoopRecorder};
use branch::{measure_fork_cost, BranchRecorder};
use nogood::{NoGoodComponents, NoGoodStore, NoGoodSupport};

/// The sensor-profile identity branch measurements share (Proposition 5:
/// candidates compared by energy must use one immutable profile).
const BRANCH_SENSOR_PROFILE: &str = "coding-suite-v1+perspt-cluster-v1:log-damped";

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
    usage: SearchUsage,
    limits: SearchLimits,
    no_goods: NoGoodStore,
}

impl ForestRun<'_> {
    fn emit(&self, event: LoopEvent) -> Result<()> {
        self.recorder.record(&event)
    }
}

impl Psp9AgentRuntime {
    /// Open a forest at the recovery ladder's refine rung (system 20): the
    /// first branch uses the primary actuator route with the default
    /// strategy; deliberately diverse branches open only on measured
    /// triggers; one candidate commits through the ordinary gate.
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
        let accepted_root = seed
            .map(|seed| seed.expected_state_root.clone())
            .unwrap_or_else(|| format!("unseeded:{node_id}:{generation}"));
        let mut forest = ForestRun {
            recorder,
            forest_id: forest_id.clone(),
            node_id: node_id.to_string(),
            generation,
            accepted_root: accepted_root.clone(),
            baseline_energy,
            rho_gate,
            usage: SearchUsage::default(),
            limits: self.search.limits.clone(),
            no_goods: NoGoodStore::default(),
        };
        forest.emit(LoopEvent::SearchOpened {
            forest_id: forest_id.clone(),
            node_id: node_id.to_string(),
            generation,
            accepted_root: accepted_root.clone(),
            limits: forest.limits.clone(),
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
        forest.usage.close();
        forest.emit(LoopEvent::SearchClosed {
            forest_id,
            usage: forest.usage.clone(),
        })?;
        Ok(chosen)
    }

    /// The sequential branch loop with continuation-before-expansion
    /// (system 20): a failed branch with accepted files seeds the next as
    /// a witnessed partial; otherwise a deliberately diverse fresh branch
    /// opens from the forest seed.
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
        let mut branch_seed: Option<CandidateSeed> = seed.cloned();
        let mut witness = WitnessRef::root(&forest.accepted_root);
        for index in 0..usize::from(self.search.max_branches) {
            let route = routes[index.min(routes.len() - 1)].clone();
            let outcome = self
                .run_one_branch(
                    forest,
                    session_id,
                    goal,
                    graph,
                    branch_seed.as_ref(),
                    &witness,
                    &route,
                    index,
                    remaining_budget,
                )
                .await?;
            let Some((candidate, attempt)) = outcome else {
                break; // budget refused the fork: the forest must close.
            };
            let eligible = candidate_eligible(forest, &candidate);
            let hard = candidate.measurement.hard_pass;
            attempts.push((candidate, attempt, eligible));
            if (eligible && hard) || index + 1 >= usize::from(self.search.max_branches) {
                break;
            }
            let last = &attempts.last().expect("just pushed").1;
            match self.partial_checkpoint(forest, last).await? {
                Some((next_seed, extended)) => {
                    branch_seed = Some(next_seed);
                    witness = extended;
                }
                None => {
                    branch_seed = seed.cloned();
                    witness = WitnessRef::root(&forest.accepted_root);
                }
            }
        }
        Ok(attempts)
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
        goal: &str,
        graph: &perspt_sdk::WorkGraphRevision,
        seed: Option<&CandidateSeed>,
        witness: &WitnessRef,
        route: &ModelId,
        index: usize,
        remaining_budget: u32,
    ) -> Result<Option<(BranchCandidate, NodeAttempt)>> {
        let branch_id = format!("{}/b{}", forest.forest_id, index + 1);
        let strategy_id = if index == 0 {
            "default"
        } else {
            "diverse-route"
        };
        let no_good = self.no_good_components(forest, goal, strategy_id, route);
        if !self.admit_fork(forest, &branch_id, &no_good)? {
            return Ok(None);
        }
        forest.emit(LoopEvent::BranchForked {
            forest_id: forest.forest_id.clone(),
            branch_id: branch_id.clone(),
            parent_branch: (index > 0).then(|| format!("{}/b{index}", forest.forest_id)),
            seed_checkpoint: witness.chain.first().cloned().unwrap_or_default(),
            seed_witness: witness.clone(),
        })?;
        forest.emit(LoopEvent::BranchStrategySelected {
            forest_id: forest.forest_id.clone(),
            branch_id: branch_id.clone(),
            strategy_id: strategy_id.into(),
        })?;
        let branch_recorder = BranchRecorder {
            inner: forest.recorder,
            forest_id: forest.forest_id.clone(),
            branch_id: branch_id.clone(),
        };
        let attempt = self
            .attempt_node_with_recorder(
                forest.recorder,
                session_id,
                goal,
                &forest.node_id,
                forest.generation,
                route,
                graph,
                seed,
                remaining_budget,
                &branch_recorder,
            )
            .await?;
        let candidate = self
            .preview_branch(forest, &branch_id, &attempt, &no_good)
            .await?;
        Ok(Some((candidate, attempt)))
    }

    /// Reserve the fork's eager-copy cost and check the exact no-good
    /// store; a refused reservation or a suppressed duplicate creates no
    /// branch (Gates AC and AB).
    fn admit_fork(
        &self,
        forest: &mut ForestRun<'_>,
        branch_id: &str,
        no_good: &NoGoodComponents,
    ) -> Result<bool> {
        let fork_cost = measure_fork_cost(&self.working_dir);
        let ticket: ReservationTicket = match forest.usage.reserve(&forest.limits, fork_cost) {
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
        if forest.no_goods.suppresses(no_good) {
            forest.usage.release_unused(ticket, Default::default());
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
    ) -> Result<BranchCandidate> {
        let measurer =
            CodingCandidateMeasurer::new(&attempt.candidate, &forest.node_id, forest.generation)
                .with_domain(self.domain.clone())
                .with_max_parallel(self.config.max_parallel_verifiers);
        let measured = crate::toolloop::CandidateMeasurer::measure(&measurer).await?;
        let candidate_id = format!("{}/{}/{branch_id}", forest.node_id, forest.generation);
        let measurement = BranchMeasurement {
            branch_id: branch_id.to_string(),
            candidate_id: candidate_id.clone(),
            energy: measured.energy,
            hard_pass: measured.hard_pass,
            residuals: measured.residuals.clone(),
            sensor_profile: BRANCH_SENSOR_PROFILE.into(),
            cost: f64::from(attempt.outcome.turns_used),
        };
        forest.emit(LoopEvent::BranchCandidateMeasured {
            forest_id: forest.forest_id.clone(),
            branch_id: branch_id.to_string(),
            candidate_id,
            measurement: measurement.clone(),
        })?;
        if !candidate_preview_accepted(forest, &measurement) {
            forest.emit(LoopEvent::BranchIneligible {
                forest_id: forest.forest_id.clone(),
                branch_id: branch_id.to_string(),
                reason: "preview did not accept (no hard pass or measured descent)".into(),
            })?;
            let support = no_good_support(&measured);
            let key = forest.no_goods.record(no_good, &support);
            forest.emit(LoopEvent::NoGoodRecorded {
                forest_id: forest.forest_id.clone(),
                branch_id: branch_id.to_string(),
                key,
                evidence_hash: support.evidence_hash(),
            })?;
        }
        Ok(BranchCandidate {
            measurement,
            targeted_improvement: 0.0,
        })
    }

    /// Persist an ineligible-but-actionable branch as a private partial
    /// checkpoint the next quantum may continue (system 19).
    async fn partial_checkpoint(
        &self,
        forest: &mut ForestRun<'_>,
        attempt: &NodeAttempt,
    ) -> Result<Option<(CandidateSeed, WitnessRef)>> {
        let Some(seed) = seed_from_attempt(forest.recorder, &forest.node_id, attempt).await? else {
            return Ok(None);
        };
        let witness = WitnessRef::root(&forest.accepted_root).extend(&seed.expected_state_root);
        let checkpoint = perspt_sdk::PartialCheckpointRef {
            state_root: seed.expected_state_root.clone(),
            accepted_ancestor: forest.accepted_root.clone(),
            parent_witness: witness.clone(),
            correction: None,
            remaining_obligations: Vec::new(),
            evidence_digest: seed.expected_state_root.clone(),
        };
        forest.emit(LoopEvent::PartialCheckpointed {
            forest_id: forest.forest_id.clone(),
            branch_id: format!("{}/partial", forest.forest_id),
            checkpoint,
        })?;
        Ok(Some((seed, witness)))
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

    /// The exact no-good components for one attempt configuration.
    fn no_good_components(
        &self,
        forest: &ForestRun<'_>,
        goal: &str,
        strategy_id: &str,
        route: &ModelId,
    ) -> NoGoodComponents {
        let digest = |label: &str, value: &str| {
            let mut encoder = perspt_sdk::canon::CanonicalEncoder::new(b"perspt.no-good.v1");
            encoder.text(label).text(value);
            encoder.digest()
        };
        NoGoodComponents {
            accepted_root: forest.accepted_root.clone(),
            domain_digest: digest("domain", &self.domain.domain_id().0),
            strategy_digest: digest("strategy", strategy_id),
            prompt_digest: digest("prompt", &format!("{route}")),
            proposal_digest: digest("proposal", goal),
            grant_digest: digest("grant", &self.working_dir.display().to_string()),
            catalog_digest: digest("catalog", "base+domain"),
            sensor_fingerprint: BRANCH_SENSOR_PROFILE.into(),
            build_digest: digest("build", env!("CARGO_PKG_VERSION")),
        }
    }
}

/// Definition 3 eligibility: required sensors ran (no required-stage
/// `SensorUnavailable`), the witness chain holds by construction, and the
/// pure gate preview accepts.
fn candidate_eligible(forest: &ForestRun<'_>, candidate: &BranchCandidate) -> bool {
    let sensors_ok = !candidate.measurement.residuals.iter().any(|residual| {
        residual.class == ResidualClass::SensorUnavailable
            && residual.evidence.summary.contains("required-stage:")
    });
    sensors_ok && candidate_preview_accepted(forest, &candidate.measurement)
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

/// Deterministic support for a failed attempt's no-good (Gate AB): the
/// dominant compiler code or failed test when present, else the unchanged
/// realized-state hash class.
fn no_good_support(measured: &crate::toolloop::Measured) -> NoGoodSupport {
    for residual in &measured.residuals {
        if residual.class == ResidualClass::TestFailure {
            return NoGoodSupport::FailedTest(residual.evidence.summary.clone());
        }
    }
    if let Some(residual) = measured.residuals.first() {
        return NoGoodSupport::CompilerCode(residual.evidence.summary.clone());
    }
    NoGoodSupport::UnchangedStateHash(format!("V={:.6}", measured.energy))
}
