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

use anyhow::Result;
use perspt_sdk::{
    check_full_admissibility, promote, AcceptedTrajectory, BarrierEvaluator, Capability,
    ContractEvaluator, Conversation, CorrectionDirection, EffectProposal, FullAdmissibilityWitness,
    GateDecision, ModelId, ModelTransport, NodeTerminalOutcome, ProviderToolCall, ResidualEvent,
    StaticCatalog, ToolCatalog, ToolChoicePolicy, ToolEntry, TurnOutput, VerificationCadence,
};

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

/// Applies admitted calls to the candidate overlay (the existing sandboxed
/// executors are the first driver).
#[async_trait::async_trait]
pub trait EffectExecutor: Send + Sync {
    async fn apply(&self, call: &ProviderToolCall, entry: &ToolEntry) -> Result<EffectOutcome>;
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
}

/// Recorded loop events (the ledger consumes these in system 14).
#[derive(Debug, Clone)]
pub enum LoopEvent {
    TurnObserved {
        turn: u32,
        tool_calls: usize,
    },
    ProposalChecked {
        call_id: String,
        witness: Box<FullAdmissibilityWitness>,
    },
    EffectApplied {
        call_id: String,
        mutated: bool,
    },
    EffectDenied {
        call_id: String,
        reason: String,
    },
    CandidateMeasured {
        energy: f64,
        hard_pass: bool,
    },
    GateDecisionRecorded {
        decision: GateDecision,
    },
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
}

