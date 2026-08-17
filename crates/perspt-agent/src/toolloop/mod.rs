//! The SRBN tool loop (PSP-9 system 6) — Paper II Appendix A Algorithm 1
//! with tool calls substituted for bundle generation.
//!
//! Every model-issued tool call becomes a Paper III proposal; the
//! deterministic admissibility kernel decides whether it may affect the
//! candidate; the gate is evaluated on the re-measured candidate, never on
//! the model's account of it (Paper I Definition 12.2); and every descent
//! and rejection consumes the stated budget (Paper II Lemma 1).
//!
//! The loop is written against `dyn ModelTransport` and cannot name a vendor
//! type or credential — that is Gate S. Executor and measurer are ports too,
//! so the whole loop runs in tests against scripted fixtures without a
//! network.

use anyhow::{Context, Result};
use perspt_sdk::{
    check_full_admissibility, classify_failure, promote, AcceptedTrajectory, BarrierEvaluator,
    CandidateStateWitness, CandidateTransition, Capability, ContextCheckpoint, ContractEvaluator,
    ControlFrame, Conversation, CorrectionDirection, EffectProposal, FailureKind,
    FullAdmissibilityWitness, GateDecision, KernelState, ModelId, ModelTransport,
    NodeTerminalOutcome, ProviderToolCall, RecoveryCascade, ResidualEvent, StateWitness,
    StaticCatalog, ToolCatalog, ToolChoicePolicy, ToolEntry, TurnOutput, VerificationCadence,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::realize::ProjectionMismatch;

mod batch;
mod context;
mod contract;
pub use contract::*;

pub(crate) use context::refold_session_context;
use context::LoopContext;

/// The loop's finite budgets (PSP-9 system 6).
#[derive(Debug, Clone)]
pub struct LoopBudgets {
    /// `N`: maximum model turns.
    pub max_turns: u32,
    /// `M`: maximum tool calls honored in one turn.
    pub max_calls_per_turn: u32,
    /// `B`: Paper II's rejection budget (descents do not consume it).
    pub rejection_budget: u32,
    /// `ρ_gate`: the measured descent tolerance.
    pub rho_gate: f64,
    /// The domain's declared energy floor, if any.
    pub declared_energy_floor: Option<f64>,
    /// Provider-neutral context projection threshold. The live route's
    /// declared context size may lower this bound.
    pub context_soft_limit_chars: usize,
    /// Paper III's one non-replenishing recovery pool for this node.
    pub recovery_budget: u32,
}

impl LoopBudgets {
    /// A configuration permitting an unbounded unchecked interval is
    /// rejected at startup (Paper II Theorem 3).
    pub fn validate(&self, cadence: &VerificationCadence) -> Result<()> {
        anyhow::ensure!(self.max_turns > 0, "turn budget N must be at least 1");
        anyhow::ensure!(
            self.max_calls_per_turn > 0,
            "per-turn call budget M must be at least 1"
        );
        anyhow::ensure!(
            self.rho_gate.is_finite() && self.rho_gate > 0.0,
            "descent tolerance rho_gate must be positive and finite"
        );
        anyhow::ensure!(
            self.context_soft_limit_chars > 0,
            "context soft limit must be finite and nonzero"
        );
        cadence.validate().map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }
}

/// The assembled tool loop for one node generation.
pub struct ToolLoop<'a> {
    pub transport: &'a dyn ModelTransport,
    pub model: ModelId,
    /// Sticky failover chain. A route is consumed only after an observed
    /// transport failure and never selected per turn.
    pub fallback_models: Vec<ModelId>,
    pub catalog: &'a StaticCatalog,
    pub capabilities: Vec<Capability>,
    pub contract: Option<&'a dyn ContractEvaluator>,
    pub barrier: Option<&'a dyn BarrierEvaluator>,
    /// The capability contract's ceiling on one certified increment.
    pub c_c_max: f64,
    pub executor: &'a dyn EffectExecutor,
    pub measurer: &'a dyn CandidateMeasurer,
    pub budgets: LoopBudgets,
    pub cadence: VerificationCadence,
    pub kernel_state: KernelState,
    pub node_id: String,
    pub generation: u32,
    pub recorder: Option<&'a dyn LoopRecorder>,
}

/// Where the last context checkpoint left off, so each new checkpoint covers
/// exactly the log since the previous one instead of claiming the whole
/// history from zero.
#[derive(Debug, Default)]
struct CompactionCursor {
    parent: Option<String>,
    next_from: u64,
}

fn finish(
    outcome: NodeTerminalOutcome,
    trajectory: AcceptedTrajectory,
    log: EventLog,
    projection: ProjectionMismatch,
    turns_used: u32,
    recovery_spent: u32,
) -> LoopOutcome {
    LoopOutcome {
        outcome,
        trajectory,
        events: log.into_events(),
        projection,
        turns_used,
        recovery_spent,
        contained_by_transport: false,
    }
}

/// Loop-lifetime mutable state threaded through every turn.
struct TurnState {
    log: EventLog,
    projection: ProjectionMismatch,
    recovery: RecoveryCascade,
    cursor: CompactionCursor,
    accepted_checkpoint: CandidateCheckpoint,
    trajectory: AcceptedTrajectory,
    turns_used: u32,
}

impl TurnState {
    fn finish(self, outcome: NodeTerminalOutcome) -> LoopOutcome {
        finish(
            outcome,
            self.trajectory,
            self.log,
            self.projection,
            self.turns_used,
            self.recovery.spent,
        )
    }
}

