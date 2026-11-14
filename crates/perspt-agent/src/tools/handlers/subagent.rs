//! Capability-attenuated child subagents (`spawn_agent`).

use std::sync::Arc;

use anyhow::{Context, Result};
use perspt_coding::OperationalSafetyBarrier;
use perspt_sdk::{
    ActorId, AgentDomainPackage, Capability, Conversation, EffectProposal, KernelState, ModelId,
    ModelTransport, NodeTerminalOutcome, ProviderToolCall, StaticCatalog, ToolCatalog,
    ToolChoicePolicy, ToolEntry, TurnOutput, exploration_capability,
};

use super::{CandidateHandlerRegistry, CandidateToolHandler, InteractionHandler};
use crate::candidate::CandidateWorkspace;
use crate::measure::CodingCandidateMeasurer;
use crate::runtime::Psp9RunConfig;
use crate::runtime::node::{
    CodingContract, loop_kernel_state, session_grant_policy, worker_capability,
};
use crate::toolloop::{EffectExecutor as _, EffectOutcome, LoopBudgets, PromptEnvelope, ToolLoop};
use crate::turn::{ActorKind, ActorTurnRunner};

/// Runtime hook backing `spawn_agent` inside [`CandidateWorkspace`].
#[async_trait::async_trait]
pub trait SubagentRunner: Send + Sync {
    async fn run_subagent(
        &self,
        parent: &CandidateWorkspace,
        role: &str,
        goal: &str,
    ) -> Result<EffectOutcome>;
}

pub(crate) struct SpawnAgentHandler;

#[async_trait::async_trait]
impl CandidateToolHandler for SpawnAgentHandler {
    async fn apply(
        &self,
        workspace: &CandidateWorkspace,
        call: &ProviderToolCall,
        _entry: &ToolEntry,
    ) -> Result<EffectOutcome> {
        let role = call
            .arguments
            .get("role")
            .and_then(serde_json::Value::as_str)
            .context("spawn_agent requires a string 'role' argument")?;
        let goal = call
            .arguments
            .get("goal")
            .and_then(serde_json::Value::as_str)
            .context("spawn_agent requires a string 'goal' argument")?;
        let Some(runner) = workspace.subagent_runner() else {
            return Ok(tool_failure(
                "spawn_agent is disabled or withheld for this agent",
            ));
        };
        runner.run_subagent(workspace, role, goal).await
    }
}

pub(crate) struct RuntimeSubagentRunner {
    pub(crate) transport: Arc<dyn ModelTransport>,
    pub(crate) model: ModelId,
    pub(crate) fallback_models: Vec<ModelId>,
    pub(crate) domain: Arc<dyn AgentDomainPackage>,
    pub(crate) tool_handlers: Arc<CandidateHandlerRegistry>,
    pub(crate) extra_tool_entries: Vec<ToolEntry>,
    pub(crate) interaction_handler: Arc<dyn InteractionHandler>,
    pub(crate) config: Psp9RunConfig,
    pub(crate) exclusions: Vec<std::path::PathBuf>,
}

#[async_trait::async_trait]
impl SubagentRunner for RuntimeSubagentRunner {
    async fn run_subagent(
        &self,
        parent: &CandidateWorkspace,
        role: &str,
        goal: &str,
    ) -> Result<EffectOutcome> {
        match role {
            "explorer" => {
                self.run_readonly_subagent(parent, role, ActorKind::Explorer, goal)
                    .await
            }
            "reviewer" => {
                self.run_readonly_subagent(parent, role, ActorKind::Adjudicator, goal)
                    .await
            }
            "worker" => self.run_worker_subagent(parent, goal).await,
            _ => Ok(tool_failure(&format!(
                "unknown spawn_agent role {role:?}; expected explorer | worker | reviewer"
            ))),
        }
    }
}

impl RuntimeSubagentRunner {
    fn open_child(&self, parent: &CandidateWorkspace, name: &str) -> Result<CandidateWorkspace> {
        let mut child = CandidateWorkspace::create_filtered(
            parent.overlay_root(),
            name,
            0,
            "subagent",
            self.config.allow_unisolated_verifiers,
            &self.exclusions,
        )?;
        child.set_tool_handlers(self.tool_handlers.clone());
        child.set_interaction_handler(self.interaction_handler.clone());
        child.set_verifier_timeouts(self.config.verifier_timeouts);
        // Theorem 1 attenuation: children never receive a subagent runner.
        Ok(child)
    }

