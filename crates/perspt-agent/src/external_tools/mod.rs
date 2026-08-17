//! Shared governed MCP runtime for agent and interactive chat lifecycles.
//!
//! Discovery is lazy, remote descriptions/results are observations, and only
//! local policy can classify effects and footprints. Constructing one runtime
//! never starts a server; `discover_server` is the first lifecycle action.

mod http;
mod protocol;
mod stdio;

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub use protocol::{McpTransport, MCP_PROTOCOL_VERSION};

use http::HttpTransport;
use perspt_core::{
    Config, ExternalToolConfig, ExternalToolMode, ExternalToolPolicy, ExternalToolTransport,
};
use perspt_sdk::{
    admit_external_tool, Capability, EffectKind, ExternalToolDeclaration, RiskClass, ToolEntry,
};
use protocol::{McpRemoteTool, McpToolPage};
use stdio::StdioTransport;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalConnectionState {
    Configured,
    Connected,
    Discovered,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ExternalToolEvent {
    Connection {
        server_id: String,
        state: ExternalConnectionState,
    },
    Admission {
        server_id: String,
        tool: String,
        admitted: bool,
        reason: Option<String>,
    },
    Proposal {
        server_id: String,
        tool: String,
    },
    Result {
        server_id: String,
        tool: String,
        is_error: bool,
    },
    ReconciliationRequired {
        server_id: String,
        tool: String,
        reason: String,
    },
}

pub trait ExternalToolObserver: Send + Sync {
    fn record(&self, event: &ExternalToolEvent) -> Result<()>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExternalToolResult {
    pub content: serde_json::Value,
    pub is_error: bool,
    pub replayed: bool,
}

struct ServerState {
    config: ExternalToolConfig,
    state: ExternalConnectionState,
    client: Option<Box<dyn McpTransport>>,
    calls: BTreeMap<String, String>,
}

impl std::fmt::Debug for ServerState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ServerState")
            .field("id", &self.config.id)
            .field("state", &self.state)
            .field("connected", &self.client.is_some())
            .field("tool_count", &self.calls.len())
            .finish()
    }
}

/// A single role-specific external-tool lifecycle. Agent and chat construct
/// separate instances even when they select the same server configuration.
pub struct ExternalToolRuntime {
    mode: ExternalToolMode,
    capabilities: Vec<Capability>,
    servers: BTreeMap<String, ServerState>,
    entries: BTreeMap<String, ToolEntry>,
    observer: Option<Arc<dyn ExternalToolObserver>>,
    replay_results: BTreeMap<String, VecDeque<ExternalToolResult>>,
    replay_only: bool,
}

impl std::fmt::Debug for ExternalToolRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExternalToolRuntime")
            .field("mode", &self.mode)
            .field("servers", &self.servers)
            .field("entry_count", &self.entries.len())
            .field("replay_only", &self.replay_only)
            .finish_non_exhaustive()
    }
}

impl ExternalToolRuntime {
    pub fn from_config(
        config: &Config,
        mode: ExternalToolMode,
        capabilities: Vec<Capability>,
    ) -> Result<Self> {
        config.validate()?;
        let servers = config
            .external_tools
            .iter()
            .filter(|server| server.supports(mode))
            .map(|server| {
                (
                    server.id.clone(),
                    ServerState {
                        config: server.clone(),
                        state: ExternalConnectionState::Configured,
                        client: None,
                        calls: BTreeMap::new(),
                    },
                )
            })
            .collect();
        Ok(Self {
            mode,
            capabilities,
            servers,
            entries: BTreeMap::new(),
            observer: None,
            replay_results: BTreeMap::new(),
            replay_only: false,
        })
    }

    /// Build a replay lifecycle from admitted entries and recorded results.
    /// It has no server configuration and therefore cannot reconnect.
    pub fn replay(
        mode: ExternalToolMode,
        entries: Vec<ToolEntry>,
        results: BTreeMap<String, VecDeque<ExternalToolResult>>,
    ) -> Self {
        Self {
            mode,
            capabilities: Vec::new(),
            servers: BTreeMap::new(),
            entries: entries
                .into_iter()
                .map(|entry| (entry.name.clone(), entry))
                .collect(),
            observer: None,
            replay_results: results,
            replay_only: true,
        }
    }

