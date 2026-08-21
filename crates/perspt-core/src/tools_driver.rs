//! The `genai` tool-calling driver (PSP-9 system 4).
//!
//! This is the *driver* half of the transport: it renders the harness's
//! provider-neutral turn into a `genai` `ChatRequest` with tools, executes
//! it, and maps the response back. The types here are the **core-native
//! mirror** of the SDK contract — `perspt-core` must not depend on
//! `perspt-sdk`, so the shapes exist twice by design and
//! `perspt-agent::transport` performs the one translation.

use anyhow::{Context, Result};
use futures::StreamExt;
use genai::chat::{
    ChatMessage, ChatOptions, ChatRequest, ChatStreamEvent, Tool, ToolCall, ToolChoice,
    ToolResponse,
};

use crate::llm_provider::GenAIProvider;

/// Mirror of the SDK's `ToolSpec`.
#[derive(Debug, Clone, PartialEq)]
pub struct CoreToolSpec {
    pub name: String,
    pub description: String,
    pub schema: serde_json::Value,
    pub strict: bool,
}

/// Mirror of the SDK's `ProviderToolCall`.
#[derive(Debug, Clone, PartialEq)]
pub struct CoreToolCall {
    pub call_id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Mirror of the SDK's `TurnOutput`.
#[derive(Debug, Clone, PartialEq)]
pub enum CoreTurnOutput {
    ToolCalls(Vec<CoreToolCall>),
    Text(String),
}

/// User-facing stream events from a tool-aware model turn.
///
/// Tool protocol chunks and thought signatures stay inside the transport;
/// callers receive only answer text and genuine model reasoning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreTurnStreamEvent {
    Text(String),
    Reasoning(String),
}

/// Mirror of the SDK's `Message`.
#[derive(Debug, Clone, PartialEq)]
pub enum CoreMessage {
    System { content: String },
    User { content: String },
    Assistant { content: String },
    AssistantToolCalls { calls: Vec<CoreToolCall> },
    ToolResponse { call_id: String, content: String },
}

/// Mirror of the SDK's `ToolChoicePolicy`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreToolChoice {
    Auto,
    None,
    Required,
    Specific(String),
}

/// Optional provider generation controls for one core turn.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CoreGenerationConfig {
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub stop_sequences: Vec<String>,
}

/// Render one provider-neutral message into the vendor shape.
fn render_message(message: &CoreMessage) -> ChatMessage {
    match message {
        CoreMessage::System { content } => ChatMessage::system(content.clone()),
        CoreMessage::User { content } => ChatMessage::user(content.clone()),
        CoreMessage::Assistant { content } => ChatMessage::assistant(content.clone()),
        CoreMessage::AssistantToolCalls { calls } => ChatMessage::from(
            calls
                .iter()
                .map(|call| ToolCall {
                    call_id: call.call_id.clone(),
                    fn_name: call.name.clone(),
                    fn_arguments: call.arguments.clone(),
                    thought_signatures: None,
                })
                .collect::<Vec<_>>(),
        ),
        CoreMessage::ToolResponse { call_id, content } => {
            ChatMessage::from(ToolResponse::new(call_id.clone(), content.clone()))
        }
    }
}

/// Render a tool spec into the vendor shape.
fn render_tool(spec: &CoreToolSpec) -> Tool {
    let tool = Tool::new(spec.name.clone())
        .with_description(spec.description.clone())
        .with_schema(spec.schema.clone());
    if spec.strict {
        tool.with_strict(true)
    } else {
        tool
    }
}

fn render_choice(choice: &CoreToolChoice) -> ToolChoice {
    match choice {
        CoreToolChoice::Auto => ToolChoice::Auto,
        CoreToolChoice::None => ToolChoice::None,
        CoreToolChoice::Required => ToolChoice::Required,
        CoreToolChoice::Specific(name) => ToolChoice::Tool { name: name.clone() },
    }
}

fn render_request(messages: &[CoreMessage], tools: &[CoreToolSpec]) -> ChatRequest {
    let rendered = messages.iter().map(render_message).collect();
    let mut request = ChatRequest::new(rendered);
    if !tools.is_empty() {
        request = request.with_tools(tools.iter().map(render_tool).collect::<Vec<_>>());
    }
    request
}

fn render_options(choice: &CoreToolChoice, config: &CoreGenerationConfig) -> ChatOptions {
    let mut options = ChatOptions::default().with_tool_choice(render_choice(choice));
    if let Some(max_tokens) = config.max_tokens {
        options = options.with_max_tokens(max_tokens);
    }
    if let Some(temperature) = config.temperature {
        options = options.with_temperature(f64::from(temperature));
    }
    if !config.stop_sequences.is_empty() {
        options = options.with_stop_sequences(config.stop_sequences.clone());
    }
    options
}

impl GenAIProvider {
    /// Execute one tool-calling chat turn against `model`.
    ///
    /// Returns the model's tool calls when any were issued, otherwise its
    /// text. The caller records the output as an observation before acting
    /// on it (R2); this driver records nothing.
    pub async fn chat_turn_with_tools(
        &self,
        model: &str,
        messages: &[CoreMessage],
        tools: &[CoreToolSpec],
        choice: CoreToolChoice,
    ) -> Result<CoreTurnOutput> {
        self.chat_turn_configured(
            model,
            messages,
            tools,
            choice,
            CoreGenerationConfig::default(),
        )
        .await
    }

