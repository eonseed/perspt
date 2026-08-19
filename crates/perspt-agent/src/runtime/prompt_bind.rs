//! Shared prompt binding for the non-worker actors (PSP-10 Gate Z): one
//! helper compiles the actor's stage for its resolved route and dialect
//! through the SDK compiler and ledgers the program and the invocation.

use super::{Psp9AgentRuntime, Psp9Recorder};
use anyhow::Result;
use perspt_sdk::ModelId;

impl Psp9AgentRuntime {
    pub(super) fn actor_invocation(
        &self,
        recorder: &Psp9Recorder,
        actor: &str,
        stage: &perspt_core::prompts::PlatformStage,
        model: &ModelId,
        specs: &[perspt_sdk::ToolSpec],
    ) -> Result<perspt_sdk::prompt::CompiledPromptInvocation> {
        let (route, dialect) = crate::turn::route_dialect(self.transport.as_ref(), model);
        let invocation = perspt_core::prompts::compile_invocation(
            stage,
            &[],
            &route,
            &dialect,
            &perspt_sdk::prompt::tool_surface_hash(specs),
        )
        .map_err(|e| anyhow::anyhow!("{actor} program: {e}"))?;
        recorder.record_prompt_program(&invocation.platform)?;
        recorder.record_prompt_invocation(actor, 1, &invocation)?;
        Ok(invocation)
    }

    /// The worker's compiled envelope over its initial hot tool surface.
    pub(super) fn worker_prompt_envelope(
        &self,
        recorder: &Psp9Recorder,
        assembly: &super::node::NodeAssembly,
        model: &ModelId,
    ) -> Result<crate::toolloop::PromptEnvelope> {
        use perspt_sdk::ToolCatalog;
        let (route, dialect) = crate::turn::route_dialect(self.transport.as_ref(), model);
        let initial_specs = assembly.catalog.deferred_specs_for(
            std::slice::from_ref(&assembly.capability),
            &Default::default(),
            false,
        );
        super::node::worker_envelope(
            recorder,
            &self.domain,
            &route,
            &dialect,
            &perspt_sdk::prompt::tool_surface_hash(&initial_specs),
            &self.prompt_overrides,
        )
    }
}