    pub fn with_observer(mut self, observer: Arc<dyn ExternalToolObserver>) -> Self {
        self.observer = Some(observer);
        self
    }

    pub fn server_ids(&self) -> Vec<&str> {
        self.servers.keys().map(String::as_str).collect()
    }

    pub fn state(&self, server_id: &str) -> Option<ExternalConnectionState> {
        self.servers
            .get(server_id)
            .map(|server| server.state.clone())
    }

    pub fn admitted_entries(&self) -> Vec<ToolEntry> {
        self.entries.values().cloned().collect()
    }

    pub async fn discover_server(&mut self, server_id: &str) -> Result<Vec<ToolEntry>> {
        anyhow::ensure!(
            !self.replay_only,
            "replay never reconnects to an MCP server"
        );
        if self
            .servers
            .get(server_id)
            .is_some_and(|server| !server.calls.is_empty())
        {
            return Ok(self.entries_for_server(server_id));
        }
        self.connect(server_id).await?;
        let remote_tools = self.list_all_tools(server_id).await?;
        let admitted = self.admit_tools(server_id, remote_tools)?;
        self.observe(ExternalToolEvent::Connection {
            server_id: server_id.to_string(),
            state: ExternalConnectionState::Discovered,
        })?;
        Ok(admitted)
    }

    pub async fn call(
        &mut self,
        namespaced_tool: &str,
        arguments: serde_json::Value,
    ) -> Result<ExternalToolResult> {
        let entry = self
            .entries
            .get(namespaced_tool)
            .cloned()
            .with_context(|| format!("external tool {namespaced_tool:?} was not admitted"))?;
        entry.validate_arguments(&arguments)?;
        let replay_key = replay_key(namespaced_tool, &arguments)?;
        if self.replay_only {
            return self.consume_replay(&replay_key);
        }
        let (server_id, remote_name) = self.resolve_call(namespaced_tool)?;
        self.observe(ExternalToolEvent::Proposal {
            server_id: server_id.clone(),
            tool: namespaced_tool.to_string(),
        })?;
        let result = self.call_live(&server_id, &remote_name, arguments).await;
        match result {
            Ok(result) => {
                self.observe(ExternalToolEvent::Result {
                    server_id,
                    tool: namespaced_tool.to_string(),
                    is_error: result.is_error,
                })?;
                Ok(result)
            }
            Err(error) => {
                self.observe(ExternalToolEvent::ReconciliationRequired {
                    server_id,
                    tool: namespaced_tool.to_string(),
                    reason: error.to_string(),
                })?;
                Err(error.context(
                    "external completion is uncertain; explicit reconciliation is required",
                ))
            }
        }
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        let ids: Vec<String> = self.servers.keys().cloned().collect();
        for id in ids {
            let server = self.servers.get_mut(&id).expect("server id collected");
            if let Some(client) = server.client.as_mut() {
                client.shutdown().await?;
            }
            server.client = None;
            server.state = ExternalConnectionState::Closed;
            self.observe(ExternalToolEvent::Connection {
                server_id: id,
                state: ExternalConnectionState::Closed,
            })?;
        }
        Ok(())
    }

    async fn connect(&mut self, server_id: &str) -> Result<()> {
        let config = self
            .servers
            .get(server_id)
            .with_context(|| format!("unknown external tool server {server_id:?}"))?
            .config
            .clone();
        if self
            .servers
            .get(server_id)
            .is_some_and(|server| server.client.is_some())
        {
            return Ok(());
        }
        let mut client = create_transport(&config).await?;
        initialize(client.as_mut(), &config).await?;
        let server = self.servers.get_mut(server_id).expect("server checked");
        server.client = Some(client);
        server.state = ExternalConnectionState::Connected;
        self.observe(ExternalToolEvent::Connection {
            server_id: server_id.to_string(),
            state: ExternalConnectionState::Connected,
        })
    }