/// Turn-scoped accounting shared by top-level and nested calls.
struct TurnBudget {
    calls_seen: u32,
    mutations: u32,
    immediate_boundary: bool,
    max_mutations: u32,
}

impl TurnBudget {
    fn new(max_mutations: u32) -> Self {
        Self {
            calls_seen: 0,
            mutations: 0,
            immediate_boundary: false,
            max_mutations,
        }
    }
}

/// Marker error: the transport recovery chain is exhausted (no retry or
/// eligible fallback absorbed a provider failure). Containment caused by
/// this error is an infrastructure outcome, not a governance anomaly, and
/// preserves the session's authority so `perspt resume` stays viable.
#[derive(Debug)]
pub(crate) struct TransportExhausted {
    detail: String,
}

impl std::fmt::Display for TransportExhausted {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl std::error::Error for TransportExhausted {}

/// Whether an error chain bottoms out in exhausted transport recovery.
pub(crate) fn is_transport_exhaustion(error: &anyhow::Error) -> bool {
    error
        .chain()
        .any(|cause| cause.downcast_ref::<TransportExhausted>().is_some())
}

/// One loop iteration's verdict after a gate boundary.
enum BoundaryStep {
    Terminal(NodeTerminalOutcome),
    Continue,
    Exhausted,
}

impl ToolLoop<'_> {
    /// Run the loop to a classified terminal state (Paper II Lemma 1).
    pub async fn run(self, goal: &str) -> Result<LoopOutcome> {
        self.run_with_conversation(goal, None, Vec::new()).await
    }

    /// Re-enter from a durable provider-neutral conversation projection. The
    /// runtime restores candidate state and budgets; this method restores only
    /// model context and deferred tool activation.
    pub async fn run_with_conversation(
        mut self,
        goal: &str,
        resumed: Option<Conversation>,
        restored_activated_tools: Vec<String>,
    ) -> Result<LoopOutcome> {
        self.budgets.validate(&self.cadence)?;
        let mut log = EventLog::new(self.recorder.is_none());

        // 1. Measured baseline and its real restore point.
        let accepted_checkpoint = self.executor.checkpoint(&[]).await?;
        let (baseline, trajectory) = self.measured_baseline(&mut log).await?;
        let mut state = TurnState {
            log,
            projection: ProjectionMismatch::default(),
            recovery: RecoveryCascade::new(self.budgets.recovery_budget),
            cursor: CompactionCursor::default(),
            accepted_checkpoint,
            trajectory,
            turns_used: 0,
        };
        if let Some(outcome) = self.baseline_terminal(&baseline) {
            return Ok(state.finish(outcome));
        }
        let mut context = self.open_context(goal, resumed, restored_activated_tools, &mut state)?;

        let mut mutations_since_boundary = 0u32;
        for turn in 1..=self.budgets.max_turns {
            state.turns_used = turn;
            let turn_result = self
                .model_turn(turn, mutations_since_boundary, &mut context, &mut state)
                .await;
            let (output, mutations, immediate_boundary) = match turn_result {
                Ok(result) => result,
                Err(error) => {
                    let transport = is_transport_exhaustion(&error);
                    let outcome = self
                        .contain(&error, &state.accepted_checkpoint, &mut state.log)
                        .await?;
                    let mut contained = state.finish(outcome);
                    contained.contained_by_transport = transport;
                    return Ok(contained);
                }
            };
            mutations_since_boundary = mutations_since_boundary.saturating_add(mutations);
            if !self.boundary_due(&output, immediate_boundary, mutations_since_boundary, turn) {
                continue;
            }
            mutations_since_boundary = 0;

            match self
                .boundary_step(goal, turn, &mut state, &mut context)
                .await?
            {
                BoundaryStep::Terminal(outcome) => return Ok(state.finish(outcome)),
                BoundaryStep::Exhausted => break,
                BoundaryStep::Continue => {}
            }
        }

        let outcome = NodeTerminalOutcome::Escalated {
            certificate_id: uuid::Uuid::new_v4().to_string(),
        };
        Ok(state.finish(outcome))
    }

    /// Seed a fresh model context or re-enter a resumed one.
    fn open_context(
        &self,
        goal: &str,
        resumed: Option<Conversation>,
        restored_activated_tools: Vec<String>,
        state: &mut TurnState,
    ) -> Result<LoopContext> {
        let mut restored_tools = restored_activated_tools;
        if let Some(conversation) = resumed.as_ref() {
            restored_tools.extend(activated_tools_from_conversation(conversation));
        }
        match resumed {
            Some(conversation) => {
                LoopContext::resume(conversation, restored_tools, self.recorder, &mut state.log)
            }
            None => LoopContext::seed(
                "You are a governed coding agent. Propose tool calls; every effect is mediated.",
                goal,
                self.recorder,
                &mut state.log,
            ),
        }
    }

    /// Unconditional containment: restore the last accepted state and
    /// escalate (Theorem 6's terminal class for an unrecoverable turn).
    async fn contain(
        &self,
        error: &anyhow::Error,
        accepted_checkpoint: &CandidateCheckpoint,
        log: &mut EventLog,
    ) -> Result<NodeTerminalOutcome> {
        self.executor.restore(accepted_checkpoint).await?;
        emit(
            self.recorder,
            log,
            LoopEvent::RecoveryContained {
                reason: error.to_string(),
                restored_checkpoint_id: accepted_checkpoint.id.clone(),
            },
        )?;
        Ok(NodeTerminalOutcome::Escalated {
            certificate_id: uuid::Uuid::new_v4().to_string(),
        })
    }

