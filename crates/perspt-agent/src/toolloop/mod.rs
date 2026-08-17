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

mod contract;
pub use contract::*;

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
    }
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
        let mut projection = ProjectionMismatch::default();
        let mut recovery = RecoveryCascade::new(self.budgets.recovery_budget);
        let mut cursor = CompactionCursor::default();
        let mut activated_tools: BTreeSet<_> = restored_activated_tools.into_iter().collect();
        if let Some(conversation) = resumed.as_ref() {
            activated_tools.extend(activated_tools_from_conversation(conversation));
        }

        // 1. Measured baseline and its real restore point.
        let mut accepted_checkpoint = self.executor.checkpoint(&[]).await?;
        let (baseline, mut trajectory) = self.measured_baseline(&mut log).await?;
        if let Some(outcome) = self.baseline_terminal(&baseline) {
            return Ok(finish(outcome, trajectory, log, projection, 0, 0));
        }

        let mut conversation = resumed.unwrap_or_else(|| {
            let mut conversation = Conversation::with_system(
                "You are a governed coding agent. Propose tool calls; every effect is mediated.",
            );
            conversation.push_user(goal.to_string());
            conversation
        });

        let mut turns_used = 0;
        let mut mutations_since_boundary = 0u32;
        for turn in 1..=self.budgets.max_turns {
            turns_used = turn;
            let turn_result = self
                .model_turn(
                    turn,
                    mutations_since_boundary,
                    &mut conversation,
                    &mut log,
                    &mut projection,
                    &mut recovery,
                    &mut activated_tools,
                )
                .await;
            let (output, mutations, immediate_boundary) = match turn_result {
                Ok(result) => result,
                Err(error) => {
                    let outcome = self.contain(&error, &accepted_checkpoint, &mut log).await?;
                    return Ok(finish(
                        outcome,
                        trajectory,
                        log,
                        projection,
                        turns_used,
                        recovery.spent,
                    ));
                }
            };
            mutations_since_boundary = mutations_since_boundary.saturating_add(mutations);
            if !self.boundary_due(&output, immediate_boundary, mutations_since_boundary, turn) {
                continue;
            }
            mutations_since_boundary = 0;

            let step = self
                .boundary_step(
                    goal,
                    turn,
                    &mut accepted_checkpoint,
                    &mut trajectory,
                    &mut recovery,
                    &mut conversation,
                    &mut activated_tools,
                    &mut log,
                    &mut cursor,
                )
                .await?;
            match step {
                BoundaryStep::Terminal(outcome) => {
                    return Ok(finish(
                        outcome,
                        trajectory,
                        log,
                        projection,
                        turns_used,
                        recovery.spent,
                    ));
                }
                BoundaryStep::Exhausted => break,
                BoundaryStep::Continue => {}
            }
        }

        let outcome = NodeTerminalOutcome::Escalated {
            certificate_id: uuid::Uuid::new_v4().to_string(),
        };
        Ok(finish(
            outcome,
            trajectory,
            log,
            projection,
            turns_used,
            recovery.spent,
        ))
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
    #[allow(clippy::too_many_arguments)]
    async fn boundary_step(
        &mut self,
        goal: &str,
        turn: u32,
        accepted_checkpoint: &mut CandidateCheckpoint,
        trajectory: &mut AcceptedTrajectory,
        recovery: &mut RecoveryCascade,
        conversation: &mut Conversation,
        activated_tools: &mut BTreeSet<String>,
        log: &mut EventLog,
        cursor: &mut CompactionCursor,
    ) -> Result<BoundaryStep> {
        let (measured, decision) = self.measure_and_gate(trajectory, log).await?;

        let accepted = decision.is_accepted();
        if accepted {
            *accepted_checkpoint = self.executor.checkpoint(&[]).await?;
        } else if !matches!(decision, GateDecision::StoppedAtDeclaredFloor) {
            self.executor.restore(accepted_checkpoint).await?;
            emit(
                self.recorder,
                log,
                LoopEvent::CandidateRestored {
                    checkpoint_id: accepted_checkpoint.id.clone(),
                },
            )?;
        }

        if let Some(outcome) = self.classify_decision(&decision, &measured) {
            if accepted {
                self.durable_checkpoint(
                    goal,
                    turn,
                    accepted_checkpoint,
                    &measured,
                    trajectory,
                    recovery,
                    conversation,
                    activated_tools,
                    log,
                )
                .await?;
            }
            return Ok(BoundaryStep::Terminal(outcome));
        }
        push_correction(conversation, &measured, accepted);
        self.maybe_compact(
            goal,
            turn,
            accepted_checkpoint,
            &measured,
            trajectory,
            recovery,
            conversation,
            activated_tools,
            log,
            cursor,
        )?;
        if accepted {
            self.durable_checkpoint(
                goal,
                turn,
                accepted_checkpoint,
                &measured,
                trajectory,
                recovery,
                conversation,
                activated_tools,
                log,
            )
            .await?;
        } else {
            if trajectory.budget_exhausted() {
                return Ok(BoundaryStep::Exhausted);
            }
            let granted = recovery.grant(classify_failure(FailureKind::GateRejection));
            emit(
                self.recorder,
                log,
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
                    let failure = if cause.to_ascii_lowercase().contains("rate limit") {
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
                        return Err(anyhow::anyhow!(
                            "transport recovery reached {:?}: {cause}",
                            granted.level
                        ));
                    }
                    let Some(next) = self.fallback_models.first().cloned() else {
                        return Err(anyhow::anyhow!(
                            "transport recovery has no eligible fallback: {cause}"
                        ));
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
    #[allow(clippy::too_many_arguments)]
    async fn model_turn(
        &mut self,
        turn: u32,
        mutations_so_far: u32,
        conversation: &mut Conversation,
        log: &mut EventLog,
        projection: &mut ProjectionMismatch,
        recovery: &mut RecoveryCascade,
        activated_tools: &mut BTreeSet<String>,
    ) -> Result<(TurnOutput, u32, bool)> {
        let specs = self
            .catalog
            .deferred_specs_for(&self.capabilities, activated_tools, false);
        let output = self
            .chat_with_failover(conversation, &specs, recovery, log)
            .await?;
        // R2: record the observation before inspecting it.
        emit(
            self.recorder,
            log,
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
                conversation,
                log,
                projection,
                activated_tools,
            )
            .await?;
        Ok((output, mutations, immediate_boundary))
    }

    /// Make a gate acceptance durably resumable (PSP-9 phase 6): export the
    /// accepted candidate's mutated contents as content-addressed artifacts
    /// and ledger them with the exact control frame, so a crashed loop can be
    /// continued from this acceptance instead of restarted.
    #[allow(clippy::too_many_arguments)]
    async fn durable_checkpoint(
        &self,
        goal: &str,
        turn: u32,
        accepted_checkpoint: &CandidateCheckpoint,
        measured: &Measured,
        trajectory: &AcceptedTrajectory,
        recovery: &RecoveryCascade,
        conversation: &Conversation,
        activated_tools: &BTreeSet<String>,
        log: &mut EventLog,
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
        let control = self.control_frame(
            goal,
            turn,
            accepted_checkpoint,
            measured,
            trajectory,
            recovery,
            conversation,
            activated_tools,
            authority_epoch,
        );
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
            log,
            LoopEvent::DurableCandidateCheckpoint {
                state_root: accepted_checkpoint.witness.state_root.clone(),
                control,
                conversation: conversation.clone(),
                canonical_scope: accepted_checkpoint.witness.canonical_scope.clone(),
                files,
            },
        )
    }

    /// The verbatim control frame a compaction must preserve (resolved
    /// design decision 3): goal, accepted state binding, live authority, and
    /// exact remaining budgets.
    #[allow(clippy::too_many_arguments)]
    fn control_frame(
        &self,
        goal: &str,
        turn: u32,
        accepted_checkpoint: &CandidateCheckpoint,
        measured: &Measured,
        trajectory: &AcceptedTrajectory,
        recovery: &RecoveryCascade,
        conversation: &Conversation,
        activated_tools: &BTreeSet<String>,
        authority_epoch: u64,
    ) -> ControlFrame {
        ControlFrame {
            goal: goal.to_string(),
            node_generation: self.generation,
            accepted_state_root: accepted_checkpoint.witness.state_root.clone(),
            graph_revision: accepted_checkpoint.witness.graph_revision.clone(),
            capability_ids: self
                .capabilities
                .iter()
                .map(|capability| capability.capability_id.clone())
                .collect(),
            authority_epoch,
            remaining_rejection_budget: trajectory
                .rejection_budget
                .saturating_sub(trajectory.rejections_used.max(recovery.spent)),
            remaining_turns: self.budgets.max_turns.saturating_sub(turn),
            active_model: self.model.clone(),
            remaining_fallback_models: self.fallback_models.clone(),
            activated_tools: activated_tools.iter().cloned().collect(),
            unresolved_call_ids: conversation.unresolved_call_ids(),
            residual_summary: measured
                .residuals
                .iter()
                .map(|residual| (format!("{:?}", residual.class), residual.score))
                .collect(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn maybe_compact(
        &self,
        goal: &str,
        turn: u32,
        accepted_checkpoint: &CandidateCheckpoint,
        measured: &Measured,
        trajectory: &AcceptedTrajectory,
        recovery: &RecoveryCascade,
        conversation: &mut Conversation,
        activated_tools: &BTreeSet<String>,
        log: &mut EventLog,
        cursor: &mut CompactionCursor,
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
        if conversation.estimated_chars() <= threshold {
            return Ok(());
        }

        let authority_epoch = self
            .kernel_state
            .witnesses
            .get("__authority_epoch")
            .map(String::as_str)
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        let control = self.control_frame(
            goal,
            turn,
            accepted_checkpoint,
            measured,
            trajectory,
            recovery,
            conversation,
            activated_tools,
            authority_epoch,
        );
        // The rolling chain root commits to every event so far in O(1); the
        // cursor makes each checkpoint cover exactly the span since the
        // previous one instead of claiming the whole history from zero.
        let covered_root = log.chain_root().to_string();
        let checkpoint = ContextCheckpoint {
            parent: cursor.parent.clone(),
            covered_from: cursor.next_from,
            covered_to: log.count().saturating_sub(1),
            covered_event_root: covered_root.clone(),
            control,
            artifact_refs: vec![accepted_checkpoint.witness.state_root.clone()],
            narrative_observation: None,
        };
        checkpoint
            .validate_against(
                &accepted_checkpoint.witness.state_root,
                &accepted_checkpoint.witness.graph_revision,
                authority_epoch,
            )
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        emit(
            self.recorder,
            log,
            LoopEvent::ContextCheckpointCreated {
                checkpoint: checkpoint.clone(),
            },
        )?;
        let control_json = serde_json::to_string(&checkpoint.control)?;
        conversation.compact_with_control(format!("PERSPECTIVE_CONTROL_FRAME_V1\n{control_json}"));
        cursor.parent = Some(covered_root);
        cursor.next_from = checkpoint.covered_to.saturating_add(1);
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

    /// Route every returned call through the kernel; execute the admitted
    /// ones. Returns the number of admitted mutations.
    async fn execute_turn(
        &mut self,
        output: &TurnOutput,
        max_mutations: u32,
        conversation: &mut Conversation,
        log: &mut EventLog,
        projection: &mut ProjectionMismatch,
        activated_tools: &mut BTreeSet<String>,
    ) -> Result<(u32, bool)> {
        let calls = output.tool_calls().to_vec();
        if calls.is_empty() {
            if let TurnOutput::Text(text) = output {
                conversation.push(perspt_sdk::Message::Assistant {
                    content: text.clone(),
                });
            }
            return Ok((0, false));
        }
        conversation.push_tool_calls(calls.clone());

        let mut mutations = 0u32;
        let mut immediate_boundary = false;
        let mut calls_seen = 0u32;
        for call in &calls {
            emit(
                self.recorder,
                log,
                LoopEvent::ToolCallObserved { call: call.clone() },
            )?;
            let entry = self.catalog.lookup(&call.name).cloned();
            let mutating = entry
                .as_ref()
                .is_some_and(|entry| candidate_mutating_effect(entry.effect));
            let invalid = entry
                .as_ref()
                .and_then(|entry| entry.validate_arguments(&call.arguments).err())
                .map(|error| anyhow::anyhow!(error.to_string()));
            let budget = self.budget_denial(calls_seen, mutating, mutations, max_mutations);
            calls_seen = calls_seen.saturating_add(1);
            let mut response = match (invalid, budget) {
                (Some(reason), _) => {
                    self.record_unchecked_proposal(
                        call,
                        entry.as_ref().expect("validated entry"),
                        log,
                    )
                    .await?;
                    self.deny(
                        log,
                        projection,
                        &call.call_id,
                        reason.to_string(),
                        perspt_sdk::ResidualClass::ToolArgumentInvalid,
                    )?
                }
                (None, Some(reason)) => {
                    if let Some(entry) = entry.as_ref() {
                        self.record_unchecked_proposal(call, entry, log).await?;
                    }
                    self.deny(
                        log,
                        projection,
                        &call.call_id,
                        reason.to_string(),
                        perspt_sdk::ResidualClass::BudgetExhausted,
                    )?
                }
                (None, None) => {
                    self.check_and_apply(call, log, projection, &mut mutations)
                        .await?
                }
            };
            if call.name == "tool_search" && !response.starts_with("denied:") {
                // The response *is* the executed search result; activate from
                // it instead of running the search a second time.
                if let Ok(specs) = serde_json::from_str::<Vec<perspt_sdk::ToolSpec>>(&response) {
                    activated_tools.extend(specs.into_iter().map(|spec| spec.name));
                }
            }
            if call.name == "tool_program" && !response.starts_with("denied:") {
                response = self
                    .run_tool_program(
                        call,
                        &response,
                        &mut calls_seen,
                        max_mutations,
                        &mut mutations,
                        &mut immediate_boundary,
                        log,
                        projection,
                    )
                    .await?;
            }
            if mutating && Self::high_risk(entry.as_ref()) && mutations > 0 {
                immediate_boundary = true;
            }
            conversation.push_tool_response(call.call_id.clone(), response);
        }
        Ok((mutations, immediate_boundary))
    }

    /// Execute a validated tool program's nested calls, each returning to the
    /// same kernel and budgets as a top-level call.
    #[allow(clippy::too_many_arguments)]
    async fn run_tool_program(
        &mut self,
        call: &ProviderToolCall,
        response: &str,
        calls_seen: &mut u32,
        max_mutations: u32,
        mutations: &mut u32,
        immediate_boundary: &mut bool,
        log: &mut EventLog,
        projection: &mut ProjectionMismatch,
    ) -> Result<String> {
        let program_calls: Vec<perspt_policy::ToolProgramCall> =
            serde_json::from_str(response).context("decoding tool program result")?;
        let mut nested_results = Vec::new();
        for (nested_ordinal, nested) in program_calls.into_iter().enumerate() {
            let nested_call = ProviderToolCall {
                call_id: format!("{}:{}", call.call_id, nested_ordinal),
                name: nested.tool,
                arguments: nested.arguments,
            };
            emit(
                self.recorder,
                log,
                LoopEvent::ToolCallObserved {
                    call: nested_call.clone(),
                },
            )?;
            let budget_ordinal = *calls_seen;
            *calls_seen = (*calls_seen).saturating_add(1);
            let nested_entry = self.catalog.lookup(&nested_call.name).cloned();
            let nested_mutating = nested_entry
                .as_ref()
                .is_some_and(|entry| candidate_mutating_effect(entry.effect));
            let nested_high_risk = Self::high_risk(nested_entry.as_ref());
            let invalid = nested_entry
                .as_ref()
                .and_then(|entry| entry.validate_arguments(&nested_call.arguments).err())
                .map(|error| anyhow::anyhow!(error.to_string()));
            let budget =
                self.budget_denial(budget_ordinal, nested_mutating, *mutations, max_mutations);
            let result = match (invalid, budget) {
                (Some(reason), _) => {
                    self.record_unchecked_proposal(
                        &nested_call,
                        nested_entry.as_ref().expect("validated entry"),
                        log,
                    )
                    .await?;
                    self.deny(
                        log,
                        projection,
                        &nested_call.call_id,
                        reason.to_string(),
                        perspt_sdk::ResidualClass::ToolArgumentInvalid,
                    )?
                }
                (None, Some(reason)) => {
                    if let Some(entry) = nested_entry.as_ref() {
                        self.record_unchecked_proposal(&nested_call, entry, log)
                            .await?;
                    }
                    self.deny(
                        log,
                        projection,
                        &nested_call.call_id,
                        reason,
                        perspt_sdk::ResidualClass::BudgetExhausted,
                    )?
                }
                (None, None) => {
                    self.check_and_apply(&nested_call, log, projection, mutations)
                        .await?
                }
            };
            if nested_mutating && nested_high_risk && *mutations > 0 {
                *immediate_boundary = true;
            }
            nested_results.push(serde_json::json!({
                "call_id": nested_call.call_id,
                "tool": nested_call.name,
                "result": result,
            }));
        }
        Ok(serde_json::to_string(&nested_results)?)
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
        let scope = proposal_scope(call);
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
        let outcome = self.executor.apply(call, entry).await?;
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

fn proposal_scope(call: &ProviderToolCall) -> Vec<String> {
    ["path", "to", "from"]
        .iter()
        .filter_map(|field| call.arguments.get(*field).and_then(|v| v.as_str()))
        .map(str::to_string)
        .collect()
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
    proposal
}

fn push_correction(conversation: &mut Conversation, measured: &Measured, accepted: bool) {
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
    conversation.push_user(instruction);
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