impl ToolLoop<'_> {
    /// Run the loop to a classified terminal state (Paper II Lemma 1).
    pub async fn run(mut self, goal: &str) -> Result<LoopOutcome> {
        self.budgets.validate(&self.cadence)?;
        let mut events = Vec::new();
        let mut projection = ProjectionMismatch::default();

        // 1. Measured baseline; a passing baseline terminates immediately.
        let baseline = self.measurer.measure().await?;
        events.push(LoopEvent::CandidateMeasured {
            energy: baseline.energy,
            hard_pass: baseline.hard_pass,
        });
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
        for turn in 1..=self.budgets.max_turns {
            turns_used = turn;
            let (output, mutations) = self
                .model_turn(turn, &mut conversation, &mut events, &mut projection)
                .await?;

            // Measure boundary: text turn, cadence bound `H`, or turn budget.
            let boundary_due = matches!(output, TurnOutput::Text(_))
                || mutations >= self.cadence.max_mutations_between_checks
                || turn == self.budgets.max_turns;
            if !boundary_due {
                continue;
            }

            let (measured, decision) = self.measure_and_gate(&mut trajectory, &mut events).await?;

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
                    if !decision.is_accepted() && trajectory.budget_exhausted() {
                        break;
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
    async fn model_turn(
        &mut self,
        turn: u32,
        conversation: &mut Conversation,
        events: &mut Vec<LoopEvent>,
        projection: &mut ProjectionMismatch,
    ) -> Result<(TurnOutput, u32)> {
        let specs = self.catalog.specs_for(&self.capabilities, false);
        let output = self
            .transport
            .chat_turn(&self.model, conversation, &specs, ToolChoicePolicy::Auto)
            .await
            .map_err(|e| anyhow::anyhow!("transport: {e}"))?;
        // R2: record the observation before inspecting it.
        events.push(LoopEvent::TurnObserved {
            turn,
            tool_calls: output.tool_calls().len(),
        });
        let mutations = self
            .execute_turn(&output, conversation, events, projection)
            .await?;
        Ok((output, mutations))
    }

    /// Measure the realized candidate and submit it to the gate.
    async fn measure_and_gate(
        &mut self,
        trajectory: &mut AcceptedTrajectory,
        events: &mut Vec<LoopEvent>,
    ) -> Result<(Measured, GateDecision)> {
        let measured = self.measurer.measure().await?;
        events.push(LoopEvent::CandidateMeasured {
            energy: measured.energy,
            hard_pass: measured.hard_pass,
        });
        let decision = trajectory.submit_with_floor(
            measured.hard_pass,
            measured.energy,
            self.budgets.declared_energy_floor,
        )?;
        events.push(LoopEvent::GateDecisionRecorded {
            decision: decision.clone(),
        });
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
        conversation: &mut Conversation,
        events: &mut Vec<LoopEvent>,
        projection: &mut ProjectionMismatch,
    ) -> Result<u32> {
        let calls = output.tool_calls().to_vec();
        if calls.is_empty() {
            if let TurnOutput::Text(text) = output {
                conversation.push(perspt_sdk::Message::Assistant {
                    content: text.clone(),
                });
            }
            return Ok(0);
        }
        conversation.push_tool_calls(calls.clone());

        let mut mutations = 0u32;
        for (ordinal, call) in calls.iter().enumerate() {
            let response = if ordinal as u32 >= self.budgets.max_calls_per_turn {
                // Even a call above M gets a recorded proposal-and-result
                // pair (Gate J); it is denied, not silently dropped.
                projection.denied_proposals += 1;
                events.push(LoopEvent::EffectDenied {
                    call_id: call.call_id.clone(),
                    reason: "ToolCallBudgetExceeded".into(),
                });
                "denied: per-turn tool-call budget exceeded".to_string()
            } else {
                self.check_and_apply(call, events, projection, &mut mutations)
                    .await?
            };
            conversation.push_tool_response(call.call_id.clone(), response);
        }
        Ok(mutations)
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
            events.push(LoopEvent::EffectDenied {
                call_id: call.call_id.clone(),
                reason: format!("unknown tool {:?}", call.name),
            });
            return Ok(format!("denied: unknown tool {:?}", call.name));
        };
        let proposal = proposal_from(call, &entry);
        let witness = check_full_admissibility(
            &proposal,
            &self.capabilities,
            &perspt_sdk::KernelState::new(),
            self.contract,
            self.barrier,
            self.c_c_max,
        )
        .map_err(|e| anyhow::anyhow!("kernel: {e}"))?;
        events.push(LoopEvent::ProposalChecked {
            call_id: call.call_id.clone(),
            witness: Box::new(witness.clone()),
        });

        if !witness.allows() {
            // Denials are evidence, not errors: returned to the model so the
            // loop can adapt, and recorded for the ledger.
            projection.denied_proposals += 1;
            let reason = format!("{:?}", witness.base.decision);
            events.push(LoopEvent::EffectDenied {
                call_id: call.call_id.clone(),
                reason: reason.clone(),
            });
            return Ok(format!("denied: {reason}"));
        }

        // Promotion is one transaction: budgets are consumed before the
        // effect lands, debiting exactly the certified increment.
        let capability_id = witness.base.capability_id.clone();
        if let Some(capability) = self
            .capabilities
            .iter_mut()
            .find(|c| Some(&c.capability_id) == capability_id.as_ref())
        {
            if let Err(e) = promote(capability, &witness) {
                projection.denied_proposals += 1;
                events.push(LoopEvent::EffectDenied {
                    call_id: call.call_id.clone(),
                    reason: format!("promotion: {e}"),
                });
                return Ok(format!("denied: {e}"));
            }
        }

        let outcome = self.executor.apply(call, &entry).await?;
        if outcome.mutated {
            *mutations += 1;
        }
        events.push(LoopEvent::EffectApplied {
            call_id: call.call_id.clone(),
            mutated: outcome.mutated,
        });
        Ok(outcome.output)
    }
}

/// Canonicalize one provider call into an effect proposal (PSP-9 system 12).
/// Provenance (which model proposed it) is recorded in the ledger, never in
/// the kernel's input, so no admissibility decision can depend on a vendor.
fn proposal_from(call: &ProviderToolCall, entry: &ToolEntry) -> EffectProposal {
    let mut proposal = EffectProposal::new(
        perspt_sdk::ActorId::new("toolloop"),
        "toolloop",
        entry.effect,
    )
    .with_risk_class(entry.risk)
    .with_idempotency_key(format!("{}:{}", call.name, call.arguments));
    if let Some(path) = call.arguments.get("path").and_then(|v| v.as_str()) {
        proposal = proposal.with_path(path);
    }
    if let Some(command) = call.arguments.get("command").and_then(|v| v.as_str()) {
        proposal = proposal.with_command(perspt_sdk::canonicalize(command, "."));
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