    /// Measure the baseline candidate and open the accepted trajectory.
    async fn measured_baseline(
        &mut self,
        log: &mut EventLog,
    ) -> Result<(Measured, AcceptedTrajectory)> {
        let baseline = self.measurer.measure().await?;
        emit(
            self.recorder,
            log,
            LoopEvent::CandidateMeasured {
                node_id: self.node_id.clone(),
                generation: self.generation,
                energy: baseline.energy,
                hard_pass: baseline.hard_pass,
                residuals: baseline.residuals.clone(),
            },
        )?;
        let trajectory = AcceptedTrajectory::new(
            "toolloop",
            0,
            baseline.energy,
            self.budgets.rho_gate,
            self.budgets.rejection_budget,
        )?;
        Ok((baseline, trajectory))
    }

    /// Gate the measured candidate: accept (new checkpoint), restore on
    /// rejection, classify terminals, and otherwise steer the next turn with
    /// a directed correction (Lemma 1: descent and rejection are
    /// non-terminal).
    async fn boundary_step(
        &mut self,
        goal: &str,
        turn: u32,
        state: &mut TurnState,
        context: &mut LoopContext,
    ) -> Result<BoundaryStep> {
        let (measured, decision) = self
            .measure_and_gate(&mut state.trajectory, &mut state.log)
            .await?;

        let accepted = decision.is_accepted();
        if accepted {
            state.accepted_checkpoint = self.executor.checkpoint(&[]).await?;
        } else if !matches!(decision, GateDecision::StoppedAtDeclaredFloor) {
            self.executor.restore(&state.accepted_checkpoint).await?;
            emit(
                self.recorder,
                &mut state.log,
                LoopEvent::CandidateRestored {
                    checkpoint_id: state.accepted_checkpoint.id.clone(),
                },
            )?;
        }

        if let Some(outcome) = self.classify_decision(&decision, &measured) {
            if accepted {
                self.durable_checkpoint(goal, turn, &measured, state, context)
                    .await?;
            }
            return Ok(BoundaryStep::Terminal(outcome));
        }
        push_correction(context, &measured, accepted, self.recorder, &mut state.log)?;
        self.maybe_compact(goal, turn, &measured, state, context)?;
        if accepted {
            self.durable_checkpoint(goal, turn, &measured, state, context)
                .await?;
        } else {
            if state.trajectory.budget_exhausted() {
                return Ok(BoundaryStep::Exhausted);
            }
            let granted = state
                .recovery
                .grant(classify_failure(FailureKind::GateRejection));
            emit(
                self.recorder,
                &mut state.log,
                LoopEvent::RecoveryControlGranted {
                    failure: FailureKind::GateRejection,
                    level: granted.level,
                    forced_escalation: granted.forced_escalation,
                    model: self.model.clone(),
                },
            )?;
            if granted.level > perspt_sdk::CascadeLevel::Retry {
                return Ok(BoundaryStep::Exhausted);
            }
        }
        Ok(BoundaryStep::Continue)
    }

    /// Send the conversation, consuming the sticky failover chain on observed
    /// transport failures (a route is never selected per turn).
    async fn chat_with_failover(
        &mut self,
        conversation: &Conversation,
        specs: &[perspt_sdk::ToolSpec],
        recovery: &mut RecoveryCascade,
        log: &mut EventLog,
    ) -> Result<TurnOutput> {
        loop {
            match self
                .transport
                .chat_turn(&self.model, conversation, specs, ToolChoicePolicy::Auto)
                .await
            {
                Ok(output) => return Ok(output),
                Err(error) => {
                    let cause = error.to_string();
                    let lowered = cause.to_ascii_lowercase();
                    let rate_limited = ["rate limit", "429", "too many requests"]
                        .iter()
                        .any(|marker| lowered.contains(marker));
                    let failure = if rate_limited {
                        FailureKind::ProviderRateLimit
                    } else {
                        FailureKind::ProviderTransport
                    };
                    let granted = recovery.grant(classify_failure(failure));
                    emit(
                        self.recorder,
                        log,
                        LoopEvent::RecoveryControlGranted {
                            failure,
                            level: granted.level,
                            forced_escalation: granted.forced_escalation,
                            model: self.model.clone(),
                        },
                    )?;
                    if granted.level != perspt_sdk::CascadeLevel::Fallback || granted.terminal {
                        return Err(anyhow::Error::new(TransportExhausted {
                            detail: format!(
                                "transport recovery reached {:?}: {cause}",
                                granted.level
                            ),
                        }));
                    }
                    let Some(next) = self.fallback_models.first().cloned() else {
                        return Err(anyhow::Error::new(TransportExhausted {
                            detail: format!("transport recovery has no eligible fallback: {cause}"),
                        }));
                    };
                    self.fallback_models.remove(0);
                    let previous = std::mem::replace(&mut self.model, next.clone());
                    emit(
                        self.recorder,
                        log,
                        LoopEvent::RouteFailover {
                            from_model: previous,
                            to_model: next,
                            cause,
                        },
                    )?;
                }
            }
        }
    }

    /// Measure boundary: text turn, cadence bound `H`, or turn budget.
    fn boundary_due(
        &self,
        output: &TurnOutput,
        immediate_boundary: bool,
        mutations_since_boundary: u32,
        turn: u32,
    ) -> bool {
        matches!(output, TurnOutput::Text(_))
            || immediate_boundary
            || mutations_since_boundary >= self.cadence.max_mutations_between_checks
            || turn == self.budgets.max_turns
    }

