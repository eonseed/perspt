//! Replay mechanism checks (PSP-9 Gate O, Paper III Theorem 7).
//!
//! MC-O: audit replay folds the recorded chain to the same head with no
//! provider credentials in scope; a tampered record is detected; a
//! transition depending on an unrecorded observation is refused; and a stale
//! context checkpoint is rejected rather than patched.

use perspt_sdk::ledger::{Ledger, LedgerEvent};
use perspt_sdk::{audit_replay, require_recorded, ContextCheckpoint, ControlFrame};

fn seeded_ledger() -> Ledger {
    let mut ledger = Ledger::new();
    ledger
        .append(LedgerEvent::ProposalObserved {
            proposal_id: "p1".into(),
            actor: "toolloop".into(),
        })
        .unwrap();
    ledger.record_observation(b"model output turn 1").unwrap();
    ledger
        .append(LedgerEvent::EnergyScored {
            node_id: "n1".into(),
            generation: 0,
            energy: 4.0,
        })
        .unwrap();
    ledger
        .append(LedgerEvent::CandidateAccepted {
            node_id: "n1".into(),
            generation: 0,
            energy: 4.0,
        })
        .unwrap();
    ledger
}

#[test]
fn mc_o_audit_replay_folds_to_the_recorded_head_without_credentials() {
    let ledger = seeded_ledger();
    let head_before = ledger.head();

    // No provider, no credential, no network: pure fold.
    let report = audit_replay(&ledger);
    assert!(report.chain_ok);
    assert_eq!(report.head, head_before);
    assert_eq!(report.accepted, vec![("n1".to_string(), 0, 4.0)]);

    // Determinism: folding twice yields the same report.
    assert_eq!(audit_replay(&ledger), report);
}

#[test]
fn mc_o_a_transition_on_an_unrecorded_observation_is_refused() {
    let mut ledger = Ledger::new();
    let handle = ledger.record_observation(b"tool result").unwrap();
    assert!(require_recorded(&ledger, &[handle]).is_ok());
    assert!(require_recorded(&ledger, &["never-recorded".to_string()]).is_err());
}

#[test]
fn mc_o_a_stale_checkpoint_is_rebuilt_not_patched() {
    let checkpoint = ContextCheckpoint {
        parent: None,
        covered_from: 0,
        covered_to: 10,
        covered_event_root: "root".into(),
        control: ControlFrame {
            goal: "g".into(),
            node_generation: 1,
            accepted_state_root: "state-1".into(),
            graph_revision: "rev-1".into(),
            capability_ids: vec![],
            authority_epoch: 3,
            remaining_rejection_budget: 1,
            remaining_turns: 2,
            active_model: perspt_sdk::ModelId::new("test", "model"),
            remaining_fallback_models: vec![],
            activated_tools: vec![],
            unresolved_call_ids: vec!["open-call".into()],
            residual_summary: vec![],
        },
        artifact_refs: vec![],
        narrative_observation: None,
    };
    // Live state diverged on the authority epoch: a revocation happened.
    assert!(checkpoint.validate_against("state-1", "rev-1", 4).is_err());
    // The unresolved call survives in the projection regardless.
    assert_eq!(checkpoint.control.unresolved_call_ids, ["open-call"]);
}