    async fn list_all_tools(&mut self, server_id: &str) -> Result<Vec<McpRemoteTool>> {
        let server = self
            .servers
            .get_mut(server_id)
            .context("server disappeared")?;
        let client = server.client.as_mut().context("server is not connected")?;
        let timeout = Duration::from_millis(server.config.timeout_ms);
        let maximum = server.config.max_result_bytes;
        let mut cursor: Option<String> = None;
        let mut seen = BTreeSet::new();
        let mut tools = Vec::new();
        loop {
            let params = cursor
                .as_ref()
                .map(|value| serde_json::json!({"cursor": value}))
                .unwrap_or_else(|| serde_json::json!({}));
            let result = client
                .request("tools/list", params, timeout, maximum)
                .await?;
            let page: McpToolPage = serde_json::from_value(result)?;
            anyhow::ensure!(
                tools.len().saturating_add(page.tools.len()) <= 10_000,
                "MCP server advertised too many tools"
            );
            tools.extend(page.tools);
            let Some(next) = page.next_cursor else {
                break;
            };
            anyhow::ensure!(seen.insert(next.clone()), "MCP tools/list cursor loop");
            cursor = Some(next);
        }
        Ok(tools)
    }

    fn admit_tools(
        &mut self,
        server_id: &str,
        remote_tools: Vec<McpRemoteTool>,
    ) -> Result<Vec<ToolEntry>> {
        let policies = self
            .servers
            .get(server_id)
            .context("server disappeared")?
            .config
            .tools
            .clone();
        let mut admitted = Vec::new();
        for remote in remote_tools {
            let policy = policies.get(&remote.name).cloned().unwrap_or_default();
            match self.admit_one(server_id, &remote, &policy) {
                Ok(entry) => {
                    self.store_entry(server_id, remote.name, entry.clone())?;
                    self.observe_admission(server_id, &entry.name, true, None)?;
                    admitted.push(entry);
                }
                Err(error) => {
                    let name = namespace(server_id, &remote.name)?;
                    self.observe_admission(server_id, &name, false, Some(error.to_string()))?;
                }
            }
        }
        self.servers
            .get_mut(server_id)
            .expect("server checked")
            .state = ExternalConnectionState::Discovered;
        Ok(admitted)
    }

    fn admit_one(
        &self,
        server_id: &str,
        remote: &McpRemoteTool,
        policy: &ExternalToolPolicy,
    ) -> Result<ToolEntry> {
        let name = namespace(server_id, &remote.name)?;
        let declaration = ExternalToolDeclaration {
            name,
            description: format!("[untrusted MCP description] {}", remote.description),
            effect: policy.effect.unwrap_or(EffectKind::RunShell),
            risk: policy.risk.unwrap_or(RiskClass::High),
            schema: remote.input_schema.clone(),
            footprint: policy.footprint.clone(),
        };
        let mut entry = admit_external_tool(server_id, declaration, &self.capabilities)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        entry.proposal_bindings = policy.proposal_bindings.clone();
        entry
            .validate()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Ok(entry)
    }

    fn store_entry(&mut self, server_id: &str, remote: String, entry: ToolEntry) -> Result<()> {
        anyhow::ensure!(
            self.entries
                .insert(entry.name.clone(), entry.clone())
                .is_none(),
            "duplicate namespaced MCP tool {:?}",
            entry.name
        );
        self.servers
            .get_mut(server_id)
            .context("server disappeared")?
            .calls
            .insert(entry.name, remote);
        Ok(())
    }

    fn observe_admission(
        &self,
        server_id: &str,
        tool: &str,
        admitted: bool,
        reason: Option<String>,
    ) -> Result<()> {
        self.observe(ExternalToolEvent::Admission {
            server_id: server_id.to_string(),
            tool: tool.to_string(),
            admitted,
            reason,
        })
    }

    fn entries_for_server(&self, server_id: &str) -> Vec<ToolEntry> {
        let prefix = format!("mcp.{server_id}.");
        self.entries
            .values()
            .filter(|entry| entry.name.starts_with(&prefix))
            .cloned()
            .collect()
    }

    fn resolve_call(&self, tool: &str) -> Result<(String, String)> {
        self.servers
            .iter()
            .find_map(|(server_id, server)| {
                server
                    .calls
                    .get(tool)
                    .map(|remote| (server_id.clone(), remote.clone()))
            })
            .with_context(|| format!("no live MCP binding for {tool:?}"))
    }

    async fn call_live(
        &mut self,
        server_id: &str,
        remote_name: &str,
        arguments: serde_json::Value,
    ) -> Result<ExternalToolResult> {
        let server = self
            .servers
            .get_mut(server_id)
            .context("server disappeared")?;
        let client = server.client.as_mut().context("server is not connected")?;
        let result = client
            .request(
                "tools/call",
                serde_json::json!({"name": remote_name, "arguments": arguments}),
                Duration::from_millis(server.config.timeout_ms),
                server.config.max_result_bytes,
            )
            .await?;
        let is_error = result
            .get("isError")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        Ok(ExternalToolResult {
            content: result,
            is_error,
            replayed: false,
        })
    }