    /// One model turn: send the conversation, record the observation, and
    /// route every returned call through the kernel. `mutations_so_far` is
    /// the count since the last boundary; the cadence bound `H` caps what
    /// this turn may add.
    async fn model_turn(
        &mut self,
        turn: u32,
        mutations_so_far: u32,
        context: &mut LoopContext,
        state: &mut TurnState,
    ) -> Result<(TurnOutput, u32, bool)> {
        let specs =
            self.catalog
                .deferred_specs_for(&self.capabilities, context.activated_tools(), false);
        let conversation = context.conversation().clone();
        let output = self
            .chat_with_failover(&conversation, &specs, &mut state.recovery, &mut state.log)
            .await?;
        // R2: record the observation before inspecting it.
        emit(
            self.recorder,
            &mut state.log,
            LoopEvent::TurnObserved {
                turn,
                output: output.clone(),
            },
        )?;
        let max_mutations = self
            .cadence
            .max_mutations_between_checks
            .saturating_sub(mutations_so_far)
            .max(1);
        let (mutations, immediate_boundary) = self
            .execute_turn(
                &output,
                max_mutations,
                context,
                &mut state.log,
                &mut state.projection,
            )
            .await?;
        Ok((output, mutations, immediate_boundary))
    }

    /// Make a gate acceptance durably resumable (PSP-9 phase 6): export the
    /// accepted candidate's mutated contents as content-addressed artifacts
    /// and ledger them with the exact control frame, so a crashed loop can be
    /// continued from this acceptance instead of restarted.
    async fn durable_checkpoint(
        &self,
        goal: &str,
        turn: u32,
        measured: &Measured,
        state: &mut TurnState,
        context: &LoopContext,
    ) -> Result<()> {
        let Some(recorder) = self.recorder else {
            return Ok(());
        };
        let authority_epoch = self
            .kernel_state
            .witnesses
            .get("__authority_epoch")
            .map(String::as_str)
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        let control = self.control_frame(goal, turn, measured, state, context, authority_epoch);
        let exported = self.executor.export_accepted().await?;
        let mut files = Vec::new();
        for seed in exported {
            let content_artifact = seed
                .content
                .as_deref()
                .map(|bytes| recorder.record_artifact(bytes, "application/octet-stream"))
                .transpose()?;
            let source_preimage_artifact = seed
                .source_preimage
                .as_deref()
                .map(|bytes| recorder.record_artifact(bytes, "application/octet-stream"))
                .transpose()?;
            files.push(DurableSeedFile {
                path: seed.path,
                content_artifact,
                source_preimage_artifact,
            });
        }
        emit(
            self.recorder,
            &mut state.log,
            LoopEvent::DurableCandidateCheckpoint {
                state_root: state.accepted_checkpoint.witness.state_root.clone(),
                control,
                conversation: context.conversation().clone(),
                canonical_scope: state.accepted_checkpoint.witness.canonical_scope.clone(),
                files,
            },
        )
    }

    /// The verbatim control frame a compaction must preserve (resolved
    /// design decision 3): goal, accepted state binding, live authority, and
    /// exact remaining budgets.
    fn control_frame(
        &self,
        goal: &str,
        turn: u32,
        measured: &Measured,
        state: &TurnState,
        context: &LoopContext,
        authority_epoch: u64,
    ) -> ControlFrame {
        ControlFrame {
            projection_digest: context.digest().to_string(),
            event_schema_version: perspt_sdk::CONVERSATION_EVENT_SCHEMA_VERSION,
            goal: goal.to_string(),
            node_generation: self.generation,
            accepted_state_root: state.accepted_checkpoint.witness.state_root.clone(),
            graph_revision: state.accepted_checkpoint.witness.graph_revision.clone(),
            capability_ids: self
                .capabilities
                .iter()
                .map(|capability| capability.capability_id.clone())
                .collect(),
            authority_epoch,
            remaining_rejection_budget: state
                .trajectory
                .rejection_budget
                .saturating_sub(state.trajectory.rejections_used.max(state.recovery.spent)),
            remaining_turns: self.budgets.max_turns.saturating_sub(turn),
            active_model: self.model.clone(),
            remaining_fallback_models: self.fallback_models.clone(),
            activated_tools: context.activated_tools().iter().cloned().collect(),
            unresolved_call_ids: context.conversation().unresolved_call_ids(),
            residual_summary: measured
                .residuals
                .iter()
                .map(|residual| (format!("{:?}", residual.class), residual.score))
                .collect(),
        }
    }

