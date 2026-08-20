//! The documented search-ledger fold (spec :2340-2341): resume
//! reconstructs the no-good store and the interrupted forest's usage from
//! `no_good_recorded`, `search_opened`, `search_usage_snapshot`, and
//! `search_closed` rows. A separate read-only fold beside
//! `refold_session_context`, so search events stay out of the
//! accepted-trajectory fold (Gate W); unknown envelope versions fail
//! closed through the shared decoder (Gate AD).

use std::collections::BTreeMap;

use anyhow::Result;

use super::nogood::NoGoodStore;
use crate::toolloop::{decode_tool_loop, LoopEvent};

/// A forest the ledger shows opened but never closed, with its last
/// snapshot usage — the honest starting point for a deterministic re-run.
pub(crate) struct InterruptedForest {
    pub forest_id: String,
    pub node_id: String,
    pub generation: u32,
    pub accepted_root: String,
    pub last_usage: perspt_sdk::SearchUsage,
}

/// The folded search state of one session's ledger.
#[derive(Default)]
pub(crate) struct SearchLedgerFold {
    pub no_goods: NoGoodStore,
    pub interrupted: Option<InterruptedForest>,
}

/// Fold every `tool_loop` row's search events. Rows that are not
/// tool-loop events are skipped; a tool-loop row that fails the versioned
/// decoder fails the fold closed.
pub(crate) fn fold_search_ledger(rows: &[perspt_store::Psp9LedgerRow]) -> Result<SearchLedgerFold> {
    let mut fold = SearchLedgerFold::default();
    let mut open: BTreeMap<String, InterruptedForest> = BTreeMap::new();
    for row in rows {
        let Ok(perspt_sdk::LedgerEvent::Custom { kind, payload }) =
            serde_json::from_str::<perspt_sdk::LedgerEvent>(&row.event_json)
        else {
            continue;
        };
        if kind != "tool_loop" {
            continue;
        }
        match decode_tool_loop(&payload)?.event {
            LoopEvent::NoGoodRecorded {
                key, evidence_hash, ..
            } => {
                fold.no_goods.fold_entry(key, evidence_hash);
            }
            LoopEvent::SearchOpened {
                forest_id,
                node_id,
                generation,
                accepted_root,
                ..
            } => {
                anyhow::ensure!(
                    !open.contains_key(&forest_id),
                    "search forest {forest_id} was opened twice without closing"
                );
                open.insert(
                    forest_id.clone(),
                    InterruptedForest {
                        forest_id,
                        node_id,
                        generation,
                        accepted_root,
                        last_usage: perspt_sdk::SearchUsage::default(),
                    },
                );
            }
            LoopEvent::SearchUsageSnapshot {
                forest_id, usage, ..
            } => {
                if let Some(forest) = open.get_mut(&forest_id) {
                    forest.last_usage = usage;
                }
            }
            LoopEvent::SearchClosed { forest_id, .. } => {
                open.remove(&forest_id);
            }
            _ => {}
        }
    }
    // Recovery forests are serialized by the runtime. More than one open
    // forest means the usage cannot be assigned to one node honestly.
    anyhow::ensure!(
        open.len() <= 1,
        "multiple interrupted search forests make resume usage ambiguous"
    );
    fold.interrupted = open.into_values().next();
    Ok(fold)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(sequence: i64, event: &LoopEvent) -> perspt_store::Psp9LedgerRow {
        let envelope = serde_json::json!({
            "schema_version": 1,
            "body": serde_json::to_value(event).unwrap(),
        });
        let ledger_event = perspt_sdk::LedgerEvent::Custom {
            kind: "tool_loop".into(),
            payload: envelope,
        };
        perspt_store::Psp9LedgerRow {
            session_id: "s1".into(),
            sequence,
            event_json: serde_json::to_string(&ledger_event).unwrap(),
            prev_hash: String::new(),
            hash: String::new(),
        }
    }

    fn opened(forest_id: &str) -> LoopEvent {
        LoopEvent::SearchOpened {
            forest_id: forest_id.into(),
            node_id: "n1".into(),
            generation: 1,
            accepted_root: "root-1".into(),
            limits: perspt_sdk::SearchLimits::release_default(),
            resumed_from: None,
        }
    }

    /// The documented fold: no-goods re-enter the store, a closed forest
    /// leaves nothing behind, and an interrupted forest surfaces with its
    /// last snapshot usage.
    #[test]
    fn fold_reconstructs_no_goods_and_the_interrupted_forest() {
        let mut usage = perspt_sdk::SearchUsage::default();
        usage.model_turns = 3;
        let rows = vec![
            row(
                1,
                &LoopEvent::NoGoodRecorded {
                    forest_id: "f0".into(),
                    branch_id: "f0/b1".into(),
                    key: "k1".into(),
                    evidence_hash: "e1".into(),
                    support_kind: "failed-test".into(),
                },
            ),
            row(2, &opened("f0")),
            row(
                3,
                &LoopEvent::SearchClosed {
                    forest_id: "f0".into(),
                    usage: perspt_sdk::SearchUsage::default(),
                },
            ),
            row(4, &opened("f1")),
            row(
                5,
                &LoopEvent::SearchUsageSnapshot {
                    forest_id: "f1".into(),
                    epoch: 2,
                    usage: usage.clone(),
                },
            ),
        ];
        let fold = fold_search_ledger(&rows).unwrap();
        assert_eq!(fold.no_goods.len(), 1);
        let interrupted = fold.interrupted.expect("f1 never closed");
        assert_eq!(interrupted.forest_id, "f1");
        assert_eq!(interrupted.node_id, "n1");
        assert_eq!(interrupted.generation, 1);
        assert_eq!(interrupted.accepted_root, "root-1");
        assert_eq!(interrupted.last_usage.model_turns, 3);
    }

    /// Gate AD: a row with an unknown envelope version fails the fold
    /// closed instead of being skipped.
    #[test]
    fn unknown_envelope_versions_refuse_the_fold() {
        let ledger_event = perspt_sdk::LedgerEvent::Custom {
            kind: "tool_loop".into(),
            payload: serde_json::json!({"schema_version": 99, "body": {}}),
        };
        let rows = vec![perspt_store::Psp9LedgerRow {
            session_id: "s1".into(),
            sequence: 1,
            event_json: serde_json::to_string(&ledger_event).unwrap(),
            prev_hash: String::new(),
            hash: String::new(),
        }];
        assert!(fold_search_ledger(&rows).is_err());
    }

    #[test]
    fn multiple_interrupted_forests_refuse_ambiguous_usage() {
        let rows = vec![row(1, &opened("f-a")), row(2, &opened("f-b"))];
        let error = match fold_search_ledger(&rows) {
            Ok(_) => panic!("usage has no unique owner"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("multiple interrupted"));
    }
}
