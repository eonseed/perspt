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
    check_admissibility, check_full_admissibility, classify_failure, promote, AcceptedTrajectory,
    BarrierEvaluator, CandidateStateWitness, CandidateTransition, Capability, ContextCheckpoint,
    ContractEvaluator, ControlFrame, Conversation, CorrectionDirection, EffectProposal,
    FailureKind, FullAdmissibilityWitness, GateDecision, KernelState, ModelId, ModelTransport,
    NodeTerminalOutcome, ProviderToolCall, RecoveryCascade, ResidualEvent, StateWitness,
    StaticCatalog, ToolCatalog, ToolChoicePolicy, ToolEntry, TurnOutput, VerificationCadence,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::realize::ProjectionMismatch;

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

/// What applying one admitted call did to the candidate overlay.
#[derive(Debug, Clone)]
pub struct EffectOutcome {
    /// Tool output returned to the model as the tool response.
    pub output: String,
    /// Whether the overlay was mutated.
    pub mutated: bool,
}

/// Opaque executor checkpoint plus its measured state witness.
#[derive(Debug, Clone)]
pub struct CandidateCheckpoint {
    pub id: String,
    pub witness: CandidateStateWitness,
}

/// Applies admitted calls to the candidate overlay (the existing sandboxed
/// executors are the first driver).
#[async_trait::async_trait]
pub trait EffectExecutor: Send + Sync {
    /// Snapshot the reversible candidate overlay.
    async fn checkpoint(&self, scope: &[String]) -> Result<CandidateCheckpoint>;

    async fn apply(&self, call: &ProviderToolCall, entry: &ToolEntry) -> Result<EffectOutcome>;

    /// Restore an earlier candidate snapshot exactly.
    async fn restore(&self, checkpoint: &CandidateCheckpoint) -> Result<()>;

    /// Re-read the materialized candidate after an effect.
    async fn state_witness(&self) -> Result<CandidateStateWitness>;
}

/// One measured evaluation of the realized candidate.
#[derive(Debug, Clone)]
pub struct Measured {
    pub hard_pass: bool,
    pub energy: f64,
    pub residuals: Vec<ResidualEvent>,
    /// The domain's directed correction for the dominant residual.
    pub correction: Option<CorrectionDirection>,
}

/// Re-reads the candidate overlay and runs the declared verifier suite
/// against it (Paper I Def. 12.2: measurement runs on the realized state).
#[async_trait::async_trait]
pub trait CandidateMeasurer: Send + Sync {
    async fn measure(&self) -> Result<Measured>;

    /// Cheapest realized-state boundary available after one mutation. The
    /// default is the complete suite, which is conservative; domain drivers
    /// may provide an incremental parser without weakening the gate suite.
    async fn measure_incremental(&self) -> Result<Measured> {
        self.measure().await
    }
}

/// Recorded loop events (the ledger consumes these in system 14).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum LoopEvent {
    TurnObserved {
        turn: u32,
        output: TurnOutput,
    },
    ToolCallObserved {
        call: ProviderToolCall,
    },
    ProposalObserved {
        call_id: String,
        proposal: EffectProposal,
    },
    ProposalChecked {
        call_id: String,
        proposal: EffectProposal,
        witness: Box<FullAdmissibilityWitness>,
    },
    EffectApplied {
        call_id: String,
        mutated: bool,
        output: String,
    },
    EffectDenied {
        call_id: String,
        reason: String,
    },
    CandidateMeasured {
        node_id: String,
        generation: u32,
        energy: f64,
        hard_pass: bool,
        residuals: Vec<ResidualEvent>,
    },
    GateDecisionRecorded {
        node_id: String,
        generation: u32,
        decision: GateDecision,
    },
    CandidateRestored {
        checkpoint_id: String,
    },
    EffectBoundaryMeasured {
        call_id: String,
        node_id: String,
        generation: u32,
        energy: f64,
        hard_pass: bool,
        residuals: Vec<ResidualEvent>,
    },
    ContextCheckpointCreated {
        checkpoint: ContextCheckpoint,
    },
    RecoveryControlGranted {
        failure: FailureKind,
        level: perspt_sdk::CascadeLevel,
        forced_escalation: bool,
        model: ModelId,
    },
    RouteFailover {
        from_model: ModelId,
        to_model: ModelId,
        cause: String,
    },
    RecoveryContained {
        reason: String,
        restored_checkpoint_id: String,
    },
}

