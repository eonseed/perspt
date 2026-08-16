//! Observability projections (PSP-8 System 12).
//!
//! The UX exposes SRBN evidence rather than hiding it. All dashboard and TUI
//! views are *read-only projections over the event ledger*, so monitoring cannot
//! mutate the running session. This module computes those projections — the
//! backlog gauge `Phi(W)`, the observed-vs-accepted trajectory, the residual
//! heatmap, and the capability audit — from ledgered events and residual
//! vectors. Rendering (axum/ratatui) belongs to `perspt-dashboard` / `perspt-tui`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ledger::{Ledger, LedgerEvent};
use crate::residual::{EnergyComponent, ResidualClass, ResidualEvent};

/// Per-workflow potential `phi_i = 1 + V_i/rho_gate + B_i` (PSP-8 System 2).
pub fn phi(accepted_energy: f64, rho_gate: f64, remaining_budget: u32) -> f64 {
    1.0 + accepted_energy / rho_gate + remaining_budget as f64
}

/// A backlog/remaining-work gauge entry (PSP-8 `WorkflowPotential`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowPotential {
    pub workflow_id: String,
    pub accepted_energy: f64,
    pub rho_gate: f64,
    pub remaining_budget: u32,
    pub potential: f64,
}

impl WorkflowPotential {
    pub fn new(
        workflow_id: impl Into<String>,
        accepted_energy: f64,
        rho_gate: f64,
        remaining_budget: u32,
    ) -> Self {
        Self {
            workflow_id: workflow_id.into(),
            accepted_energy,
            rho_gate,
            remaining_budget,
            potential: phi(accepted_energy, rho_gate, remaining_budget),
        }
    }
}

/// Aggregate backlog gauge `Phi(W) = sum_i phi_i` (PSP-8 System 2 / Gate H).
pub fn backlog_gauge(workflows: &[WorkflowPotential]) -> f64 {
    workflows.iter().map(|w| w.potential).sum()
}

/// Theorem 9's sub-criticality load `α`, computed from arriving **potential**
/// per step, not arrival count (PSP-9 system 17): a single arriving workflow
/// with a large `V_a/ρ_gate` can exceed the bound on its own. The gauge is
/// labeled as arriving potential wherever it is displayed; positive
/// recurrence is reported only when every Theorem 9 hypothesis is evidenced.
pub fn arriving_potential_per_step(arrivals_per_step: &[Vec<WorkflowPotential>]) -> f64 {
    if arrivals_per_step.is_empty() {
        return 0.0;
    }
    let total: f64 = arrivals_per_step
        .iter()
        .map(|arrivals| backlog_gauge(arrivals))
        .sum();
    total / arrivals_per_step.len() as f64
}

/// Observed-vs-accepted trajectory projection over the ledger.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TrajectoryProjection {
    pub accepted: usize,
    pub rejected: usize,
    pub certificates: usize,
    pub graph_revisions: usize,
    /// Accepted-energy timeline (node_id, generation, energy), in ledger order.
    pub energy_timeline: Vec<(String, u32, f64)>,
}

impl TrajectoryProjection {
    /// Build the projection from ledger events (read-only).
    pub fn from_ledger(ledger: &Ledger) -> Self {
        let mut p = TrajectoryProjection::default();
        for rec in ledger.records() {
            match &rec.event {
                LedgerEvent::CandidateAccepted {
                    node_id,
                    generation,
                    energy,
                } => {
                    p.accepted += 1;
                    p.energy_timeline
                        .push((node_id.clone(), *generation, *energy));
                }
                LedgerEvent::CandidateRejected { .. } => p.rejected += 1,
                LedgerEvent::ResidualCertificateIssued { .. } => p.certificates += 1,
                LedgerEvent::GraphRevisionAccepted { .. } => p.graph_revisions += 1,
                _ => {}
            }
        }
        p
    }
}

/// A residual heatmap by energy component and class (PSP-8 System 12).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ResidualHeatmap {
    pub by_component: BTreeMap<EnergyComponent, usize>,
    pub by_class: BTreeMap<ResidualClass, usize>,
}

/// Build a residual heatmap from a residual vector.
pub fn residual_heatmap(residuals: &[ResidualEvent]) -> ResidualHeatmap {
    let mut heatmap = ResidualHeatmap::default();
    for r in residuals {
        *heatmap.by_component.entry(r.component).or_default() += 1;
        *heatmap.by_class.entry(r.class).or_default() += 1;
    }
    heatmap
}

/// Capability audit projection over the ledger.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CapabilityAudit {
    pub grants: usize,
    pub revocations: usize,
    pub denials: usize,
}