    fn maybe_compact(
        &self,
        goal: &str,
        turn: u32,
        measured: &Measured,
        state: &mut TurnState,
        context: &mut LoopContext,
    ) -> Result<()> {
        let route_limit = self.transport.capabilities(&self.model).max_context_tokens;
        let route_chars = usize::try_from(route_limit)
            .unwrap_or(usize::MAX)
            .saturating_mul(3);
        let threshold = if route_chars == 0 {
            self.budgets.context_soft_limit_chars
        } else {
            self.budgets.context_soft_limit_chars.min(route_chars)
        };
        if context.conversation().estimated_chars() <= threshold {
            return Ok(());
        }

        let authority_epoch = self
            .kernel_state
            .witnesses
            .get("__authority_epoch")
            .map(String::as_str)
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        let control = self.control_frame(goal, turn, measured, state, context, authority_epoch);
        // The rolling chain root commits to every event so far in O(1); the
        // cursor makes each checkpoint cover exactly the span since the
        // previous one instead of claiming the whole history from zero.
        let covered_root = state.log.chain_root().to_string();
        let checkpoint = ContextCheckpoint {
            parent: state.cursor.parent.clone(),
            covered_from: state.cursor.next_from,
            covered_to: state.log.count().saturating_sub(1),
            covered_event_root: covered_root.clone(),
            control,
            artifact_refs: vec![state.accepted_checkpoint.witness.state_root.clone()],
            narrative_observation: None,
        };
        checkpoint
            .validate_against(
                &state.accepted_checkpoint.witness.state_root,
                &state.accepted_checkpoint.witness.graph_revision,
                authority_epoch,
            )
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        emit(
            self.recorder,
            &mut state.log,
            LoopEvent::ContextCheckpointCreated {
                checkpoint: checkpoint.clone(),
            },
        )?;
        let control_json = serde_json::to_string(&checkpoint.control)?;
        context.compact(
            format!("PERSPECTIVE_CONTROL_FRAME_V1\n{control_json}"),
            self.recorder,
            &mut state.log,
        )?;
        state.cursor.parent = Some(covered_root);
        state.cursor.next_from = checkpoint.covered_to.saturating_add(1);
        Ok(())
    }

    /// Measure the realized candidate and submit it to the gate.
    async fn measure_and_gate(
        &mut self,
        trajectory: &mut AcceptedTrajectory,
        log: &mut EventLog,
    ) -> Result<(Measured, GateDecision)> {
        let measured = self.measurer.measure().await?;
        emit(
            self.recorder,
            log,
            LoopEvent::CandidateMeasured {
                node_id: self.node_id.clone(),
                generation: self.generation,
                energy: measured.energy,
                hard_pass: measured.hard_pass,
                residuals: measured.residuals.clone(),
            },
        )?;
        let decision = trajectory.submit_with_floor(
            measured.hard_pass,
            measured.energy,
            self.budgets.declared_energy_floor,
        )?;
        emit(
            self.recorder,
            log,
            LoopEvent::GateDecisionRecorded {
                node_id: self.node_id.clone(),
                generation: self.generation,
                decision: decision.clone(),
            },
        )?;
        Ok((measured, decision))
    }

    /// Terminal classification of the measured baseline, if any.
    fn baseline_terminal(&self, baseline: &Measured) -> Option<NodeTerminalOutcome> {
        if baseline.hard_pass {
            return Some(NodeTerminalOutcome::HardPass);
        }
        if let Some(floor) = self.budgets.declared_energy_floor {
            if baseline.energy <= floor {
                return Some(NodeTerminalOutcome::DeclaredFloor { floor });
            }
        }
        None
    }

    /// Map a gate decision to a terminal outcome, or `None` to continue.
    fn classify_decision(
        &self,
        decision: &GateDecision,
        measured: &Measured,
    ) -> Option<NodeTerminalOutcome> {
        match decision {
            GateDecision::HardPass => Some(NodeTerminalOutcome::HardPass),
            GateDecision::StoppedAtDeclaredFloor => {
                let floor = self
                    .budgets
                    .declared_energy_floor
                    .unwrap_or(measured.energy);
                Some(NodeTerminalOutcome::DeclaredFloor { floor })
            }
            _ => None,
        }
    }

    /// Record a denial as evidence: counted in the projection mismatch,
    /// ledgered with its typed residual class, and returned to the model.
    fn deny(
        &self,
        log: &mut EventLog,
        projection: &mut ProjectionMismatch,
        call_id: &str,
        reason: String,
        class: perspt_sdk::ResidualClass,
    ) -> Result<String> {
        projection.denied_proposals += 1;
        emit(
            self.recorder,
            log,
            LoopEvent::EffectDenied {
                call_id: call_id.to_string(),
                reason: reason.clone(),
                class,
            },
        )?;
        Ok(format!("denied: {reason}"))
    }

    /// Per-turn budget denials (Gate J: a call above the budget still gets a
    /// recorded proposal-and-result pair; it is denied, not silently
    /// dropped). Returns `None` when the call is within budget.
    #[allow(clippy::too_many_arguments)]
    fn budget_denial(
        &self,
        ordinal: u32,
        mutating: bool,
        mutations: u32,
        max_mutations: u32,
    ) -> Option<String> {
        if ordinal >= self.budgets.max_calls_per_turn {
            return Some("per-turn tool-call budget exceeded".into());
        }
        if mutating && mutations >= max_mutations {
            return Some("verification boundary required before another mutation".into());
        }
        None
    }

    async fn record_unchecked_proposal(
        &self,
        call: &ProviderToolCall,
        entry: &ToolEntry,
        log: &mut EventLog,
    ) -> Result<()> {
        let before = self.executor.checkpoint(&[]).await?;
        let proposal = proposal_from(call, entry, &self.node_id, self.generation, &before.witness);
        emit(
            self.recorder,
            log,
            LoopEvent::ProposalObserved {
                call_id: call.call_id.clone(),
                proposal,
            },
        )
    }

    fn high_risk(entry: Option<&ToolEntry>) -> bool {
        entry.is_some_and(|entry| {
            matches!(
                entry.risk,
                perspt_sdk::RiskClass::High | perspt_sdk::RiskClass::Critical
            )
        })
    }