    /// Execute a tool-calling turn with explicit sampling controls.
    pub async fn chat_turn_configured(
        &self,
        model: &str,
        messages: &[CoreMessage],
        tools: &[CoreToolSpec],
        choice: CoreToolChoice,
        config: CoreGenerationConfig,
    ) -> Result<CoreTurnOutput> {
        let request = render_request(messages, tools);
        let options = render_options(&choice, &config);

        let response = self
            .client()
            .exec_chat(model, request, Some(&options))
            .await
            .with_context(|| format!("tool-calling chat turn against {model:?}"))?;

        self.add_tokens(response.usage.total_tokens.map(|t| t as usize).unwrap_or(0))
            .await;

        let calls: Vec<CoreToolCall> = response
            .tool_calls()
            .into_iter()
            .map(|call| CoreToolCall {
                call_id: call.call_id.clone(),
                name: call.fn_name.clone(),
                arguments: call.fn_arguments.clone(),
            })
            .collect();
        if !calls.is_empty() {
            return Ok(CoreTurnOutput::ToolCalls(calls));
        }
        Ok(CoreTurnOutput::Text(
            response.first_text().unwrap_or_default().to_string(),
        ))
    }

    /// Stream one tool-aware turn while retaining the complete tool calls.
    ///
    /// Reasoning is forwarded as it arrives. Answer text is held until the
    /// terminal event proves that the model did not select a tool, preventing
    /// tool-call preambles and protocol fragments from leaking into the chat.
    pub async fn stream_tool_turn(
        &self,
        model: &str,
        messages: &[CoreMessage],
        tools: &[CoreToolSpec],
        choice: CoreToolChoice,
        mut emit: impl FnMut(CoreTurnStreamEvent),
    ) -> Result<CoreTurnOutput> {
        let request = render_request(messages, tools);
        let options = render_options(&choice, &CoreGenerationConfig::default())
            .with_capture_content(true)
            .with_capture_tool_calls(true)
            .with_capture_usage(true);
        let response = self
            .client()
            .exec_chat_stream(model, request, Some(&options))
            .await
            .with_context(|| format!("streaming tool-aware chat turn against {model:?}"))?;
        let mut stream = response.stream;
        let mut end = None;

        while let Some(event) = stream.next().await {
            match event? {
                ChatStreamEvent::ReasoningChunk(chunk) if !chunk.content.is_empty() => {
                    emit(CoreTurnStreamEvent::Reasoning(chunk.content));
                }
                ChatStreamEvent::End(value) => {
                    end = Some(value);
                    break;
                }
                ChatStreamEvent::Start
                | ChatStreamEvent::Chunk(_)
                | ChatStreamEvent::ReasoningChunk(_)
                | ChatStreamEvent::ToolCallChunk(_)
                | ChatStreamEvent::ThoughtSignatureChunk(_) => {}
            }
        }

        let end = end.context("tool-aware model stream ended without a terminal event")?;
        self.add_tokens(
            end.captured_usage
                .as_ref()
                .and_then(|usage| usage.total_tokens)
                .map(|tokens| tokens as usize)
                .unwrap_or(0),
        )
        .await;

        let calls = end
            .captured_tool_calls()
            .unwrap_or_default()
            .into_iter()
            .map(|call| CoreToolCall {
                call_id: call.call_id.clone(),
                name: call.fn_name.clone(),
                arguments: call.fn_arguments.clone(),
            })
            .collect::<Vec<_>>();
        if !calls.is_empty() {
            return Ok(CoreTurnOutput::ToolCalls(calls));
        }

        let text = end.captured_first_text().unwrap_or_default().to_string();
        if !text.is_empty() {
            emit(CoreTurnStreamEvent::Text(text.clone()));
        }
        Ok(CoreTurnOutput::Text(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_every_message_role() {
        let messages = [
            CoreMessage::System {
                content: "s".into(),
            },
            CoreMessage::User {
                content: "u".into(),
            },
            CoreMessage::Assistant {
                content: "a".into(),
            },
            CoreMessage::AssistantToolCalls {
                calls: vec![CoreToolCall {
                    call_id: "c1".into(),
                    name: "read_file".into(),
                    arguments: serde_json::json!({"path": "x"}),
                }],
            },
            CoreMessage::ToolResponse {
                call_id: "c1".into(),
                content: "ok".into(),
            },
        ];
        let rendered: Vec<ChatMessage> = messages.iter().map(render_message).collect();
        assert_eq!(rendered.len(), 5);
        use genai::chat::ChatRole;
        assert_eq!(rendered[0].role, ChatRole::System);
        assert_eq!(rendered[3].role, ChatRole::Assistant);
        assert_eq!(rendered[4].role, ChatRole::Tool);
    }

    #[test]
    fn strict_flag_reaches_the_vendor_tool() {
        let spec = CoreToolSpec {
            name: "edit_file".into(),
            description: "d".into(),
            schema: serde_json::json!({"type": "object"}),
            strict: true,
        };
        let tool = render_tool(&spec);
        assert_eq!(tool.strict, Some(true));
        let lax = render_tool(&CoreToolSpec {
            strict: false,
            ..spec
        });
        assert_eq!(lax.strict, None);
    }

    #[test]
    fn choice_policies_map_one_to_one() {
        assert!(matches!(
            render_choice(&CoreToolChoice::Auto),
            ToolChoice::Auto
        ));
        assert!(matches!(
            render_choice(&CoreToolChoice::Required),
            ToolChoice::Required
        ));
        assert!(matches!(
            render_choice(&CoreToolChoice::Specific("grep".into())),
            ToolChoice::Tool { name } if name == "grep"
        ));
    }
}
