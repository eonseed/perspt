//! The open execution plane: an exact-name handler registry the governed
//! candidate dispatches through.
//!
//! A new tool family is a *registration* at the composition root — catalog
//! entries via `StaticCatalog::with_base`, handlers via
//! [`CandidateHandlerRegistry::register`], and a capability grant — never an
//! edit to the loop, the candidate, or the node assembly. The governance
//! wrapper (catalog → validate → budget → certify → execute → re-certify)
//! is uniform and lives outside the handlers.

mod exec;
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

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.handlers.keys().map(String::as_str)
    }
}

impl Default for CandidateHandlerRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}
