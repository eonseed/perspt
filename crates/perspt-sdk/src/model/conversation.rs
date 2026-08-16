//! The harness-owned message log (PSP-9 system 4).
//!
//! `Conversation` is Perspt's own log rather than a vendor type: it is
//! re-rendered per provider on each turn, which is what makes a mid-run
//! provider swap possible at all. Closed tool-call/result pairs may later be
//! replaced by content-addressed references (system 14); the full arguments
//! and results always remain in the ledger.

use serde::{Deserialize, Serialize};

use super::tool::ProviderToolCall;

/// One entry in the harness conversation log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "role")]
pub enum Message {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        content: String,
    },
    /// An assistant turn that requested tool calls.
    AssistantToolCalls {
        calls: Vec<ProviderToolCall>,
    },
    /// The recorded result of one tool call, keyed by its call id.
    ToolResponse {
        call_id: String,
        content: String,
    },
}

/// The message log the harness owns: system, user, assistant,
/// assistant-tool-calls, tool-response. Never a vendor type.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Conversation {
    messages: Vec<Message>,
}

impl Conversation {
    /// Start a conversation with a system prompt.
    pub fn with_system(prompt: impl Into<String>) -> Self {
        Self {
            messages: vec![Message::System {
                content: prompt.into(),
            }],
        }
    }

    /// Append a message.
    pub fn push(&mut self, message: Message) {
        self.messages.push(message);
    }

    /// Append a user message (goals, directed corrections).
    pub fn push_user(&mut self, content: impl Into<String>) {
        self.push(Message::User {
            content: content.into(),
        });
    }

    /// Append an assistant tool-call turn.
    pub fn push_tool_calls(&mut self, calls: Vec<ProviderToolCall>) {
        self.push(Message::AssistantToolCalls { calls });
    }

    /// Append one tool response, correlated by call id.
    pub fn push_tool_response(&mut self, call_id: impl Into<String>, content: impl Into<String>) {
        self.push(Message::ToolResponse {
            call_id: call_id.into(),
            content: content.into(),
        });
    }

    /// The log, in order.
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Approximate serialized context size without relying on a provider
    /// tokenizer. Runtime thresholds apply a conservative chars-per-token
    /// conversion before calling this method.
    pub fn estimated_chars(&self) -> usize {
        self.messages
            .iter()
            .map(|message| serde_json::to_string(message).map_or(0, |value| value.len()))
            .sum()
    }

    /// Replace closed history with a deterministic control projection while
    /// preserving every unresolved provider call exactly. Full closed history
    /// remains in the event ledger; this only changes the next model request.
    pub fn compact_with_control(&mut self, control: impl Into<String>) {
        let unresolved = self.unresolved_call_ids();
        let mut projected = Vec::new();
        if let Some(system) = self
            .messages
            .iter()
            .find(|message| matches!(message, Message::System { .. }))
        {
            projected.push(system.clone());
        }
        projected.push(Message::System {
            content: control.into(),
        });
        if let Some(user) = self
            .messages
            .iter()
            .rev()
            .find(|message| matches!(message, Message::User { .. }))
        {
            projected.push(user.clone());
        }
        for message in &self.messages {
            if let Message::AssistantToolCalls { calls } = message {
                let calls: Vec<_> = calls
                    .iter()
                    .filter(|call| unresolved.contains(&call.call_id))
                    .cloned()
                    .collect();
                if !calls.is_empty() {
                    projected.push(Message::AssistantToolCalls { calls });
                }
            }
        }
        self.messages = projected;
    }

    /// Tool calls that have no recorded response yet. A checkpoint must
    /// preserve these exactly (system 14's `ControlFrame`).
    pub fn unresolved_call_ids(&self) -> Vec<String> {
        let mut open: Vec<String> = Vec::new();
        for message in &self.messages {
            match message {
                Message::AssistantToolCalls { calls } => {
                    open.extend(calls.iter().map(|c| c.call_id.clone()));
                }
                Message::ToolResponse { call_id, .. } => {
                    open.retain(|id| id != call_id);
                }
                _ => {}
            }
        }
        open
    }

    /// Number of messages in the log.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(id: &str) -> ProviderToolCall {
        ProviderToolCall {
            call_id: id.into(),
            name: "read_file".into(),
            arguments: serde_json::json!({}),
        }
    }

    #[test]
    fn tracks_unresolved_tool_calls() {
        let mut conversation = Conversation::with_system("harness");
        conversation.push_tool_calls(vec![call("a"), call("b")]);
        conversation.push_tool_response("a", "contents");
        assert_eq!(conversation.unresolved_call_ids(), ["b"]);
        conversation.push_tool_response("b", "denied");
        assert!(conversation.unresolved_call_ids().is_empty());
    }

    #[test]
    fn conversation_is_vendor_neutral_serializable() {
        let mut conversation = Conversation::with_system("s");
        conversation.push_user("fix the test");
        let json = serde_json::to_string(&conversation).unwrap();
        let back: Conversation = serde_json::from_str(&json).unwrap();
        assert_eq!(back, conversation);
    }

    #[test]
    fn compaction_drops_closed_history_but_preserves_open_calls() {
        let mut conversation = Conversation::with_system("harness");
        conversation.push_user("large goal");
        conversation.push_tool_calls(vec![call("closed"), call("open")]);
        conversation.push_tool_response("closed", "contents");
        conversation.compact_with_control("CONTROL");

        assert_eq!(conversation.unresolved_call_ids(), ["open"]);
        assert_eq!(conversation.len(), 4);
        assert!(matches!(
            &conversation.messages()[3],
            Message::AssistantToolCalls { calls } if calls == &vec![call("open")]
        ));
    }
}
