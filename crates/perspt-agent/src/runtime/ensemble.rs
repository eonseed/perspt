//! Live proposal ensembles (PSP-9 system 7, Gates M and T).
//!
//! An exploration policy at the recovery ladder's refine rung: after a gate
//! failure, draw one round of distinct-family proposers. Ensembles propose,
//! never decide — every candidate runs the ordinary governed loop, is
//! scored by the same deterministic verifier suite, consumes exactly one
//! gate decision, and selection is strictly by measured energy. No voting.

use super::*;

impl Psp9AgentRuntime {
    /// One ladder attempt: an ensemble round at the refine rung when the
    /// policy triggers, otherwise a single attempt on the current route.
    /// Returns the surviving attempt and the gate decisions consumed.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn ladder_attempt(
        &self,
        recorder: &Psp9Recorder,
        session_id: &str,
        goal: &str,
        node_id: &str,
        generation: u32,
        model: &ModelId,
        graph: &WorkGraphRevision,
        remaining_budget: u32,
        refine_rung: bool,
    ) -> Result<(NodeAttempt, u32)> {
        if refine_rung {
            if let Some(routes) = self.ensemble_routes(recorder, remaining_budget)? {
                return self
                    .run_ensemble_round(
                        recorder, session_id, goal, node_id, generation, graph, routes,
                    )
                    .await;
            }
        }
        let attempt = self
            .attempt_node(
                recorder,
                session_id,
                goal,
                node_id,
                generation,
                model,
                graph,
                None,
                remaining_budget,
            )
            .await?;
        let spent = spent_of(&attempt);
        Ok((attempt, spent))
    }

    /// Draw the round's proposer routes, or `None` when the policy refuses
    /// (disabled, insufficient budget, or no distinct families) — a refusal
    /// falls back to the ordinary single attempt and is recorded.
    fn ensemble_routes(
        &self,
        recorder: &Psp9Recorder,
        remaining_budget: u32,
    ) -> Result<Option<Vec<ModelId>>> {
        if self.ensemble.trigger == perspt_sdk::EnsembleTrigger::Never {
            return Ok(None);
        }
        let mut portfolio = vec![self.model.clone()];
        portfolio.extend(self.fallback_models.iter().cloned());
        portfolio.extend(self.handoff_model.clone());
        let family_of = |model: &ModelId| self.transport.family_of(model);
        match self
            .ensemble
            .select_round(&portfolio, &family_of, u64::from(remaining_budget))
        {
            Ok(routes) => {
                recorder.record_custom(
                    "ensemble_round",
                    serde_json::json!({
                        "routes": routes,
                        "width": routes.len(),
                        "gate_decisions_remaining": remaining_budget,
                    }),
                )?;
                Ok(Some(routes))
            }
            Err(reason) => {
                recorder.record_custom(
                    "ensemble_round_refused",
                    serde_json::json!({"reason": reason.to_string()}),
                )?;
                Ok(None)
            }
        }
    }

    /// Run the round sequentially: each proposer gets its own reversible
    /// candidate and exactly one gate decision. The surviving attempt is
    /// the measured-energy winner (hard pass first, then lowest V).
    #[allow(clippy::too_many_arguments)]
    async fn run_ensemble_round(
        &self,
        recorder: &Psp9Recorder,
        session_id: &str,
        goal: &str,
        node_id: &str,
        generation: u32,
        graph: &WorkGraphRevision,
        routes: Vec<ModelId>,
    ) -> Result<(NodeAttempt, u32)> {
        let mut spent_total = 0u32;
        let mut best: Option<NodeAttempt> = None;
        for route in routes {
            let attempt = self
                .attempt_node(
                    recorder, session_id, goal, node_id, generation, &route, graph, None, 1,
                )
                .await?;
            spent_total = spent_total.saturating_add(spent_of(&attempt).max(1));
            recorder.record_custom(
                "ensemble_candidate",
                serde_json::json!({
                    "model": route,
                    "energy": attempt.outcome.trajectory.best_accepted_energy,
                    "hard_pass": matches!(
                        attempt.outcome.outcome,
                        NodeTerminalOutcome::HardPass
                    ),
                }),
            )?;
            best = Some(match best {
                None => attempt,
                Some(current) => pick_by_energy(current, attempt),
            });
        }
        let winner = best.context("ensemble round selected no routes")?;
        recorder.record_custom(
            "ensemble_selected",
            serde_json::json!({
                "energy": winner.outcome.trajectory.best_accepted_energy,
                "hard_pass": matches!(winner.outcome.outcome, NodeTerminalOutcome::HardPass),
            }),
        )?;
        Ok((winner, spent_total))
    }
}

/// Strictly measured selection: a hard pass beats a non-pass; otherwise the
/// lower best accepted energy wins; ties keep the earlier (arrival) one.
fn pick_by_energy(current: NodeAttempt, challenger: NodeAttempt) -> NodeAttempt {
    let current_pass = matches!(current.outcome.outcome, NodeTerminalOutcome::HardPass);
    let challenger_pass = matches!(challenger.outcome.outcome, NodeTerminalOutcome::HardPass);
    if current_pass != challenger_pass {
        return if challenger_pass { challenger } else { current };
    }
    if challenger.outcome.trajectory.best_accepted_energy
        < current.outcome.trajectory.best_accepted_energy
    {
        challenger
    } else {
        current
    }
}
