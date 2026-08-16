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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Psp9VerdictRow {
    pub session_id: String,
    pub candidate_id: String,
    pub validator_id: String,
    pub stratum: String,
    pub missed: bool,
    pub unsafe_label: Option<bool>,
    pub evidence_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Psp9CalibrationEpochRow {
    pub epoch_id: String,
    pub stratum: String,
    pub target_rho: f64,
    pub threshold: Option<f64>,
    pub state: String,
    pub sample_count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Psp9ExternalEffectRow {
    pub idempotency_key: String,
    pub intent_hash: String,
    pub intent_json: String,
    pub result_json: Option<String>,
    pub status: String,
}

impl SessionStore {
    pub fn put_psp9_artifact(
        &self,
        content_hash: &str,
        content: &[u8],
        media_type: &str,
    ) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO psp9_artifacts (content_hash, content, byte_len, media_type) \
             VALUES (?, ?, ?, ?) ON CONFLICT (content_hash) DO NOTHING",
            duckdb::params![content_hash, content, content.len() as i64, media_type],
        )?;
        Ok(())
    }

    pub fn get_psp9_artifact(&self, content_hash: &str) -> Result<Option<Vec<u8>>> {
        let conn = self.conn.lock().unwrap();
        let mut statement =
            conn.prepare("SELECT content FROM psp9_artifacts WHERE content_hash = ?")?;
        let mut rows = statement.query([content_hash])?;
        Ok(rows.next()?.map(|row| row.get(0)).transpose()?)
    }

    pub fn record_psp9_checkpoint(
        &self,
        session_id: &str,
        covered_event_root: &str,
        checkpoint_json: &str,
    ) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO psp9_context_checkpoints \
             (session_id, covered_event_root, checkpoint_json) VALUES (?, ?, ?) \
             ON CONFLICT (session_id, covered_event_root) DO NOTHING",
            duckdb::params![session_id, covered_event_root, checkpoint_json],
        )?;
        Ok(())
    }

    pub fn record_psp9_verdict(&self, row: &Psp9VerdictRow) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO psp9_verdicts (session_id, candidate_id, validator_id, stratum, \
             missed, unsafe_label, evidence_hash) VALUES (?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT (session_id, candidate_id, validator_id) DO UPDATE SET \
             unsafe_label = excluded.unsafe_label, evidence_hash = excluded.evidence_hash",
            duckdb::params![
                row.session_id,
                row.candidate_id,
                row.validator_id,
                row.stratum,
                row.missed,
                row.unsafe_label,
                row.evidence_hash
            ],
        )?;
        Ok(())
    }

    /// Per-validator verdict history for one session, in insertion order.
    pub fn get_psp9_verdicts(&self, session_id: &str) -> Result<Vec<Psp9VerdictRow>> {
        let conn = self.conn.lock().unwrap();
        let mut statement = conn.prepare(
            "SELECT session_id, candidate_id, validator_id, stratum, missed, \
             unsafe_label, evidence_hash FROM psp9_verdicts WHERE session_id = ?",
        )?;
        let rows = statement.query_map([session_id], |row| {
            Ok(Psp9VerdictRow {
                session_id: row.get(0)?,
                candidate_id: row.get(1)?,
                validator_id: row.get(2)?,
                stratum: row.get(3)?,
                missed: row.get(4)?,
                unsafe_label: row.get(5)?,
                evidence_hash: row.get(6)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn record_psp9_calibration_epoch(&self, row: &Psp9CalibrationEpochRow) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO psp9_calibration_epochs \
             (epoch_id, stratum, target_rho, threshold, state, sample_count) \
             VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT (epoch_id) DO NOTHING",
            duckdb::params![
                row.epoch_id,
                row.stratum,
                row.target_rho,
                row.threshold,
                row.state,
                row.sample_count
            ],
        )?;
        Ok(())
    }

    /// Return the newest immutable epoch for exactly one serialized stratum.
    /// A caller must never reuse an epoch across a changed stratum.
    pub fn latest_psp9_calibration_epoch(
        &self,
        stratum: &str,
    ) -> Result<Option<Psp9CalibrationEpochRow>> {
        let conn = self.conn.lock().unwrap();
        let mut statement = conn.prepare(
            "SELECT epoch_id, stratum, target_rho, threshold, state, sample_count \
             FROM psp9_calibration_epochs WHERE stratum = ? \
             ORDER BY created_at DESC, epoch_id DESC LIMIT 1",
        )?;
        let mut rows = statement.query([stratum])?;
        rows.next()?
            .map(|row| {
                Ok(Psp9CalibrationEpochRow {
                    epoch_id: row.get(0)?,
                    stratum: row.get(1)?,
                    target_rho: row.get(2)?,
                    threshold: row.get(3)?,
                    state: row.get(4)?,
                    sample_count: row.get(5)?,
                })
            })
            .transpose()
    }

    pub fn psp9_calibration_samples(
        &self,
        epoch_id: &str,
    ) -> Result<Vec<(String, f64, bool, bool)>> {
        let conn = self.conn.lock().unwrap();
        let mut statement = conn.prepare(
            "SELECT sample_id, score, unsafe_label, audit_selected \
             FROM psp9_calibration_samples WHERE epoch_id = ? ORDER BY created_at, sample_id",
        )?;
        let rows = statement.query_map([epoch_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn record_psp9_calibration_sample(
        &self,
        epoch_id: &str,
        sample_id: &str,
        score: f64,
        unsafe_label: bool,
        audit_selected: bool,
    ) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO psp9_calibration_samples \
             (epoch_id, sample_id, score, unsafe_label, audit_selected) VALUES (?, ?, ?, ?, ?) \
             ON CONFLICT (epoch_id, sample_id) DO NOTHING",
            duckdb::params![epoch_id, sample_id, score, unsafe_label, audit_selected],
        )?;
        Ok(())
    }

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

    pub fn initialize_authority_epoch(&self, session_id: &str, epoch: u64) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO psp9_authority_epochs (session_id, epoch) VALUES (?, ?) \
             ON CONFLICT (session_id) DO NOTHING",
            duckdb::params![session_id, epoch as i64],
        )?;
        Ok(())
    }

    pub fn authority_epoch(&self, session_id: &str) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        let mut statement =
            conn.prepare("SELECT epoch FROM psp9_authority_epochs WHERE session_id = ?")?;
        let mut rows = statement.query([session_id])?;
        let row = rows.next()?.context("authority epoch is not initialized")?;
        let epoch: i64 = row.get(0)?;
        Ok(epoch as u64)
    }

    pub fn revoke_authority(&self, session_id: &str) -> Result<u64> {
        let mut conn = self.conn.lock().unwrap();
        let transaction = conn.transaction()?;
        transaction.execute(
            "UPDATE psp9_authority_epochs SET epoch = epoch + 1 WHERE session_id = ?",
            [session_id],
        )?;
        let epoch: i64 = transaction.query_row(
            "SELECT epoch FROM psp9_authority_epochs WHERE session_id = ?",
            [session_id],
            |row| row.get(0),
        )?;
        transaction.commit()?;
        Ok(epoch as u64)
    }

    pub fn record_grant_policy(
        &self,
        session_id: &str,
        policy_id: &str,
        policy_json: &str,
    ) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO psp9_grant_policies (policy_id, session_id, policy_json) \
             VALUES (?, ?, ?)",
            duckdb::params![policy_id, session_id, policy_json],
        )?;
        Ok(())
    }

    /// R1/R5: durably single-assign an external-effect intent before the
    /// filesystem or network is touched. Redelivery with identical bytes is
    /// idempotent; key reuse with different content is rejected.
    pub fn record_external_effect_intent(
        &self,
        session_id: &str,
        idempotency_key: &str,
        intent_hash: &str,
        intent_json: &str,
    ) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let transaction = conn.transaction()?;
        let existing: Option<String> = transaction
            .query_row(
                "SELECT intent_hash FROM psp9_external_effects \
                 WHERE session_id = ? AND idempotency_key = ?",
                duckdb::params![session_id, idempotency_key],
                |row| row.get(0),
            )
            .ok();
        match existing {
            Some(hash) if hash == intent_hash => {}
            Some(_) => anyhow::bail!("idempotency key reused for different external effect"),
            None => {
                transaction.execute(
                    "INSERT INTO psp9_external_effects \
                     (session_id, idempotency_key, intent_hash, intent_json, status) \
                     VALUES (?, ?, ?, ?, 'INTENT_RECORDED')",
                    duckdb::params![session_id, idempotency_key, intent_hash, intent_json],
                )?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn complete_external_effect(
        &self,
        session_id: &str,
        idempotency_key: &str,
        result_json: &str,
    ) -> Result<()> {
        let updated = self.conn.lock().unwrap().execute(
            "UPDATE psp9_external_effects SET result_json = ?, status = 'COMPLETED', \
             completed_at = CURRENT_TIMESTAMP \
             WHERE session_id = ? AND idempotency_key = ? AND status = 'INTENT_RECORDED'",
            duckdb::params![result_json, session_id, idempotency_key],
        )?;
        anyhow::ensure!(
            updated == 1,
            "external effect has no pending write-ahead intent"
        );
        Ok(())
    }

    pub fn pending_external_effects(&self, session_id: &str) -> Result<Vec<Psp9ExternalEffectRow>> {
        let conn = self.conn.lock().unwrap();
        let mut statement = conn.prepare(
            "SELECT idempotency_key, intent_hash, intent_json, result_json, status \
             FROM psp9_external_effects WHERE session_id = ? AND status != 'COMPLETED' \
             ORDER BY created_at",
        )?;
        let rows = statement.query_map([session_id], |row| {
            Ok(Psp9ExternalEffectRow {
                idempotency_key: row.get(0)?,
                intent_hash: row.get(1)?,
                intent_json: row.get(2)?,
                result_json: row.get(3)?,
                status: row.get(4)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_store() -> (std::path::PathBuf, SessionStore) {
        let dir = std::env::temp_dir().join(format!("perspt_p9_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = SessionStore::open(&dir.join("t.db")).unwrap();
        (dir, store)
    }

    #[test]
    fn psp9_events_round_trip_in_order() {
        let (dir, store) = scratch_store();
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

    #[test]
    fn external_effects_are_single_assignment() {
        let (dir, store) = scratch_store();
        store
            .record_external_effect_intent("s1", "promote:n1", "h1", "{\"a\":1}")
            .unwrap();
        store
            .record_external_effect_intent("s1", "promote:n1", "h1", "{\"a\":1}")
            .unwrap();
        assert!(store
            .record_external_effect_intent("s1", "promote:n1", "h2", "{\"a\":2}")
            .is_err());
        store
            .complete_external_effect("s1", "promote:n1", "{\"ok\":true}")
            .unwrap();
        assert!(store.pending_external_effects("s1").unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn artifacts_verdicts_and_calibration_rows_persist() {
        let (dir, store) = scratch_store();
        let bytes = b"complete compiler output";
        let hash = "artifact-hash".to_string();
        store.put_psp9_artifact(&hash, bytes, "text/plain").unwrap();
        assert_eq!(store.get_psp9_artifact(&hash).unwrap().unwrap(), bytes);
        store
            .record_psp9_checkpoint("s1", "root-1", "{\"control\":{}}")
            .unwrap();
        store
            .record_psp9_verdict(&Psp9VerdictRow {
                session_id: "s1".into(),
                candidate_id: "c1".into(),
                validator_id: "cargo-test".into(),
                stratum: "rust:test:v1".into(),
                missed: false,
                unsafe_label: Some(false),
                evidence_hash: hash.clone(),
            })
            .unwrap();
        store
            .record_psp9_calibration_epoch(&Psp9CalibrationEpochRow {
                epoch_id: "e1".into(),
                stratum: "rust:test:v1".into(),
                target_rho: 0.1,
                threshold: Some(0.5),
                state: "shadow".into(),
                sample_count: 1,
            })
            .unwrap();
        store
            .record_psp9_calibration_sample("e1", "sample-1", 0.2, false, true)
            .unwrap();
        assert_eq!(
            store
                .latest_psp9_calibration_epoch("rust:test:v1")
                .unwrap()
                .unwrap()
                .state,
            "shadow"
        );
        assert_eq!(
            store.psp9_calibration_samples("e1").unwrap(),
            vec![("sample-1".into(), 0.2, false, true)]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
