//! Durable event recording for the PSP-9 runtime.

use super::*;

/// Durable sink for all live PSP-9 events.
pub struct Psp9Recorder {
    pub(crate) session_id: String,
    pub(crate) store: Arc<SessionStore>,
    ledger: Mutex<Ledger>,
    event_sender: Option<perspt_core::events::channel::EventSender>,
}

impl std::fmt::Debug for Psp9Recorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Psp9Recorder")
            .field("session_id", &self.session_id)
            .field("head", &self.head())
            .finish_non_exhaustive()
    }
}

impl Psp9Recorder {
    pub(crate) fn create(
        session_id: &str,
        task: &str,
        working_dir: &Path,
        database_path: Option<&Path>,
        shared_store: Option<Arc<SessionStore>>,
        event_sender: Option<perspt_core::events::channel::EventSender>,
    ) -> Result<Self> {
        // An embedder-supplied handle wins: sharing one connection with a
        // dashboard avoids a second live handle on the same database file.
        let store = match (shared_store, database_path) {
            (Some(store), _) => store,
            (None, Some(path)) => Arc::new(SessionStore::open(&path.to_path_buf())?),
            (None, None) => Arc::new(SessionStore::new()?),
        };
        store.create_session(&SessionRecord {
            session_id: session_id.into(),
            task: task.into(),
            working_dir: working_dir.display().to_string(),
            merkle_root: None,
            detected_toolchain: None,
            status: "RUNNING_PSP9".into(),
        })?;
        store.initialize_authority_epoch(session_id, 0)?;
        Ok(Self {
            session_id: session_id.into(),
            store,
            ledger: Mutex::new(Ledger::new()),
            event_sender,
        })
    }

    pub fn record_custom(&self, kind: &str, payload: serde_json::Value) -> Result<()> {
        self.append(LedgerEvent::Custom {
            kind: kind.into(),
            payload,
        })
    }

    /// Write-ahead append: stage the record, persist it durably, and only
    /// then extend the in-memory chain. Staging avoids the previous
    /// clone-the-whole-ledger transaction, which made session recording
    /// O(n²) in events.
    pub(crate) fn append(&self, event: LedgerEvent) -> Result<()> {
        let mut guard = self.ledger.lock().unwrap();
        let record = guard
            .stage(event)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        self.store.record_psp9_event(&Psp9LedgerRow {
            session_id: self.session_id.clone(),
            sequence: record.sequence as i64,
            event_json: serde_json::to_string(&record.event)?,
            prev_hash: record.prev_hash.clone(),
            hash: record.hash.clone(),
        })?;
        guard
            .commit_staged(record)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        Ok(())
    }

    pub fn head(&self) -> String {
        self.ledger.lock().unwrap().head()
    }

    pub(crate) fn finish(&self, status: &str) -> Result<()> {
        self.store.update_session_status(&self.session_id, status)
    }

    pub(crate) fn authority_epoch(&self) -> Result<u64> {
        self.store.authority_epoch(&self.session_id)
    }

    pub(crate) fn record_grant_policy(
        &self,
        policy: &GrantPolicy,
        signed: Option<&perspt_sdk::SignedGrantPolicy>,
    ) -> Result<()> {
        let durable = signed
            .map(serde_json::to_string)
            .transpose()?
            .unwrap_or_else(|| serde_json::to_string(policy).expect("serializable grant policy"));
        self.store
            .record_grant_policy(&self.session_id, &policy.policy_id, &durable)?;
        self.record_custom(
            "grant_policy",
            serde_json::json!({
                "policy": policy,
                "signed": signed.is_some(),
            }),
        )
    }

    pub(crate) fn record_external_intent(
        &self,
        key: &str,
        intent: &serde_json::Value,
    ) -> Result<()> {
        let bytes = serde_json::to_vec(intent)?;
        self.store.record_external_effect_intent(
            &self.session_id,
            key,
            &perspt_sdk::ledger::content_hash(&bytes),
            &String::from_utf8(bytes)?,
        )?;
        self.record_custom("external_effect_intent", intent.clone())
    }

    pub(crate) fn complete_external_effect(
        &self,
        key: &str,
        result: &serde_json::Value,
    ) -> Result<()> {
        self.store.complete_external_effect(
            &self.session_id,
            key,
            &serde_json::to_string(result)?,
        )?;
        self.record_custom("external_effect_completed", result.clone())
    }
}

impl LoopRecorder for Psp9Recorder {
    fn record(&self, event: &LoopEvent) -> Result<()> {
        self.record_custom("tool_loop", serde_json::to_value(event)?)?;
        if let LoopEvent::ContextCheckpointCreated { checkpoint } = event {
            self.store.record_psp9_checkpoint(
                &self.session_id,
                &checkpoint.covered_event_root,
                &serde_json::to_string(checkpoint)?,
            )?;
        }
        if let Some(sender) = &self.event_sender {
            // Structured surfaces first: the TUI's energy panel consumes the
            // typed event, not the narration line.
            if let LoopEvent::CandidateMeasured {
                node_id, energy, ..
            } = event
            {
                let _ = sender.send(perspt_core::AgentEvent::EnergyUpdated {
                    node_id: node_id.clone(),
                    energy: *energy as f32,
                });
            }
            let message = match event {
                LoopEvent::CandidateMeasured {
                    node_id,
                    generation,
                    energy,
                    hard_pass,
                    residuals,
                } => Some(format!(
                    "Measured {node_id} generation {generation}: V={energy:.3}, \
                         hard_pass={hard_pass}, residuals={}",
                    residuals.len()
                )),
                LoopEvent::GateDecisionRecorded {
                    node_id,
                    generation,
                    decision,
                } => Some(format!(
                    "Gate {node_id} generation {generation}: {decision:?}"
                )),
                LoopEvent::EffectDenied {
                    call_id, reason, ..
                } => Some(format!("Effect {call_id} denied: {reason}")),
                LoopEvent::EffectApplied {
                    call_id, mutated, ..
                } => Some(format!(
                    "Effect {call_id} applied to candidate (mutated={mutated})"
                )),
                LoopEvent::RouteFailover {
                    from_model,
                    to_model,
                    cause,
                } => Some(format!(
                    "Route failover {from_model} -> {to_model}: {cause}"
                )),
                LoopEvent::ContextCheckpointCreated { checkpoint } => Some(format!(
                    "Context checkpoint {} covers events {}..{}",
                    &checkpoint.covered_event_root[..12.min(checkpoint.covered_event_root.len())],
                    checkpoint.covered_from,
                    checkpoint.covered_to
                )),
                LoopEvent::RecoveryContained { reason, .. } => {
                    Some(format!("Recovery contained the node: {reason}"))
                }
                _ => None,
            };
            if let Some(message) = message {
                let _ = sender.send(perspt_core::AgentEvent::Log(message));
            }
        }
        Ok(())
    }

    fn record_artifact(&self, content: &[u8], media_type: &str) -> Result<String> {
        let handle = perspt_sdk::ledger::content_hash(content);
        self.store.put_psp9_artifact(&handle, content, media_type)?;
        Ok(handle)
    }
}
