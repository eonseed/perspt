//! Ergonomic, modular runtime builder for the `perspt` `"Omni"` facade.

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use perspt_agent::tools::families::{register_standard_families, standard_family_entries};
use perspt_agent::{
    AsyncFnInteractionHandler, CandidateHandlerRegistry, CandidateToolHandler,
    FnInteractionHandler, InteractionHandler, Psp9AgentRuntime, Psp9ModelRoutes, Psp9RunConfig,
};
use perspt_core::events::channel::{ActionReceiver, EventSender};
use perspt_sdk::{AgentDomainPackage, ApprovalPolicy, ModelId, ModelTransport, ToolEntry};

/// Fluent builder for configuring and launching a governed [`Psp9AgentRuntime`].
///
/// Supports arbitrary domain packages (`CodingDomain`, `ResearchDomain`, or
/// custom non-coding domains), standard and custom governed tools, synchronous
/// and asynchronous `ask_user` handlers, capability-attenuated subagents, and
/// interactive human-in-the-loop approval/correction channels.
pub struct AgentBuilder {
    working_dir: PathBuf,
    config: Option<perspt_core::Config>,
    model: Option<ModelId>,
    transport: Option<Arc<dyn ModelTransport>>,
    domain: Option<Arc<dyn AgentDomainPackage>>,
    run_config: Psp9RunConfig,
    database_path: Option<PathBuf>,
    include_standard_families: bool,
    custom_tools: Vec<(ToolEntry, Arc<dyn CandidateToolHandler>)>,
    interaction_handler: Option<Arc<dyn InteractionHandler>>,
    channels: Option<(EventSender, ActionReceiver)>,
}

impl std::fmt::Debug for AgentBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentBuilder")
            .field("working_dir", &self.working_dir)
            .field("model", &self.model)
            .field("run_config", &self.run_config)
            .field("include_standard_families", &self.include_standard_families)
            .field("custom_tools_count", &self.custom_tools.len())
            .finish_non_exhaustive()
    }
}

impl AgentBuilder {
    /// Start configuring an agent rooted at `working_dir`.
    pub fn new(working_dir: impl Into<PathBuf>) -> Self {
        Self {
            working_dir: working_dir.into(),
            config: None,
            model: None,
            transport: None,
            domain: None,
            run_config: Psp9RunConfig::default(),
            database_path: None,
            include_standard_families: true,
            custom_tools: Vec::new(),
            interaction_handler: None,
            channels: None,
        }
    }

    /// Provide an explicit [`perspt_core::Config`] instead of loading from disk.
    pub fn config(mut self, config: perspt_core::Config) -> Self {
        self.config = Some(config);
        self
    }

    /// Select a `provider` and `name` pair as the primary actuator model route.
    pub fn model(mut self, provider: impl Into<String>, name: impl Into<String>) -> Self {
        self.model = Some(ModelId::new(provider, name));
        self
    }

    /// Select a pre-constructed [`ModelId`] as the primary actuator model route.
    pub fn model_id(mut self, model: ModelId) -> Self {
        self.model = Some(model);
        self
    }

    /// Inject a custom [`ModelTransport`] (deterministic mocks, in-process models, etc.).
    pub fn transport(mut self, transport: Arc<dyn ModelTransport>) -> Self {
        self.transport = Some(transport);
        self
    }

    /// Attach a domain package controlling verification, residuals, energy, and corrections.
    pub fn domain(mut self, domain: Arc<dyn AgentDomainPackage>) -> Self {
        self.domain = Some(domain);
        self
    }

    /// Select the built-in coding domain (`perspt_coding::CodingDomain`).
    pub fn coding_domain(self) -> Self {
        self.domain(Arc::new(perspt_coding::CodingDomain::new()))
    }

    /// Select the built-in research domain (`perspt_research::ResearchDomain`).
    pub fn research_domain(self) -> Self {
        self.domain(Arc::new(perspt_research::ResearchDomain::new()))
    }

    /// Set the promotion [`ApprovalPolicy`] (`Auto`, `Ask`, `SessionApproved`, or `Deny`).
    pub fn approval_policy(mut self, policy: ApprovalPolicy) -> Self {
        self.run_config.approval_policy = policy;
        self
    }

    /// Enable or disable delegated child subagents (`spawn_agent`).
    pub fn allow_subagents(mut self, allow: bool) -> Self {
        self.run_config.allow_subagents = allow;
        self
    }

    /// Enable or disable governed dependency mutations.
    pub fn allow_dependency_mutation(mut self, allow: bool) -> Self {
        self.run_config.allow_dependency_mutation = allow;
        self
    }

