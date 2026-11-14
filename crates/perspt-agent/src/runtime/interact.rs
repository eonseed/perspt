//! Interactive approval, correction refinement, and run conclusion.

use super::*;

/// Outcome of the runtime's post-verification approval boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ApprovalOutcome {
    Approved,
    Rejected,
    CorrectionRequested(String),
}

/// Owned attempt/graph state threaded through post-run human correction.
pub(super) struct CorrectionSession<'a> {
    pub(super) recorder: &'a Psp9Recorder,
    pub(super) session_id: &'a str,
    pub(super) node_id: &'a str,
    pub(super) task: &'a str,
    pub(super) scheduler: &'a mut Scheduler,
    pub(super) graph: WorkGraphRevision,
    pub(super) attempt: NodeAttempt,
    pub(super) generation: u32,
}

impl Psp9AgentRuntime {
    pub(super) async fn conclude_run(
        &self,
        ctx: ConcludeContext<'_>,
    ) -> Result<(NodeTerminalOutcome, &'static str, Vec<String>)> {
        let hard_pass = matches!(ctx.loop_outcome, NodeTerminalOutcome::HardPass);
        let promotion_approved = self.validate_and_approve(&ctx, hard_pass).await?;
        self.finish_conclude(&ctx, hard_pass, promotion_approved)
            .await
    }

    pub(super) async fn finish_conclude(
        &self,
        ctx: &ConcludeContext<'_>,
        hard_pass: bool,
        promotion_approved: bool,
    ) -> Result<(NodeTerminalOutcome, &'static str, Vec<String>)> {
        let mut final_outcome = if hard_pass && !promotion_approved {
            NodeTerminalOutcome::Escalated {
                certificate_id: uuid::Uuid::new_v4().to_string(),
            }
        } else {
            ctx.loop_outcome.clone()
        };
        let mut promoted_paths = Vec::new();
        let status = if matches!(final_outcome, NodeTerminalOutcome::HardPass) {
            let certified = self
                .promote_certified(
                    ctx.recorder,
                    ctx.candidate,
                    ctx.node_id,
                    ctx.grant_policy,
                    ctx.capability,
                    ctx.contract,
                    ctx.barrier,
                    ctx.kernel_state,
                    ctx.calibration,
                )
                .await?;
            match certified {
                Some(paths) => {
                    promoted_paths = paths;
                    self.emit(perspt_core::AgentEvent::NodeCompleted {
                        node_id: ctx.node_id.to_string(),
                        goal: ctx.task.to_string(),
                    });
                    "COMPLETED_PSP9"
                }
                None => {
                    final_outcome = NodeTerminalOutcome::Escalated {
                        certificate_id: uuid::Uuid::new_v4().to_string(),
                    };
                    self.emit(perspt_core::AgentEvent::TaskStatusChanged {
                        node_id: ctx.node_id.to_string(),
                        status: perspt_core::NodeStatus::Escalated,
                    });
                    "ESCALATED_PSP9"
                }
            }
        } else {
            self.emit(perspt_core::AgentEvent::TaskStatusChanged {
                node_id: ctx.node_id.to_string(),
                status: perspt_core::NodeStatus::Escalated,
            });
            "ESCALATED_PSP9"
        };
        Ok((final_outcome, status, promoted_paths))
    }

    /// Record the deterministic-suite verdict, run adjudication, and resolve
    /// conformal or human approval into a boolean (non-interactive path).
    pub(super) async fn validate_and_approve(
        &self,
        ctx: &ConcludeContext<'_>,
        hard_pass: bool,
    ) -> Result<bool> {
        Ok(matches!(
            self.validate_and_approve_interactive(ctx, hard_pass)
                .await?,
            ApprovalOutcome::Approved
        ))
    }

    pub(super) async fn validate_and_approve_interactive(
        &self,
        ctx: &ConcludeContext<'_>,
        hard_pass: bool,
    ) -> Result<ApprovalOutcome> {
        let candidate_id = ctx.candidate.checkpoint(&[]).await?.witness.state_root;
        record_deterministic_verdict(
            ctx.recorder,
            &candidate_id,
            &ctx.calibration.stratum,
            hard_pass,
        )?;
        if !hard_pass {
            return Ok(ApprovalOutcome::Rejected);
        }
        let adjudicated = self
            .adjudicate_candidate(
                ctx.recorder,
                ctx.candidate,
                ctx.task,
                &ctx.calibration.stratum,
            )
            .await?;
        if !adjudicated {
            return Ok(ApprovalOutcome::Rejected);
        }
        self.conformal_or_human_approval_outcome(ctx).await
    }

