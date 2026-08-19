//! Intra-turn commuting tool-call batches (Gate P).
//!
//! Theorem 8 makes the ordering rule precise: for *commuting* calls,
//! arrival order is not an "invented order" — every order yields the same
//! state — so commuting mutators run sequentially in the model's own
//! arrival order, and only identity transitions (reads with precise
//! declared footprints) get true concurrency. A pair of same-turn calls
//! whose footprints do **not** commute, where at least one mutates, is
//! never ordered by the host: the later call is returned to the model as a
//! [`ResidualClass::ToolBatchConflict`] observation so the model can
//! re-sequence it across turns.
//!
//! Provider rate-limit delays live in the transport and never appear here —
//! a throughput wait is not a required serialization (Gate P).

use perspt_sdk::{EffectKind, ProviderToolCall, Resource, StaticCatalog, ToolCatalog};

/// How the planner classified one call of the turn, in arrival order.
#[derive(Debug)]
pub(crate) enum PlannedClass {
    /// Identity transition with a precise footprint: eligible for
    /// concurrent execution after sequential admission.
    ConcurrentRead,
    /// Executed alone in arrival order: mutators (commuting by disjoint
    /// footprints), opaque reads (verifier processes), host-surface tools,
    /// and unknown names (their denial path needs the ordinary flow).
    Sequential,
    /// Footprint collides with an earlier same-turn call and at least one
    /// side mutates: returned as an observation, never ordered.
    Conflict {
        conflicts_with: String,
        resources: Vec<String>,
    },
}

#[derive(Debug)]
pub(crate) struct PlannedCall {
    pub(crate) call: ProviderToolCall,
    pub(crate) class: PlannedClass,
}

/// Pure batch planning over the declared footprints. Greedy in arrival
/// order: each call joins the batch unless it collides with an earlier
/// admitted call.
pub(crate) fn plan_batch(calls: &[ProviderToolCall], catalog: &StaticCatalog) -> Vec<PlannedCall> {
    struct Admitted {
        call_id: String,
        footprint: perspt_sdk::Footprint,
        mutating: bool,
    }
    let mut admitted: Vec<Admitted> = Vec::new();
    let mut planned = Vec::new();
    for call in calls {
        let Some(entry) = catalog.lookup(&call.name) else {
            planned.push(PlannedCall {
                call: call.clone(),
                class: PlannedClass::Sequential,
            });
            continue;
        };
        let footprint = entry.footprint.resolve(&call.arguments, "");
        let mutating = super::candidate_mutating_effect(entry.effect);
        let host = matches!(
            entry.effect,
            EffectKind::ToolSearch | EffectKind::ToolProgram | EffectKind::AskUser
        );
        let opaque_read = !mutating && footprint.reads.contains(&Resource::OpaqueWorkspace);

        let collision = admitted.iter().find(|earlier| {
            (mutating || earlier.mutating) && earlier.footprint.conflicts_with(&footprint)
        });
        if let Some(earlier) = collision {
            let resources: Vec<String> = footprint
                .writes
                .iter()
                .chain(footprint.reads.iter())
                .map(|resource| format!("{resource:?}"))
                .collect();
            planned.push(PlannedCall {
                call: call.clone(),
                class: PlannedClass::Conflict {
                    conflicts_with: earlier.call_id.clone(),
                    resources,
                },
            });
            continue;
        }
        admitted.push(Admitted {
            call_id: call.call_id.clone(),
            footprint,
            mutating,
        });
        planned.push(PlannedCall {
            call: call.clone(),
            class: if !mutating && !host && !opaque_read {
                PlannedClass::ConcurrentRead
            } else {
                PlannedClass::Sequential
            },
        });
    }
    planned
}

use anyhow::{Context as _, Result};

use super::{
    bounded_model_output, candidate_mutating_effect, emit, promote_matching_capability,
    proposal_from, proposal_scope, uncertified_reason, EventLog, LoopContext, LoopEvent, ToolLoop,
    TurnBudget,
};
use crate::realize::ProjectionMismatch;
use perspt_sdk::{CandidateTransition, ToolEntry, TurnOutput};

