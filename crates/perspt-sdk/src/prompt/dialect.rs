//! Model dialects (PSP-10 system 24).
//!
//! Structural differences between model families never fork prose: system
//! slot layout, tool-call convention, and reasoning-trace handling live in
//! one typed value per resolved route. The dialect id and version enter the
//! program digest.

use serde::{Deserialize, Serialize};

use super::accountant::TokenAccountantRef;

/// How compiled system sections are laid out at the transport boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemSlotPolicy {
    /// The transport accepts multiple system messages.
    Many,
    /// One system slot; sections concatenate with the boundary marker.
    SingleConcatenated,
}

/// How the model emits tool calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallConvention {
    /// Provider-native structured tool calls.
    Native,
    /// JSON tool-call envelopes inside plain text.
    JsonInText,
}

/// How reasoning traces are requested or removed before parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningTracePolicy {
    None,
    Requested,
    Stripped,
}

/// A dialect reference as recorded in compiled programs and the ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialectRef {
    pub id: String,
    pub version: u32,
}

/// The typed structural profile of one resolved model route.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelDialect {
    pub id: String,
    pub version: u32,
    pub system_slots: SystemSlotPolicy,
    pub tool_calls: ToolCallConvention,
    pub reasoning: ReasoningTracePolicy,
    /// Delimiter between concatenated system sections.
    pub boundary_marker: String,
    pub context_window_tokens: u64,
    pub max_output_tokens: u64,
    pub token_accountant: TokenAccountantRef,
    /// Independent transport byte bound; not derived from tokens.
    pub max_prompt_bytes: u64,
}

impl ModelDialect {
    pub fn dialect_ref(&self) -> DialectRef {
        DialectRef {
            id: self.id.clone(),
            version: self.version,
        }
    }

    /// A family with native tool calls and multiple system slots.
    pub fn native_tools_v1() -> Self {
        Self {
            id: "native_tools_v1".into(),
            version: 1,
            system_slots: SystemSlotPolicy::Many,
            tool_calls: ToolCallConvention::Native,
            reasoning: ReasoningTracePolicy::Stripped,
            boundary_marker: String::new(),
            context_window_tokens: 131_072,
            max_output_tokens: 16_384,
            token_accountant: TokenAccountantRef::approx_bytes_v1(),
            max_prompt_bytes: 262_144,
        }
    }

    /// The live genai adapter: native tool calls behind one system slot.
    /// The compiled system sections join with the stage's declared
    /// separator (see `StageComposition::system_separator`), which keeps
    /// the migrated platform literals byte-identical on the wire.
    pub fn genai_single_slot_v1() -> Self {
        Self {
            id: "genai_single_slot_v1".into(),
            version: 1,
            system_slots: SystemSlotPolicy::SingleConcatenated,
            tool_calls: ToolCallConvention::Native,
            reasoning: ReasoningTracePolicy::Stripped,
            boundary_marker: String::new(),
            context_window_tokens: 131_072,
            max_output_tokens: 16_384,
            token_accountant: TokenAccountantRef::approx_bytes_v1(),
            max_prompt_bytes: 262_144,
        }
    }

    /// The same dialect fitted to one route's declared context window. The
    /// effective token budget is identity-bearing and enters the program
    /// digest, so narrowing it is recorded, never silent.
    pub fn with_context_window(mut self, tokens: u64) -> Self {
        self.context_window_tokens = tokens.max(1);
        self
    }

    /// A family reached through a single-system-slot endpoint that emits
    /// tool calls as JSON in text.
    pub fn json_single_slot_v1() -> Self {
        Self {
            id: "json_single_slot_v1".into(),
            version: 1,
            system_slots: SystemSlotPolicy::SingleConcatenated,
            tool_calls: ToolCallConvention::JsonInText,
            reasoning: ReasoningTracePolicy::None,
            boundary_marker: "\n===== perspt:boundary =====\n".into(),
            context_window_tokens: 65_536,
            max_output_tokens: 8_192,
            token_accountant: TokenAccountantRef::approx_bytes_v1(),
            max_prompt_bytes: 131_072,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_contrasting_dialects_differ_structurally() {
        let native = ModelDialect::native_tools_v1();
        let json = ModelDialect::json_single_slot_v1();
        assert_ne!(native.system_slots, json.system_slots);
        assert_ne!(native.tool_calls, json.tool_calls);
        assert!(json.boundary_marker.contains("perspt:boundary"));
        assert_eq!(native.dialect_ref().id, "native_tools_v1");
    }
}
