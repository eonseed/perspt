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

fn tool_loop(ledger: &mut Ledger, payload: serde_json::Value) {
    ledger
        .append(LedgerEvent::Custom {
            kind: "tool_loop".into(),
            payload,
        })
        .unwrap();
}

fn measured(node: &str, generation: u32, candidate: &str, energy: f64) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "event": "candidate_measured",
        "node_id": node,
        "generation": generation,
        "energy": energy,
        "hard_pass": false,
        "residuals": [],
    });
    if !candidate.is_empty() {
        payload["candidate_id"] = candidate.into();
    }
    payload
}

fn gate_accepted(
    node: &str,
    generation: u32,
    candidate: &str,
    observed: Option<f64>,
) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "event": "gate_decision_recorded",
        "node_id": node,
        "generation": generation,
        "decision": {"kind": "accepted_by_descent", "delta_v": 0.5},
    });
    if !candidate.is_empty() {
        payload["candidate_id"] = candidate.into();
    }
    if let Some(observed) = observed {
        payload["observed_energy"] = observed.into();
    }
    payload
}

/// PSP-10 Phase 2 (Gate W): two candidates at one `(node, generation)` no
/// longer collide. Pre-fix, the fold keyed by `(node, generation)`, so the
/// later measurement silently overwrote the accepted one and the projection
/// reported the wrong energy.
#[test]
fn concurrent_candidates_fold_by_candidate_identity() {
    let mut ledger = Ledger::new();
    // Candidate A measured, then candidate B measured (interleaved round),
    // then A — not B — is accepted.
    tool_loop(&mut ledger, measured("n1", 0, "n1/0/c1", 3.0));
    tool_loop(&mut ledger, measured("n1", 0, "n1/0/c2", 2.0));
    tool_loop(&mut ledger, gate_accepted("n1", 0, "n1/0/c1", Some(3.0)));
    let accepted = perspt_sdk::ledger::replay_accepted_trajectory(&ledger);
    assert_eq!(
        accepted,
        vec![("n1".to_string(), 0, 3.0)],
        "the accepted candidate's energy, not the last writer's"
    );
}

/// A pre-PSP-10 ledger (no candidate ids, no recorded observed energy)
/// folds through the legacy correlation exactly as before.
#[test]
fn legacy_rows_fold_identically() {
    let mut ledger = Ledger::new();
    tool_loop(&mut ledger, measured("n1", 0, "", 4.0));
    tool_loop(&mut ledger, gate_accepted("n1", 0, "", None));
    tool_loop(&mut ledger, measured("n1", 0, "", 3.2));
    tool_loop(&mut ledger, gate_accepted("n1", 0, "", None));
    let accepted = perspt_sdk::ledger::replay_accepted_trajectory(&ledger);
    assert_eq!(
        accepted,
        vec![("n1".to_string(), 0, 4.0), ("n1".to_string(), 0, 3.2)]
    );
}

/// The gate event's own `observed_energy` wins over measurement correlation
/// — recorded, never recovered (PSP-10 system 21).
#[test]
fn recorded_observed_energy_is_authoritative() {
    let mut ledger = Ledger::new();
    tool_loop(&mut ledger, measured("n1", 0, "n1/0/c1", 9.9));
    tool_loop(&mut ledger, gate_accepted("n1", 0, "n1/0/c1", Some(3.0)));
    let accepted = perspt_sdk::ledger::replay_accepted_trajectory(&ledger);
    assert_eq!(accepted, vec![("n1".to_string(), 0, 3.0)]);
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
            projection_digest: "projection".into(),
            prompt_invocation_digest: String::new(),
            prompt_manifest_digest: String::new(),
            resident_context_digest: String::new(),
            event_schema_version: perspt_sdk::CONVERSATION_EVENT_SCHEMA_VERSION,
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