    /// Allow unisolated verifier execution when no OS sandbox is registered.
    pub fn allow_unisolated_verifiers(mut self, allow: bool) -> Self {
        self.run_config.allow_unisolated_verifiers = allow;
        self
    }

    /// Set the finite per-node model turn bound.
    pub fn max_turns(mut self, turns: u32) -> Self {
        self.run_config.max_turns = turns;
        self
    }

    /// Set the minimum Lyapunov energy descent step `rho_gate`.
    pub fn rho_gate(mut self, rho_gate: f64) -> Self {
        self.run_config.rho_gate = rho_gate.max(0.000_001);
        self
    }

    /// Override the DuckDB ledger database path.
    pub fn database_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.database_path = Some(path.into());
        self
    }

    /// Exclude shipped standard tool families (`sys_*`, `db_*`) from registration.
    pub fn without_standard_tool_families(mut self) -> Self {
        self.include_standard_families = false;
        self
    }

    /// Register a custom governed tool entry alongside its execution handler.
    pub fn register_tool(
        mut self,
        entry: ToolEntry,
        handler: Arc<dyn CandidateToolHandler>,
    ) -> Self {
        self.custom_tools.push((entry, handler));
        self
    }

    /// Install a synchronous callback backing the `ask_user` tool.
    pub fn on_ask_user<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) -> Result<String> + Send + Sync + 'static,
    {
        self.interaction_handler = Some(Arc::new(FnInteractionHandler(callback)));
        self
    }

    /// Install an asynchronous callback backing the `ask_user` tool.
    pub fn on_ask_user_async<F, Fut>(mut self, callback: F) -> Self
    where
        F: Fn(String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<String>> + Send + 'static,
    {
        self.interaction_handler = Some(Arc::new(AsyncFnInteractionHandler(callback)));
        self
    }

    /// Connect live [`EventSender`] and [`ActionReceiver`] channels for HITL workflows.
    pub fn connect_channels(mut self, sender: EventSender, receiver: ActionReceiver) -> Self {
        self.channels = Some((sender, receiver));
        self
    }

    /// Assemble and validate the configured [`Psp9AgentRuntime`].
    pub fn build(mut self) -> Result<Psp9AgentRuntime> {
        let mut runtime = self.build_base_runtime()?;
        let (handlers, entries) = self.assemble_tools()?;
        if !entries.is_empty() {
            runtime = runtime.with_tool_family(entries);
        }
        runtime = runtime.with_tool_handlers(handlers);
        Ok(self.apply_attachments(runtime))
    }

    fn build_base_runtime(&mut self) -> Result<Psp9AgentRuntime> {
        let model_override = self.model.take();
        if let Some(transport) = self.transport.take() {
            let model = model_override.unwrap_or_else(|| ModelId::new("custom", "omni-default"));
            return Ok(Psp9AgentRuntime::with_transport(
                self.working_dir.clone(),
                transport,
                model,
                self.run_config.clone(),
            ));
        }
        let config = match self.config.take() {
            Some(cfg) => cfg,
            None => load_default_config()?,
        };
        let routes = Psp9ModelRoutes {
            primary: model_override.map(|m| m.to_string()),
            ..Psp9ModelRoutes::default()
        };
        Psp9AgentRuntime::from_config(
            self.working_dir.clone(),
            &config,
            routes,
            self.run_config.clone(),
        )
    }

    fn assemble_tools(&mut self) -> Result<(CandidateHandlerRegistry, Vec<ToolEntry>)> {
        let mut handlers = CandidateHandlerRegistry::with_builtins();
        let mut entries = Vec::new();
        if self.include_standard_families {
            register_standard_families(&mut handlers)?;
            entries.extend(standard_family_entries());
        }
        for (entry, handler) in self.custom_tools.drain(..) {
            handlers.register(&entry.name, handler)?;
            entries.push(entry);
        }
        Ok((handlers, entries))
    }

    fn apply_attachments(mut self, mut runtime: Psp9AgentRuntime) -> Psp9AgentRuntime {
        if let Some(domain) = self.domain.take() {
            runtime = runtime.with_domain(domain);
        }
        if let Some(db_path) = self.database_path.take() {
            runtime = runtime.with_database_path(db_path);
        }
        if let Some(handler) = self.interaction_handler.take() {
            runtime = runtime.with_interaction_handler(handler);
        }
        if let Some((sender, receiver)) = self.channels.take() {
            runtime = runtime.connect_tui(sender, receiver);
        }
        runtime
    }
}

fn load_default_config() -> Result<perspt_core::Config> {
    let config_path =
        perspt_core::paths::resolve_config_file().or_else(perspt_core::paths::config_file);
    match config_path {
        Some(path) => perspt_core::Config::load_from_path(&path),
        None => Ok(perspt_core::Config::default()),
    }
}
