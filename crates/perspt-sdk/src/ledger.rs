//! Event-sourced replay ledger and durable-execution obligations (PSP-8
//! System 11).
//!
//! The ledger is an append-only, Merkle-chained event stream. Replay is
//! deterministic over recorded observations: any nondeterministic observation a
//! transition depends on is recorded *before* it is used, and the kernel refuses
//! to commit a transition that references an observation lacking a ledger
//! record. This structurally discharges the recording obligation at the SDK
//! effect boundary rather than relying on convention.
//!
//! The six durable-execution obligations (PSP-8 Def 9.1):
//!
//! * R1 durable single-assignment outcomes ([`IdempotencyLog`]);
//! * R2 recorded nondeterminism ([`Ledger::record_observation`]);
//! * R3 deterministic transition ([`replay_accepted_trajectory`]);
//! * R4 unforgeable capability transport ([`crate::capability`]);
//! * R5 write-ahead external effects ([`ExternalEffectLog`]);
//! * R6 ordered non-commuting durable turns ([`crate::scheduler`]).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{Result, SdkError};

/// A ledgered event (PSP-8 System 11). Representative of the full event family;
/// `Custom` carries any additional structured event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum LedgerEvent {
    ProposalObserved {
        proposal_id: String,
        actor: String,
    },
    AdmissibilityChecked {
        proposal_id: String,
        allowed: bool,
    },
    EffectApplied {
        proposal_id: String,
        idempotency_key: String,
    },
    EffectDenied {
        proposal_id: String,
        reason: String,
    },
    VerifierCompleted {
        node_id: String,
        generation: u32,
    },
    ResidualEmitted {
        residual_id: String,
        node_id: String,
    },
    EnergyScored {
        node_id: String,
        generation: u32,
        energy: f64,
    },
    GateDecisionRecorded {
        node_id: String,
        accepted: bool,
    },
    CandidateAccepted {
        node_id: String,
        generation: u32,
        energy: f64,
    },
    CandidateRejected {
        node_id: String,
        generation: u32,
    },
    GraphRevisionAccepted {
        revision_id: String,
        sequence: u32,
    },
    NodeGenerationRetired {
        node_id: String,
        generation: u32,
    },
    ResidualCertificateIssued {
        certificate_id: String,
        node_id: String,
    },
    RollbackApplied {
        target_event: String,
    },
    CapabilityGranted {
        capability_id: String,
        holder: String,
    },
    CapabilityRevoked {
        capability_id: String,
    },
    /// An observation of nondeterministic data, recorded before use (R2).
    ObservationRecorded {
        handle: String,
        content_hash: String,
    },
    Custom {
        kind: String,
        payload: serde_json::Value,
    },
}

/// One record in the Merkle-chained ledger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerRecord {
    pub sequence: u64,
    pub event: LedgerEvent,
    /// Hash of the previous record (`"GENESIS"` for the first).
    pub prev_hash: String,
    /// `sha256(prev_hash || sequence || canonical(event))`.
    pub hash: String,
}

/// Compute the chained hash for a record.
fn chain_hash(prev_hash: &str, sequence: u64, event: &LedgerEvent) -> Result<String> {
    let canonical = serde_json::to_vec(event)
        .map_err(|e| SdkError::Domain(format!("event serialization failed: {e}")))?;
    let mut hasher = Sha256::new();
    hasher.update(prev_hash.as_bytes());
    hasher.update(sequence.to_le_bytes());
    hasher.update(&canonical);
    Ok(hex(&hasher.finalize()))
}

/// Hash arbitrary content (for observations and state witnesses).
pub fn content_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex(&hasher.finalize())
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// The append-only, Merkle-chained event ledger.
#[derive(Debug, Clone, Default)]
pub struct Ledger {
    records: Vec<LedgerRecord>,
    /// Recorded observation handles -> content hash (R2).
    observations: HashMap<String, String>,
}

impl Ledger {
    pub fn new() -> Self {
        Self::default()
    }

