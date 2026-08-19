//! Per-call prompt and resident-context binding for the worker loop
//! (PSP-10 systems 23–24, Gate Z and Definition 6).

use anyhow::Result;

use super::{emit, resident, LoopContext, LoopEvent, PromptEnvelope, ToolLoop, TurnState};

/// The live per-call prompt identity (recompiled when the tool surface or
/// the failover route changes).
#[derive(Default)]
pub(super) struct PromptBinding {
    pub(super) surface_hash: String,
    pub(super) route_key: String,
    pub(super) invocation_digest: String,
    pub(super) platform_digest: String,
    pub(super) domain_digest: String,
    pub(super) resident_digest: String,
}

impl PromptBinding {
    /// The binding the seed-time envelope compiled, when one exists.
    pub(super) fn seed(envelope: &PromptEnvelope, model: &perspt_sdk::ModelId) -> Self {
        match &envelope.invocation {
            Some(invocation) => Self {
                surface_hash: invocation.platform.tool_spec_hash.clone(),
                route_key: format!("{model}"),
                invocation_digest: invocation.invocation_digest.clone(),
                platform_digest: invocation.platform.program_digest.clone(),
                domain_digest: invocation.domain.program_digest.clone(),
                resident_digest: String::new(),
            },
            None => Self::default(),
        }
    }
}

impl ToolLoop<'_> {
    /// Definition 6, live: assemble the resident working set over the
    /// conversation's content-addressed pages before every transport call.
    /// An infeasible mandatory closure records `ContextInfeasible` and
    /// makes no model call; the resident digest enters the control frame
    /// and the per-call invocation record.
    pub(super) fn assemble_resident_context(
        &self,
        turn: u32,
        specs: &[perspt_sdk::ToolSpec],
        context: &LoopContext,
        state: &mut TurnState,
    ) -> Result<()> {
        let accountant = perspt_sdk::prompt::TokenAccountantRef::approx_bytes_v1();
        let tool_reserve: u64 = specs
            .iter()
            .map(|spec| {
                accountant.count_text(&format!("{}{}{}", spec.name, spec.description, spec.schema))
            })
            .sum();
        let window = u64::from(
            self.transport
                .capabilities(&self.model)
                .max_context_tokens
                .max(1),
        );
        let outcome = resident::assemble_worker_resident(
            context.conversation(),
            &state.accepted_checkpoint.witness.state_root,
            window,
            tool_reserve,
            &self.budgets.resident,
        )
        .map_err(|e| anyhow::anyhow!("resident assembly: {e}"))?;
        match outcome {
            perspt_sdk::prompt::ResidentOutcome::Infeasible {
                required,
                allowance,
            } => {
                emit(
                    self.recorder,
                    &mut state.log,
                    LoopEvent::ContextInfeasible {
                        forest_id: String::new(),
                        branch_id: String::new(),
                        turn,
                        required,
                        allowance,
                    },
                )?;
                anyhow::bail!(
                    "context budget infeasible: the mandatory closure needs {required} \
                     tokens over the {allowance}-token input allowance; no model call \
                     was made"
                );
            }
            perspt_sdk::prompt::ResidentOutcome::Assembled(assembled) => {
                self.record_working_set(turn, &assembled, state)?;
            }
        }
        Ok(())
    }

    /// Ledger the assembled working set: the selection (with its digest)
    /// when it changed, and the per-call resident page list always.
    fn record_working_set(
        &self,
        turn: u32,
        assembled: &perspt_sdk::prompt::ResidentContext,
        state: &mut TurnState,
    ) -> Result<()> {
        let page_ids: Vec<String> = assembled
            .pages
            .iter()
            .map(|page| page.page_id.clone())
            .collect();
        if state.prompt.resident_digest != assembled.resident_digest {
            emit(
                self.recorder,
                &mut state.log,
                LoopEvent::ContextPagesSelected {
                    forest_id: String::new(),
                    branch_id: String::new(),
                    turn,
                    resident_digest: assembled.resident_digest.clone(),
                    page_ids: page_ids.clone(),
                },
            )?;
        }
        emit(
            self.recorder,
            &mut state.log,
            LoopEvent::ContextWorkingSet {
                forest_id: String::new(),
                branch_id: String::new(),
                turn,
                page_ids,
            },
        )?;
        state.prompt.resident_digest = assembled.resident_digest.clone();
        Ok(())
    }

    /// Ledger the exact prompt binding of this call (PSP-10 Gate Z):
    /// recompile through the SDK compiler when the offered tool surface or
    /// the failover route changed, then record one invocation per call.
    pub(super) fn bind_prompt_program(
        &self,
        turn: u32,
        specs: &[perspt_sdk::ToolSpec],
        state: &mut TurnState,
    ) -> Result<()> {
        let Some(stage) = &self.system_prompt.stage else {
            return Ok(());
        };
        let surface_hash = perspt_sdk::prompt::tool_surface_hash(specs);
        let route_key = format!("{}", self.model);
        if state.prompt.surface_hash != surface_hash || state.prompt.route_key != route_key {
            let (route, dialect) = crate::turn::route_dialect(self.transport, &self.model);
            let invocation = perspt_core::prompts::compile_invocation(
                stage,
                &self.system_prompt.domain_sections,
                &route,
                &dialect,
                &surface_hash,
            )
            .map_err(|e| anyhow::anyhow!("prompt recompilation failed closed: {e}"))?;
            emit(
                self.recorder,
                &mut state.log,
                LoopEvent::PromptProgramCompiled {
                    turn,
                    program: invocation.platform.clone(),
                },
            )?;
            if !invocation.domain.messages.is_empty() {
                emit(
                    self.recorder,
                    &mut state.log,
                    LoopEvent::PromptProgramCompiled {
                        turn,
                        program: invocation.domain.clone(),
                    },
                )?;
            }
            state.prompt = PromptBinding {
                surface_hash,
                route_key,
                invocation_digest: invocation.invocation_digest.clone(),
                platform_digest: invocation.platform.program_digest.clone(),
                domain_digest: invocation.domain.program_digest.clone(),
                resident_digest: std::mem::take(&mut state.prompt.resident_digest),
            };
        }
        emit(
            self.recorder,
            &mut state.log,
            LoopEvent::PromptProgramInvoked {
                turn,
                invocation_digest: state.prompt.invocation_digest.clone(),
                platform_digest: state.prompt.platform_digest.clone(),
                domain_digest: state.prompt.domain_digest.clone(),
                tool_spec_hash: state.prompt.surface_hash.clone(),
                resident_context_digest: state.prompt.resident_digest.clone(),
            },
        )
    }
}
