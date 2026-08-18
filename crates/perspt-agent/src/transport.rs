//! The transport adapter (PSP-9 system 4).
//!
//! This is the only place where the SDK's provider-neutral contract and
//! `perspt-core`'s `genai` driver are both in scope. `GenAiTransport` wraps a
//! [`perspt_core::ModelPortfolio`] and implements
//! [`perspt_sdk::ModelTransport`], translating the SDK types to and from the
//! core mirrors. Everything above the transport — the turn loop, routing,
//! adjudication, calibration — sees only SDK types (Gate S).

use std::sync::Arc;
use std::time::Duration;

use perspt_core::{CoreMessage, CoreToolCall, CoreToolChoice, CoreToolSpec, CoreTurnOutput};
use perspt_sdk::{
    Conversation, Message, ModelFamily, ModelId, ModelTransport, ProviderCapabilities,
    ProviderToolCall, SdkError, ToolChoicePolicy, ToolSpec, TransportFuture, TurnOutput,
};

/// Retries per turn for transient provider failures. A 429 or 5xx is a
/// throughput delay, not a route failure (Gate P: rate-limit waits stay
/// inside the transport); only after these bounded retries does an error
/// reach the recovery cascade, where it may consume a sticky failover.
const TRANSIENT_RETRIES: u32 = 3;
const BACKOFF_BASE: Duration = Duration::from_secs(2);

/// The backoff before retry `attempt` (0-based), or `None` when the cause
/// is not transient or the retry budget is exhausted. Backoff doubles per
/// attempt: 2s, 4s, 8s.
fn transient_retry_delay(cause: &str, attempt: u32) -> Option<Duration> {
    if attempt >= TRANSIENT_RETRIES {
        return None;
    }
    let lowered = cause.to_ascii_lowercase();
    const TRANSIENT_MARKERS: &[&str] = &[
        "429",
        "too many requests",
        "rate limit",
        "resource exhausted",
        "resource_exhausted",
        "status code '500",
        "internal server error",
        "502",
        "bad gateway",
        "503",
        "service unavailable",
        "504",
        "gateway timeout",
        "timed out",
        "timeout",
        "connection reset",
        "connection closed before",
        "overloaded",
        "temporarily unavailable",
    ];
    if !TRANSIENT_MARKERS
        .iter()
        .any(|marker| lowered.contains(marker))
    {
        return None;
    }
    Some(BACKOFF_BASE * 2u32.pow(attempt))
}

/// `perspt_sdk::ModelTransport` implemented over the core portfolio.
pub struct GenAiTransport {
    portfolio: Arc<perspt_core::ModelPortfolio>,
}

impl GenAiTransport {
    /// Wrap a live portfolio.
    pub fn new(portfolio: Arc<perspt_core::ModelPortfolio>) -> Self {
        Self { portfolio }
    }

    /// The wrapped portfolio, for surfaces such as `perspt providers`.
    pub fn portfolio(&self) -> &perspt_core::ModelPortfolio {
        &self.portfolio
    }
}

/// Render the SDK conversation into core mirror messages.
fn render_conversation(conversation: &Conversation) -> Vec<CoreMessage> {
    conversation
        .messages()
        .iter()
        .map(|message| match message {
            Message::System { content } => CoreMessage::System {
                content: content.clone(),
            },
            Message::User { content } => CoreMessage::User {
                content: content.clone(),
            },
            Message::Assistant { content } => CoreMessage::Assistant {
                content: content.clone(),
            },
            Message::AssistantToolCalls { calls } => CoreMessage::AssistantToolCalls {
                calls: calls.iter().map(render_call).collect(),
            },
            Message::ToolResponse { call_id, content } => CoreMessage::ToolResponse {
                call_id: call_id.clone(),
                content: content.clone(),
            },
        })
        .collect()
}

fn render_call(call: &ProviderToolCall) -> CoreToolCall {
    CoreToolCall {
        call_id: call.call_id.clone(),
        name: call.name.clone(),
        arguments: call.arguments.clone(),
    }
}

fn render_specs(tools: &[ToolSpec]) -> Vec<CoreToolSpec> {
    tools
        .iter()
        .map(|spec| CoreToolSpec {
            name: spec.name.clone(),
            description: spec.description.clone(),
            schema: spec.schema.clone(),
            strict: spec.strict,
        })
        .collect()
}

fn render_choice(choice: ToolChoicePolicy) -> CoreToolChoice {
    match choice {
        ToolChoicePolicy::Auto => CoreToolChoice::Auto,
        ToolChoicePolicy::None => CoreToolChoice::None,
        ToolChoicePolicy::Required => CoreToolChoice::Required,
        ToolChoicePolicy::Specific(name) => CoreToolChoice::Specific(name),
    }
}

fn lift_output(output: CoreTurnOutput) -> TurnOutput {
    match output {
        CoreTurnOutput::Text(text) => TurnOutput::Text(text),
        CoreTurnOutput::ToolCalls(calls) => TurnOutput::ToolCalls(
            calls
                .into_iter()
                .map(|call| ProviderToolCall {
                    call_id: call.call_id,
                    name: call.name,
                    arguments: call.arguments,
                })
                .collect(),
        ),
    }
}