    /// The current ledger head (Merkle root), or `"GENESIS"` when empty.
    pub fn head(&self) -> String {
        self.records
            .last()
            .map(|r| r.hash.clone())
            .unwrap_or_else(|| "GENESIS".to_string())
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn records(&self) -> &[LedgerRecord] {
        &self.records
    }

    /// Append an event, extending the Merkle chain. Returns the new head.
    pub fn append(&mut self, event: LedgerEvent) -> Result<String> {
        let sequence = self.records.len() as u64;
        let prev_hash = self.head();
        let hash = chain_hash(&prev_hash, sequence, &event)?;
        // Track observation records as they are appended (R2).
        if let LedgerEvent::ObservationRecorded {
            handle,
            content_hash,
        } = &event
        {
            self.observations
                .insert(handle.clone(), content_hash.clone());
        }
        self.records.push(LedgerRecord {
            sequence,
            event,
            prev_hash,
            hash: hash.clone(),
        });
        Ok(hash)
    }

    /// Record a nondeterministic observation before it is used (R2). Returns the
    /// observation handle (content address).
    pub fn record_observation(&mut self, content: &[u8]) -> Result<String> {
        let content_hash = content_hash(content);
        let handle = content_hash.clone();
        self.append(LedgerEvent::ObservationRecorded {
            handle: handle.clone(),
            content_hash,
        })?;
        Ok(handle)
    }

    /// Whether an observation handle has a ledger record.
    pub fn has_observation(&self, handle: &str) -> bool {
        self.observations.contains_key(handle)
    }

    /// The kernel-refusal rule: refuse to commit a transition that references an
    /// observation lacking a ledger record (PSP-8 System 11). Returns an error
    /// naming the first unrecorded handle.
    pub fn commit_transition(
        &mut self,
        event: LedgerEvent,
        referenced_observations: &[String],
    ) -> Result<String> {
        for handle in referenced_observations {
            if !self.has_observation(handle) {
                return Err(SdkError::Domain(format!(
                    "kernel-refusal: transition references unrecorded observation `{handle}`"
                )));
            }
        }
        self.append(event)
    }

    /// Verify the Merkle chain end to end (tamper detection).
    pub fn verify_chain(&self) -> Result<()> {
        let mut prev = "GENESIS".to_string();
        for (i, rec) in self.records.iter().enumerate() {
            if rec.sequence != i as u64 {
                return Err(SdkError::Domain(format!("sequence gap at index {i}")));
            }
            if rec.prev_hash != prev {
                return Err(SdkError::Domain(format!(
                    "broken chain at sequence {}",
                    rec.sequence
                )));
            }
            let expected = chain_hash(&rec.prev_hash, rec.sequence, &rec.event)?;
            if expected != rec.hash {
                return Err(SdkError::Domain(format!(
                    "hash mismatch at sequence {}",
                    rec.sequence
                )));
            }
            prev = rec.hash.clone();
        }
        Ok(())
    }
}

/// Reconstruct the accepted trajectory deterministically from the recorded
/// events (R3). Replay reads recorded observations rather than re-running
/// nondeterministic sources.
pub fn replay_accepted_trajectory(ledger: &Ledger) -> Vec<(String, u32, f64)> {
    ledger
        .records()
        .iter()
        .filter_map(|r| match &r.event {
            LedgerEvent::CandidateAccepted {
                node_id,
                generation,
                energy,
            } => Some((node_id.clone(), *generation, *energy)),
            _ => None,
        })
        .collect()
}

/// R1 durable single-assignment outcomes. Every proposal/commit outcome is
/// written once and never reassigned; redelivery of the same idempotency key
/// with equivalent content returns the prior result, and key reuse for
/// different content is invalid.
#[derive(Debug, Clone, Default)]
pub struct IdempotencyLog {
    entries: HashMap<String, (String, String)>, // key -> (content_hash, outcome)
}

impl IdempotencyLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an outcome under an idempotency key. On first write, stores and
    /// returns the outcome. On redelivery with equivalent content, returns the
    /// prior outcome. On key reuse with different content, returns an error.
    pub fn record(&mut self, key: &str, content: &[u8], outcome: &str) -> Result<String> {
        let ch = content_hash(content);
        match self.entries.get(key) {
            Some((existing_hash, existing_outcome)) => {
                if existing_hash == &ch {
                    Ok(existing_outcome.clone())
                } else {
                    Err(SdkError::Domain(format!(
                        "idempotency key `{key}` reused for different content"
                    )))
                }
            }
            None => {
                self.entries
                    .insert(key.to_string(), (ch, outcome.to_string()));
                Ok(outcome.to_string())
            }
        }
    }
}

/// R5 write-ahead external effects: each irreversible external effect is
/// bracketed by intent, result, and (where defined) compensation records under
/// a stable idempotency key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExternalEffectPhase {
    Intent,
    Result,
    Compensation,
}

#[derive(Debug, Clone, Default)]
pub struct ExternalEffectLog {
    phases: HashMap<String, Vec<ExternalEffectPhase>>, // idempotency_key -> phases
}

