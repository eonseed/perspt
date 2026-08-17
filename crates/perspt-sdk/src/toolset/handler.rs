//! Exact-name handler registry and capability-scoped host services.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use async_trait::async_trait;

use super::{ToolCatalog, ToolEntry};
use crate::capability::{Capability, EffectKind};
use crate::command::CommandInvocation;
use crate::error::{Result, SdkError};
use crate::scheduler::Footprint;

/// Scheduling class declared by a trusted handler implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConcurrencyClass {
    /// A read-only handler whose footprint may commute with another read.
    ParallelRead,
    /// A handler serialized by default. Mutation, LSP, MCP, dependency, and
    /// composition handlers belong here unless explicitly reviewed.
    Serial,
}

/// Capability-scoped workspace access. Implementations enforce their scope;
/// handlers never receive an unrestricted host path.
#[async_trait]
pub trait WorkspaceService: Send + Sync {
    async fn read(&self, path: &str, max_bytes: usize) -> Result<Vec<u8>>;
    async fn write(&self, path: &str, content: &[u8]) -> Result<()>;
}

/// Capability-scoped direct process execution.
#[async_trait]
pub trait ProcessService: Send + Sync {
    async fn execute(&self, invocation: &CommandInvocation, max_bytes: usize) -> Result<String>;
}

/// Capability-scoped network observation service.
#[async_trait]
pub trait NetworkService: Send + Sync {
    async fn fetch(&self, url: &str, max_bytes: usize) -> Result<Vec<u8>>;
}

/// Durable observation sink used by handlers.
pub trait ObservationService: Send + Sync {
    fn record(&self, kind: &str, observation: &serde_json::Value) -> Result<()>;
}

/// Host services available to a handler. Each service is optional so a
/// capability role can expose only the surfaces it actually grants.
#[derive(Clone, Default)]
pub struct HandlerServices {
    pub workspace: Option<Arc<dyn WorkspaceService>>,
    pub process: Option<Arc<dyn ProcessService>>,
    pub network: Option<Arc<dyn NetworkService>>,
    pub observations: Option<Arc<dyn ObservationService>>,
}

impl std::fmt::Debug for HandlerServices {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HandlerServices")
            .field("workspace", &self.workspace.is_some())
            .field("process", &self.process.is_some())
            .field("network", &self.network.is_some())
            .field("observations", &self.observations.is_some())
            .finish()
    }
}

/// Immutable context for one governed handler call.
#[derive(Debug, Clone)]
pub struct HandlerContext {
    session_id: Arc<str>,
    node_id: Arc<str>,
    generation: u32,
    capabilities: Arc<[Capability]>,
    services: HandlerServices,
}

impl HandlerContext {
    pub fn new(
        session_id: impl Into<Arc<str>>,
        node_id: impl Into<Arc<str>>,
        generation: u32,
        capabilities: Vec<Capability>,
        services: HandlerServices,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            node_id: node_id.into(),
            generation,
            capabilities: capabilities.into(),
            services,
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }

    pub fn capabilities(&self) -> &[Capability] {
        &self.capabilities
    }

    pub fn services(&self) -> &HandlerServices {
        &self.services
    }
}

/// A handler's declared result. The dispatcher rejects effect or footprint
/// claims outside the catalog entry.
#[derive(Debug, Clone)]
pub struct ToolExecution {
    pub effect: EffectKind,
    pub footprint: Footprint,
    pub output: serde_json::Value,
    pub artifacts: Vec<String>,
}

#[async_trait]
pub trait ToolHandler: Send + Sync {
    fn catalog_name(&self) -> &str;

    fn concurrency_class(&self) -> ConcurrencyClass {
        ConcurrencyClass::Serial
    }

    async fn execute(
        &self,
        context: &HandlerContext,
        arguments: &serde_json::Value,
    ) -> Result<ToolExecution>;
}

/// Exact-name registry and governed dispatch boundary.
#[derive(Default)]
pub struct ToolDispatcher {
    handlers: BTreeMap<String, Arc<dyn ToolHandler>>,
}

impl std::fmt::Debug for ToolDispatcher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ToolDispatcher")
            .field("handlers", &self.handlers.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl ToolDispatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, entry: &ToolEntry, handler: Arc<dyn ToolHandler>) -> Result<()> {
        if handler.catalog_name() != entry.name {
            return Err(SdkError::Domain(format!(
                "handler {:?} does not match catalog entry {:?}",
                handler.catalog_name(),
                entry.name
            )));
        }
        if self.handlers.contains_key(&entry.name) {
            return Err(SdkError::Domain(format!(
                "duplicate handler for catalog entry {:?}",
                entry.name
            )));
        }
        self.handlers.insert(entry.name.clone(), handler);
        Ok(())
    }

    /// Require a handler for every catalog entry and reject orphan handlers.
    pub fn validate_complete(&self, catalog: &dyn ToolCatalog) -> Result<()> {
        let entries: BTreeSet<&str> = catalog
            .entries()
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        if let Some(missing) = entries
            .iter()
            .find(|name| !self.handlers.contains_key(**name))
        {
            return Err(SdkError::Domain(format!(
                "missing handler for catalog entry {missing:?}"
            )));
        }
        if let Some(orphan) = self
            .handlers
            .keys()
            .find(|name| !entries.contains(name.as_str()))
        {
            return Err(SdkError::Domain(format!(
                "handler {orphan:?} has no catalog entry"
            )));
        }
        Ok(())
    }

    pub fn concurrency_class(&self, name: &str) -> Option<ConcurrencyClass> {
        self.handlers
            .get(name)
            .map(|handler| handler.concurrency_class())
    }

    pub async fn dispatch(
        &self,
        entry: &ToolEntry,
        context: &HandlerContext,
        arguments: &serde_json::Value,
        provider: &str,
    ) -> Result<ToolExecution> {
        entry.validate_arguments(arguments)?;
        let handler = self.handlers.get(&entry.name).ok_or_else(|| {
            SdkError::Domain(format!(
                "missing handler for catalog entry {:?}",
                entry.name
            ))
        })?;
        let result = handler.execute(context, arguments).await?;
        if result.effect != entry.effect {
            return Err(SdkError::Domain(format!(
                "handler {:?} returned undeclared effect {:?}",
                entry.name, result.effect
            )));
        }
        let declared = entry.footprint.resolve(arguments, provider);
        if !result.footprint.reads.is_subset(&declared.reads)
            || !result.footprint.writes.is_subset(&declared.writes)
        {
            return Err(SdkError::Domain(format!(
                "handler {:?} returned an undeclared resource footprint",
                entry.name
            )));
        }
        Ok(result)
    }
}
