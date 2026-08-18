//! Exploration-only sessions: the read-only explorer tool loop.

use super::*;

impl Psp9AgentRuntime {
    /// Exploration-only session (PSP-9 phase 7): the deterministic repository
    /// map plus an interactive read-only explorer tool loop. The explorer
    /// holds only read capabilities; every tool call still passes the kernel,
    /// so a mutation attempt is denied and recorded rather than prevented by
    /// convention. Nothing is measured, gated, or promoted.
    pub async fn run_exploration(mut self, task: String) -> Result<Psp9RunSummary> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let node_id = "explore-1".to_string();
        let (recorder, _goal, mut scheduler, running_graph) =
            self.open_session(&task, &session_id, &node_id).await?;
        let candidate = self.open_candidate(&node_id, 0, &running_graph.revision_id)?;
        let explorer = perspt_sdk::exploration_capability(perspt_sdk::ActorId::new("toolloop"));
        debug_assert!(perspt_sdk::is_read_only_capability(&explorer));
        let mut capability = explorer;
        capability.session_id = session_id.clone();
        capability.graph_revision = running_graph.revision_id.clone();
        capability.role = CapabilityRole::Explorer;
        let domain = self.domain.clone();
        let scope = perspt_sdk::DomainScope {
            label: node_id.clone(),
            paths: Vec::new(),
        };
        let catalog = StaticCatalog::with_base(domain.tool_entries(&scope))
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let mut kernel_state = perspt_sdk::KernelState::new();
        kernel_state.set_witness("__graph_revision", running_graph.revision_id.clone());
        kernel_state.set_witness("__authority_epoch", capability.authority_epoch.to_string());
        let outcome = self
            .explorer_loop(
                &recorder,
                &candidate,
                &catalog,
                capability,
                kernel_state,
                &task,
            )
            .await;
        scheduler.finish(&node_id, 0);
        let status = if outcome.is_ok() {
            "COMPLETED_PSP9"
        } else {
            "FAILED_PSP9"
        };
        recorder.finish(status)?;
        outcome?;
        Ok(Psp9RunSummary {
            session_id,
            node_id,
            outcome: NodeTerminalOutcome::HardPass,
            turns_used: 0,
            ledger_head: recorder.head(),
            promoted_paths: Vec::new(),
        })
    }

    /// The interactive explorer: read-only calls execute after a kernel
    /// check; denials (including any mutation attempt) are returned to the
    /// model and ledgered.
    async fn explorer_loop(
        &self,
        recorder: &Psp9Recorder,
        candidate: &CandidateWorkspace,
        catalog: &StaticCatalog,
        capability: Capability,
        kernel_state: perspt_sdk::KernelState,
        task: &str,
    ) -> Result<()> {
        let model = self.explorer_model.as_ref().unwrap_or(&self.model);
        let budget = perspt_sdk::ExplorationBudget::default();
        let specs = catalog.specs_for(std::slice::from_ref(&capability), false);
        let envelope = perspt_core::prompts::PlatformPromptLibrary::repository_explore()
            .map_err(|e| anyhow::anyhow!("repository explore prompt: {e}"))?;
        recorder.record_prompt_program("repository_explore", &envelope)?;
        let mut conversation = Conversation::with_system(envelope.text);
        conversation.push_user(task.to_string());
        let mut tool_calls = 0u32;
        for _turn in 0..16u32 {
            let output = self
                .transport
                .chat_turn(model, &conversation, &specs, ToolChoicePolicy::Auto)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            match output {
                TurnOutput::Text(text) => {
                    recorder
                        .record_custom("exploration_answer", serde_json::json!({"text": text}))?;
                    self.emit(perspt_core::AgentEvent::Log(format!("Explorer: {text}")));
                    println!("{text}");
                    return Ok(());
                }
                TurnOutput::ToolCalls(calls) => {
                    conversation.push_tool_calls(calls.clone());
                    for call in calls {
                        tool_calls += 1;
                        let response = if tool_calls > budget.max_tool_calls {
                            "denied: exploration tool-call budget exhausted".to_string()
                        } else {
                            self.explorer_call(
                                recorder,
                                candidate,
                                catalog,
                                &capability,
                                &kernel_state,
                                &call,
                            )
                            .await?
                        };
                        conversation.push_tool_response(call.call_id.clone(), response);
                    }
                }
            }
        }
        anyhow::bail!("explorer did not produce a final answer within the turn budget")
    }

    /// One explorer call through the kernel; the read-only capability makes
    /// every mutation a recorded denial.
    async fn explorer_call(
        &self,
        recorder: &Psp9Recorder,
        candidate: &CandidateWorkspace,
        catalog: &StaticCatalog,
        capability: &Capability,
        kernel_state: &perspt_sdk::KernelState,
        call: &perspt_sdk::ProviderToolCall,
    ) -> Result<String> {
        use crate::toolloop::EffectExecutor as _;
        let Some(entry) = catalog.lookup(&call.name).cloned() else {
            return Ok(format!("denied: unknown tool {:?}", call.name));
        };
        let mut proposal =
            perspt_sdk::EffectProposal::new(capability.holder.clone(), "explore-1", entry.effect);
        if let Some(path) = call.arguments.get("path").and_then(|v| v.as_str()) {
            proposal = proposal.with_path(path);
        }
        let witness = perspt_sdk::check_admissibility(
            &proposal,
            std::slice::from_ref(capability),
            kernel_state,
        );
        if !matches!(witness.decision, perspt_sdk::AdmissibilityDecision::Allow) {
            recorder.record_custom(
                "exploration_denied",
                serde_json::json!({
                    "tool": call.name,
                    "decision": format!("{:?}", witness.decision),
                }),
            )?;
            return Ok(format!("denied: {:?}", witness.decision));
        }
        let outcome = candidate.apply(call, &entry).await?;
        recorder.record_custom(
            "exploration_tool_call",
            serde_json::json!({"tool": call.name, "mutated": outcome.mutated}),
        )?;
        Ok(outcome.output)
    }
}