    fn consume_replay(&mut self, key: &str) -> Result<ExternalToolResult> {
        let mut result = self
            .replay_results
            .get_mut(key)
            .and_then(VecDeque::pop_front)
            .with_context(|| format!("no recorded MCP observation for replay key {key}"))?;
        result.replayed = true;
        Ok(result)
    }

    fn observe(&self, event: ExternalToolEvent) -> Result<()> {
        match &self.observer {
            Some(observer) => observer.record(&event),
            None => Ok(()),
        }
    }
}

async fn create_transport(config: &ExternalToolConfig) -> Result<Box<dyn McpTransport>> {
    match config.transport {
        ExternalToolTransport::Stdio => Ok(Box::new(StdioTransport::spawn(config).await?)),
        ExternalToolTransport::StreamableHttp => Ok(Box::new(HttpTransport::new(config)?)),
    }
}

async fn initialize(client: &mut dyn McpTransport, config: &ExternalToolConfig) -> Result<()> {
    let result = client
        .request(
            "initialize",
            serde_json::json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "perspt", "version": env!("CARGO_PKG_VERSION")},
            }),
            Duration::from_millis(config.timeout_ms),
            config.max_result_bytes,
        )
        .await?;
    anyhow::ensure!(
        result
            .get("protocolVersion")
            .and_then(serde_json::Value::as_str)
            == Some(MCP_PROTOCOL_VERSION),
        "MCP server negotiated an unsupported protocol version"
    );
    client
        .notify("notifications/initialized", serde_json::json!({}))
        .await
}

fn namespace(server_id: &str, remote_name: &str) -> Result<String> {
    anyhow::ensure!(
        !remote_name.is_empty()
            && remote_name.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
            }),
        "MCP tool name {remote_name:?} contains unsupported characters"
    );
    Ok(format!("mcp.{server_id}.{remote_name}"))
}

fn replay_key(name: &str, arguments: &serde_json::Value) -> Result<String> {
    let canonical = canonical_json(arguments);
    Ok(format!("{name}:{}", serde_json::to_string(&canonical)?))
}

fn canonical_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) => {
            let sorted: BTreeMap<_, _> = object
                .iter()
                .map(|(key, value)| (key.clone(), canonical_json(value)))
                .collect();
            serde_json::to_value(sorted).expect("JSON map serialization is infallible")
        }
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(canonical_json).collect())
        }
        scalar => scalar.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perspt_sdk::{ActorId, Capability};

    fn config() -> Config {
        Config::from_toml_str(
            r#"
            [[external_tools]]
            id = "agent-only"
            transport = "stdio"
            command = ["does-not-start-during-construction"]

            [[external_tools]]
            id = "both"
            transport = "stdio"
            command = ["also-lazy"]
            modes = ["agent", "chat"]
            "#,
        )
        .unwrap()
    }

    #[test]
    fn construction_is_lazy_and_filters_modes() {
        let capabilities = vec![Capability::new(
            ActorId::new("chat"),
            vec![EffectKind::RunShell],
        )];
        let chat =
            ExternalToolRuntime::from_config(&config(), ExternalToolMode::Chat, capabilities)
                .unwrap();
        assert_eq!(chat.server_ids(), vec!["both"]);
        assert_eq!(
            chat.state("both"),
            Some(ExternalConnectionState::Configured)
        );
    }

    #[test]
    fn namespace_is_deterministic_and_rejects_control_characters() {
        assert_eq!(namespace("docs", "find").unwrap(), "mcp.docs.find");
        assert!(namespace("docs", "bad\nname").is_err());
    }

    #[test]
    fn replay_keys_canonicalize_object_order() {
        let left = serde_json::json!({"a": 1, "b": 2});
        let right: serde_json::Value = serde_json::from_str("{\"b\":2,\"a\":1}").unwrap();
        assert_eq!(
            replay_key("mcp.s.t", &left).unwrap(),
            replay_key("mcp.s.t", &right).unwrap()
        );
    }
}