    /// Execute one certified non-mutating call: the host-side tool surface
    /// (`tool_search`, `tool_program` validation) or the sandboxed executor.
    async fn apply_non_mutating(
        &self,
        call: &ProviderToolCall,
        entry: &ToolEntry,
    ) -> Result<String> {
        if call.name == "tool_search" {
            let query = call
                .arguments
                .get("query")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let limit = call
                .arguments
                .get("limit")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or(8);
            let matches = self
                .catalog
                .search_specs(&self.capabilities, query, limit, false);
            return Ok(serde_json::to_string(&matches)?);
        }
        if call.name == "tool_program" {
            let source = call
                .arguments
                .get("source")
                .and_then(serde_json::Value::as_str)
                .context("tool_program requires string source")?;
            let calls = perspt_policy::evaluate_tool_program(
                source,
                perspt_policy::ToolProgramLimits::default(),
            )?;
            return Ok(serde_json::to_string(&calls)?);
        }
        Ok(self.executor.apply(call, entry).await?.output)
    }

    /// Evaluate all five clauses for a transition and ledger the witness.
    fn certify(
        &self,
        call_id: &str,
        transition: &CandidateTransition,
        log: &mut EventLog,
    ) -> Result<FullAdmissibilityWitness> {
        let witness = check_full_admissibility(
            transition,
            &self.capabilities,
            &self.kernel_state,
            self.contract,
            self.barrier,
            self.c_c_max,
        )
        .map_err(|e| anyhow::anyhow!("kernel: {e}"))?;
        emit(
            self.recorder,
            log,
            LoopEvent::ProposalChecked {
                call_id: call_id.to_string(),
                proposal: transition.proposal.clone(),
                witness: Box::new(witness.clone()),
            },
        )?;
        Ok(witness)
    }

    /// Kernel check, promote, and execute one call.
    ///
    /// Both effect classes are certified by exactly one evaluator
    /// (`check_full_admissibility`): non-mutating effects on the identity
    /// transition before any process launches, mutating effects again on the
    /// realized transition after the reversible overlay was touched.
    async fn check_and_apply(
        &mut self,
        call: &ProviderToolCall,
        log: &mut EventLog,
        projection: &mut ProjectionMismatch,
        mutations: &mut u32,
    ) -> Result<String> {
        let Some(entry) = self.catalog.lookup(&call.name).cloned() else {
            return self.deny(
                log,
                projection,
                &call.call_id,
                format!("unknown tool {:?}", call.name),
                perspt_sdk::ResidualClass::CapabilityDenied,
            );
        };
        let scope = proposal_scope(call, &entry);
        let before = self.executor.checkpoint(&scope).await?;
        let proposal = proposal_from(
            call,
            &entry,
            &self.node_id,
            self.generation,
            &before.witness,
        );
        emit(
            self.recorder,
            log,
            LoopEvent::ProposalObserved {
                call_id: call.call_id.clone(),
                proposal: proposal.clone(),
            },
        )?;
        self.kernel_state
            .set_witness("__candidate_root", before.witness.state_root.clone());

        // Full five-clause certification on the identity transition before
        // even the reversible candidate overlay is touched.
        let identity = CandidateTransition::new(
            proposal.clone(),
            before.witness.clone(),
            before.witness.clone(),
        );
        let witness = self.certify(&call.call_id, &identity, log)?;
        if let Some(reason) = uncertified_reason(&witness) {
            return self.deny(
                log,
                projection,
                &call.call_id,
                reason,
                perspt_sdk::ResidualClass::CapabilityDenied,
            );
        }

        if !candidate_mutating_effect(entry.effect) {
            // Reads and governed verifier processes do not change the logical
            // candidate state, so the identity certification above is their
            // complete Def. 3.2 witness; no external process launches on a
            // partial one.
            promote_matching_capability(&mut self.capabilities, &witness)
                .map_err(|error| anyhow::anyhow!("promotion: {error}"))?;
            let output = self.apply_non_mutating(call, &entry).await?;
            let output = bounded_model_output(self.recorder, output)?;
            emit(
                self.recorder,
                log,
                LoopEvent::EffectApplied {
                    call_id: call.call_id.clone(),
                    mutated: false,
                    output: output.clone(),
                },
            )?;
            return Ok(output);
        }

        self.apply_mutating(call, &entry, proposal, &before, log, projection, mutations)
            .await
    }

    /// R5 bracketing: a durable effect's intent is ledgered before it
    /// runs, so an interruption leaves a visible open bracket.
    fn open_effect_bracket(
        &self,
        call: &ProviderToolCall,
        entry: &ToolEntry,
    ) -> Result<Option<String>> {
        let Some(key) = entry
            .durable
            .then(|| format!("tool:{}:{}", call.name, call.call_id))
        else {
            return Ok(None);
        };
        if let Some(recorder) = self.recorder {
            recorder.external_intent(
                &key,
                &serde_json::json!({
                    "tool": call.name,
                    "call_id": call.call_id,
                    "arguments": call.arguments,
                    "node_id": self.node_id,
                    "generation": self.generation,
                }),
            )?;
        }
        Ok(Some(key))
    }

