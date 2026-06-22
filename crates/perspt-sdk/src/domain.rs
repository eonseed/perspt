//! Domain-package contract (PSP-8 System 1 / System 5).
//!
//! The SDK owns the domain-neutral control plane; a domain package provides
//! task-specific semantics. This module defines the Phase-0/1 surface of the
//! [`AgentDomainPackage`] trait — the part exercised by the energy/gate core and
//! implemented by `perspt-coding` as the first consumer. Later phases extend the
//! trait with exploration plans, verifier suites, hard gates, context packages,
//! capability policies, and graph hints; those types are intentionally omitted
//! here until their owning phases land, so every contract ships with a real
//! consumer rather than ahead of one.

use serde::{Deserialize, Serialize};

use crate::energy::EnergyModel;
use crate::residual::{CorrectionDirection, ResidualClass, ResidualEvent};

/// Stable identifier for a domain (e.g. `"coding"`, `"research"`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DomainId(pub String);

impl DomainId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl std::fmt::Display for DomainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A minimal read-only snapshot of the workspace used for domain detection.
/// Richer snapshots (state witnesses, ledger head, capability set) arrive with
/// the scheduler and capability phases.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    pub root: String,
    /// Workspace-relative paths discovered by a read-only scan.
    pub files: Vec<String>,
}

impl WorkspaceSnapshot {
    pub fn new(root: impl Into<String>, files: Vec<String>) -> Self {
        Self {
            root: root.into(),
            files,
        }
    }

    /// Whether any discovered file ends with the given suffix.
    pub fn has_file_named(&self, name: &str) -> bool {
        self.files
            .iter()
            .any(|f| f == name || f.ends_with(&format!("/{name}")))
    }
}

/// Evidence that a domain package activates for a workspace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DomainDetection {
    pub domain: DomainId,
    pub activated: bool,
    /// Confidence in `[0, 1]`.
    pub confidence: f64,
    pub evidence: Vec<String>,
}

/// The scope a domain operates over (e.g. a node, a package, a subtree).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DomainScope {
    pub label: String,
    pub paths: Vec<String>,
}

/// The residual schema a domain declares: the classes it can emit and the
/// allowed sensors per class. Normalization, weights, and rollup mapping live in
/// the [`EnergyModel`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ResidualSchema {
    pub classes: Vec<ResidualClass>,
}

impl ResidualSchema {
    pub fn new(classes: Vec<ResidualClass>) -> Self {
        Self { classes }
    }

    pub fn allows(&self, class: ResidualClass) -> bool {
        self.classes.contains(&class)
    }
}

/// The Phase-0/1 domain-package contract.
///
/// A domain package maps verifier evidence into residuals, declares the residual
/// schema and energy model that gate acceptance, and derives correction
/// directions from dominant residuals. `perspt-coding` is the first consumer.
pub trait AgentDomainPackage: Send + Sync {
    /// Stable domain identifier.
    fn domain_id(&self) -> DomainId;

    /// Detect whether this domain applies to a workspace.
    fn detect(&self, workspace: &WorkspaceSnapshot) -> DomainDetection;

    /// The residual classes this domain can emit for a scope.
    fn residual_schema(&self, scope: &DomainScope) -> ResidualSchema;

    /// The energy model (weights, `rho_gate`, tolerance, budget) for a scope.
    fn energy_model(&self, scope: &DomainScope) -> EnergyModel;

    /// Derive correction directions from dominant residuals. Returning an empty
    /// vector for residuals that genuinely have no direction is honest; the
    /// runtime then escalates rather than issuing an undirected retry.
    fn correction_directions(&self, residuals: &[ResidualEvent]) -> Vec<CorrectionDirection>;
}

/// A registry of domain packages and the routing logic that selects one
/// (PSP-8 System 1 / Phase 10). Domain selection is a routing decision; it does
/// not bypass the SDK's residual, scheduler, capability, ledger, or dashboard
/// contracts — every registered package implements the same trait.
#[derive(Default)]
pub struct DomainRegistry {
    packages: Vec<Box<dyn AgentDomainPackage>>,
}

