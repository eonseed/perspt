//! Provider capability record and honest degradation (PSP-9 system 2).
//!
//! Providers differ in what they support, and a platform that hides those
//! differences will silently violate its own contracts. The record type lives
//! here so routing can filter on it; probing and the degradation *ladder* are
//! runtime concerns and live with the transport in `perspt-core`.
//!
//! **Silent emulation is prohibited (Gate U).** A provider without tool
//! calling is degraded to bundle mode with the reason recorded — never given
//! a text protocol that imitates tool calls, because emulated calls are not
//! recorded structured observations and would break the recording obligation
//! (R2) that Theorem 7 depends on.

use serde::{Deserialize, Serialize};

/// Declared and probed capabilities of one provider route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub tool_calling: bool,
    pub strict_schema: bool,
    pub parallel_tool_calls: bool,
    pub streaming_tool_calls: bool,
    pub prompt_caching: bool,
    pub structured_output: bool,
    pub max_context_tokens: u32,
}

impl ProviderCapabilities {
    /// The most conservative record: nothing beyond plain text chat.
    pub fn text_only(max_context_tokens: u32) -> Self {
        Self {
            tool_calling: false,
            strict_schema: false,
            parallel_tool_calls: false,
            streaming_tool_calls: false,
            prompt_caching: false,
            structured_output: false,
            max_context_tokens,
        }
    }

    /// Whether this record satisfies every capability `required` asks for.
    pub fn satisfies(&self, required: &ProviderCapabilityMask) -> bool {
        (!required.tool_calling || self.tool_calling)
            && (!required.strict_schema || self.strict_schema)
            && (!required.parallel_tool_calls || self.parallel_tool_calls)
            && (!required.streaming_tool_calls || self.streaming_tool_calls)
            && (!required.prompt_caching || self.prompt_caching)
            && (!required.structured_output || self.structured_output)
            && self.max_context_tokens >= required.min_context_tokens
    }
}

/// A requirement mask for route resolution (`RouteObjective::require`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProviderCapabilityMask {
    pub tool_calling: bool,
    pub strict_schema: bool,
    pub parallel_tool_calls: bool,
    pub streaming_tool_calls: bool,
    pub prompt_caching: bool,
    pub structured_output: bool,
    pub min_context_tokens: u32,
}

impl ProviderCapabilityMask {
    /// The requirement the tool loop itself imposes on a route.
    pub fn tool_loop() -> Self {
        Self {
            tool_calling: true,
            ..Self::default()
        }
    }
}

/// One recorded degradation: a capability the route lacks and the explicit,
/// invariant-preserving fallback taken instead. Every degradation is a ledger
/// event and is visible in `perspt providers`; none may silently emulate the
/// missing capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "missing")]
pub enum CapabilityDegradation {
    /// Non-strict schemas sent; arguments validated locally, violations become
    /// `ToolArgumentInvalid` residuals with a directed correction.
    StrictSchema,
    /// One tool call per turn. Costs turns, changes no invariant.
    ParallelToolCalls,
    /// Non-streaming turn; progress is turn-granular rather than
    /// token-granular.
    StreamingToolCalls,
    /// Route accounting marks the route cache-cold; routing weights it
    /// accordingly.
    PromptCaching,
    /// Node falls back to `Bundle` execution mode, with the reason recorded.
    ToolCalling,
    /// The route's context window is smaller than the node's packed context;
    /// the route is ineligible for that node.
    ContextWindow { needed: u32, available: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full() -> ProviderCapabilities {
        ProviderCapabilities {
            tool_calling: true,
            strict_schema: true,
            parallel_tool_calls: true,
            streaming_tool_calls: true,
            prompt_caching: true,
            structured_output: true,
            max_context_tokens: 200_000,
        }
    }

    #[test]
    fn a_full_record_satisfies_the_tool_loop_mask() {
        assert!(full().satisfies(&ProviderCapabilityMask::tool_loop()));
    }

    #[test]
    fn text_only_fails_the_tool_loop_mask() {
        assert!(
            !ProviderCapabilities::text_only(8192).satisfies(&ProviderCapabilityMask::tool_loop())
        );
    }

    #[test]
    fn context_window_is_a_hard_requirement() {
        let mask = ProviderCapabilityMask {
            min_context_tokens: 1_000_000,
            ..ProviderCapabilityMask::default()
        };
        assert!(!full().satisfies(&mask));
    }
}