    /// Apply a mutating call to the reversible overlay, certify the realized
    /// transition, and either keep it (debiting the capability) or restore.
    #[allow(clippy::too_many_arguments)]
    async fn apply_mutating(
        &mut self,
        call: &ProviderToolCall,
        entry: &ToolEntry,
        proposal: EffectProposal,
        before: &CandidateCheckpoint,
        log: &mut EventLog,
        projection: &mut ProjectionMismatch,
        mutations: &mut u32,
    ) -> Result<String> {
        let bracket_key = self.open_effect_bracket(call, entry)?;
        let outcome = self.executor.apply(call, entry).await?;
        if let (Some(recorder), Some(key)) = (self.recorder, bracket_key.as_deref()) {
            recorder.external_result(key, &serde_json::json!({"mutated": outcome.mutated}))?;
        }
        let after = self.executor.state_witness().await?;
        let transition = CandidateTransition::new(proposal, before.witness.clone(), after);
        let witness = self.certify(&call.call_id, &transition, log)?;
        let output = bounded_model_output(self.recorder, outcome.output)?;

        if let Some(reason) = uncertified_reason(&witness) {
            self.executor.restore(before).await?;
            // Denials are evidence, not errors: returned to the model so the
            // loop can adapt, and recorded for the ledger.
            return self.deny(
                log,
                projection,
                &call.call_id,
                reason,
                perspt_sdk::ResidualClass::CapabilityDenied,
            );
        }

        // The debit and the effect stand or fall together: a failed promotion
        // restores the overlay and consumes no capability budget.
        if let Err(e) = promote_matching_capability(&mut self.capabilities, &witness) {
            self.executor.restore(before).await?;
            return self.deny(
                log,
                projection,
                &call.call_id,
                format!("promotion: {e}"),
                perspt_sdk::ResidualClass::CapabilityDenied,
            );
        }
        if outcome.mutated {
            *mutations += 1;
            let boundary = self.measurer.measure_incremental().await?;
            emit(
                self.recorder,
                log,
                LoopEvent::EffectBoundaryMeasured {
                    call_id: call.call_id.clone(),
                    node_id: self.node_id.clone(),
                    generation: self.generation,
                    energy: boundary.energy,
                    hard_pass: boundary.hard_pass,
                    residuals: boundary.residuals,
                },
            )?;
        }
        emit(
            self.recorder,
            log,
            LoopEvent::EffectApplied {
                call_id: call.call_id.clone(),
                mutated: outcome.mutated,
                output: output.clone(),
            },
        )?;
        Ok(output)
    }
}

const MODEL_OUTPUT_PREVIEW_BYTES: usize = 8 * 1024;

fn bounded_model_output(recorder: Option<&dyn LoopRecorder>, output: String) -> Result<String> {
    if output.len() <= MODEL_OUTPUT_PREVIEW_BYTES || recorder.is_none() {
        return Ok(output);
    }
    let recorder = recorder.expect("checked above");
    let handle = recorder.record_artifact(output.as_bytes(), "text/plain; charset=utf-8")?;
    let mut boundary = MODEL_OUTPUT_PREVIEW_BYTES;
    while !output.is_char_boundary(boundary) {
        boundary -= 1;
    }
    Ok(format!(
        "{}\n[full output: artifact:{handle}; {} bytes]",
        &output[..boundary],
        output.len()
    ))
}

fn activated_tools_from_conversation(conversation: &Conversation) -> BTreeSet<String> {
    let mut search_calls = BTreeSet::new();
    let mut activated = BTreeSet::new();
    for message in conversation.messages() {
        match message {
            perspt_sdk::Message::AssistantToolCalls { calls } => {
                search_calls.extend(
                    calls
                        .iter()
                        .filter(|call| call.name == "tool_search")
                        .map(|call| call.call_id.clone()),
                );
            }
            perspt_sdk::Message::ToolResponse { call_id, content }
                if search_calls.contains(call_id) =>
            {
                if let Ok(specs) = serde_json::from_str::<Vec<perspt_sdk::ToolSpec>>(content) {
                    activated.extend(specs.into_iter().map(|spec| spec.name));
                }
            }
            _ => {}
        }
    }
    activated
}

/// Paths a call names, for the checkpoint scope. An entry that declares
/// `proposal_bindings` is authoritative; the conventional `path`/`to`/`from`
/// fields cover the builtins, which declare none.
fn proposal_scope(call: &ProviderToolCall, entry: &ToolEntry) -> Vec<String> {
    if entry.proposal_bindings.is_empty() {
        return ["path", "to", "from"]
            .iter()
            .filter_map(|field| call.arguments.get(*field).and_then(|v| v.as_str()))
            .map(str::to_string)
            .collect();
    }
    let mut scope = Vec::new();
    for binding in &entry.proposal_bindings {
        match binding {
            perspt_sdk::ProposalBinding::Path { field } => {
                if let Some(path) = call.arguments.get(field).and_then(|v| v.as_str()) {
                    scope.push(path.to_string());
                }
            }
            perspt_sdk::ProposalBinding::MultiValue { field, target }
                if *target == perspt_sdk::MultiValueTarget::Path =>
            {
                scope.extend(string_array(call, field));
            }
            _ => {}
        }
    }
    scope
}