impl std::fmt::Debug for DomainRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DomainRegistry")
            .field(
                "domains",
                &self
                    .packages
                    .iter()
                    .map(|p| p.domain_id())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl DomainRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a domain package. The SDK admits any number of domains without
    /// forking the control plane.
    pub fn register(&mut self, package: Box<dyn AgentDomainPackage>) {
        self.packages.push(package);
    }

    pub fn len(&self) -> usize {
        self.packages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }

    /// All registered domain ids.
    pub fn domain_ids(&self) -> Vec<DomainId> {
        self.packages.iter().map(|p| p.domain_id()).collect()
    }

    /// Look up a package by explicit domain id.
    pub fn by_id(&self, id: &DomainId) -> Option<&dyn AgentDomainPackage> {
        self.packages
            .iter()
            .find(|p| &p.domain_id() == id)
            .map(|p| p.as_ref())
    }

    /// The activated package with the highest detection confidence.
    pub fn detect_best(&self, workspace: &WorkspaceSnapshot) -> Option<&dyn AgentDomainPackage> {
        self.packages
            .iter()
            .map(|p| (p, p.detect(workspace)))
            .filter(|(_, d)| d.activated)
            .max_by(|(_, a), (_, b)| {
                a.confidence
                    .partial_cmp(&b.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(p, _)| p.as_ref())
    }

    /// Select a domain: an explicit id wins; otherwise detect the best match.
    pub fn select(
        &self,
        explicit: Option<&DomainId>,
        workspace: &WorkspaceSnapshot,
    ) -> Option<&dyn AgentDomainPackage> {
        match explicit {
            Some(id) => self.by_id(id),
            None => self.detect_best(workspace),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::energy::EnergyModel;

    struct StubDomain {
        id: &'static str,
        marker: &'static str,
        confidence: f64,
    }

    impl AgentDomainPackage for StubDomain {
        fn domain_id(&self) -> DomainId {
            DomainId::new(self.id)
        }
        fn detect(&self, ws: &WorkspaceSnapshot) -> DomainDetection {
            let activated = ws.has_file_named(self.marker);
            DomainDetection {
                domain: self.domain_id(),
                activated,
                confidence: if activated { self.confidence } else { 0.0 },
                evidence: vec![],
            }
        }
        fn residual_schema(&self, _: &DomainScope) -> ResidualSchema {
            ResidualSchema::new(vec![])
        }
        fn energy_model(&self, _: &DomainScope) -> EnergyModel {
            EnergyModel::new(self.id, 0.5)
        }
        fn correction_directions(&self, _: &[ResidualEvent]) -> Vec<CorrectionDirection> {
            vec![]
        }
    }

    fn registry() -> DomainRegistry {
        let mut r = DomainRegistry::new();
        r.register(Box::new(StubDomain {
            id: "coding",
            marker: "Cargo.toml",
            confidence: 0.9,
        }));
        r.register(Box::new(StubDomain {
            id: "research",
            marker: "refs.bib",
            confidence: 0.8,
        }));
        r
    }

    #[test]
    fn explicit_selection_wins() {
        let r = registry();
        let ws = WorkspaceSnapshot::new("/r", vec!["Cargo.toml".into(), "refs.bib".into()]);
        let chosen = r.select(Some(&DomainId::new("research")), &ws).unwrap();
        assert_eq!(chosen.domain_id(), DomainId::new("research"));
    }

    #[test]
    fn detection_selects_best_when_no_explicit() {
        let r = registry();
        let ws = WorkspaceSnapshot::new("/r", vec!["refs.bib".into()]);
        let chosen = r.select(None, &ws).unwrap();
        assert_eq!(chosen.domain_id(), DomainId::new("research"));
    }

    #[test]
    fn registry_admits_multiple_domains() {
        let r = registry();
        assert_eq!(r.len(), 2);
        assert!(r.by_id(&DomainId::new("coding")).is_some());
        assert!(r.by_id(&DomainId::new("missing")).is_none());
    }
}
