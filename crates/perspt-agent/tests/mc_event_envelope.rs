//! Event-envelope mechanism checks (PSP-10 Gate AD, Phase 7).
//!
//! One fact, one layer, one decoder per version: legacy rows decode only
//! through the strict legacy decoder; version 1 rows only as
//! `LoopEventEnvelopeV1`; unknown versions fail authoritative replay and
//! resume closed while raw forensic display remains possible.

use perspt_agent::toolloop::{decode_tool_loop, LoopEvent, LoopEventEnvelopeV1};
use perspt_sdk::ledger::{tool_loop_body, Ledger, LedgerEvent, ToolLoopBody};

fn legacy_payload() -> serde_json::Value {
    // A true pre-PSP-10 row: no candidate_id, no observed_energy.
    serde_json::json!({
        "event": "candidate_measured",
        "node_id": "n1",
        "generation": 0,
        "energy": 2.0,
        "hard_pass": false,
        "residuals": [],
    })
}

fn v1_payload() -> serde_json::Value {
    serde_json::json!({
        "schema_version": 1,
        "body": legacy_payload(),
    })
}

#[test]
fn legacy_rows_use_the_legacy_decoder_and_v1_rows_the_envelope() {
    let legacy = decode_tool_loop(&legacy_payload()).unwrap();
    assert_eq!(legacy.version, 0);
    assert!(matches!(legacy.event, LoopEvent::CandidateMeasured { .. }));

    let versioned = decode_tool_loop(&v1_payload()).unwrap();
    assert_eq!(versioned.version, 1);
    assert!(matches!(
        versioned.event,
        LoopEvent::CandidateMeasured { .. }
    ));
}

#[test]
fn a_new_variant_in_an_unversioned_row_is_refused() {
    let smuggled = serde_json::json!({
        "event": "search_opened",
        "forest_id": "f1",
        "node_id": "n1",
        "generation": 0,
        "accepted_root": "root",
        "limits": perspt_sdk::SearchLimits::release_default(),
    });
    let error = decode_tool_loop(&smuggled).unwrap_err();
    assert!(error.to_string().contains("unversioned"), "{error}");
    // The same event inside the v1 envelope decodes.
    let wrapped = serde_json::json!({"schema_version": 1, "body": smuggled});
    assert!(decode_tool_loop(&wrapped).is_ok());
}

#[test]
fn unknown_versions_fail_authoritative_folds_but_stay_displayable() {
    let unknown = serde_json::json!({"schema_version": 2, "body": legacy_payload()});
    assert!(decode_tool_loop(&unknown).is_err());
    assert!(tool_loop_body(&unknown).is_err());
    // The accepted fold fails closed rather than truncating silently.
    let mut ledger = Ledger::new();
    ledger
        .append(LedgerEvent::Custom {
            kind: "tool_loop".into(),
            payload: unknown.clone(),
        })
        .unwrap();
    assert!(perspt_sdk::ledger::replay_accepted_trajectory(&ledger).is_err());
    let report = perspt_sdk::audit_replay(&ledger);
    assert!(!report.chain_ok, "an unfoldable stream is a failed audit");
    // Raw forensic display needs no decoding at all: the payload is intact.
    assert_eq!(unknown["schema_version"], 2);
}

#[test]
fn the_envelope_round_trips_and_pins_version_one() {
    let event = LoopEvent::SearchClosed {
        forest_id: "f1".into(),
        usage: perspt_sdk::SearchUsage::default(),
    };
    let envelope = LoopEventEnvelopeV1::new(event);
    assert_eq!(envelope.schema_version, 1);
    let value = serde_json::to_value(&envelope).unwrap();
    match tool_loop_body(&value).unwrap() {
        ToolLoopBody::V1(body) => assert_eq!(body["event"], "search_closed"),
        ToolLoopBody::Legacy(_) => panic!("versioned rows must not decode as legacy"),
    }
    let decoded = decode_tool_loop(&value).unwrap();
    assert!(matches!(decoded.event, LoopEvent::SearchClosed { .. }));
}

/// A pre-PSP-10 fixture stream folds identically through the legacy
/// decoder (the compatibility contract for old ledgers).
#[test]
fn pre_psp10_streams_fold_identically() {
    let mut ledger = Ledger::new();
    ledger
        .append(LedgerEvent::Custom {
            kind: "tool_loop".into(),
            payload: legacy_payload(),
        })
        .unwrap();
    ledger
        .append(LedgerEvent::Custom {
            kind: "tool_loop".into(),
            payload: serde_json::json!({
                "event": "gate_decision_recorded",
                "node_id": "n1",
                "generation": 0,
                "decision": {"kind": "accepted_by_descent", "delta_v": 0.5},
            }),
        })
        .unwrap();
    let accepted = perspt_sdk::ledger::replay_accepted_trajectory(&ledger).unwrap();
    assert_eq!(accepted, vec![("n1".to_string(), 0, 2.0)]);
}