/// Synchronous write-ahead event sink. Implementations must durably append
/// before returning; the loop records observations before inspecting them.
pub trait LoopRecorder: Send + Sync {
    fn record(&self, event: &LoopEvent) -> Result<()>;

    /// Persist exact observation bytes and return their content handle. The
    /// default supports in-memory conformance fixtures; production recorders
    /// override it with durable content-addressed storage.
    fn record_artifact(&self, content: &[u8], _media_type: &str) -> Result<String> {
        Ok(perspt_sdk::ledger::content_hash(content))
    }
}

/// The loop's terminal report.
#[derive(Debug)]
pub struct LoopOutcome {
    pub outcome: NodeTerminalOutcome,
    pub trajectory: AcceptedTrajectory,
    pub events: Vec<LoopEvent>,
    pub projection: ProjectionMismatch,
    pub turns_used: u32,
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

impl ToolLoop<'_> {
    /// Run the loop to a classified terminal state (Paper II Lemma 1).
    pub async fn run(mut self, goal: &str) -> Result<LoopOutcome> {
        self.budgets.validate(&self.cadence)?;
        let mut events = Vec::new();
        let mut projection = ProjectionMismatch::default();
        let mut recovery = RecoveryCascade::new(self.budgets.recovery_budget);
        let mut context_parent = None;
        let mut activated_tools = BTreeSet::new();

        // 1. Measured baseline and its real restore point.
        let mut accepted_checkpoint = self.executor.checkpoint(&[]).await?;
        let baseline = self.measurer.measure().await?;
        emit(
            self.recorder,
            &mut events,
            LoopEvent::CandidateMeasured {
                node_id: self.node_id.clone(),
                generation: self.generation,
                energy: baseline.energy,
                hard_pass: baseline.hard_pass,
                residuals: baseline.residuals.clone(),
            },
        )?;
        let mut trajectory = AcceptedTrajectory::new(
            "toolloop",
            0,
            baseline.energy,
            self.budgets.rho_gate,
            self.budgets.rejection_budget,
        )?;
        if let Some(outcome) = self.baseline_terminal(&baseline) {
            return Ok(LoopOutcome {
                outcome,
                trajectory,
                events,
                projection,
                turns_used: 0,
            });
        }

        let mut conversation = Conversation::with_system(
            "You are a governed coding agent. Propose tool calls; every effect is mediated.",
        );
        conversation.push_user(goal.to_string());

        let mut turns_used = 0;
        let mut mutations_since_boundary = 0u32;
        for turn in 1..=self.budgets.max_turns {
            turns_used = turn;
            let remaining = self
                .cadence
                .max_mutations_between_checks
                .saturating_sub(mutations_since_boundary)
                .max(1);
            let turn_result = self
                .model_turn(
                    turn,
                    remaining,
                    &mut conversation,
                    &mut events,
                    &mut projection,
                    &mut recovery,
                    &mut activated_tools,
                )
                .await;
            let (output, mutations, immediate_boundary) = match turn_result {
                Ok(result) => result,
                Err(error) => {
                    self.executor.restore(&accepted_checkpoint).await?;
                    emit(
                        self.recorder,
                        &mut events,
                        LoopEvent::RecoveryContained {
                            reason: error.to_string(),
                            restored_checkpoint_id: accepted_checkpoint.id.clone(),
                        },
                    )?;
                    return Ok(LoopOutcome {
                        outcome: NodeTerminalOutcome::Escalated {
                            certificate_id: uuid::Uuid::new_v4().to_string(),
                        },
                        trajectory,
                        events,
                        projection,
                        turns_used,
                    });
                }
            };
            mutations_since_boundary = mutations_since_boundary.saturating_add(mutations);

            // Measure boundary: text turn, cadence bound `H`, or turn budget.
            let boundary_due = matches!(output, TurnOutput::Text(_))
                || immediate_boundary
                || mutations_since_boundary >= self.cadence.max_mutations_between_checks
                || turn == self.budgets.max_turns;
            if !boundary_due {
                continue;
            }

            let (measured, decision) = self.measure_and_gate(&mut trajectory, &mut events).await?;
            mutations_since_boundary = 0;

            if decision.is_accepted() {
                accepted_checkpoint = self.executor.checkpoint(&[]).await?;
            } else if !matches!(decision, GateDecision::StoppedAtDeclaredFloor) {
                self.executor.restore(&accepted_checkpoint).await?;
                emit(
                    self.recorder,
                    &mut events,
                    LoopEvent::CandidateRestored {
                        checkpoint_id: accepted_checkpoint.id.clone(),
                    },
                )?;
            }

            match self.classify_decision(&decision, &measured) {
                Some(outcome) => {
                    return Ok(LoopOutcome {
                        outcome,
                        trajectory,
                        events,
                        projection,
                        turns_used,
                    })
                }
                None => {
                    // Lemma 1: descent and rejection are non-terminal;
                    // continue from the checkpoint with a directed correction.
                    push_correction(&mut conversation, &measured);
                    self.maybe_compact(
                        goal,
                        turn,
                        &accepted_checkpoint,
                        &measured,
                        &trajectory,
                        &mut conversation,
                        &mut events,
                        &mut context_parent,
                    )?;
                    if !decision.is_accepted() && trajectory.budget_exhausted() {
                        break;
                    }
                    if !decision.is_accepted() {
                        let granted = recovery.grant(classify_failure(FailureKind::GateRejection));
                        emit(
                            self.recorder,
                            &mut events,
                            LoopEvent::RecoveryControlGranted {
                                failure: FailureKind::GateRejection,
                                level: granted.level,
                                forced_escalation: granted.forced_escalation,
                                model: self.model.clone(),
                            },
                        )?;
                        if granted.level > perspt_sdk::CascadeLevel::Retry {
                            break;
                        }
                    }
                }
            }
        }

        let certificate_id = uuid::Uuid::new_v4().to_string();
        Ok(LoopOutcome {
            outcome: NodeTerminalOutcome::Escalated { certificate_id },
            trajectory,
            events,
            projection,
            turns_used,
        })
    }

