//! Node attempt execution: assembly-before-workspace preparation and the
//! governed loop run, split from `runtime/mod.rs` under the file-length
//! rules. `prepare_attempt` exists before any workspace so the search
//! forest can derive exact no-good components ahead of fork admission.

use anyhow::Result;
use perspt_sdk::{ModelId, WorkGraphRevision};

use super::node::{
    loop_kernel_state, restore_seed, run_seeded, CandidateSeed, NodeAttempt, PreparedAttempt,
};
use super::{Psp9AgentRuntime, Psp9Recorder};
use crate::candidate::CodingCandidateMeasurer;
use crate::toolloop::ToolLoop;

impl Psp9AgentRuntime {
    /// One governed attempt at a node generation: fresh candidate overlay,
    /// fresh assembly, one tool loop. `seed` rebuilds an accepted candidate
    /// state from a durable checkpoint before the loop starts (resume).
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn attempt_node(
        &self,
        recorder: &Psp9Recorder,
        session_id: &str,
        goal: &str,
        node_id: &str,
        generation: u32,
        model: &ModelId,
        graph: &WorkGraphRevision,
        seed: Option<&CandidateSeed>,
        shared_recovery_budget: u32,
    ) -> Result<NodeAttempt> {
        self.attempt_node_with_recorder(
            recorder,
            session_id,
            goal,
            node_id,
            generation,
            model,
            graph,
            seed,
            shared_recovery_budget,
            recorder,
            None,
            None,
        )
        .await
    }

    /// The attempt body, parameterized over the loop's event recorder so a
    /// search branch can rewrite its trajectory events into the search
    /// alphabet (PSP-10 Gate W) while assembly records still reach the
    /// session ledger.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn attempt_node_with_recorder(
        &self,
        recorder: &Psp9Recorder,
        session_id: &str,
        goal: &str,
        node_id: &str,
        generation: u32,
        model: &ModelId,
        graph: &WorkGraphRevision,
        seed: Option<&CandidateSeed>,
        shared_recovery_budget: u32,
        loop_recorder: &dyn crate::toolloop::LoopRecorder,
        partial_root: Option<String>,
        search_budget: Option<perspt_sdk::search::SharedSearchBudget>,
    ) -> Result<NodeAttempt> {
        let prepared = self
            .prepare_attempt(recorder, session_id, node_id, generation, model, graph)
            .await?;
        self.attempt_prepared(
            prepared,
            goal,
            node_id,
            generation,
            model,
            graph,
            seed,
            shared_recovery_budget,
            loop_recorder,
            partial_root,
            search_budget,
        )
        .await
    }

    /// Assemble a node attempt without touching any workspace: the tool
    /// catalog, capability, and compiled prompt envelope. The search forest
    /// derives exact no-good components (Gate AB) from this before it even
    /// admits the fork; the eager candidate copy stays after `admit_fork`.
    pub(crate) async fn prepare_attempt(
        &self,
        recorder: &Psp9Recorder,
        session_id: &str,
        node_id: &str,
        generation: u32,
        model: &ModelId,
        graph: &WorkGraphRevision,
    ) -> Result<PreparedAttempt> {
        let assembly = {
            let external_entries = self.external_tool_entries(recorder).await?;
            self.assemble_node(
                recorder,
                session_id,
                graph,
                node_id,
                generation,
                &external_entries,
            )?
        };
        let kernel_state = loop_kernel_state(&assembly.grant_policy, &graph.revision_id);
        let envelope = self.worker_prompt_envelope(recorder, &assembly, model)?;
        Ok(PreparedAttempt {
            assembly,
            envelope,
            kernel_state,
        })
    }

    /// Run one prepared attempt: fresh candidate overlay, one tool loop.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn attempt_prepared(
        &self,
        prepared: PreparedAttempt,
        goal: &str,
        node_id: &str,
        generation: u32,
        model: &ModelId,
        graph: &WorkGraphRevision,
        seed: Option<&CandidateSeed>,
        shared_recovery_budget: u32,
        loop_recorder: &dyn crate::toolloop::LoopRecorder,
        partial_root: Option<String>,
        search_budget: Option<perspt_sdk::search::SharedSearchBudget>,
    ) -> Result<NodeAttempt> {
        let PreparedAttempt {
            assembly,
            envelope,
            kernel_state,
        } = prepared;
        let candidate = self.open_candidate(node_id, generation, &graph.revision_id)?;
        restore_seed(&candidate, seed).await?;
        let measurer = CodingCandidateMeasurer::new(&candidate, node_id, generation)
            .with_domain(self.domain.clone())
            .with_max_parallel(self.config.max_parallel_verifiers)
            .with_require_format(self.config.require_format)
            .with_correction_packets(!self.config.ablate_correction_packets);
        let tool_loop = ToolLoop {
            transport: self.transport.as_ref(),
            model: model.clone(),
            fallback_models: self.fallback_models.clone(),
            catalog: &assembly.catalog,
            capabilities: vec![assembly.capability.clone()],
            contract: Some(&assembly.contract),
            barrier: Some(&assembly.barrier),
            c_c_max: 0.25,
            executor: &candidate,
            measurer: &measurer,
            budgets: self.loop_budgets(assembly.energy.rho_gate, shared_recovery_budget),
            cadence: assembly.cadence.clone(),
            kernel_state: kernel_state.clone(),
            node_id: node_id.to_string(),
            generation,
            system_prompt: envelope,
            recorder: Some(loop_recorder),
            partial_seed_root: partial_root,
            search_budget,
        };
        let outcome = run_seeded(tool_loop, goal, seed).await?;
        Ok(NodeAttempt {
            outcome,
            candidate,
            assembly,
            kernel_state,
        })
    }
}
