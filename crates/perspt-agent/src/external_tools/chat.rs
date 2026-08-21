//! Chat-mode MCP tools: the same admission and transports as the agent,
//! behind the interactive chat interface.
//!
//! Chat is not kernel-governed, so its admission capability grants
//! **read-only effects only** — a server tool that declares (or defaults
//! to) any mutating effect is rejected in chat mode. `simple-chat` never
//! constructs this session and keeps the plain streaming path.

#![allow(deprecated)]

use std::sync::Arc;

use anyhow::Result;
use perspt_core::tools_driver::CoreGenerationConfig;
use perspt_core::tools_driver::{CoreMessage, CoreToolCall, CoreToolChoice, CoreToolSpec};
use perspt_core::{Config, ExternalToolMode, GenAIProvider};
use perspt_sdk::{ActorId, Capability, EffectKind};

use super::{
    ExternalToolRuntime, McpClientServices, McpElicitationAction, McpElicitationBroker,
    McpPendingElicitation, McpSamplingProvider,
};

/// Bounded tool rounds per chat message.
const MAX_TOOL_ROUNDS: usize = 4;

/// A chat-scoped MCP lifecycle: discovery happened, specs are ready, and
/// every call goes through the shared runtime.
#[derive(Clone)]
pub struct ChatToolSession {
    runtime: Arc<tokio::sync::Mutex<ExternalToolRuntime>>,
    specs: Arc<std::sync::RwLock<Vec<CoreToolSpec>>>,
    elicitation: Option<McpElicitationBroker>,
}

struct ChatSamplingProvider {
    provider: Arc<GenAIProvider>,
    model: String,
}

#[async_trait::async_trait]
impl McpSamplingProvider for ChatSamplingProvider {
    async fn create_message(
        &self,
        _server_id: &str,
        request: rmcp::model::CreateMessageRequestParams,
    ) -> Result<rmcp::model::CreateMessageResult> {
        let sample = prepare_sample(request)?;
        let output = self
            .provider
            .chat_turn_configured(
                &self.model,
                &sample.messages,
                &sample.tools,
                sample.choice,
                sample.config,
            )
            .await?;
        let result = sample_result(output, &self.model)?;
        result.validate().map_err(anyhow::Error::msg)?;
        Ok(result)
    }
}

struct ChatSample {
    messages: Vec<CoreMessage>,
    tools: Vec<CoreToolSpec>,
    choice: CoreToolChoice,
    config: CoreGenerationConfig,
}

fn prepare_sample(request: rmcp::model::CreateMessageRequestParams) -> Result<ChatSample> {
    request.validate().map_err(anyhow::Error::msg)?;
    anyhow::ensure!(
        request.include_context.is_none()
            || request.include_context == Some(rmcp::model::ContextInclusion::None),
        "MCP sampling includeContext is not advertised by Perspt"
    );
    let mut messages = request
        .system_prompt
        .map(|content| vec![CoreMessage::System { content }])
        .unwrap_or_default();
    for message in request.messages {
        append_sampling(&mut messages, message)?;
    }
    let tools = request
        .tools
        .unwrap_or_default()
        .into_iter()
        .map(core_tool)
        .collect();
    let choice = match request.tool_choice.and_then(|choice| choice.mode) {
        Some(rmcp::model::ToolChoiceMode::Required) => CoreToolChoice::Required,
        Some(rmcp::model::ToolChoiceMode::None) => CoreToolChoice::None,
        _ => CoreToolChoice::Auto,
    };
    Ok(ChatSample {
        messages,
        tools,
        choice,
        config: CoreGenerationConfig {
            max_tokens: Some(request.max_tokens),
            temperature: request.temperature,
            stop_sequences: request.stop_sequences.unwrap_or_default(),
        },
    })
}

fn core_tool(tool: rmcp::model::Tool) -> CoreToolSpec {
    CoreToolSpec {
        name: tool.name.into_owned(),
        description: tool
            .description
            .map(|value| value.into_owned())
            .unwrap_or_default(),
        schema: serde_json::Value::Object(tool.input_schema.as_ref().clone()),
        strict: false,
    }
}

