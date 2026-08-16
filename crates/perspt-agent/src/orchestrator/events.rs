use super::*;

impl SRBNOrchestrator {
    /// Emit an event to the TUI (if connected)
    pub(crate) fn emit_event(&self, event: perspt_core::AgentEvent) {
        if let Some(ref sender) = self.event_sender {
            let _ = sender.send(event);
        }
    }

    /// Emit a log message to TUI
    pub(crate) fn emit_log(&self, msg: impl Into<String>) {
        self.emit_event(perspt_core::AgentEvent::Log(msg.into()));
    }

    /// PSP-7: Record an orchestration step transition to the store.
    pub(crate) fn record_step_quietly(
        &self,
        node_id: &str,
        step: &str,
        outcome: &str,
        energy: Option<&perspt_core::types::EnergyComponents>,
        attempt_count: i32,
        duration_ms: i32,
    ) {
        let record = perspt_store::SrbnStepRecord {
            session_id: self.context.session_id.clone(),
            node_id: node_id.to_string(),
            step: step.to_string(),
            outcome: outcome.to_string(),
            energy_json: energy.and_then(|e| serde_json::to_string(e).ok()),
            parse_state: None,
            retry_classification: None,
            attempt_count,
            duration_ms,
        };
        if let Err(e) = self.ledger.record_step(&record) {
            log::warn!("Failed to record step '{}' for {}: {}", step, node_id, e);
        }
    }

    /// Request approval from user and await response
    /// Returns ApprovalResult with optional edited value.
    /// `review_node_id` is used for persisting the review audit record.
    pub(crate) async fn await_approval(
        &mut self,
        action_type: perspt_core::ActionType,
        description: String,
        diff: Option<String>,
    ) -> ApprovalResult {
        self.await_approval_for_node(action_type, description, diff, None)
            .await
    }

    /// Internal approval with optional node_id for audit persistence.
    pub(crate) async fn await_approval_for_node(
        &mut self,
        action_type: perspt_core::ActionType,
        description: String,
        diff: Option<String>,
        review_node_id: Option<&str>,
    ) -> ApprovalResult {
        // If auto_approve is enabled, skip approval
        if self.auto_approve {
            if let Some(nid) = review_node_id {
                self.persist_review_decision(nid, "auto_approved", None);
            }
            return ApprovalResult::Approved;
        }

        // If no TUI connected, default to approve (headless with --yes)
        if self.action_receiver.is_none() {
            if let Some(nid) = review_node_id {
                self.persist_review_decision(nid, "auto_approved", None);
            }
            return ApprovalResult::Approved;
        }

        // Generate unique request ID
        let request_id = uuid::Uuid::new_v4().to_string();

        // Emit approval request
        self.emit_event(perspt_core::AgentEvent::ApprovalRequest {
            request_id: request_id.clone(),
            node_id: review_node_id.unwrap_or("current").to_string(),
            action_type,
            description,
            diff,
        });

        // Wait for response
        if let Some(ref mut receiver) = self.action_receiver {
            while let Some(action) = receiver.recv().await {
                match action {
                    perspt_core::AgentAction::Approve { request_id: rid } if rid == request_id => {
                        self.emit_log("✓ Approved by user");
                        if let Some(nid) = review_node_id {
                            self.persist_review_decision(nid, "approved", None);
                        }
                        return ApprovalResult::Approved;
                    }
                    perspt_core::AgentAction::ApproveWithEdit {
                        request_id: rid,
                        edited_value,
                    } if rid == request_id => {
                        self.emit_log(format!("✓ Approved with edit: {}", edited_value));
                        if let Some(nid) = review_node_id {
                            self.persist_review_decision(nid, "approved_with_edit", None);
                        }
                        return ApprovalResult::ApprovedWithEdit(edited_value);
                    }
                    perspt_core::AgentAction::Reject {
                        request_id: rid,
                        reason,
                    } if rid == request_id => {
                        let msg = reason.unwrap_or_else(|| "User rejected".to_string());
                        self.emit_log(format!("✗ Rejected: {}", msg));
                        if let Some(nid) = review_node_id {
                            self.persist_review_decision(nid, "rejected", Some(&msg));
                        }
                        return ApprovalResult::Rejected;
                    }
                    perspt_core::AgentAction::RequestCorrection {
                        request_id: rid,
                        feedback,
                    } if rid == request_id => {
                        self.emit_log(format!("🔄 Correction requested: {}", feedback));
                        if let Some(nid) = review_node_id {
                            self.persist_review_decision(
                                nid,
                                "correction_requested",
                                Some(&feedback),
                            );
                        }
                        return ApprovalResult::Rejected;
                    }
                    perspt_core::AgentAction::Abort => {
                        self.emit_log("⚠️ Session aborted by user");
                        self.abort_requested.store(true, Ordering::Relaxed);
                        if let Some(nid) = review_node_id {
                            self.persist_review_decision(nid, "aborted", None);
                        }
                        return ApprovalResult::Rejected;
                    }
                    _ => {
                        // Ignore other actions while waiting for this specific approval
                        continue;
                    }
                }
            }
        }

        ApprovalResult::Rejected // Channel closed
    }

    /// Persist a review decision to the audit trail.
    pub(crate) fn persist_review_decision(&self, node_id: &str, outcome: &str, note: Option<&str>) {
        let degraded = self.last_verification_result.as_ref().map(|vr| vr.degraded);
        if let Err(e) = self
            .ledger
            .record_review_outcome(node_id, outcome, note, None, degraded, None)
        {
            log::warn!("Failed to persist review decision for {}: {}", node_id, e);
        }
    }

    /// Add a dependency edge between nodes
    pub fn add_dependency(&mut self, from_id: &str, to_id: &str, kind: &str) -> Result<()> {
        let from_idx = self
            .node_indices
            .get(from_id)
            .context(format!("Node not found: {}", from_id))?;
        let to_idx = self
            .node_indices
            .get(to_id)
            .context(format!("Node not found: {}", to_id))?;

        self.graph.add_edge(
            *from_idx,
            *to_idx,
            Dependency {
                kind: kind.to_string(),
            },
        );
        Ok(())
    }
}
