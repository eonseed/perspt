//! Wire-neutral tool declaration and call types (PSP-9 system 4).
//!
//! `ToolCatalog::specs_for` produces these and the turn loop consumes them;
//! no vendor's wire format reaches the harness. The `genai` driver mirrors
//! them in `perspt-core`, and `perspt-agent::transport` performs the one
//! translation.

use serde::{Deserialize, Serialize};

/// A provider-agnostic tool declaration sent with a chat request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema for the tool's arguments.
    pub schema: serde_json::Value,
    /// Ask the provider to enforce the schema. Routes without
    /// `strict_schema` capability degrade to local validation with a
    /// `ToolArgumentInvalid` residual on violation — never silently.
    pub strict: bool,
}

/// One tool call requested by the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderToolCall {
    /// Provider-issued id correlating the eventual tool response.
    pub call_id: String,
    pub name: String,
    /// Arguments as returned by the model, before schema validation.
    pub arguments: serde_json::Value,
}

/// The result of one assistant turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TurnOutput {
    /// The model requested tool calls; each becomes an `EffectProposal`.
    ToolCalls(Vec<ProviderToolCall>),
    /// The model produced text and requested nothing.
    Text(String),
}

impl TurnOutput {
    /// The requested calls, empty for a text turn.
    pub fn tool_calls(&self) -> &[ProviderToolCall] {
        match self {
            TurnOutput::ToolCalls(calls) => calls,
            TurnOutput::Text(_) => &[],
        }
    }
}

/// How the harness constrains tool selection for a turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoicePolicy {
    /// The model decides whether to call tools.
    Auto,
    /// Tool calls are forbidden this turn (e.g. a summary turn).
    None,
    /// The model must call at least one tool.
    Required,
    /// The model must call this specific tool.
    Specific(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_turn_has_no_tool_calls() {
        assert!(TurnOutput::Text("done".into()).tool_calls().is_empty());
    }

    #[test]
    fn tool_calls_round_trip_through_serde() {
        let turn = TurnOutput::ToolCalls(vec![ProviderToolCall {
            call_id: "c1".into(),
            name: "read_file".into(),
            arguments: serde_json::json!({"path": "src/lib.rs"}),
        }]);
        let json = serde_json::to_string(&turn).unwrap();
        let back: TurnOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(back, turn);
        assert_eq!(back.tool_calls().len(), 1);
    }
}