fn sample_result(
    output: perspt_core::CoreTurnOutput,
    model: &str,
) -> Result<rmcp::model::CreateMessageResult> {
    match output {
        perspt_core::CoreTurnOutput::Text(text) => Ok(rmcp::model::CreateMessageResult::new(
            rmcp::model::SamplingMessage::assistant_text(text),
            model.to_string(),
        )
        .with_stop_reason(rmcp::model::CreateMessageResult::STOP_REASON_END_TURN)),
        perspt_core::CoreTurnOutput::ToolCalls(calls) => {
            let blocks = calls
                .into_iter()
                .map(|call| {
                    let input = call.arguments.as_object().cloned().ok_or_else(|| {
                        anyhow::anyhow!("MCP sampling tool arguments must be an object")
                    })?;
                    Ok(rmcp::model::SamplingMessageContentBlock::tool_use(
                        call.call_id,
                        call.name,
                        input,
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(rmcp::model::CreateMessageResult::new(
                rmcp::model::SamplingMessage::new_multiple(rmcp::model::Role::Assistant, blocks),
                model.to_string(),
            )
            .with_stop_reason(rmcp::model::CreateMessageResult::STOP_REASON_TOOL_USE))
        }
    }
}

fn append_sampling(
    messages: &mut Vec<CoreMessage>,
    message: rmcp::model::SamplingMessage,
) -> Result<()> {
    let mut text = Vec::new();
    let mut calls = Vec::new();
    let mut results = Vec::new();
    for content in message.content.into_vec() {
        match content {
            rmcp::model::SamplingMessageContentBlock::Text(value) => text.push(value.text),
            rmcp::model::SamplingMessageContentBlock::ToolUse(value) => {
                calls.push(CoreToolCall {
                    call_id: value.id,
                    name: value.name,
                    arguments: serde_json::Value::Object(value.input),
                });
            }
            rmcp::model::SamplingMessageContentBlock::ToolResult(value) => {
                results.push((value.tool_use_id, serde_json::to_string(&value.content)?));
            }
            rmcp::model::SamplingMessageContentBlock::Image(_)
            | rmcp::model::SamplingMessageContentBlock::Audio(_) => {
                anyhow::bail!("the Perspt chat provider does not accept MCP media sampling content")
            }
            _ => anyhow::bail!("unknown MCP sampling content block"),
        }
    }
    if !text.is_empty() {
        messages.push(match message.role {
            rmcp::model::Role::User => CoreMessage::User {
                content: text.join("\n"),
            },
            rmcp::model::Role::Assistant => CoreMessage::Assistant {
                content: text.join("\n"),
            },
        });
    }
    if !calls.is_empty() {
        messages.push(CoreMessage::AssistantToolCalls { calls });
    }
    messages.extend(
        results
            .into_iter()
            .map(|(call_id, content)| CoreMessage::ToolResponse { call_id, content }),
    );
    Ok(())
}

/// Every read-only effect, the most a chat-admitted tool may declare.
fn read_only_effects() -> Vec<EffectKind> {
    [
        EffectKind::ReadFile,
        EffectKind::SystemProbe,
        EffectKind::DataRead,
        EffectKind::ToolSearch,
        EffectKind::ToolProgram,
        EffectKind::Search,
        EffectKind::List,
        EffectKind::LspQuery,
        EffectKind::GitRead,
    ]
    .into()
}

fn chat_services(
    config: &Config,
    provider: Arc<GenAIProvider>,
    model: &str,
) -> (McpClientServices, Option<McpElicitationBroker>) {
    let enabled = |select: fn(&perspt_core::ExternalToolConfig) -> bool| {
        config
            .external_tools
            .iter()
            .any(|server| server.supports(ExternalToolMode::Chat) && select(server))
    };
    let sampling = enabled(|server| server.sampling).then(|| {
        Arc::new(ChatSamplingProvider {
            provider,
            model: model.to_string(),
        }) as Arc<dyn McpSamplingProvider>
    });
    let elicitation = enabled(|server| server.elicitation).then(McpElicitationBroker::new);
    let services = McpClientServices {
        sampling,
        elicitation: elicitation
            .clone()
            .map(|broker| Arc::new(broker) as Arc<dyn super::McpElicitationProvider>),
    };
    (services, elicitation)
}

async fn discover_all(runtime: &mut ExternalToolRuntime) -> Vec<String> {
    let servers: Vec<String> = runtime
        .server_ids()
        .into_iter()
        .map(str::to_string)
        .collect();
    let mut notices = Vec::new();
    for server_id in servers {
        match runtime.discover_server(&server_id).await {
            Ok(admitted) => add_discovery_notice(runtime, &server_id, admitted.len(), &mut notices),
            Err(error) => notices.push(format!("MCP server {server_id} failed: {error:#}")),
        }
    }
    notices
}

fn add_discovery_notice(
    runtime: &ExternalToolRuntime,
    server_id: &str,
    admitted: usize,
    notices: &mut Vec<String>,
) {
    let rejected = runtime.admission_rejections(server_id);
    notices.push(format!(
        "MCP server {server_id}: {admitted} tool(s) admitted, {} rejected by local policy",
        rejected.len()
    ));
    notices.extend(rejected.iter().take(8).map(|item| {
        format!(
            "MCP tool {}.{} rejected: {}",
            item.server_id, item.remote_tool, item.reason
        )
    }));
    if rejected.len() > 8 {
        notices.push(format!(
            "MCP server {server_id}: {} additional rejection(s) omitted",
            rejected.len() - 8
        ));
    }
}

fn core_specs(runtime: &ExternalToolRuntime) -> Vec<CoreToolSpec> {
    runtime
        .admitted_entries()
        .into_iter()
        .map(|entry| CoreToolSpec {
            name: entry.name,
            description: entry.description,
            schema: entry.schema,
            strict: false,
        })
        .collect()
}

impl ChatToolSession {
    /// Build and discover the chat-mode session; `None` when no server is
    /// configured for chat. Failing servers are skipped with a notice.
    pub async fn from_config(
        config: &Config,
        provider: Arc<GenAIProvider>,
        model: &str,
    ) -> Result<Option<(Self, Vec<String>)>> {
        let has_chat_servers = config
            .external_tools
            .iter()
            .any(|server| server.supports(ExternalToolMode::Chat));
        if !has_chat_servers {
            return Ok(None);
        }
        let capability = Capability::new(ActorId::new("chat"), read_only_effects());
        let mut runtime =
            ExternalToolRuntime::from_config(config, ExternalToolMode::Chat, vec![capability])?;
        let (services, elicitation) = chat_services(config, provider, model);
        runtime.set_client_services(services);
        let notices = discover_all(&mut runtime).await;
        let specs = core_specs(&runtime);
        Ok(Some((
            Self {
                runtime: Arc::new(tokio::sync::Mutex::new(runtime)),
                specs: Arc::new(std::sync::RwLock::new(specs)),
                elicitation,
            },
            notices,
        )))
    }

    pub fn has_tools(&self) -> bool {
        !self
            .specs
            .read()
            .expect("MCP tool specs poisoned")
            .is_empty()
    }

    /// Namespaced names of the tools admitted for this chat lifecycle.
    ///
    /// The TUI uses this for local discovery only (`/mcp`); descriptions and
    /// schemas remain in the provider request and are never treated as local
    /// instructions.
    pub fn tool_names(&self) -> Vec<String> {
        self.specs
            .read()
            .expect("MCP tool specs poisoned")
            .iter()
            .map(|spec| spec.name.clone())
            .collect()
    }

    pub fn try_next_elicitation(&self) -> Option<McpPendingElicitation> {
        self.elicitation
            .as_ref()
            .and_then(McpElicitationBroker::try_next)
    }

    pub fn respond_elicitation(
        &self,
        id: u64,
        action: McpElicitationAction,
        content: Option<serde_json::Value>,
    ) -> Result<()> {
        self.elicitation
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("MCP elicitation is not enabled"))?
            .respond(id, action, content)
    }

    /// One chat message with bounded tool rounds. Tool activity notices are
    /// sent through `notices` as they happen; the final text is returned.
    pub async fn run_turn(
        &self,
        provider: &GenAIProvider,
        model: &str,
        mut messages: Vec<CoreMessage>,
        notices: &tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<String> {
        for _round in 0..MAX_TOOL_ROUNDS {
            self.sync_notifications(notices).await?;
            let specs = self.specs.read().expect("MCP tool specs poisoned").clone();
            let output = provider
                .chat_turn_with_tools(model, &messages, &specs, CoreToolChoice::Auto)
                .await?;
            let calls = match output {
                perspt_core::tools_driver::CoreTurnOutput::Text(text) => return Ok(text),
                perspt_core::tools_driver::CoreTurnOutput::ToolCalls(calls) => calls,
            };
            messages.push(CoreMessage::AssistantToolCalls {
                calls: calls.clone(),
            });
            for call in calls {
                let _ = notices.send(format!("__PERSPT_REASONING__:🔧 {}\n", call.name));
                let content = self.execute(&call).await;
                messages.push(CoreMessage::ToolResponse {
                    call_id: call.call_id,
                    content,
                });
            }
        }
        Ok("(tool round limit reached without a final answer)".into())
    }

    async fn sync_notifications(
        &self,
        notices: &tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<()> {
        let mut runtime = self.runtime.lock().await;
        let events = runtime.sync_server_events().await?;
        for (server, event) in events {
            let _ = notices.send(format!("__PERSPT_REASONING__:MCP {server}: {event:?}\n"));
        }
        let specs = runtime
            .admitted_entries()
            .into_iter()
            .map(|entry| CoreToolSpec {
                name: entry.name,
                description: entry.description,
                schema: entry.schema,
                strict: false,
            })
            .collect();
        *self.specs.write().expect("MCP tool specs poisoned") = specs;
        Ok(())
    }

    async fn execute(&self, call: &CoreToolCall) -> String {
        let mut runtime = self.runtime.lock().await;
        match runtime.call(&call.name, call.arguments.clone()).await {
            Ok(result) => format!(
                "[untrusted MCP result{}] {}",
                if result.is_error { ", tool error" } else { "" },
                result.content
            ),
            Err(error) => format!("tool failed: {error:#}"),
        }
    }
}
