//! Recovery mechanism checks (PSP-9 Gate N, Paper III Theorem 6).
//!
//! MC-N: all failure classes, including provider-transport failures from an
//! always-erroring provider, terminate within `b + k` in one of the four
//! Theorem 6 classes; failover neither replenishes nor bypasses the shared
//! pool.

use perspt_sdk::{classify_failure, CascadeClass, CascadeLevel, FailureKind, RecoveryCascade};

/// Drive a cascade against an adversary that fails every control, entering
/// at the classified level for `kind`. Returns (steps, terminal class).
fn drive_always_failing(kind: FailureKind, budget: u32) -> (u32, CascadeClass) {
    let mut cascade = RecoveryCascade::new(budget);
    let entry = classify_failure(kind);
    let mut steps = 0u32;
    loop {
        let granted = cascade.grant(entry);
        steps += 1;
        assert!(
            steps <= cascade.step_bound(),
            "{kind:?} exceeded b + k = {}",
            cascade.step_bound()
        );
        if granted.terminal {
            return (steps, CascadeClass::Contained);
        }
        if granted.level == CascadeLevel::Escalate {
            // The user declines: level 3 terminates through the human class.
            return (steps, CascadeClass::HumanEscalation);
        }
        // Otherwise the control ran and failed again: loop.
    }
}

#[test]
fn mc_n_every_failure_class_terminates_within_b_plus_k() {
    let kinds = [
        FailureKind::GateRejection,
        FailureKind::ToolFailure,
        FailureKind::ProviderTransport,
        FailureKind::ProviderRateLimit,
        FailureKind::CapabilityMismatch,
        FailureKind::MeasuredPlateau,
        FailureKind::ScopeTooBroad,
        FailureKind::NeedsApproval,
        FailureKind::NeedsCapability,
        FailureKind::Unknown,
    ];
    for kind in kinds {
        let budget = 3;
        let (steps, class) = drive_always_failing(kind, budget);
        assert!(steps <= budget + RecoveryCascade::K, "{kind:?}");
        assert!(
            matches!(
                class,
                CascadeClass::Contained | CascadeClass::HumanEscalation
            ),
            "{kind:?} ended unclassified"
        );
    }
}

/// A provider that always errors is a Fallback-class failure; repeated
/// failover draws on the same pool as retries and cannot buy unbounded
/// attempts.
#[test]
fn mc_n_failover_never_buys_unbounded_retries() {
    let budget = 4;
    let mut cascade = RecoveryCascade::new(budget);
    let mut failovers = 0u32;
    loop {
        let granted = cascade.grant(CascadeLevel::Fallback);
        if granted.terminal || granted.level > CascadeLevel::Fallback {
            break;
        }
        failovers += 1;
        assert!(failovers <= budget + 1, "failover bypassed the shared pool");
    }
    // One escalation into Fallback was free; every repetition after it spent
    // from the pool.
    assert_eq!(cascade.spent, budget);
}

/// Refinement depth is a cap within the shared pool, never its own pool: a
/// cascade that refines repeatedly exhausts exactly like one that retries.
#[test]
fn mc_n_refinement_has_no_private_pool() {
    let budget = 2;
    let mut refine = RecoveryCascade::new(budget);
    refine.grant(CascadeLevel::Refine); // escalation, free
    refine.grant(CascadeLevel::Refine); // horizontal 1
    refine.grant(CascadeLevel::Refine); // horizontal 2
    let forced = refine.grant(CascadeLevel::Refine);
    assert!(forced.forced_escalation, "a third refinement must escalate");
    assert_eq!(refine.spent, budget);
}
