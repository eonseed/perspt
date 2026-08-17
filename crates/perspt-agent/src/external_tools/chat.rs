//! Chat-mode MCP tools: the same admission and transports as the agent,
//! behind the interactive chat interface.
//!
//! Chat is not kernel-governed, so its admission capability grants
//! **read-only effects only** — a server tool that declares (or defaults
//! to) any mutating effect is rejected in chat mode. `simple-chat` never
//! constructs this session and keeps the plain streaming path.

use std::sync::Arc;

use anyhow::Result;
use perspt_core::tools_driver::{CoreMessage, CoreToolCall, CoreToolChoice, CoreToolSpec};
use perspt_core::{Config, ExternalToolMode, GenAIProvider};
use perspt_sdk::{ActorId, Capability, EffectKind};

use super::ExternalToolRuntime;

/// Bounded tool rounds per chat message.
const MAX_TOOL_ROUNDS: usize = 4;

/// A chat-scoped MCP lifecycle: discovery happened, specs are ready, and
/// every call goes through the shared runtime.
#[derive(Clone)]
pub struct ChatToolSession {
    runtime: Arc<tokio::sync::Mutex<ExternalToolRuntime>>,
    specs: Vec<CoreToolSpec>,
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

impl ChatToolSession {
    /// Build and discover the chat-mode session; `None` when no server is
    /// configured for chat. Failing servers are skipped with a notice.
    pub async fn from_config(config: &Config) -> Result<Option<(Self, Vec<String>)>> {
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
        let mut notices = Vec::new();
        let servers: Vec<String> = runtime
            .server_ids()
            .into_iter()
            .map(str::to_string)
            .collect();
        for server_id in servers {
            match runtime.discover_server(&server_id).await {
                Ok(admitted) => notices.push(format!(
                    "MCP server {server_id}: {} tool(s) admitted",
                    admitted.len()
                )),
                Err(error) => notices.push(format!("MCP server {server_id} failed: {error:#}")),
            }
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
        Ok(Some((
            Self {
                runtime: Arc::new(tokio::sync::Mutex::new(runtime)),
                specs,
            },
            notices,
        )))
    }

    pub fn has_tools(&self) -> bool {
        !self.specs.is_empty()
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
            let output = provider
                .chat_turn_with_tools(model, &messages, &self.specs, CoreToolChoice::Auto)
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
