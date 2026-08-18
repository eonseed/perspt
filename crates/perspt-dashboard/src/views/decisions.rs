//! Decisions view model: the raw PSP-9 event trace (system 14). Each row is
//! one Merkle-chained ledger record, summarized for scanning.

use perspt_store::Psp9LedgerRow;

/// View model for the decisions trace page.
pub struct DecisionsViewModel {
    pub session_id: String,
    pub psp9_events: Vec<Psp9EventRow>,
}

pub struct Psp9EventRow {
    pub sequence: i64,
    pub kind: String,
    pub summary: String,
    pub hash_short: String,
}

impl DecisionsViewModel {
    pub fn from_store(session_id: String, psp9_events: Vec<Psp9LedgerRow>) -> Self {
        Self {
            session_id,
            psp9_events: psp9_events.into_iter().map(psp9_event_row).collect(),
        }
    }
}

fn psp9_event_row(row: Psp9LedgerRow) -> Psp9EventRow {
    let value: serde_json::Value = serde_json::from_str(&row.event_json).unwrap_or_default();
    let outer = value
        .get("event")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let custom = value.get("kind").and_then(|value| value.as_str());
    let nested = value
        .get("payload")
        .and_then(|payload| payload.get("body").or(Some(payload)))
        .and_then(|payload| payload.get("event"))
        .and_then(|value| value.as_str());
    let kind = [Some(outer), custom, nested]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" / ");
    let mut summary = value.get("payload").cloned().unwrap_or(value).to_string();
    if summary.len() > 280 {
        // Walk back to a char boundary: ledger payloads carry model text and
        // paths, and truncating inside a multibyte char panics.
        let mut boundary = 277;
        while !summary.is_char_boundary(boundary) {
            boundary -= 1;
        }
        summary.truncate(boundary);
        summary.push_str("...");
    }
    Psp9EventRow {
        sequence: row.sequence,
        kind,
        summary,
        hash_short: row.hash.chars().take(12).collect(),
    }
}
