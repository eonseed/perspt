//! Ledger-folded provider-neutral model context.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{Conversation, Message};
use crate::error::{Result, SdkError};
use crate::ledger::content_hash;

pub const CONVERSATION_EVENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationSeeded {
    pub event_schema_version: u32,
    pub conversation: Conversation,
    pub digest: String,
}

impl ConversationSeeded {
    pub fn new(conversation: Conversation) -> Result<Self> {
        let payload = serde_json::to_vec(&conversation)
            .map_err(|error| SdkError::Domain(error.to_string()))?;
        let digest = content_hash(
            [b"conversation-seeded:v1:".as_slice(), payload.as_slice()]
                .concat()
                .as_slice(),
        );
        Ok(Self {
            event_schema_version: CONVERSATION_EVENT_SCHEMA_VERSION,
            conversation,
            digest,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "delta", rename_all = "snake_case")]
pub enum ConversationDelta {
    Message { message: Message },
    ToolActivated { name: String },
    Compacted { control: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationDeltaRecord {
    pub event_schema_version: u32,
    pub prior_digest: String,
    pub delta: ConversationDelta,
    pub digest: String,
}

/// The sole fold owner for model context, activated tools, unresolved calls,
/// and the canonical conversation digest.
#[derive(Debug, Clone, PartialEq)]
pub struct ConversationProjection {
    conversation: Conversation,
    activated_tools: BTreeSet<String>,
    digest: String,
}

impl ConversationProjection {
    pub fn from_seed(seed: &ConversationSeeded) -> Result<Self> {
        anyhow_seed(seed)?;
        Ok(Self {
            conversation: seed.conversation.clone(),
            activated_tools: BTreeSet::new(),
            digest: seed.digest.clone(),
        })
    }

    /// Prepare a delta for durable append. The caller persists this record
    /// before passing it to `apply`.
    pub fn prepare(&self, delta: ConversationDelta) -> Result<ConversationDeltaRecord> {
        let digest = delta_digest(&self.digest, &delta)?;
        Ok(ConversationDeltaRecord {
            event_schema_version: CONVERSATION_EVENT_SCHEMA_VERSION,
            prior_digest: self.digest.clone(),
            delta,
            digest,
        })
    }

    /// Validate a persisted delta completely, then apply it.
    pub fn apply(&mut self, record: &ConversationDeltaRecord) -> Result<()> {
        if record.event_schema_version != CONVERSATION_EVENT_SCHEMA_VERSION {
            return Err(SdkError::Domain(
                "unsupported conversation event schema".into(),
            ));
        }
        if record.prior_digest != self.digest {
            return Err(SdkError::Domain(
                "conversation delta has a stale parent".into(),
            ));
        }
        if delta_digest(&record.prior_digest, &record.delta)? != record.digest {
            return Err(SdkError::Domain(
                "conversation delta digest mismatch".into(),
            ));
        }
        match &record.delta {
            ConversationDelta::Message { message } => self.conversation.push(message.clone()),
            ConversationDelta::ToolActivated { name } => {
                self.activated_tools.insert(name.clone());
            }
            ConversationDelta::Compacted { control } => {
                self.conversation.compact_with_control(control.clone());
            }
        }
        self.digest = record.digest.clone();
        Ok(())
    }

    pub fn refold(seed: &ConversationSeeded, records: &[ConversationDeltaRecord]) -> Result<Self> {
        let mut projection = Self::from_seed(seed)?;
        for record in records {
            projection.apply(record)?;
        }
        Ok(projection)
    }

    pub fn conversation(&self) -> &Conversation {
        &self.conversation
    }

    pub fn activated_tools(&self) -> &BTreeSet<String> {
        &self.activated_tools
    }

    pub fn unresolved_call_ids(&self) -> Vec<String> {
        self.conversation.unresolved_call_ids()
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }
}

fn anyhow_seed(seed: &ConversationSeeded) -> Result<()> {
    if seed.event_schema_version != CONVERSATION_EVENT_SCHEMA_VERSION {
        return Err(SdkError::Domain(
            "unsupported conversation seed schema".into(),
        ));
    }
    let expected = ConversationSeeded::new(seed.conversation.clone())?;
    if expected.digest != seed.digest {
        return Err(SdkError::Domain("conversation seed digest mismatch".into()));
    }
    Ok(())
}

fn delta_digest(parent: &str, delta: &ConversationDelta) -> Result<String> {
    let payload = serde_json::to_vec(delta).map_err(|error| SdkError::Domain(error.to_string()))?;
    Ok(content_hash(
        [parent.as_bytes(), b":", payload.as_slice()]
            .concat()
            .as_slice(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refold_matches_incremental_application() {
        let seed = ConversationSeeded::new(Conversation::with_system("governed")).unwrap();
        let mut live = ConversationProjection::from_seed(&seed).unwrap();
        let first = live
            .prepare(ConversationDelta::Message {
                message: Message::User {
                    content: "hello".into(),
                },
            })
            .unwrap();
        live.apply(&first).unwrap();
        let second = live
            .prepare(ConversationDelta::ToolActivated {
                name: "lookup".into(),
            })
            .unwrap();
        live.apply(&second).unwrap();
        let folded = ConversationProjection::refold(&seed, &[first, second]).unwrap();
        assert_eq!(live, folded);
    }

    #[test]
    fn tampered_or_reordered_deltas_fail_closed() {
        let seed = ConversationSeeded::new(Conversation::with_system("s")).unwrap();
        let projection = ConversationProjection::from_seed(&seed).unwrap();
        let mut record = projection
            .prepare(ConversationDelta::ToolActivated { name: "a".into() })
            .unwrap();
        record.delta = ConversationDelta::ToolActivated { name: "b".into() };
        assert!(ConversationProjection::refold(&seed, &[record]).is_err());
    }
}