/// The string elements of a schema-validated scalar array argument.
fn string_array(call: &ProviderToolCall, field: &str) -> Vec<String> {
    call.arguments
        .get(field)
        .and_then(|v| v.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|v| v.as_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// The one definition of "this effect mutates the candidate overlay", shared
/// by the loop's accounting and the executor's journal so they cannot drift.
pub fn candidate_mutating_effect(effect: perspt_sdk::EffectKind) -> bool {
    matches!(
        effect,
        perspt_sdk::EffectKind::WriteArtifact
            | perspt_sdk::EffectKind::ApplyPatch
            | perspt_sdk::EffectKind::MoveFile
            | perspt_sdk::EffectKind::DeleteFile
            | perspt_sdk::EffectKind::MutateDependencies
    )
}

/// `Some(reason)` when a witness does not certify autonomous commitment.
fn uncertified_reason(witness: &FullAdmissibilityWitness) -> Option<String> {
    if witness.allows() && witness.profile == perspt_sdk::AdmissibilityProfile::SrbnCertified {
        return None;
    }
    Some(if witness.allows() {
        format!(
            "admissibility profile {:?} is not autonomously committable",
            witness.profile
        )
    } else {
        format!("{:?}", witness.base.decision)
    })
}

fn promote_matching_capability(
    capabilities: &mut [Capability],
    witness: &FullAdmissibilityWitness,
) -> Result<()> {
    let capability_id = witness.base.capability_id.as_ref();
    let capability = capabilities
        .iter_mut()
        .find(|capability| Some(&capability.capability_id) == capability_id)
        .context("admissibility witness references a missing capability")?;
    let mut promoted = capability.clone();
    promote(&mut promoted, witness).map_err(|error| anyhow::anyhow!(error.to_string()))?;
    *capability = promoted;
    Ok(())
}

/// Canonicalize one provider call into an effect proposal (PSP-9 system 12).
/// Provenance (which model proposed it) is recorded in the ledger, never in
/// the kernel's input, so no admissibility decision can depend on a vendor.
fn proposal_from(
    call: &ProviderToolCall,
    entry: &ToolEntry,
    node_id: &str,
    generation: u32,
    before: &CandidateStateWitness,
) -> EffectProposal {
    let mut proposal =
        EffectProposal::new(perspt_sdk::ActorId::new("toolloop"), node_id, entry.effect)
            .with_generation(generation)
            .with_risk_class(entry.risk)
            .with_idempotency_key(format!("{}:{}", call.name, call.arguments))
            .with_preconditions(vec![StateWitness {
                resource: "__candidate_root".into(),
                content_hash: before.state_root.clone(),
            }]);
    if entry.proposal_bindings.is_empty() {
        // Builtins: conventional field names.
        if let Some(path) = call.arguments.get("path").and_then(|v| v.as_str()) {
            proposal = proposal.with_path(path);
        }
        if let Some(command) = call.arguments.get("command").and_then(|v| v.as_str()) {
            proposal = proposal.with_command(perspt_sdk::canonicalize(command, "."));
        }
        if let Some(path) = call.arguments.get("to").and_then(|v| v.as_str()) {
            proposal = proposal.with_additional_paths(vec![path.to_string()]);
        }
        if let Some(url) = call.arguments.get("url").and_then(|v| v.as_str()) {
            proposal = proposal.with_network_target(url);
        }
        return proposal;
    }
    bind_declared(proposal, call, entry)
}

/// Bind a registered entry's declared proposal channels; the loop holds no
/// per-tool field knowledge.
fn bind_declared(
    mut proposal: EffectProposal,
    call: &ProviderToolCall,
    entry: &ToolEntry,
) -> EffectProposal {
    let mut primary_path_bound = false;
    for binding in &entry.proposal_bindings {
        match binding {
            perspt_sdk::ProposalBinding::Path { field } => {
                if let Some(path) = call.arguments.get(field).and_then(|v| v.as_str()) {
                    if primary_path_bound {
                        proposal = proposal.with_additional_paths(vec![path.to_string()]);
                    } else {
                        proposal = proposal.with_path(path);
                        primary_path_bound = true;
                    }
                }
            }
            perspt_sdk::ProposalBinding::Command { field } => {
                if let Some(command) = call.arguments.get(field).and_then(|v| v.as_str()) {
                    proposal = proposal.with_command(perspt_sdk::canonicalize(command, "."));
                }
            }
            perspt_sdk::ProposalBinding::Url { field } => {
                if let Some(url) = call.arguments.get(field).and_then(|v| v.as_str()) {
                    proposal = proposal.with_network_target(url);
                }
            }
            perspt_sdk::ProposalBinding::MultiValue { field, target } => {
                let values = string_array(call, field);
                match target {
                    perspt_sdk::MultiValueTarget::Path => {
                        proposal = proposal.with_additional_paths(values);
                    }
                    perspt_sdk::MultiValueTarget::Command => {
                        for value in values {
                            proposal = proposal.with_command(perspt_sdk::canonicalize(&value, "."));
                        }
                    }
                    perspt_sdk::MultiValueTarget::Url => {
                        for value in values {
                            proposal = proposal.with_network_target(&value);
                        }
                    }
                }
            }
        }
    }
    proposal
}

fn push_correction(
    context: &mut LoopContext,
    measured: &Measured,
    accepted: bool,
    recorder: Option<&dyn LoopRecorder>,
    log: &mut EventLog,
) -> Result<()> {
    let instruction = measured
        .correction
        .as_ref()
        .map(|c| c.instruction.clone())
        .unwrap_or_else(|| {
            if accepted {
                format!(
                    "The candidate descended to V = {:.3}. Continue addressing the remaining residuals.",
                    measured.energy
                )
            } else {
                format!(
                    "The candidate did not descend (V = {:.3}). Address the dominant residual.",
                    measured.energy
                )
            }
        });
    context.push_user(instruction, recorder, log)
}

/// The finite decision bound the loop must respect (logged at node entry;
/// exceeding it fails the run — Gate M).
pub fn loop_decision_bound(baseline_energy: f64, budgets: &LoopBudgets) -> Result<u64> {
    perspt_sdk::finite_decision_bound(baseline_energy, budgets.rho_gate, budgets.rejection_budget)
        .map_err(|e| anyhow::anyhow!("{e}"))
}

fn emit(recorder: Option<&dyn LoopRecorder>, log: &mut EventLog, event: LoopEvent) -> Result<()> {
    if let Some(recorder) = recorder {
        recorder.record(&event)?;
    }
    log.push(&event)?;
    Ok(())
}
