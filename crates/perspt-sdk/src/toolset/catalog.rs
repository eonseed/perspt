//! Catalog assembly and capability filtering (PSP-9 system 5).
//!
//! The catalog offered to the model is derived from the actor's *held
//! capabilities* via [`ToolCatalog::specs_for`]: a model holding no
//! `RunShell` capability never sees `run_shell` in its tool list. This is
//! defence in depth, not the security boundary — the kernel is. It also
//! normalizes across providers: the same capability set yields the same tool
//! list whichever vendor serves the turn, which is half of Gate S.

use super::entry::ToolEntry;
use crate::capability::Capability;
use crate::error::{Result, SdkError};
use crate::model::ToolSpec;

/// The catalog port.
pub trait ToolCatalog: Send + Sync {
    /// Every registered entry.
    fn entries(&self) -> &[ToolEntry];

    /// Look up one entry by tool name.
    fn lookup(&self, name: &str) -> Option<&ToolEntry> {
        self.entries().iter().find(|e| e.name == name)
    }

    /// The wire-neutral specs an actor holding `capabilities` may see.
    ///
    /// An entry is offered when some capability grants its effect. `strict`
    /// asks providers with strict-schema support to enforce arguments;
    /// routes without it degrade to local validation.
    fn specs_for(&self, capabilities: &[Capability], strict: bool) -> Vec<ToolSpec> {
        self.entries()
            .iter()
            .filter(|entry| {
                capabilities
                    .iter()
                    .any(|c| c.effects.contains(&entry.effect))
            })
            .map(|entry| entry.to_spec(strict))
            .collect()
    }
}

/// An assembled, validated catalog.
#[derive(Debug, Default)]
pub struct StaticCatalog {
    entries: Vec<ToolEntry>,
}

impl StaticCatalog {
    /// Assemble a catalog, validating every entry (Gates J and P) and
    /// rejecting duplicate names.
    pub fn assemble(entries: Vec<ToolEntry>) -> Result<Self> {
        let mut seen = std::collections::BTreeSet::new();
        for entry in &entries {
            entry.validate()?;
            if !seen.insert(entry.name.clone()) {
                return Err(SdkError::Domain(format!(
                    "duplicate tool name {:?} in catalog",
                    entry.name
                )));
            }
        }
        Ok(Self { entries })
    }

    /// The SDK's base catalog plus `extra` entries (domain or external).
    pub fn with_base(extra: Vec<ToolEntry>) -> Result<Self> {
        let mut entries = super::base::base_entries();
        entries.extend(extra);
        Self::assemble(entries)
    }
}

impl ToolCatalog for StaticCatalog {
    fn entries(&self) -> &[ToolEntry] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{ActorId, EffectKind};

    fn actor() -> ActorId {
        ActorId("tester".into())
    }

    #[test]
    fn base_catalog_assembles() {
        let catalog = StaticCatalog::with_base(Vec::new()).unwrap();
        assert!(catalog.lookup("edit_file").is_some());
        assert!(catalog.lookup("sed_replace").is_none());
    }

    #[test]
    fn specs_follow_held_capabilities_not_the_full_catalog() {
        let catalog = StaticCatalog::with_base(Vec::new()).unwrap();
        let read_only = Capability::new(
            actor(),
            vec![EffectKind::ReadFile, EffectKind::Search, EffectKind::List],
        );
        let specs = catalog.specs_for(&[read_only], false);
        let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"grep"));
        assert!(!names.contains(&"run_shell"), "no RunShell capability held");
        assert!(!names.contains(&"edit_file"));
    }

    #[test]
    fn no_capabilities_means_no_tools() {
        let catalog = StaticCatalog::with_base(Vec::new()).unwrap();
        assert!(catalog.specs_for(&[], false).is_empty());
    }

    #[test]
    fn duplicate_names_are_rejected_at_assembly() {
        let mut entries = super::super::base::base_entries();
        entries.push(entries[0].clone());
        assert!(StaticCatalog::assemble(entries).is_err());
    }

    #[test]
    fn the_catalog_is_provider_invariant_for_a_fixed_capability_set() {
        // Gate S: nothing about spec derivation may read a provider identity;
        // the same capability set yields the same list, and strictness only
        // toggles enforcement, never membership.
        let catalog = StaticCatalog::with_base(Vec::new()).unwrap();
        let capability = Capability::new(actor(), vec![EffectKind::ReadFile]);
        let strict = catalog.specs_for(std::slice::from_ref(&capability), true);
        let lax = catalog.specs_for(std::slice::from_ref(&capability), false);
        let strict_names: Vec<&str> = strict.iter().map(|s| s.name.as_str()).collect();
        let lax_names: Vec<&str> = lax.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(strict_names, lax_names);
    }
}
