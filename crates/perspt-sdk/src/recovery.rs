//! The recovery lattice (PSP-9 system 11, Paper III Definition 8.1 and
//! Theorem 6).
//!
//! Rejection is the steady state of a stochastic proposer, so recovery must
//! terminate *by construction*. Definition 8.1's well-formedness conditions
//! are adopted literally, because the `b + k` count is a consequence of them:
//!
//! * **One pool, every level.** `b` is a single non-replenishing budget
//!   shared across the whole cascade. *Every* horizontal repetition consumes
//!   one unit — a second fallback route, a second refinement, and a second
//!   escalation each cost one unit exactly as a re-proposal does. A per-level
//!   budget (including a separate "refinement depth" counter) would give
//!   `Σ_λ b_λ + k`, not `b + k`; refinement depth is a *cap* on level-2
//!   repetitions within the shared pool, never its own pool.
//! * **Exhaustion forbids repetition.** With zero remaining budget below the
//!   top level, the next non-terminal control MUST strictly escalate.
//! * **The top level is unconditionally terminating.**

use serde::{Deserialize, Serialize};

/// The escalation chain. `k` is the number of strict escalations available
/// from level 0, i.e. `Contain as u8`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CascadeLevel {
    /// Re-propose after a directed correction.
    Retry = 0,
    /// Switch route: alternate tool, alternate provider or family, or bundle
    /// mode. Where multi-provider pays for itself operationally.
    Fallback = 1,
    /// Topological Escalation: split or revise the node through the mutable
    /// work graph. Without certified proxy geometry this is empirical
    /// recovery and carries no convergence claim (Paper I Remark 12.3).
    Refine = 2,
    /// `ask_user`, or hand the node to a higher-capability route.
    Escalate = 3,
    /// Revoke mutating capabilities, freeze the node, restore best accepted
    /// state, issue the residual certificate. Unconditionally terminating.
    Contain = 4,
}

impl CascadeLevel {
    fn next(self) -> CascadeLevel {
        match self {
            CascadeLevel::Retry => CascadeLevel::Fallback,
            CascadeLevel::Fallback => CascadeLevel::Refine,
            CascadeLevel::Refine => CascadeLevel::Escalate,
            CascadeLevel::Escalate | CascadeLevel::Contain => CascadeLevel::Contain,
        }
    }
}

/// A failure observation's deterministic classification. The classifier is
/// total: unknown classes map to [`CascadeLevel::Contain`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    GateRejection,
    ToolFailure,
    ProviderTransport,
    ProviderRateLimit,
    CapabilityMismatch,
    MeasuredPlateau,
    ScopeTooBroad,
    NeedsApproval,
    NeedsCapability,
    Unknown,
}

/// Map every failure class to its entry control (PSP-9 system 11). Total by
/// construction — the compiler enforces it.
pub fn classify_failure(kind: FailureKind) -> CascadeLevel {
    match kind {
        FailureKind::GateRejection | FailureKind::ToolFailure => CascadeLevel::Retry,
        FailureKind::ProviderTransport
        | FailureKind::ProviderRateLimit
        | FailureKind::CapabilityMismatch => CascadeLevel::Fallback,
        FailureKind::MeasuredPlateau | FailureKind::ScopeTooBroad => CascadeLevel::Refine,
        FailureKind::NeedsApproval | FailureKind::NeedsCapability => CascadeLevel::Escalate,
        FailureKind::Unknown => CascadeLevel::Contain,
    }
}

/// Theorem 6's four terminal classes. Recorded alongside — not in place of —
/// the node's Paper II `NodeTerminalOutcome`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CascadeClass {
    /// A recovery control succeeded and the node re-entered the gate.
    Committed,
    /// An irreversible external effect was rolled back through its
    /// compensation record (R5). Compensations are declared forward actions
    /// and consume the same `b`.
    Compensated,
    /// Level 3 handed the node to the user, whose decision terminated it.
    HumanEscalation,
    /// Level 4: capabilities revoked, node frozen, best state restored.
    Contained,
}

/// What the cascade granted for one requested control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrantedControl {
    pub level: CascadeLevel,
    /// Whether this control is unconditionally terminating.
    pub terminal: bool,
    /// Whether the request was forced upward by budget exhaustion.
    pub forced_escalation: bool,
}

/// The shared-pool cascade for one node (Definition 8.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryCascade {
    /// The non-replenishing shared budget `b`.
    pub budget: u32,
    pub spent: u32,
    pub current_level: CascadeLevel,
    pub escalations_used: u32,
    pub steps: u32,
}

impl RecoveryCascade {
    pub fn new(budget: u32) -> Self {
        Self {
            budget,
            spent: 0,
            current_level: CascadeLevel::Retry,
            escalations_used: 0,
            steps: 0,
        }
    }

    /// The escalation depth `k`: strict escalations available from level 0.
    pub const K: u32 = 4;

    /// The Theorem 6 bound: every cascade terminates within `b + k` steps.
    pub fn step_bound(&self) -> u32 {
        self.budget + Self::K
    }