    /// One model turn: send the conversation, record the observation, and
    /// route every returned call through the kernel.
    #[allow(clippy::too_many_arguments)]
    async fn model_turn(
        &mut self,
        turn: u32,
        max_mutations: u32,
        conversation: &mut Conversation,
        events: &mut Vec<LoopEvent>,
        projection: &mut ProjectionMismatch,
        recovery: &mut RecoveryCascade,
        activated_tools: &mut BTreeSet<String>,
    ) -> Result<(TurnOutput, u32, bool)> {
        let specs = self
            .catalog
            .deferred_specs_for(&self.capabilities, activated_tools, false);
        let output = loop {
            match self
                .transport
                .chat_turn(&self.model, conversation, &specs, ToolChoicePolicy::Auto)
                .await
            {
                Ok(output) => break output,
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
                        events,
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
                        events,
                        LoopEvent::RouteFailover {
                            from_model: previous,
                            to_model: next,
                            cause,
                        },
                    )?;
                }
            }
        };
        // R2: record the observation before inspecting it.
        emit(
            self.recorder,
            events,
            LoopEvent::TurnObserved {
                turn,
                output: output.clone(),
            },
        )?;
        let (mutations, immediate_boundary) = self
            .execute_turn(
                &output,
                max_mutations,
                conversation,
                events,
                projection,
                activated_tools,
            )
            .await?;
        Ok((output, mutations, immediate_boundary))
    }

    #[allow(clippy::too_many_arguments)]
    fn maybe_compact(
        &self,
        goal: &str,
        turn: u32,
        accepted_checkpoint: &CandidateCheckpoint,
        measured: &Measured,
        trajectory: &AcceptedTrajectory,
        conversation: &mut Conversation,
        events: &mut Vec<LoopEvent>,
        parent: &mut Option<String>,
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
        let control = ControlFrame {
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
                .saturating_sub(trajectory.rejections_used),
            remaining_turns: self.budgets.max_turns.saturating_sub(turn),
            unresolved_call_ids: conversation.unresolved_call_ids(),
            residual_summary: measured
                .residuals
                .iter()
                .map(|residual| (format!("{:?}", residual.class), residual.score))
                .collect(),
        };
        let covered = serde_json::to_vec(events)?;
        let covered_root = perspt_sdk::ledger::content_hash(&covered);
        let checkpoint = ContextCheckpoint {
            parent: parent.clone(),
            covered_from: 0,
            covered_to: events.len().saturating_sub(1) as u64,
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
            events,
            LoopEvent::ContextCheckpointCreated {
                checkpoint: checkpoint.clone(),
            },
        )?;
        let control_json = serde_json::to_string(&checkpoint.control)?;
        conversation.compact_with_control(format!("PERSPECTIVE_CONTROL_FRAME_V1\n{control_json}"));
        *parent = Some(covered_root);
        Ok(())
    }

    /// Measure the realized candidate and submit it to the gate.
    async fn measure_and_gate(
        &mut self,
        trajectory: &mut AcceptedTrajectory,
        events: &mut Vec<LoopEvent>,
    ) -> Result<(Measured, GateDecision)> {
        let measured = self.measurer.measure().await?;
        emit(
            self.recorder,
            events,
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
            events,
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

    /// Route every returned call through the kernel; execute the admitted
    /// ones. Returns the number of admitted mutations.
    async fn execute_turn(
        &mut self,
        output: &TurnOutput,
        max_mutations: u32,
        conversation: &mut Conversation,
        events: &mut Vec<LoopEvent>,
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
        for (ordinal, call) in calls.iter().enumerate() {
            emit(
                self.recorder,
                events,
                LoopEvent::ToolCallObserved { call: call.clone() },
            )?;
            let entry = self.catalog.lookup(&call.name);
            let mutating = entry.is_some_and(|entry| candidate_mutating_effect(entry.effect));
            let mut response = if ordinal as u32 >= self.budgets.max_calls_per_turn {
                // Even a call above M gets a recorded proposal-and-result
                // pair (Gate J); it is denied, not silently dropped.
                projection.denied_proposals += 1;
                emit(
                    self.recorder,
                    events,
                    LoopEvent::EffectDenied {
                        call_id: call.call_id.clone(),
                        reason: "ToolCallBudgetExceeded".into(),
                    },
                )?;
                "denied: per-turn tool-call budget exceeded".to_string()
            } else if mutating && mutations >= max_mutations {
                projection.denied_proposals += 1;
                emit(
                    self.recorder,
                    events,
                    LoopEvent::EffectDenied {
                        call_id: call.call_id.clone(),
                        reason: "EvidenceBoundaryRequired".into(),
                    },
                )?;
                "denied: verification boundary required before another mutation".to_string()
            } else {
                self.check_and_apply(call, events, projection, &mut mutations)
                    .await?
            };
            if call.name == "tool_search" && !response.starts_with("denied:") {
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
                activated_tools.extend(
                    self.catalog
                        .search_specs(&self.capabilities, query, limit, false)
                        .into_iter()
                        .map(|spec| spec.name),
                );
            }
            if call.name == "tool_program" && !response.starts_with("denied:") {
                let program_calls: Vec<perspt_policy::ToolProgramCall> =
                    serde_json::from_str(&response).context("decoding tool program result")?;
                let mut nested_results = Vec::new();
                for (nested_ordinal, nested) in program_calls.into_iter().enumerate() {
                    let nested_call = ProviderToolCall {
                        call_id: format!("{}:{}", call.call_id, nested_ordinal),
                        name: nested.tool,
                        arguments: nested.arguments,
                    };
                    emit(
                        self.recorder,
                        events,
                        LoopEvent::ToolCallObserved {
                            call: nested_call.clone(),
                        },
                    )?;
                    let budget_ordinal = ordinal + nested_ordinal + 1;
                    let nested_entry = self.catalog.lookup(&nested_call.name);
                    let nested_mutating =
                        nested_entry.is_some_and(|entry| candidate_mutating_effect(entry.effect));
                    let nested_high_risk = nested_entry.is_some_and(|entry| {
                        matches!(
                            entry.risk,
                            perspt_sdk::RiskClass::High | perspt_sdk::RiskClass::Critical
                        )
                    });
                    let result = if budget_ordinal as u32 >= self.budgets.max_calls_per_turn {
                        projection.denied_proposals += 1;
                        emit(
                            self.recorder,
                            events,
                            LoopEvent::EffectDenied {
                                call_id: nested_call.call_id.clone(),
                                reason: "ToolCallBudgetExceeded".into(),
                            },
                        )?;
                        "denied: per-turn tool-call budget exceeded".to_string()
                    } else if nested_mutating && mutations >= max_mutations {
                        projection.denied_proposals += 1;
                        emit(
                            self.recorder,
                            events,
                            LoopEvent::EffectDenied {
                                call_id: nested_call.call_id.clone(),
                                reason: "EvidenceBoundaryRequired".into(),
                            },
                        )?;
                        "denied: verification boundary required before another mutation".to_string()
                    } else {
                        self.check_and_apply(&nested_call, events, projection, &mut mutations)
                            .await?
                    };
                    if nested_mutating && nested_high_risk && mutations > 0 {
                        immediate_boundary = true;
                    }
                    nested_results.push(serde_json::json!({
                        "call_id": nested_call.call_id,
                        "tool": nested_call.name,
                        "result": result,
                    }));
                }
                response = serde_json::to_string(&nested_results)?;
            }
            if mutating
                && entry.is_some_and(|entry| {
                    matches!(
                        entry.risk,
                        perspt_sdk::RiskClass::High | perspt_sdk::RiskClass::Critical
                    )
                })
                && mutations > 0
            {
                immediate_boundary = true;
            }
            conversation.push_tool_response(call.call_id.clone(), response);
        }
        Ok((mutations, immediate_boundary))
    }

    /// Kernel check, promote, and execute one call.
    async fn check_and_apply(
        &mut self,
        call: &ProviderToolCall,
        events: &mut Vec<LoopEvent>,
        projection: &mut ProjectionMismatch,
        mutations: &mut u32,
    ) -> Result<String> {
        let Some(entry) = self.catalog.lookup(&call.name).cloned() else {
            projection.denied_proposals += 1;
            emit(
                self.recorder,
                events,
                LoopEvent::EffectDenied {
                    call_id: call.call_id.clone(),
                    reason: format!("unknown tool {:?}", call.name),
                },
            )?;
            return Ok(format!("denied: unknown tool {:?}", call.name));
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
            events,
            LoopEvent::ProposalObserved {
                call_id: call.call_id.clone(),
                proposal: proposal.clone(),
            },
        )?;
        self.kernel_state
            .set_witness("__candidate_root", before.witness.state_root.clone());

        // Authority, effect scope, approval, and state preconditions are
        // checked before even the reversible candidate overlay is touched.
        let preflight = check_admissibility(&proposal, &self.capabilities, &self.kernel_state);
        if !matches!(preflight.decision, perspt_sdk::AdmissibilityDecision::Allow) {
            let transition = CandidateTransition::new(
                proposal.clone(),
                before.witness.clone(),
                before.witness.clone(),
            );
            let witness = check_full_admissibility(
                &transition,
                &self.capabilities,
                &self.kernel_state,
                self.contract,
                self.barrier,
                self.c_c_max,
            )
            .map_err(|e| anyhow::anyhow!("kernel: {e}"))?;
            emit(
                self.recorder,
                events,
                LoopEvent::ProposalChecked {
                    call_id: call.call_id.clone(),
                    proposal: proposal.clone(),
                    witness: Box::new(witness),
                },
            )?;
            projection.denied_proposals += 1;
            let reason = format!("{:?}", preflight.decision);
            emit(
                self.recorder,
                events,
                LoopEvent::EffectDenied {
                    call_id: call.call_id.clone(),
                    reason: reason.clone(),
                },
            )?;
            return Ok(format!("denied: {reason}"));
        }

        // Reads and governed verifier processes do not change the logical
        // candidate state. Their complete five-clause transition can therefore
        // be certified before invocation; no external process launches on a
        // partial witness.
        if !candidate_mutating_effect(entry.effect) {
            let transition =
                CandidateTransition::new(proposal, before.witness.clone(), before.witness.clone());
            let witness = check_full_admissibility(
                &transition,
                &self.capabilities,
                &self.kernel_state,
                self.contract,
                self.barrier,
                self.c_c_max,
            )
            .map_err(|e| anyhow::anyhow!("kernel: {e}"))?;
            emit(
                self.recorder,
                events,
                LoopEvent::ProposalChecked {
                    call_id: call.call_id.clone(),
                    proposal: transition.proposal.clone(),
                    witness: Box::new(witness.clone()),
                },
            )?;
            if !witness.allows()
                || witness.profile != perspt_sdk::AdmissibilityProfile::SrbnCertified
            {
                projection.denied_proposals += 1;
                let reason = format!(
                    "non-candidate effect failed full admissibility: {:?}",
                    witness.base.decision
                );
                emit(
                    self.recorder,
                    events,
                    LoopEvent::EffectDenied {
                        call_id: call.call_id.clone(),
                        reason: reason.clone(),
                    },
                )?;
                return Ok(format!("denied: {reason}"));
            }
            promote_matching_capability(&mut self.capabilities, &witness)
                .map_err(|error| anyhow::anyhow!("promotion: {error}"))?;
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
                let output = bounded_model_output(self.recorder, serde_json::to_string(&matches)?)?;
                emit(
                    self.recorder,
                    events,
                    LoopEvent::EffectApplied {
                        call_id: call.call_id.clone(),
                        mutated: false,
                        output: output.clone(),
                    },
                )?;
                return Ok(output);
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
                let output = serde_json::to_string(&calls)?;
                emit(
                    self.recorder,
                    events,
                    LoopEvent::EffectApplied {
                        call_id: call.call_id.clone(),
                        mutated: false,
                        output: output.clone(),
                    },
                )?;
                return Ok(output);
            }
            let outcome = self.executor.apply(call, &entry).await?;
            let output = bounded_model_output(self.recorder, outcome.output)?;
            emit(
                self.recorder,
                events,
                LoopEvent::EffectApplied {
                    call_id: call.call_id.clone(),
                    mutated: false,
                    output: output.clone(),
                },
            )?;
            return Ok(output);
        }

        let outcome = self.executor.apply(call, &entry).await?;
        let after = self.executor.state_witness().await?;
        let transition = CandidateTransition::new(proposal, before.witness.clone(), after);
        let witness = check_full_admissibility(
            &transition,
            &self.capabilities,
            &self.kernel_state,
            self.contract,
            self.barrier,
            self.c_c_max,
        )
        .map_err(|e| anyhow::anyhow!("kernel: {e}"))?;
        let output = bounded_model_output(self.recorder, outcome.output)?;
        emit(
            self.recorder,
            events,
            LoopEvent::ProposalChecked {
                call_id: call.call_id.clone(),
                proposal: transition.proposal.clone(),
                witness: Box::new(witness.clone()),
            },
        )?;

        if !witness.allows() || witness.profile != perspt_sdk::AdmissibilityProfile::SrbnCertified {
            self.executor.restore(&before).await?;
            // Denials are evidence, not errors: returned to the model so the
            // loop can adapt, and recorded for the ledger.
            projection.denied_proposals += 1;
            let reason = if witness.allows() {
                format!(
                    "admissibility profile {:?} is not autonomously committable",
                    witness.profile
                )
            } else {
                format!("{:?}", witness.base.decision)
            };
            emit(
                self.recorder,
                events,
                LoopEvent::EffectDenied {
                    call_id: call.call_id.clone(),
                    reason: reason.clone(),
                },
            )?;
            return Ok(format!("denied: {reason}"));
        }

        // Stage the debit on a clone. Executor failure or a failed promotion
        // cannot consume capability budget without the corresponding effect.
        let capability_id = witness.base.capability_id.clone();
        if let Some(capability) = self
            .capabilities
            .iter_mut()
            .find(|c| Some(&c.capability_id) == capability_id.as_ref())
        {
            let mut promoted = capability.clone();
            if let Err(e) = promote(&mut promoted, &witness) {
                self.executor.restore(&before).await?;
                projection.denied_proposals += 1;
                emit(
                    self.recorder,
                    events,
                    LoopEvent::EffectDenied {
                        call_id: call.call_id.clone(),
                        reason: format!("promotion: {e}"),
                    },
                )?;
                return Ok(format!("denied: {e}"));
            }
            *capability = promoted;
        }
        if outcome.mutated {
            *mutations += 1;
            let boundary = self.measurer.measure_incremental().await?;
            emit(
                self.recorder,
                events,
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
            events,
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

fn proposal_scope(call: &ProviderToolCall) -> Vec<String> {
    ["path", "to", "from"]
        .iter()
        .filter_map(|field| call.arguments.get(*field).and_then(|v| v.as_str()))
        .map(str::to_string)
        .collect()
}

fn candidate_mutating_effect(effect: perspt_sdk::EffectKind) -> bool {
    matches!(
        effect,
        perspt_sdk::EffectKind::WriteArtifact
            | perspt_sdk::EffectKind::ApplyPatch
            | perspt_sdk::EffectKind::MoveFile
            | perspt_sdk::EffectKind::DeleteFile
            | perspt_sdk::EffectKind::MutateDependencies
    )
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

fn push_correction(conversation: &mut Conversation, measured: &Measured) {
    let instruction = measured
        .correction
        .as_ref()
        .map(|c| c.instruction.clone())
        .unwrap_or_else(|| {
            format!(
                "The candidate did not descend (V = {:.3}). Address the dominant residual.",
                measured.energy
            )
        });
    conversation.push_user(instruction);
}

/// The finite decision bound the loop must respect (logged at node entry;
/// exceeding it fails the run — Gate M).
pub fn loop_decision_bound(baseline_energy: f64, budgets: &LoopBudgets) -> Result<u64> {
    perspt_sdk::finite_decision_bound(baseline_energy, budgets.rho_gate, budgets.rejection_budget)
        .map_err(|e| anyhow::anyhow!("{e}"))
}

fn emit(
    recorder: Option<&dyn LoopRecorder>,
    events: &mut Vec<LoopEvent>,
    event: LoopEvent,
) -> Result<()> {
    if let Some(recorder) = recorder {
        recorder.record(&event)?;
    }
    events.push(event);
    Ok(())
}