/// Lift the core capability mirror into the SDK record.
fn lift_caps(caps: &perspt_core::ProviderCaps) -> ProviderCapabilities {
    ProviderCapabilities {
        tool_calling: caps.tool_calling,
        strict_schema: caps.strict_schema,
        parallel_tool_calls: caps.parallel_tool_calls,
        streaming_tool_calls: caps.streaming_tool_calls,
        prompt_caching: caps.prompt_caching,
        structured_output: caps.structured_output,
        max_context_tokens: caps.max_context_tokens,
    }
}

impl ModelTransport for GenAiTransport {
    fn chat_turn<'a>(
        &'a self,
        model: &'a ModelId,
        conversation: &'a Conversation,
        tools: &'a [ToolSpec],
        choice: ToolChoicePolicy,
    ) -> TransportFuture<'a, TurnOutput> {
        let core_choice = render_choice(choice);
        Box::pin(async move {
            let handle = self
                .portfolio
                .resolve(&model.provider)
                .map_err(|e| SdkError::Domain(format!("route resolution failed: {e}")))?;
            let messages = render_conversation(conversation);
            let specs = render_specs(tools);
            let mut attempt = 0u32;
            loop {
                match handle
                    .provider
                    .chat_turn_with_tools(&model.model, &messages, &specs, core_choice.clone())
                    .await
                {
                    Ok(output) => return Ok(lift_output(output)),
                    Err(error) => {
                        let cause = format!("{error:#}");
                        let Some(delay) = transient_retry_delay(&cause, attempt) else {
                            return Err(SdkError::Domain(format!("chat turn failed: {cause}")));
                        };
                        log::warn!(
                            "transient provider error from {model} (retry {} of {TRANSIENT_RETRIES} \
                             in {delay:?}): {cause}",
                            attempt + 1
                        );
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                    }
                }
            }
        })
    }

    fn capabilities(&self, model: &ModelId) -> ProviderCapabilities {
        match self.portfolio.resolve(&model.provider) {
            Ok(handle) => lift_caps(&handle.caps),
            // An unknown route declares nothing; the caller's route
            // resolution surfaces the real error.
            Err(_) => ProviderCapabilities::text_only(0),
        }
    }

    fn family_of(&self, model: &ModelId) -> ModelFamily {
        ModelFamily::from_model_name(&model.model)
    }

    /// The genai driver's adapter identity (PSP-10 system 24). One identity
    /// for the whole driver: endpoint/provider ids stay credentials, and a
    /// gateway-served model keeps its own family via `family_of`.
    fn adapter_kind(&self) -> &'static str {
        "genai"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transient_causes_get_bounded_doubling_backoff() {
        let cause = "Request failed with status code '429 Too Many Requests'";
        assert_eq!(
            transient_retry_delay(cause, 0),
            Some(Duration::from_secs(2))
        );
        assert_eq!(
            transient_retry_delay(cause, 1),
            Some(Duration::from_secs(4))
        );
        assert_eq!(
            transient_retry_delay(cause, 2),
            Some(Duration::from_secs(8))
        );
        assert_eq!(transient_retry_delay(cause, 3), None, "retries are bounded");
        assert!(transient_retry_delay("504 Gateway Timeout", 0).is_some());
        assert!(transient_retry_delay("connection reset by peer", 0).is_some());
        assert!(transient_retry_delay("RESOURCE_EXHAUSTED", 0).is_some());
    }

    #[test]
    fn non_transient_causes_surface_immediately() {
        assert_eq!(transient_retry_delay("401 Unauthorized", 0), None);
        assert_eq!(transient_retry_delay("invalid request schema", 0), None);
        assert_eq!(transient_retry_delay("model not found", 0), None);
    }

    #[test]
    fn conversation_renders_role_for_role() {
        let mut conversation = Conversation::with_system("harness");
        conversation.push_user("go");
        conversation.push_tool_calls(vec![ProviderToolCall {
            call_id: "c1".into(),
            name: "read_file".into(),
            arguments: serde_json::json!({"path": "a"}),
        }]);
        conversation.push_tool_response("c1", "contents");
        let rendered = render_conversation(&conversation);
        assert_eq!(rendered.len(), 4);
        assert!(
            matches!(&rendered[2], CoreMessage::AssistantToolCalls { calls } if calls.len() == 1)
        );
        assert!(
            matches!(&rendered[3], CoreMessage::ToolResponse { call_id, .. } if call_id == "c1")
        );
    }

    #[test]
    fn tool_call_output_lifts_back_to_sdk_types() {
        let output = lift_output(CoreTurnOutput::ToolCalls(vec![CoreToolCall {
            call_id: "c9".into(),
            name: "grep".into(),
            arguments: serde_json::json!({"query": "fn main"}),
        }]));
        assert_eq!(output.tool_calls().len(), 1);
        assert_eq!(output.tool_calls()[0].name, "grep");
    }

    #[test]
    fn family_is_derived_from_the_model_name_not_the_provider_key() {
        let config =
            perspt_core::Config::from_toml_str("[providers.local]\nadapter = \"ollama\"\n")
                .unwrap();
        let portfolio = Arc::new(perspt_core::ModelPortfolio::from_config(&config).unwrap());
        let transport = GenAiTransport::new(portfolio);
        let model = ModelId::new("local", "llama3.3:70b");
        assert_eq!(transport.family_of(&model), ModelFamily::Llama);
    }
}
