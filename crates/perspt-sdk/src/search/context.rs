//! Search context and the re-homed deterministic repository map
//! (PSP-10 Compatibility: `ProjectMap` moves here from the removed
//! `exploration` module as the seed of `SearchContext`; the read-only
//! explorer capability moves with it).

use serde::{Deserialize, Serialize};

use crate::capability::{ActorId, Capability, EffectKind};

/// A structured map of the repository (PSP-8 System 3; re-homed by PSP-10).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectMap {
    pub languages: Vec<String>,
    pub package_roots: Vec<String>,
    pub build_systems: Vec<String>,
    pub entry_points: Vec<String>,
    pub risk_hotspots: Vec<String>,
}

/// Build a read-only exploration capability for an actor. Exploration SHALL
/// NOT write files, mutate dependencies, change graph policy, or apply
/// patches.
pub fn exploration_capability(actor: ActorId) -> Capability {
    Capability::new(
        actor,
        vec![
            EffectKind::ReadFile,
            EffectKind::Search,
            EffectKind::List,
            EffectKind::LspQuery,
            EffectKind::GitRead,
        ],
    )
    .with_paths(vec!["*"])
}

/// Whether a capability is strictly read-only (the exploration invariant).
pub fn is_read_only_capability(cap: &Capability) -> bool {
    cap.effects.iter().all(|e| e.is_read_only())
}