impl ToolLoop<'_> {
    /// Route every returned call through the kernel; execute the admitted
    /// ones. Returns the number of admitted mutations.
    pub(super) async fn execute_turn(
        &mut self,
        output: &TurnOutput,
        turn: u32,
        max_mutations: u32,
        context: &mut LoopContext,
        log: &mut EventLog,
        projection: &mut ProjectionMismatch,
    ) -> Result<(u32, bool)> {
        let calls = output.tool_calls().to_vec();
        if calls.is_empty() {
            if let TurnOutput::Text(text) = output {
                context.push_message(
                    perspt_sdk::Message::Assistant {
                        content: text.clone(),
                    },
                    self.recorder,
                    log,
                )?;
            }
            return Ok((0, false));
        }
        // The recall store: every projection page by id, built only when
        // this turn actually asks for one (Definition 6 backing store).
        let recall = calls
            .iter()
            .any(|call| call.name == "context_recall")
            .then(|| super::resident::page_contents(context.conversation()));
        context.push_tool_calls(calls.clone(), self.recorder, log)?;

        let plan = plan_batch(&calls, self.catalog);
        let mut budget = TurnBudget::new(max_mutations);
        let mut deferred: Vec<(ProviderToolCall, ToolEntry)> = Vec::new();
        let mut responses: std::collections::BTreeMap<String, String> = Default::default();
        for planned in &plan {
            self.route_planned(
                planned,
                turn,
                recall.as_ref(),
                &mut budget,
                &mut deferred,
                &mut responses,
                context,
                log,
                projection,
            )
            .await?;
        }

        // Identity transitions run with true concurrency; their events and
        // responses are still emitted in arrival order so the recorded
        // chain stays deterministic (Gate P).
        for (call_id, output) in self.apply_reads_concurrently(&deferred, log).await? {
            responses.insert(call_id, output);
        }
        for call in &calls {
            let response = responses
                .remove(&call.call_id)
                .unwrap_or_else(|| "internal: response missing".into());
            context.push_tool_response(call.call_id.clone(), response, self.recorder, log)?;
        }
        Ok((budget.mutations, budget.immediate_boundary))
    }

    /// Dispatch one planned call by its Gate P class.
    #[allow(clippy::too_many_arguments)]
    async fn route_planned(
        &mut self,
        planned: &PlannedCall,
        turn: u32,
        recall: Option<&std::collections::BTreeMap<String, String>>,
        budget: &mut TurnBudget,
        deferred: &mut Vec<(ProviderToolCall, ToolEntry)>,
        responses: &mut std::collections::BTreeMap<String, String>,
        context: &mut LoopContext,
        log: &mut EventLog,
        projection: &mut ProjectionMismatch,
    ) -> Result<()> {
        let call = &planned.call;
        emit(
            self.recorder,
            log,
            LoopEvent::ToolCallObserved { call: call.clone() },
        )?;
        match &planned.class {
            PlannedClass::Conflict {
                conflicts_with,
                resources,
            } => {
                let observation =
                    self.conflict_observation(call, conflicts_with, resources, log)?;
                responses.insert(call.call_id.clone(), observation);
            }
            PlannedClass::ConcurrentRead => {
                let entry = self
                    .catalog
                    .lookup(&call.name)
                    .cloned()
                    .expect("planner classified a cataloged read");
                match self
                    .screen_call(call, Some(&entry), budget, log, projection)
                    .await?
                {
                    Some(denial) => {
                        responses.insert(call.call_id.clone(), denial);
                    }
                    None => match self.admit_read(call, &entry, log, projection).await? {
                        Ok(()) => deferred.push((call.clone(), entry)),
                        Err(denial) => {
                            responses.insert(call.call_id.clone(), denial);
                        }
                    },
                }
            }
            PlannedClass::Sequential => {
                let response = self
                    .sequential_call(call, turn, recall, budget, context, log, projection)
                    .await?;
                responses.insert(call.call_id.clone(), response);
            }
        }
        Ok(())
    }

    /// Per-call screening shared by every class: argument validation and
    /// the per-turn call/mutation budgets. `Some(response)` is a recorded
    /// denial.
    async fn screen_call(
        &mut self,
        call: &ProviderToolCall,
        entry: Option<&ToolEntry>,
        budget: &mut TurnBudget,
        log: &mut EventLog,
        projection: &mut ProjectionMismatch,
    ) -> Result<Option<String>> {
        let mutating = entry.is_some_and(|entry| candidate_mutating_effect(entry.effect));
        let invalid = entry
            .and_then(|entry| entry.validate_arguments(&call.arguments).err())
            .map(|error| anyhow::anyhow!(error.to_string()));
        let denial = self.budget_denial(
            budget.calls_seen,
            mutating,
            budget.mutations,
            budget.max_mutations,
        );
        budget.calls_seen = budget.calls_seen.saturating_add(1);
        match (invalid, denial) {
            (Some(reason), _) => {
                self.record_unchecked_proposal(call, entry.expect("validated entry"), log)
                    .await?;
                Ok(Some(self.deny(
                    log,
                    projection,
                    &call.call_id,
                    reason.to_string(),
                    perspt_sdk::ResidualClass::ToolArgumentInvalid,
                )?))
            }
            (None, Some(reason)) => {
                if let Some(entry) = entry {
                    self.record_unchecked_proposal(call, entry, log).await?;
                }
                Ok(Some(self.deny(
                    log,
                    projection,
                    &call.call_id,
                    reason.to_string(),
                    perspt_sdk::ResidualClass::BudgetExhausted,
                )?))
            }
            (None, None) => Ok(None),
        }
    }

    /// The ordinary sequential path: screen, kernel-check, execute, and
    /// post-process the host-surface tools.
    #[allow(clippy::too_many_arguments)]
    async fn sequential_call(
        &mut self,
        call: &ProviderToolCall,
        turn: u32,
        recall: Option<&std::collections::BTreeMap<String, String>>,
        budget: &mut TurnBudget,
        context: &mut LoopContext,
        log: &mut EventLog,
        projection: &mut ProjectionMismatch,
    ) -> Result<String> {
        let entry = self.catalog.lookup(&call.name).cloned();
        let mutating = entry
            .as_ref()
            .is_some_and(|entry| candidate_mutating_effect(entry.effect));
        if let Some(denial) = self
            .screen_call(call, entry.as_ref(), budget, log, projection)
            .await?
        {
            return Ok(denial);
        }
        let mut response = self
            .check_and_apply(call, recall, log, projection, &mut budget.mutations)
            .await?;
        if call.name == "context_recall" && !response.starts_with("denied:") {
            self.record_recall(call, turn, &response, log)?;
        }
        if call.name == "tool_search" && !response.starts_with("denied:") {
            // The response *is* the executed search result; activate from
            // it instead of running the search a second time.
            if let Ok(specs) = serde_json::from_str::<Vec<perspt_sdk::ToolSpec>>(&response) {
                for spec in specs {
                    context.activate_tool(&spec.name, self.recorder, log)?;
                }
            }
        }
        if call.name == "tool_program" && !response.starts_with("denied:") {
            response = self
                .run_tool_program(call, &response, budget, log, projection)
                .await?;
        }
        if mutating && Self::high_risk(entry.as_ref()) && budget.mutations > 0 {
            budget.immediate_boundary = true;
        }
        Ok(response)
    }

    /// Admit one identity transition for concurrent execution: proposal,
    /// five-clause certification, capability debit. `Err(response)` is a
    /// recorded denial.
    async fn admit_read(
        &mut self,
        call: &ProviderToolCall,
        entry: &ToolEntry,
        log: &mut EventLog,
        projection: &mut ProjectionMismatch,
    ) -> Result<std::result::Result<(), String>> {
        let scope = proposal_scope(call, entry);
        let before = self.executor.checkpoint(&scope).await?;
        let proposal = proposal_from(call, entry, &self.node_id, self.generation, &before.witness);
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
        let identity = CandidateTransition::new(
            proposal.clone(),
            before.witness.clone(),
            before.witness.clone(),
        );
        let witness = self.certify(&call.call_id, &identity, log)?;
        if let Some(reason) = uncertified_reason(&witness) {
            return Ok(Err(self.deny(
                log,
                projection,
                &call.call_id,
                reason,
                perspt_sdk::ResidualClass::CapabilityDenied,
            )?));
        }
        promote_matching_capability(&mut self.capabilities, &witness)
            .map_err(|error| anyhow::anyhow!("promotion: {error}"))?;
        Ok(Ok(()))
    }

    /// Run every admitted identity transition concurrently, then record
    /// outputs in arrival order.
    async fn apply_reads_concurrently(
        &mut self,
        admitted: &[(ProviderToolCall, ToolEntry)],
        log: &mut EventLog,
    ) -> Result<Vec<(String, String)>> {
        let executor = self.executor;
        let outcomes = futures::future::join_all(
            admitted
                .iter()
                .map(|(call, entry)| executor.apply(call, entry)),
        )
        .await;
        let mut results = Vec::with_capacity(admitted.len());
        for ((call, _), outcome) in admitted.iter().zip(outcomes) {
            let output = bounded_model_output(self.recorder, outcome?.output)?;
            emit(
                self.recorder,
                log,
                LoopEvent::EffectApplied {
                    call_id: call.call_id.clone(),
                    mutated: false,
                    output: output.clone(),
                },
            )?;
            results.push((call.call_id.clone(), output));
        }
        Ok(results)
    }

    /// Ledger one `context_recall` outcome: a restored page or a typed
    /// miss (Definition 6 recall alphabet).
    fn record_recall(
        &self,
        call: &ProviderToolCall,
        turn: u32,
        response: &str,
        log: &mut EventLog,
    ) -> Result<()> {
        let page_id = call
            .arguments
            .get("page_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let event = if response.starts_with("recalled page ") {
            LoopEvent::ContextPageRecalled {
                forest_id: String::new(),
                branch_id: String::new(),
                turn,
                page_id,
            }
        } else {
            LoopEvent::ContextMiss {
                forest_id: String::new(),
                branch_id: String::new(),
                turn,
                key: page_id,
            }
        };
        emit(self.recorder, log, event)
    }

    /// A same-turn footprint collision (Gate P): recorded and returned as
    /// an observation; the call executes nothing and consumes no budget.
    fn conflict_observation(
        &self,
        call: &ProviderToolCall,
        conflicts_with: &str,
        resources: &[String],
        log: &mut EventLog,
    ) -> Result<String> {
        emit(
            self.recorder,
            log,
            LoopEvent::ToolBatchConflict {
                call_id: call.call_id.clone(),
                conflicts_with: conflicts_with.to_string(),
                resources: resources.to_vec(),
            },
        )?;
        Ok(format!(
            "conflict: this call's footprint collides with same-turn call {conflicts_with} \
             (resources: {}); it was not executed and consumed no budget. Re-issue it in a \
             later turn if still needed.",
            resources.join(", ")
        ))
    }

    /// Execute a validated tool program's nested calls, each returning to the
    /// same kernel and budgets as a top-level call. Nested calls run
    /// sequentially in program order (the program author already committed
    /// to that order).
    async fn run_tool_program(
        &mut self,
        call: &ProviderToolCall,
        response: &str,
        budget: &mut TurnBudget,
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
            let nested_entry = self.catalog.lookup(&nested_call.name).cloned();
            let nested_mutating = nested_entry
                .as_ref()
                .is_some_and(|entry| candidate_mutating_effect(entry.effect));
            let nested_high_risk = Self::high_risk(nested_entry.as_ref());
            let result = match self
                .screen_call(&nested_call, nested_entry.as_ref(), budget, log, projection)
                .await?
            {
                Some(denial) => denial,
                None => {
                    self.check_and_apply(&nested_call, None, log, projection, &mut budget.mutations)
                        .await?
                }
            };
            if nested_mutating && nested_high_risk && budget.mutations > 0 {
                budget.immediate_boundary = true;
            }
            nested_results.push(serde_json::json!({
                "call_id": nested_call.call_id,
                "tool": nested_call.name,
                "result": result,
            }));
        }
        Ok(serde_json::to_string(&nested_results)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> StaticCatalog {
        StaticCatalog::with_base(Vec::new()).unwrap()
    }

    fn call(id: &str, name: &str, arguments: serde_json::Value) -> ProviderToolCall {
        ProviderToolCall {
            call_id: id.into(),
            name: name.into(),
            arguments,
        }
    }

    #[test]
    fn same_path_double_edit_is_a_conflict_observation() {
        let plan = plan_batch(
            &[
                call(
                    "e1",
                    "edit_file",
                    serde_json::json!({"path": "a.rs", "old_string": "x", "new_string": "y"}),
                ),
                call(
                    "e2",
                    "edit_file",
                    serde_json::json!({"path": "a.rs", "old_string": "p", "new_string": "q"}),
                ),
            ],
            &catalog(),
        );
        assert!(matches!(plan[0].class, PlannedClass::Sequential));
        let PlannedClass::Conflict { conflicts_with, .. } = &plan[1].class else {
            panic!("second same-path edit must be a conflict");
        };
        assert_eq!(conflicts_with, "e1");
    }

    #[test]
    fn disjoint_path_edits_both_apply_in_arrival_order() {
        let plan = plan_batch(
            &[
                call(
                    "e1",
                    "edit_file",
                    serde_json::json!({"path": "a.rs", "old_string": "x", "new_string": "y"}),
                ),
                call(
                    "e2",
                    "edit_file",
                    serde_json::json!({"path": "b.rs", "old_string": "p", "new_string": "q"}),
                ),
            ],
            &catalog(),
        );
        assert!(matches!(plan[0].class, PlannedClass::Sequential));
        assert!(matches!(plan[1].class, PlannedClass::Sequential));
    }

    #[test]
    fn read_after_write_on_the_same_path_is_a_conflict() {
        let plan = plan_batch(
            &[
                call(
                    "w1",
                    "write_file",
                    serde_json::json!({"path": "a.rs", "content": "x"}),
                ),
                call("r1", "read_file", serde_json::json!({"path": "a.rs"})),
            ],
            &catalog(),
        );
        assert!(matches!(plan[1].class, PlannedClass::Conflict { .. }));
    }

    #[test]
    fn multiple_reads_run_concurrently() {
        let plan = plan_batch(
            &[
                call("r1", "read_file", serde_json::json!({"path": "a.rs"})),
                call("r2", "read_file", serde_json::json!({"path": "a.rs"})),
                call("r3", "grep", serde_json::json!({"query": "fn"})),
            ],
            &catalog(),
        );
        assert!(plan
            .iter()
            .all(|p| matches!(p.class, PlannedClass::ConcurrentRead)));
    }

    #[test]
    fn verifier_processes_stay_sequential_but_never_conflict_with_each_other() {
        let plan = plan_batch(
            &[
                call("t1", "run_test", serde_json::json!({})),
                call("b1", "run_build", serde_json::json!({})),
            ],
            &catalog(),
        );
        assert!(matches!(plan[0].class, PlannedClass::Sequential));
        assert!(matches!(plan[1].class, PlannedClass::Sequential));
    }

    #[test]
    fn unknown_tools_take_the_sequential_denial_path() {
        let plan = plan_batch(
            &[call("u1", "no_such_tool", serde_json::json!({}))],
            &catalog(),
        );
        assert!(matches!(plan[0].class, PlannedClass::Sequential));
    }
}