    /// Request a control. A request at or below the current level is a
    /// horizontal repetition and consumes one unit of the shared pool; a
    /// request above it is a strict escalation and consumes a level. When
    /// the pool is exhausted below the top, the grant is forced strictly
    /// upward (Definition 8.1's second condition).
    pub fn grant(&mut self, requested: CascadeLevel) -> GrantedControl {
        if self.current_level == CascadeLevel::Contain {
            return GrantedControl {
                level: CascadeLevel::Contain,
                terminal: true,
                forced_escalation: false,
            };
        }
        self.steps += 1;
        if requested > self.current_level {
            // Strict escalation: consumes a level, never the pool.
            self.current_level = requested;
            self.escalations_used += 1;
            return GrantedControl {
                level: requested,
                terminal: requested == CascadeLevel::Contain,
                forced_escalation: false,
            };
        }
        // Horizontal repetition at or below the current level.
        if self.spent < self.budget {
            self.spent += 1;
            return GrantedControl {
                level: requested,
                terminal: false,
                forced_escalation: false,
            };
        }
        // Exhaustion forbids repetition: force a strict escalation.
        let forced = self.current_level.next();
        self.current_level = forced;
        self.escalations_used += 1;
        GrantedControl {
            level: forced,
            terminal: forced == CascadeLevel::Contain,
            forced_escalation: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MC-N core: an always-failing adversary terminates within `b + k`.
    #[test]
    fn always_failing_adversary_terminates_within_b_plus_k() {
        let b = 5;
        let mut cascade = RecoveryCascade::new(b);
        let mut steps = 0;
        loop {
            let granted = cascade.grant(CascadeLevel::Retry);
            steps += 1;
            if granted.terminal {
                break;
            }
            assert!(steps <= cascade.step_bound(), "exceeded b + k");
        }
        assert!(steps <= b + RecoveryCascade::K);
        assert_eq!(cascade.current_level, CascadeLevel::Contain);
    }

    /// MC-N companion: `b` horizontal steps distributed across levels
    /// exhaust the pool exactly as `b` retries at level 0 do.
    #[test]
    fn horizontal_repetitions_at_any_level_draw_on_the_one_pool() {
        let b = 6;
        // Distributed: two retries, escalate, two fallbacks, escalate, two
        // refinements = six horizontal steps across three levels.
        let mut distributed = RecoveryCascade::new(b);
        distributed.grant(CascadeLevel::Retry);
        distributed.grant(CascadeLevel::Retry);
        distributed.grant(CascadeLevel::Fallback); // escalation (free)
        distributed.grant(CascadeLevel::Fallback); // horizontal
        distributed.grant(CascadeLevel::Fallback); // horizontal
        distributed.grant(CascadeLevel::Refine); // escalation (free)
        distributed.grant(CascadeLevel::Refine); // horizontal
        assert_eq!(distributed.spent, 5);
        let granted = distributed.grant(CascadeLevel::Refine); // horizontal: last unit
        assert_eq!(distributed.spent, 6);
        assert!(!granted.forced_escalation);

        // Level 0 only: six retries exhaust identically.
        let mut level0 = RecoveryCascade::new(b);
        for _ in 0..b {
            level0.grant(CascadeLevel::Retry);
        }
        assert_eq!(level0.spent, distributed.spent);

        // Both now force strict escalation on the next horizontal request.
        let forced = distributed.grant(CascadeLevel::Retry);
        assert!(forced.forced_escalation);
        assert!(forced.level > CascadeLevel::Refine);
        let forced0 = level0.grant(CascadeLevel::Retry);
        assert!(forced0.forced_escalation);
    }

    /// Exhausting `b` below level `k` forces strict escalation, never a
    /// stall and never a replenish.
    #[test]
    fn exhaustion_forces_strict_escalation() {
        let mut cascade = RecoveryCascade::new(1);
        cascade.grant(CascadeLevel::Retry); // spends the only unit
        let g1 = cascade.grant(CascadeLevel::Retry);
        assert!(g1.forced_escalation);
        assert_eq!(g1.level, CascadeLevel::Fallback);
        let g2 = cascade.grant(CascadeLevel::Retry);
        assert!(g2.forced_escalation);
        assert_eq!(g2.level, CascadeLevel::Refine);
        let g3 = cascade.grant(CascadeLevel::Fallback);
        assert_eq!(g3.level, CascadeLevel::Escalate);
        let g4 = cascade.grant(CascadeLevel::Retry);
        assert_eq!(g4.level, CascadeLevel::Contain);
        assert!(g4.terminal);
    }

    /// The top level never returns control to the loop.
    #[test]
    fn contain_is_unconditionally_terminating() {
        let mut cascade = RecoveryCascade::new(3);
        cascade.grant(CascadeLevel::Contain);
        for _ in 0..5 {
            let granted = cascade.grant(CascadeLevel::Retry);
            assert!(granted.terminal);
            assert_eq!(granted.level, CascadeLevel::Contain);
        }
    }

    /// The failure classifier is total; provider-transport failures map to
    /// fallback (failover is recovery, not retry) and unknown classes
    /// contain.
    #[test]
    fn every_failure_class_maps_to_a_control() {
        assert_eq!(
            classify_failure(FailureKind::ProviderTransport),
            CascadeLevel::Fallback
        );
        assert_eq!(
            classify_failure(FailureKind::MeasuredPlateau),
            CascadeLevel::Refine
        );
        assert_eq!(
            classify_failure(FailureKind::Unknown),
            CascadeLevel::Contain
        );
    }
}
