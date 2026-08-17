//! The open execution plane: an exact-name handler registry the governed
//! candidate dispatches through.
//!
//! A new tool family is a *registration* at the composition root — catalog
//! entries via `StaticCatalog::with_base`, handlers via
//! [`CandidateHandlerRegistry::register`], and a capability grant — never an
//! edit to the loop, the candidate, or the node assembly. The governance
//! wrapper (catalog → validate → budget → certify → execute → re-certify)
//! is uniform and lives outside the handlers.

mod deps;
mod exec;
pub mod external;
mod fs;
mod lsp;
mod verify;

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::Result;
use perspt_sdk::ToolEntry;

use crate::candidate::CandidateWorkspace;
use crate::toolloop::EffectOutcome;

pub(crate) use lsp::LspSessions;

/// One governed tool executor. Handlers never decide admission, budgets, or
/// certification — the kernel already did; they only realize the effect
/// against the reversible candidate.
#[async_trait::async_trait]
pub trait CandidateToolHandler: Send + Sync {
    async fn apply(
        &self,
        workspace: &CandidateWorkspace,
        call: &perspt_sdk::ProviderToolCall,
        entry: &ToolEntry,
    ) -> Result<EffectOutcome>;
}

/// Exact-name registry. Duplicate registration fails closed so a later
/// family can never silently shadow a builtin.
pub struct CandidateHandlerRegistry {
    handlers: BTreeMap<String, Arc<dyn CandidateToolHandler>>,
    /// Consulted only when no exact name matches — the external (MCP)
    /// dispatcher, whose namespaced tool names are discovered at runtime.
    fallback: Option<Arc<dyn CandidateToolHandler>>,
}

impl std::fmt::Debug for CandidateHandlerRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CandidateHandlerRegistry")
            .field("handlers", &self.handlers.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl CandidateHandlerRegistry {
    /// An empty registry, for tests that assemble their own surface.
    pub fn empty() -> Self {
        Self {
            handlers: BTreeMap::new(),
            fallback: None,
        }
    }

    /// The builtin coding-domain execution surface, matching the base
    /// catalog's executor-backed entries.
    pub fn with_builtins() -> Self {
        let mut registry = Self::empty();
        fs::register_workspace_ops(&mut registry);
        verify::register(&mut registry);
        exec::register(&mut registry);
        lsp::register(&mut registry);
        deps::register(&mut registry);
        registry
    }

    pub fn register(
        &mut self,
        name: impl Into<String>,
        handler: Arc<dyn CandidateToolHandler>,
    ) -> Result<()> {
        let name = name.into();
        anyhow::ensure!(
            !self.handlers.contains_key(&name),
            "duplicate tool handler registration: {name}"
        );
        self.handlers.insert(name, handler);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn CandidateToolHandler>> {
        self.handlers.get(name)
    }

    /// Install the fallback handler for names with no exact registration.
    pub fn set_fallback(&mut self, handler: Arc<dyn CandidateToolHandler>) {
        self.fallback = Some(handler);
    }

    pub fn has_fallback(&self) -> bool {
        self.fallback.is_some()
    }

    /// Exact name first, then the fallback.
    pub fn resolve(&self, name: &str) -> Option<&Arc<dyn CandidateToolHandler>> {
        self.handlers.get(name).or(self.fallback.as_ref())
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.handlers.keys().map(String::as_str)
    }
}

impl Default for CandidateHandlerRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}