    async fn conformal_or_human_approval_outcome(
        &self,
        ctx: &ConcludeContext<'_>,
    ) -> Result<ApprovalOutcome> {
        const HARD_PASS_SCORE: f64 = 1.0;
        let certified = self.config.approval_policy == ApprovalPolicy::Ask
            && ctx.calibration.state == "active"
            && ctx
                .calibration
                .threshold
                .is_some_and(|theta| HARD_PASS_SCORE > theta);
        if certified {
            ctx.recorder.record_custom(
                "conformal_certified_accept",
                serde_json::json!({
                    "epoch_id": ctx.calibration.epoch_id,
                    "threshold": ctx.calibration.threshold,
                    "score": HARD_PASS_SCORE,
                    "node_id": ctx.node_id,
                }),
            )?;
            return Ok(ApprovalOutcome::Approved);
        }
        self.request_approval_outcome(
            ctx.recorder,
            ctx.node_id,
            ctx.candidate.touched_paths(),
            ctx.candidate.realized_diff().ok(),
        )
        .await
    }

    async fn request_approval_outcome(
        &self,
        recorder: &Psp9Recorder,
        node_id: &str,
        paths: Vec<String>,
        diff: Option<String>,
    ) -> Result<ApprovalOutcome> {
        match self.config.approval_policy {
            ApprovalPolicy::Auto | ApprovalPolicy::SessionApproved => {
                return Ok(ApprovalOutcome::Approved);
            }
            ApprovalPolicy::Deny => return Ok(ApprovalOutcome::Rejected),
            ApprovalPolicy::Ask => {}
        }
        let request_id = uuid::Uuid::new_v4().to_string();
        recorder.record_custom(
            "approval_requested",
            serde_json::json!({"request_id": request_id, "node_id": node_id, "paths": paths}),
        )?;
        self.emit(perspt_core::AgentEvent::ApprovalRequest {
            request_id: request_id.clone(),
            node_id: node_id.into(),
            action_type: perspt_core::ActionType::BundleWrite {
                node_id: node_id.into(),
                files: paths,
            },
            description: "Promote the verified PSP-9 candidate to the workspace".into(),
            diff,
        });
        let mut receiver_slot = self.action_receiver.lock().await;
        let Some(receiver) = receiver_slot.as_mut() else {
            recorder.record_custom(
                "approval_unavailable",
                serde_json::json!({"request_id": request_id}),
            )?;
            return Ok(ApprovalOutcome::Rejected);
        };
        wait_for_approval_action(recorder, receiver, &request_id).await
    }

