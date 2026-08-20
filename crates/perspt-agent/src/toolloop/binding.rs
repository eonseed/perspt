//! Per-call prompt and resident-context binding for the worker loop
//! (PSP-10 systems 23–24, Gate Z and Definition 6).

use anyhow::Result;

use super::{emit, resident, LoopContext, LoopEvent, PromptEnvelope, ToolLoop, TurnState};

/// The serialized token and byte cost of one composed request.
fn wire_cost(
    accountant: &perspt_sdk::prompt::TokenAccountantRef,
    conversation: &perspt_sdk::Conversation,
) -> (u64, u64) {
    let mut tokens = 0u64;
    let mut bytes = 0u64;
    for message in conversation.messages() {
        let serialized = serde_json::to_string(message).unwrap_or_default();
        tokens += accountant.count_message(&serialized);
        bytes += serialized.len() as u64;
    }
    (tokens, bytes)
}

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
    /// The token reserve the offered tool schemas consume on the wire.
    pub(super) fn tool_schema_reserve(&self, specs: &[perspt_sdk::ToolSpec]) -> u64 {
        let accountant = perspt_sdk::prompt::TokenAccountantRef::approx_bytes_v1();
        specs
            .iter()
            .map(|spec| {
                accountant.count_text(&format!("{}{}{}", spec.name, spec.description, spec.schema))
            })
            .sum()
    }

    /// Definition 6's input allowance for this call.
    fn input_allowance(&self, tool_reserve: u64) -> u64 {
        let window = u64::from(
            self.transport
                .capabilities(&self.model)
                .max_context_tokens
                .max(1),
        );
        window
            .saturating_sub(self.budgets.resident.output_reserve_tokens)
            .saturating_sub(tool_reserve)
            .saturating_sub(self.budgets.resident.guard_reserve_tokens)
    }

    /// Definition 6, live: assemble the resident working set over the
    /// conversation's content-addressed pages before every transport call.
    /// An infeasible mandatory closure records `ContextInfeasible` and
    /// makes no model call. Recording happens after the composed-request
    /// fit, so the ledger carries exactly what was transmitted.
    pub(super) fn assemble_resident_context(
        &self,
        turn: u32,
        specs: &[perspt_sdk::ToolSpec],
        context: &LoopContext,
        state: &mut TurnState,
    ) -> Result<Option<perspt_sdk::prompt::ResidentContext>> {
        if !self.budgets.resident.paging_enabled {
            return Ok(None);
        }
        let tool_reserve = self.tool_schema_reserve(specs);
        let window = u64::from(
            self.transport
                .capabilities(&self.model)
                .max_context_tokens
                .max(1),
        );
        let outcome = resident::assemble_worker_resident(
            context.conversation(),
            context.birth_deps(),
            &state.accepted_checkpoint.witness.state_root,
            self.partial_seed_root.as_deref(),
            window,
            tool_reserve,
            &self.budgets.resident,
        )
        .map_err(|e| anyhow::anyhow!("resident assembly: {e}"))?;
        match outcome {
            perspt_sdk::prompt::ResidentOutcome::Infeasible {
                required,
                allowance,
            } => self.refuse_infeasible(turn, required, allowance, state),
            perspt_sdk::prompt::ResidentOutcome::Assembled(assembled) => Ok(Some(assembled)),
        }
    }

    /// Record `ContextInfeasible` and fail closed — no model call is made.
    fn refuse_infeasible<T>(
        &self,
        turn: u32,
        required: u64,
        allowance: u64,
        state: &mut TurnState,
    ) -> Result<T> {
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
            "context budget infeasible: the composed request needs {required} \
             tokens over the {allowance}-token input allowance; no model call \
             was made"
        );
    }

    /// Enforce Definition 6 on the **composed transport request**: the
    /// serialized view (tombstones included) must fit the input allowance
    /// and the dialect's byte limit. On overflow, optional pages are popped
    /// in reverse selection order and the view rebuilt; a mandatory-only
    /// view that still overflows fails closed with `ContextInfeasible`.
    /// Only then is the working set ledgered — the record is exactly what
    /// was transmitted.
    pub(super) fn fit_composed_request(
        &self,
        turn: u32,
        specs: &[perspt_sdk::ToolSpec],
        context: &LoopContext,
        assembled: Option<perspt_sdk::prompt::ResidentContext>,
        state: &mut TurnState,
    ) -> Result<perspt_sdk::Conversation> {
        let accountant = perspt_sdk::prompt::TokenAccountantRef::approx_bytes_v1();
        let allowance = self.input_allowance(self.tool_schema_reserve(specs));
        let byte_limit = crate::turn::route_dialect(self.transport, &self.model)
            .1
            .max_prompt_bytes;
        let Some(mut assembled) = assembled else {
            // Paging off (evaluation ablation): the bound still holds per
            // Gate AF — an oversized request fails closed, it is not sent.
            let conversation = context.conversation().clone();
            let (tokens, bytes) = wire_cost(&accountant, &conversation);
            if tokens > allowance || bytes > byte_limit {
                return self.refuse_infeasible(turn, tokens, allowance, state);
            }
            return Ok(conversation);
        };
        loop {
            let ids: std::collections::BTreeSet<String> = assembled
                .pages
                .iter()
                .map(|page| page.page_id.clone())
                .collect();
            let (mut view, evicted) = resident::resident_view(context.conversation(), &ids);
            if evicted == 0 {
                view = context.conversation().clone();
            } else if let Some(note) = resident::evicted_index_note(context.conversation(), &ids) {
                // The transport-only page index: the model's typed view of
                // what context_recall can restore. Never enters the
                // projection; its cost is part of the composed request.
                view.push(perspt_sdk::Message::User { content: note });
            }
            let (tokens, bytes) = wire_cost(&accountant, &view);
            if tokens <= allowance && bytes <= byte_limit {
                assembled.resident_digest = perspt_sdk::prompt::resident_page_digest(
                    assembled.pages.iter().map(|page| page.page_id.as_str()),
                );
                self.record_working_set(turn, &assembled, state)?;
                return Ok(view);
            }
            if assembled.pages.len() > assembled.mandatory_len {
                assembled.pages.pop();
                continue;
            }
            return self.refuse_infeasible(turn, tokens, allowance, state);
        }
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
