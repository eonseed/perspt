use super::*;

/// One durable PSP-9 ledger record (system 14). The SDK's `Ledger` owns the
/// chain semantics; this row keeps the canonical bytes for resume and audit
/// replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Psp9LedgerRow {
    pub session_id: String,
    pub sequence: i64,
    pub event_json: String,
    pub prev_hash: String,
    pub hash: String,
}

impl SessionStore {
    /// Append one PSP-9 ledger record durably.
    pub fn record_psp9_event(&self, row: &Psp9LedgerRow) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO psp9_ledger_events (session_id, sequence, event_json, prev_hash, hash) \
             VALUES (?, ?, ?, ?, ?)",
            duckdb::params![
                row.session_id,
                row.sequence,
                row.event_json,
                row.prev_hash,
                row.hash
            ],
        )?;
        Ok(())
    }

    /// Load a session's PSP-9 event stream in sequence order.
    pub fn get_psp9_events(&self, session_id: &str) -> Result<Vec<Psp9LedgerRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, sequence, event_json, prev_hash, hash \
             FROM psp9_ledger_events WHERE session_id = ? ORDER BY sequence",
        )?;
        let rows = stmt.query_map([session_id], |row| {
            Ok(Psp9LedgerRow {
                session_id: row.get(0)?,
                sequence: row.get(1)?,
                event_json: row.get(2)?,
                prev_hash: row.get(3)?,
                hash: row.get(4)?,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn psp9_events_round_trip_in_order() {
        let dir = std::env::temp_dir().join(format!("perspt_p9_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = SessionStore::open(&dir.join("t.db")).unwrap();
        for sequence in 0..3i64 {
            store
                .record_psp9_event(&Psp9LedgerRow {
                    session_id: "s1".into(),
                    sequence,
                    event_json: format!("{{\"seq\":{sequence}}}"),
                    prev_hash: format!("h{}", sequence.saturating_sub(1)),
                    hash: format!("h{sequence}"),
                })
                .unwrap();
        }
        let rows = store.get_psp9_events("s1").unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[2].hash, "h2");
        assert!(store.get_psp9_events("other").unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