    /// Conclude a single-node run, allowing one human correction refinement
    /// round when the reviewer sends `AgentAction::RequestCorrection`.
    pub(super) async fn conclude_with_correction_refinement(
        &self,
        mut s: CorrectionSession<'_>,
    ) -> Result<(
        NodeAttempt,
        WorkGraphRevision,
        u32,
        (NodeTerminalOutcome, &'static str, Vec<String>),
    )> {
        let approval = {
            let ctx = ConcludeContext {
                recorder: s.recorder,
                candidate: &s.attempt.candidate,
                node_id: s.node_id,
                task: s.task,
                grant_policy: &s.attempt.assembly.grant_policy,
                capability: &s.attempt.assembly.capability,
                contract: &s.attempt.assembly.contract,
                barrier: &s.attempt.assembly.barrier,
                kernel_state: &s.attempt.kernel_state,
                loop_outcome: &s.attempt.outcome.outcome,
                calibration: &s.attempt.assembly.calibration,
            };
            let hard_pass = matches!(ctx.loop_outcome, NodeTerminalOutcome::HardPass);
            let outcome = self
                .validate_and_approve_interactive(&ctx, hard_pass)
                .await?;
            match outcome {
                ApprovalOutcome::Approved => {
                    let verdict = self.finish_conclude(&ctx, hard_pass, true).await?;
                    return Ok((s.attempt, s.graph, s.generation, verdict));
                }
                ApprovalOutcome::Rejected => {
                    let verdict = self.finish_conclude(&ctx, hard_pass, false).await?;
                    return Ok((s.attempt, s.graph, s.generation, verdict));
                }
                ApprovalOutcome::CorrectionRequested(feedback) => feedback,
            }
        };

        let (revised, refined_goal) = revise_for_human_correction(&mut s, &approval)?;
        let mut seed = seed_from_attempt(s.recorder, s.node_id, &s.attempt).await?;
        if let Some(seeded) = seed.as_mut() {
            seeded.require_model_turn = true;
        }
        let next_gen = s.generation + 1;
        let next = self
            .attempt_node(
                s.recorder,
                s.session_id,
                &refined_goal,
                s.node_id,
                next_gen,
                &self.model.clone(),
                &revised,
                seed.as_ref(),
                self.config.rejection_budget,
            )
            .await?;
        let verdict = self
            .conclude_attempt(s.recorder, &next, s.node_id, s.task)
            .await?;
        Ok((next, revised, next_gen, verdict))
    }
}

async fn wait_for_approval_action(
    recorder: &Psp9Recorder,
    receiver: &mut perspt_core::events::channel::ActionReceiver,
    request_id: &str,
) -> Result<ApprovalOutcome> {
    while let Some(action) = receiver.recv().await {
        match action {
            perspt_core::AgentAction::Approve { request_id: id }
            | perspt_core::AgentAction::ApproveWithEdit { request_id: id, .. }
                if id == request_id =>
            {
                recorder.record_custom(
                    "approval_resolved",
                    serde_json::json!({"request_id": request_id, "approved": true}),
                )?;
                return Ok(ApprovalOutcome::Approved);
            }
            perspt_core::AgentAction::Reject { request_id: id, .. } if id == request_id => {
                recorder.record_custom(
                    "approval_resolved",
                    serde_json::json!({"request_id": request_id, "approved": false}),
                )?;
                return Ok(ApprovalOutcome::Rejected);
            }
            perspt_core::AgentAction::Abort => {
                recorder.record_custom(
                    "approval_resolved",
                    serde_json::json!({"request_id": request_id, "approved": false}),
                )?;
                return Ok(ApprovalOutcome::Rejected);
            }
            perspt_core::AgentAction::RequestCorrection {
                request_id: id,
                feedback,
            } if id == request_id => {
                recorder.record_custom(
                    "approval_correction_requested",
                    serde_json::json!({"request_id": request_id, "feedback": feedback}),
                )?;
                return Ok(ApprovalOutcome::CorrectionRequested(feedback));
            }
            _ => {}
        }
    }
    Ok(ApprovalOutcome::Rejected)
}

fn revise_for_human_correction(
    s: &mut CorrectionSession<'_>,
    feedback: &str,
) -> Result<(WorkGraphRevision, String)> {
    let mut node = s
        .graph
        .node(s.node_id)
        .context("revising an unknown node for human correction")?
        .clone();
    let refined_goal = format!(
        "{}\n\nHuman reviewer requested a correction before promotion:\n{feedback}",
        node.goal
    );
    node.generation = s.generation + 1;
    node.goal = refined_goal.clone();
    node.state = WorkNodeState::Running;
    let evidence = vec![perspt_sdk::ResidualEventRef {
        residual_id: format!("human-correction:{}:{}", s.node_id, s.generation),
        class: perspt_sdk::ResidualClass::Policy,
        component: perspt_sdk::EnergyComponent::Log,
        weighted_energy: s.attempt.outcome.trajectory.best_accepted_energy,
    }];
    let revised = s
        .graph
        .revise(
            perspt_sdk::GraphRevisionReason::UserEdit,
            &[perspt_sdk::GraphEdit::ReplaceNode { node }],
            evidence,
        )
        .map_err(|e| anyhow::anyhow!("human correction graph revision: {e}"))?;
    s.recorder
        .record_custom("graph_revision", serde_json::to_value(&revised)?)?;
    redispatch_refined(s.recorder, s.scheduler, &revised, s.node_id, s.generation)?;
    Ok((revised, refined_goal))
}