impl ExternalEffectLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record intent before the external effect executes.
    pub fn intent(&mut self, key: &str) {
        self.phases
            .entry(key.to_string())
            .or_default()
            .push(ExternalEffectPhase::Intent);
    }

    /// Record the result after the external effect executes.
    pub fn result(&mut self, key: &str) -> Result<()> {
        let phases = self.phases.get(key).cloned().unwrap_or_default();
        if !phases.contains(&ExternalEffectPhase::Intent) {
            return Err(SdkError::Domain(format!(
                "R5 violation: result recorded for `{key}` without prior intent"
            )));
        }
        self.phases
            .get_mut(key)
            .unwrap()
            .push(ExternalEffectPhase::Result);
        Ok(())
    }

    pub fn compensation(&mut self, key: &str) {
        self.phases
            .entry(key.to_string())
            .or_default()
            .push(ExternalEffectPhase::Compensation);
    }

    /// Whether an effect was properly bracketed (intent precedes result).
    pub fn is_bracketed(&self, key: &str) -> bool {
        match self.phases.get(key) {
            Some(p) => {
                let i = p.iter().position(|x| *x == ExternalEffectPhase::Intent);
                let r = p.iter().position(|x| *x == ExternalEffectPhase::Result);
                matches!((i, r), (Some(i), Some(r)) if i < r)
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_is_verifiable() {
        let mut ledger = Ledger::new();
        ledger
            .append(LedgerEvent::CandidateAccepted {
                node_id: "a".into(),
                generation: 0,
                energy: 5.0,
            })
            .unwrap();
        ledger
            .append(LedgerEvent::CandidateAccepted {
                node_id: "b".into(),
                generation: 0,
                energy: 0.0,
            })
            .unwrap();
        assert_eq!(ledger.len(), 2);
        assert!(ledger.verify_chain().is_ok());
    }

    #[test]
    fn tampering_breaks_the_chain() {
        let mut ledger = Ledger::new();
        ledger
            .append(LedgerEvent::CandidateAccepted {
                node_id: "a".into(),
                generation: 0,
                energy: 5.0,
            })
            .unwrap();
        ledger
            .append(LedgerEvent::CandidateAccepted {
                node_id: "b".into(),
                generation: 0,
                energy: 0.0,
            })
            .unwrap();
        // Tamper with a recorded energy.
        if let LedgerEvent::CandidateAccepted { energy, .. } = &mut ledger.records[0].event {
            *energy = 999.0;
        }
        assert!(ledger.verify_chain().is_err());
    }

    #[test]
    fn replay_reconstructs_accepted_trajectory() {
        let mut ledger = Ledger::new();
        ledger
            .append(LedgerEvent::CandidateRejected {
                node_id: "a".into(),
                generation: 0,
            })
            .unwrap();
        ledger
            .append(LedgerEvent::CandidateAccepted {
                node_id: "a".into(),
                generation: 1,
                energy: 8.0,
            })
            .unwrap();
        ledger
            .append(LedgerEvent::CandidateAccepted {
                node_id: "b".into(),
                generation: 0,
                energy: 0.0,
            })
            .unwrap();
        let traj = replay_accepted_trajectory(&ledger);
        assert_eq!(traj, vec![("a".into(), 1, 8.0), ("b".into(), 0, 0.0)]);
    }

    #[test]
    fn kernel_refuses_unrecorded_observation() {
        let mut ledger = Ledger::new();
        let event = LedgerEvent::EffectApplied {
            proposal_id: "p1".into(),
            idempotency_key: "k1".into(),
        };
        // Referencing an observation that was never recorded is refused.
        let err = ledger.commit_transition(event.clone(), &["never-recorded".into()]);
        assert!(err.is_err());

        // After recording the observation, the same commit succeeds.
        let handle = ledger.record_observation(b"llm output bytes").unwrap();
        assert!(ledger.has_observation(&handle));
        assert!(ledger.commit_transition(event, &[handle]).is_ok());
    }

    #[test]
    fn idempotency_redelivery_returns_prior_outcome() {
        let mut log = IdempotencyLog::new();
        let first = log.record("k1", b"patch-content", "applied").unwrap();
        assert_eq!(first, "applied");
        // Redelivery with same content returns the prior result.
        let again = log.record("k1", b"patch-content", "applied-again").unwrap();
        assert_eq!(again, "applied");
    }

    #[test]
    fn idempotency_key_reuse_for_different_content_is_invalid() {
        let mut log = IdempotencyLog::new();
        log.record("k1", b"content-a", "applied").unwrap();
        assert!(log.record("k1", b"content-b", "applied").is_err());
    }

    #[test]
    fn external_effect_must_be_bracketed() {
        let mut log = ExternalEffectLog::new();
        // Result without intent is an R5 violation.
        assert!(log.result("k1").is_err());
        log.intent("k1");
        assert!(log.result("k1").is_ok());
        assert!(log.is_bracketed("k1"));
    }
}