    async fn run_readonly_subagent(
        &self,
        parent: &CandidateWorkspace,
        role: &str,
        actor: ActorKind,
        goal: &str,
    ) -> Result<EffectOutcome> {
        let child = self.open_child(parent, &format!("subagent-{role}"))?;
        let catalog = StaticCatalog::with_base(self.extra_tool_entries.clone())?;
        let capability = exploration_capability(ActorId::new(format!("subagent-{role}")));
        let specs = catalog.specs_for(std::slice::from_ref(&capability), true);
        let kernel_state = now_kernel_state();
        let mut conversation = Conversation::with_system(format!(
            "You are a read-only {role} subagent. Inspect the workspace and answer \
             concisely. Write tools and subagents are withheld by capability policy."
        ));
        conversation.push_user(goal.to_string());
        for turn in 0..4u32 {
            let mut runner = ActorTurnRunner {
                transport: self.transport.as_ref(),
                model: self.model.clone(),
                fallbacks: self.fallback_models.clone(),
                recorder: None,
                actor,
                deadline_secs: self.config.turn_deadline_secs,
                turn: turn + 1,
            };
            match runner
                .run_turn(&conversation, &specs, ToolChoicePolicy::Auto)
                .await?
            {
                TurnOutput::Text(answer) => {
                    return Ok(EffectOutcome {
                        output: answer,
                        mutated: false,
                        completed: true,
                    });
                }
                TurnOutput::ToolCalls(calls) => {
                    conversation.push_tool_calls(calls.clone());
                    for call in calls {
                        let out = apply_readonly_call(
                            &child,
                            &catalog,
                            &capability,
                            &kernel_state,
                            &call,
                        )
                        .await?;
                        conversation.push_tool_response(call.call_id, out);
                    }
                }
            }
        }
        Ok(tool_failure(&format!(
            "{role} subagent did not finish within 4 turns"
        )))
    }

    fn worker_budgets(&self, rho_floor: f64) -> LoopBudgets {
        LoopBudgets {
            max_turns: 6,
            max_calls_per_turn: self.config.max_calls_per_turn.max(1),
            rejection_budget: 2,
            rho_gate: self.config.rho_gate.max(rho_floor),
            declared_energy_floor: None,
            context_soft_limit_chars: 120_000,
            recovery_budget: 2,
            turn_deadline_secs: self.config.turn_deadline_secs,
            resident: self.config.resident,
        }
    }

    async fn run_worker_subagent(
        &self,
        parent: &CandidateWorkspace,
        goal: &str,
    ) -> Result<EffectOutcome> {
        let child = self.open_child(parent, "subagent-worker")?;
        let catalog = StaticCatalog::with_base(self.extra_tool_entries.clone())?;
        let (capability, kernel_state) = mint_worker_capability(parent.overlay_root(), &catalog)?;
        let contract = CodingContract {
            graph_revision: "subagent".into(),
            node_id: "subagent-worker".into(),
            generation: 0,
            policy: perspt_policy::engine::PolicyEngine::new()?,
        };
        let barrier = OperationalSafetyBarrier::default();
        let scope = perspt_sdk::DomainScope {
            label: "subagent-worker".into(),
            paths: Vec::new(),
        };
        let cadence = self.domain.verifier_suite(&scope).cadence;
        let energy = self.domain.energy_model(&scope);
        let measurer = CodingCandidateMeasurer::new(&child, "subagent-worker", 0)
            .with_domain(self.domain.clone())
            .with_max_parallel(self.config.max_parallel_verifiers)
            .with_require_format(self.config.require_format)
            .with_test_policy(self.config.test_policy, self.config.external_oracle.clone());
        let outcome = ToolLoop {
            transport: self.transport.as_ref(),
            model: self.model.clone(),
            fallback_models: self.fallback_models.clone(),
            catalog: &catalog,
            capabilities: vec![capability],
            contract: Some(&contract),
            barrier: Some(&barrier),
            c_c_max: 0.25,
            executor: &child,
            measurer: &measurer,
            budgets: self.worker_budgets(energy.rho_gate),
            cadence,
            kernel_state,
            node_id: "subagent-worker".into(),
            generation: 0,
            system_prompt: PromptEnvelope {
                text: format!(
                    "You are an isolated child worker subagent. Fulfill the goal strictly \
                     within the candidate overlay; recursive spawn_agent is withheld.\nGoal: {goal}"
                ),
                ..Default::default()
            },
            recorder: None,
            partial_seed_root: None,
            resumed_birth_deps: Vec::new(),
            search_budget: None,
        }
        .run(goal)
        .await?;
        finish_worker_outcome(parent, &child, &outcome).await
    }
}