impl CapabilityAudit {
    pub fn from_ledger(ledger: &Ledger) -> Self {
        let mut a = CapabilityAudit::default();
        for rec in ledger.records() {
            match &rec.event {
                LedgerEvent::CapabilityGranted { .. } => a.grants += 1,
                LedgerEvent::CapabilityRevoked { .. } => a.revocations += 1,
                LedgerEvent::EffectDenied { .. } => a.denials += 1,
                _ => {}
            }
        }
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::residual::{IndependenceRoute, ResidualSeverity, SensorRef};

    #[test]
    fn phi_matches_formula() {
        // 1 + 10/0.5 + 3 = 1 + 20 + 3 = 24.
        assert_eq!(phi(10.0, 0.5, 3), 24.0);
    }

    #[test]
    fn backlog_gauge_sums_potentials() {
        let workflows = vec![
            WorkflowPotential::new("w1", 10.0, 0.5, 3), // 24
            WorkflowPotential::new("w2", 5.0, 0.5, 1),  // 1 + 10 + 1 = 12
        ];
        assert_eq!(backlog_gauge(&workflows), 36.0);
    }

    #[test]
    fn trajectory_projection_counts_ledger_events() {
        let mut ledger = Ledger::new();
        ledger
            .append(LedgerEvent::CandidateAccepted {
                node_id: "a".into(),
                generation: 0,
                energy: 5.0,
            })
            .unwrap();
        ledger
            .append(LedgerEvent::CandidateRejected {
                node_id: "a".into(),
                generation: 1,
            })
            .unwrap();
        ledger
            .append(LedgerEvent::CandidateAccepted {
                node_id: "a".into(),
                generation: 2,
                energy: 0.0,
            })
            .unwrap();
        ledger
            .append(LedgerEvent::GraphRevisionAccepted {
                revision_id: "r1".into(),
                sequence: 1,
            })
            .unwrap();
        let p = TrajectoryProjection::from_ledger(&ledger);
        assert_eq!(p.accepted, 2);
        assert_eq!(p.rejected, 1);
        assert_eq!(p.graph_revisions, 1);
        assert_eq!(p.energy_timeline.len(), 2);
    }

    #[test]
    fn residual_heatmap_groups_by_component_and_class() {
        let sensor = SensorRef::new("c", IndependenceRoute::Compiler);
        let residuals = vec![
            ResidualEvent::new(
                "n",
                0,
                ResidualClass::Type,
                ResidualSeverity::Error,
                1.0,
                sensor.clone(),
            )
            .unwrap(),
            ResidualEvent::new(
                "n",
                0,
                ResidualClass::Type,
                ResidualSeverity::Error,
                1.0,
                sensor.clone(),
            )
            .unwrap(),
            ResidualEvent::new(
                "n",
                0,
                ResidualClass::TestFailure,
                ResidualSeverity::Error,
                1.0,
                sensor,
            )
            .unwrap(),
        ];
        let heatmap = residual_heatmap(&residuals);
        assert_eq!(heatmap.by_component[&EnergyComponent::Syn], 2);
        assert_eq!(heatmap.by_component[&EnergyComponent::Log], 1);
        assert_eq!(heatmap.by_class[&ResidualClass::Type], 2);
    }

    #[test]
    fn capability_audit_counts_grants_and_denials() {
        let mut ledger = Ledger::new();
        ledger
            .append(LedgerEvent::CapabilityGranted {
                capability_id: "c1".into(),
                holder: "a".into(),
            })
            .unwrap();
        ledger
            .append(LedgerEvent::EffectDenied {
                proposal_id: "p1".into(),
                reason: "scope".into(),
            })
            .unwrap();
        ledger
            .append(LedgerEvent::EffectDenied {
                proposal_id: "p2".into(),
                reason: "budget".into(),
            })
            .unwrap();
        let audit = CapabilityAudit::from_ledger(&ledger);
        assert_eq!(audit.grants, 1);
        assert_eq!(audit.denials, 2);
    }

    #[test]
    fn alpha_is_arriving_potential_not_arrival_count() {
        // Step 1: three trivial arrivals. Step 2: one heavy arrival whose
        // potential alone exceeds the three combined — a count-based gauge
        // would rank the steps backwards.
        let light = vec![
            WorkflowPotential::new("a", 0.0, 0.5, 0),
            WorkflowPotential::new("b", 0.0, 0.5, 0),
            WorkflowPotential::new("c", 0.0, 0.5, 0),
        ];
        let heavy = vec![WorkflowPotential::new("d", 50.0, 0.5, 3)];
        assert!(backlog_gauge(&heavy) > backlog_gauge(&light));
        let alpha = arriving_potential_per_step(&[light, heavy]);
        assert!((alpha - (3.0 + 104.0) / 2.0).abs() < 1e-9, "{alpha}");
    }
}
