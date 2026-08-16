//! Open language-adapter registry (PSP-9 system 5).
//!
//! Replaces closed `CodingLanguage`-enum dispatch with a `LanguageId`-keyed
//! registry, so adding a language means *registering* an adapter — never
//! editing the orchestrator. The runtime joins this registry with
//! `perspt-core`'s plugin registry by id: the adapter parses diagnostics and
//! never shells out; the plugin knows commands and availability and never
//! parses diagnostics.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::lang::{adapter_for, LanguageAdapter};
use crate::CodingLanguage;

/// Open, stable language identifier (lowercase by convention).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LanguageId(pub String);

impl LanguageId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into().to_ascii_lowercase())
    }
}

impl std::fmt::Display for LanguageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<CodingLanguage> for LanguageId {
    fn from(language: CodingLanguage) -> Self {
        match language {
            CodingLanguage::Rust => LanguageId::new("rust"),
            CodingLanguage::Python => LanguageId::new("python"),
            CodingLanguage::TypeScript => LanguageId::new("typescript"),
        }
    }
}

/// The adapter registry: one diagnostic-normalization adapter per language.
pub struct CodingAdapterRegistry {
    adapters: BTreeMap<LanguageId, Box<dyn LanguageAdapter>>,
}

impl std::fmt::Debug for CodingAdapterRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodingAdapterRegistry")
            .field("languages", &self.language_ids())
            .finish()
    }
}

impl CodingAdapterRegistry {
    /// An empty registry.
    pub fn empty() -> Self {
        Self {
            adapters: BTreeMap::new(),
        }
    }

    /// The registry with the first-party adapters registered.
    pub fn with_builtins() -> Self {
        let mut registry = Self::empty();
        for language in [
            CodingLanguage::Rust,
            CodingLanguage::Python,
            CodingLanguage::TypeScript,
        ] {
            registry.register(language.into(), adapter_for(language));
        }
        registry
    }

    /// Register (or replace) the adapter for a language.
    pub fn register(&mut self, id: LanguageId, adapter: Box<dyn LanguageAdapter>) {
        self.adapters.insert(id, adapter);
    }

    /// Resolve an adapter by id.
    pub fn get(&self, id: &LanguageId) -> Option<&dyn LanguageAdapter> {
        self.adapters.get(id).map(|a| a.as_ref())
    }

    /// Registered ids, sorted.
    pub fn language_ids(&self) -> Vec<LanguageId> {
        self.adapters.keys().cloned().collect()
    }
}

impl Default for CodingAdapterRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perspt_sdk::{CorrectionDirection, IndependenceRoute, ResidualEvent, SensorRef};

    /// A fixture adapter for a language the enum has never heard of. Its
    /// existence in this test is the Phase 2 exit criterion: registering a
    /// language edits no orchestrator code.
    struct GleamAdapter;

    impl LanguageAdapter for GleamAdapter {
        fn language(&self) -> CodingLanguage {
            // The legacy enum has no Gleam variant; the id is authoritative
            // and the enum hint is only used for correction phrasing.
            CodingLanguage::Rust
        }
        fn diagnostic_sensor(&self) -> SensorRef {
            SensorRef::new("gleam-check", IndependenceRoute::DeterministicTool)
        }
        fn parse_diagnostics(&self, _n: &str, _g: u32, _raw: &str) -> Vec<ResidualEvent> {
            Vec::new()
        }
        fn correction_for(&self, _r: &ResidualEvent) -> Option<CorrectionDirection> {
            None
        }
    }

    #[test]
    fn builtins_resolve_by_id() {
        let registry = CodingAdapterRegistry::with_builtins();
        for id in ["rust", "python", "typescript"] {
            assert!(registry.get(&LanguageId::new(id)).is_some(), "{id}");
        }
        assert!(registry.get(&LanguageId::new("cobol")).is_none());
    }

    #[test]
    fn a_new_language_registers_without_editing_the_orchestrator() {
        let mut registry = CodingAdapterRegistry::with_builtins();
        registry.register(LanguageId::new("gleam"), Box::new(GleamAdapter));
        assert!(registry.get(&LanguageId::new("gleam")).is_some());
        assert_eq!(registry.language_ids().len(), 4);
    }

    #[test]
    fn ids_are_case_insensitive_by_construction() {
        assert_eq!(LanguageId::new("Rust"), LanguageId::new("rust"));
    }
}