fn mint_worker_capability(
    root: &std::path::Path,
    catalog: &StaticCatalog,
) -> Result<(Capability, KernelState)> {
    // Empty opt-in slice withholds `SpawnAgent` and `MutateDependencies`.
    let raw = worker_capability("subagent-worker", "subagent", 0, 0, catalog, &[]);
    let grant = session_grant_policy(root, "subagent", false, catalog, &[])?;
    let minted = grant
        .mint(raw)
        .map_err(|e| anyhow::anyhow!("child grant intersection: {e}"))?;
    Ok((minted, loop_kernel_state(&grant, "subagent")))
}

async fn finish_worker_outcome(
    parent: &CandidateWorkspace,
    child: &CandidateWorkspace,
    outcome: &crate::toolloop::LoopOutcome,
) -> Result<EffectOutcome> {
    if matches!(outcome.outcome, NodeTerminalOutcome::HardPass) {
        let imported = parent.import_child_mutations(child).await?;
        let summary = serde_json::json!({
            "status": "hard_pass",
            "turns_used": outcome.turns_used,
            "imported_paths": imported,
        });
        return Ok(EffectOutcome {
            output: summary.to_string(),
            mutated: false,
            completed: true,
        });
    }
    let summary = serde_json::json!({
        "status": "did_not_pass",
        "turns_used": outcome.turns_used,
        "best_energy": outcome.trajectory.best_accepted_energy,
        "imported_paths": Vec::<String>::new(),
    });
    Ok(tool_failure(&format!(
        "worker subagent did not reach HardPass; mutations discarded ({summary})"
    )))
}

async fn apply_readonly_call(
    child: &CandidateWorkspace,
    catalog: &StaticCatalog,
    capability: &Capability,
    kernel_state: &KernelState,
    call: &ProviderToolCall,
) -> Result<String> {
    let Some(entry) = catalog.lookup(&call.name).cloned() else {
        return Ok(format!("denied: unknown tool {:?}", call.name));
    };
    let mut proposal = EffectProposal::new(capability.holder.clone(), "subagent-ro", entry.effect);
    if let Some(path) = call.arguments.get("path").and_then(|v| v.as_str()) {
        proposal = proposal.with_path(path);
    }
    let witness =
        perspt_sdk::check_admissibility(&proposal, std::slice::from_ref(capability), kernel_state);
    if !matches!(witness.decision, perspt_sdk::AdmissibilityDecision::Allow) {
        return Ok(format!("denied: {:?}", witness.decision));
    }
    let outcome = child.apply(call, &entry).await?;
    Ok(outcome.output)
}

fn now_kernel_state() -> KernelState {
    let mut state = KernelState::new();
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    state.set_witness("__now", now_unix.to_string());
    state
}

fn tool_failure(message: &str) -> EffectOutcome {
    EffectOutcome {
        output: format!("tool failed: {message}"),
        mutated: false,
        completed: true,
    }
}

pub(crate) fn register(registry: &mut CandidateHandlerRegistry) {
    registry
        .register("spawn_agent", Arc::new(SpawnAgentHandler))
        .expect("builtin spawn_agent registration must succeed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use perspt_sdk::EffectKind;

    #[test]
    fn child_capabilities_never_grant_spawn_agent() {
        let catalog = StaticCatalog::with_base(Vec::new()).unwrap();
        let ro = exploration_capability(ActorId::new("subagent-explorer"));
        assert!(!ro.effects.contains(&EffectKind::SpawnAgent));
        assert!(!ro.effects.contains(&EffectKind::WriteArtifact));

        let child_worker = worker_capability("s", "r", 0, 0, &catalog, &[]);
        assert!(!child_worker.effects.contains(&EffectKind::SpawnAgent));
        assert!(
            !child_worker
                .effects
                .contains(&EffectKind::MutateDependencies)
        );
    }
}
