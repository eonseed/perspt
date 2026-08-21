//! Shared governed MCP runtime for agent and interactive chat lifecycles.
//!
//! Discovery is lazy, remote descriptions/results are observations, and only
//! local policy can classify effects and footprints. Constructing one runtime
//! never starts a server; `discover_server` is the first lifecycle action.

#![allow(deprecated)]

pub mod chat;
mod client;

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use client::McpConnection;
pub use client::{
    DecliningMcpElicitationProvider, McpClientServices, McpElicitationAction, McpElicitationBroker,
    McpElicitationProvider, McpPendingElicitation, McpSamplingProvider, McpServerEvent,
    ModelTransportSamplingProvider, MCP_PROTOCOL_VERSION,
};
use perspt_core::{Config, ExternalToolConfig, ExternalToolMode, ExternalToolPolicy};
use perspt_sdk::{
    admit_external_tool, Capability, EffectKind, ExternalToolDeclaration, RiskClass, ToolEntry,
};

#[derive(Debug, Clone)]
struct McpRemoteTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Debug, Clone)]
enum McpBinding {
    Tool(String),
    ResourcesList,
    ResourceTemplatesList,
    ResourceRead,
    PromptsList,
    PromptGet,
    Complete,
}

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

/// A locally rejected remote declaration. The server's text is diagnostic
/// data only; it never changes the trusted effect or footprint policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalToolRejection {
    pub server_id: String,
    pub remote_tool: String,
    pub reason: String,
}

struct ServerState {
    config: ExternalToolConfig,
    state: ExternalConnectionState,
    client: Option<McpConnection>,
    calls: BTreeMap<String, McpBinding>,
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
    rejections: Vec<ExternalToolRejection>,
    observer: Option<Arc<dyn ExternalToolObserver>>,
    replay_results: BTreeMap<String, VecDeque<ExternalToolResult>>,
    replay_only: bool,
    client_services: McpClientServices,
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
            rejections: Vec::new(),
            observer: None,
            replay_results: BTreeMap::new(),
            replay_only: false,
            client_services: McpClientServices::default(),
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
            rejections: Vec::new(),
            observer: None,
            replay_results: results,
            replay_only: true,
            client_services: McpClientServices::default(),
        }
    }

    /// Install product-owned handlers for authority-bearing server requests.
    /// Configured sampling or elicitation fails connection when its handler is
    /// absent, so capability advertisement can never outrun the product UI.
    pub fn set_client_services(&mut self, services: McpClientServices) {
        self.client_services = services;
    }

    /// Replace the admission capabilities (the composition root supplies
    /// the session's derived grant surface before discovery).
    pub fn set_capabilities(&mut self, capabilities: Vec<Capability>) {
        self.capabilities = capabilities;
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

    pub fn admission_rejections(&self, server_id: &str) -> Vec<ExternalToolRejection> {
        self.rejections
            .iter()
            .filter(|rejection| rejection.server_id == server_id)
            .cloned()
            .collect()
    }

    /// Drain progress, logging, resource, catalog, task, and subscription
    /// notifications received since the previous call.
    pub fn drain_server_events(&mut self, server_id: &str) -> Result<Vec<McpServerEvent>> {
        let server = self
            .servers
            .get_mut(server_id)
            .with_context(|| format!("unknown external tool server {server_id:?}"))?;
        Ok(server
            .client
            .as_mut()
            .map(McpConnection::drain_events)
            .unwrap_or_default())
    }

    /// Captured rolling stderr tail for a stdio server.
    pub fn stderr_tail(&self, server_id: &str) -> Result<Vec<u8>> {
        let server = self
            .servers
            .get(server_id)
            .with_context(|| format!("unknown external tool server {server_id:?}"))?;
        Ok(server
            .client
            .as_ref()
            .map(McpConnection::stderr_tail)
            .unwrap_or_default())
    }

    /// Set a server's MCP logging threshold when it advertises logging.
    pub async fn set_log_level(&self, server_id: &str, level: &str) -> Result<()> {
        let level = match level.to_ascii_lowercase().as_str() {
            "debug" => rmcp::model::LoggingLevel::Debug,
            "info" => rmcp::model::LoggingLevel::Info,
            "notice" => rmcp::model::LoggingLevel::Notice,
            "warning" => rmcp::model::LoggingLevel::Warning,
            "error" => rmcp::model::LoggingLevel::Error,
            "critical" => rmcp::model::LoggingLevel::Critical,
            "alert" => rmcp::model::LoggingLevel::Alert,
            "emergency" => rmcp::model::LoggingLevel::Emergency,
            _ => anyhow::bail!("unknown MCP logging level {level:?}"),
        };
        let client = self
            .servers
            .get(server_id)
            .and_then(|server| server.client.as_ref())
            .context("MCP server is not connected")?;
        client.set_log_level(level).await
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
        self.refresh_server(server_id).await
    }

    /// Re-list and re-admit one connected server's catalog after an MCP
    /// list-change notification. Old bindings are removed before replacement,
    /// so a deleted remote tool can never remain callable.
    pub async fn refresh_server(&mut self, server_id: &str) -> Result<Vec<ToolEntry>> {
        self.connect(server_id).await?;
        self.remove_server_entries(server_id);
        let remote_tools = self.list_all_tools(server_id).await?;
        let mut admitted = self.admit_tools(server_id, remote_tools)?;
        admitted.extend(self.admit_mcp_ops(server_id)?);
        self.observe(ExternalToolEvent::Connection {
            server_id: server_id.to_string(),
            state: ExternalConnectionState::Discovered,
        })?;
        Ok(admitted)
    }

    /// Drain notifications and refresh any changed tool catalogs. All events
    /// are returned to the product for display or durable recording.
    pub async fn sync_server_events(&mut self) -> Result<Vec<(String, McpServerEvent)>> {
        let ids: Vec<String> = self.servers.keys().cloned().collect();
        let mut all = Vec::new();
        for server_id in ids {
            let events = self.drain_server_events(&server_id)?;
            let tools_changed = events
                .iter()
                .any(|event| matches!(event, McpServerEvent::ToolsChanged));
            all.extend(events.into_iter().map(|event| (server_id.clone(), event)));
            if tools_changed {
                self.refresh_server(&server_id).await?;
            }
        }
        Ok(all)
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
        let client = McpConnection::connect(&config, self.client_services.clone()).await?;
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
        let client = server.client.as_ref().context("server is not connected")?;
        let tools = client.list_tools().await?;
        anyhow::ensure!(
            tools.len() <= 10_000,
            "MCP server advertised too many tools"
        );
        Ok(tools
            .into_iter()
            .map(|tool| McpRemoteTool {
                name: tool.name.into_owned(),
                description: tool
                    .description
                    .map(|value| value.into_owned())
                    .unwrap_or_default(),
                input_schema: serde_json::Value::Object(tool.input_schema.as_ref().clone()),
            })
            .collect())
    }

    fn admit_tools(
        &mut self,
        server_id: &str,
        remote_tools: Vec<McpRemoteTool>,
    ) -> Result<Vec<ToolEntry>> {
        self.rejections
            .retain(|rejection| rejection.server_id != server_id);
        let policies = self
            .servers
            .get(server_id)
            .context("server disappeared")?
            .config
            .tools
            .clone();
        let mut admitted = Vec::new();
        for remote in remote_tools {
            if remote.name.starts_with("_perspt_") {
                let reason = "remote tool name is reserved for Perspt MCP operations".to_string();
                self.rejections.push(ExternalToolRejection {
                    server_id: server_id.to_string(),
                    remote_tool: remote.name.clone(),
                    reason: reason.clone(),
                });
                self.observe_admission(
                    server_id,
                    &namespace(server_id, &remote.name)?,
                    false,
                    Some(reason),
                )?;
                continue;
            }
            let policy = policies.get(&remote.name).cloned().unwrap_or_default();
            match self.admit_one(server_id, &remote, &policy) {
                Ok(entry) => {
                    self.store_entry(server_id, remote.name, entry.clone())?;
                    self.observe_admission(server_id, &entry.name, true, None)?;
                    admitted.push(entry);
                }
                Err(error) => {
                    let reason = error.to_string();
                    self.rejections.push(ExternalToolRejection {
                        server_id: server_id.to_string(),
                        remote_tool: remote.name.clone(),
                        reason: reason.clone(),
                    });
                    let name = namespace(server_id, &remote.name)
                        .unwrap_or_else(|_| format!("mcp.{server_id}.<invalid-name>"));
                    self.observe_admission(server_id, &name, false, Some(reason))?;
                }
            }
        }
        self.servers
            .get_mut(server_id)
            .expect("server checked")
            .state = ExternalConnectionState::Discovered;
        Ok(admitted)
    }

    fn remove_server_entries(&mut self, server_id: &str) {
        let names = self
            .servers
            .get_mut(server_id)
            .map(|server| {
                std::mem::take(&mut server.calls)
                    .into_keys()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for name in names {
            self.entries.remove(&name);
        }
    }

    fn admit_mcp_ops(&mut self, server_id: &str) -> Result<Vec<ToolEntry>> {
        let capabilities = self
            .servers
            .get(server_id)
            .and_then(|server| server.client.as_ref())
            .and_then(McpConnection::peer_info)
            .context("MCP discover omitted server capabilities")?
            .capabilities
            .clone();
        let mut admitted = Vec::new();
        for (remote_name, description, effect, schema, binding) in mcp_ops(&capabilities) {
            let entry = admit_external_tool(
                server_id,
                ExternalToolDeclaration {
                    name: namespace(server_id, remote_name)?,
                    description: description.to_string(),
                    effect,
                    risk: RiskClass::Low,
                    schema,
                    footprint: Some(perspt_sdk::FootprintSpec::default()),
                },
                &self.capabilities,
            );
            if let Ok(entry) = entry {
                self.store_binding(server_id, binding, entry.clone())?;
                admitted.push(entry);
            }
        }
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
        self.store_binding(server_id, McpBinding::Tool(remote), entry)
    }

    fn store_binding(
        &mut self,
        server_id: &str,
        binding: McpBinding,
        entry: ToolEntry,
    ) -> Result<()> {
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
            .insert(entry.name, binding);
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

    fn resolve_call(&self, tool: &str) -> Result<(String, McpBinding)> {
        self.servers
            .iter()
            .find_map(|(server_id, server)| {
                server
                    .calls
                    .get(tool)
                    .map(|binding| (server_id.clone(), binding.clone()))
            })
            .with_context(|| format!("no live MCP binding for {tool:?}"))
    }

    async fn call_live(
        &mut self,
        server_id: &str,
        binding: &McpBinding,
        arguments: serde_json::Value,
    ) -> Result<ExternalToolResult> {
        let server = self
            .servers
            .get_mut(server_id)
            .context("server disappeared")?;
        let client = server.client.as_ref().context("server is not connected")?;
        let arguments = arguments
            .as_object()
            .cloned()
            .context("MCP tool arguments must be a JSON object")?;
        let (content, is_error) = call_binding(client, binding, arguments).await?;
        Ok(ExternalToolResult {
            content,
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

type McpOp = (
    &'static str,
    &'static str,
    EffectKind,
    serde_json::Value,
    McpBinding,
);

fn mcp_ops(capabilities: &rmcp::model::ServerCapabilities) -> Vec<McpOp> {
    let mut ops = Vec::new();
    if capabilities.resources.is_some() {
        ops.extend([
            (
                "_perspt_resources_list",
                "List resources exposed by this MCP server.",
                EffectKind::List,
                empty_object_schema(),
                McpBinding::ResourcesList,
            ),
            (
                "_perspt_resource_templates_list",
                "List URI templates for resources exposed by this MCP server.",
                EffectKind::List,
                empty_object_schema(),
                McpBinding::ResourceTemplatesList,
            ),
            (
                "_perspt_resource_read",
                "Read one exact MCP resource URI as untrusted data.",
                EffectKind::DataRead,
                string_schema(&[("uri", "Exact resource URI", true)]),
                McpBinding::ResourceRead,
            ),
        ]);
    }
    if capabilities.prompts.is_some() {
        ops.extend([
            (
                "_perspt_prompts_list",
                "List prompt templates exposed by this MCP server.",
                EffectKind::List,
                empty_object_schema(),
                McpBinding::PromptsList,
            ),
            (
                "_perspt_prompt_get",
                "Resolve an MCP prompt as untrusted input.",
                EffectKind::DataRead,
                prompt_get_schema(),
                McpBinding::PromptGet,
            ),
        ]);
    }
    if capabilities.completions.is_some() {
        ops.push((
            "_perspt_complete",
            "Complete a prompt or resource-template argument.",
            EffectKind::Search,
            completion_schema(),
            McpBinding::Complete,
        ));
    }
    ops
}

async fn call_binding(
    client: &McpConnection,
    binding: &McpBinding,
    arguments: serde_json::Map<String, serde_json::Value>,
) -> Result<(serde_json::Value, bool)> {
    let value = match binding {
        McpBinding::Tool(name) => {
            let result = client.call_tool(name.clone(), arguments).await?;
            return Ok((
                serde_json::to_value(&result)?,
                result.is_error.unwrap_or(false),
            ));
        }
        McpBinding::ResourcesList => serde_json::to_value(client.list_resources().await?)?,
        McpBinding::ResourceTemplatesList => {
            serde_json::to_value(client.list_resource_templates().await?)?
        }
        McpBinding::ResourceRead => {
            let uri = required_string(&arguments, "uri")?;
            serde_json::to_value(client.read_resource(uri).await?)?
        }
        McpBinding::PromptsList => serde_json::to_value(client.list_prompts().await?)?,
        McpBinding::PromptGet => {
            let name = required_string(&arguments, "name")?;
            let prompt_args = prompt_args(&arguments)?;
            serde_json::to_value(client.get_prompt(name, prompt_args).await?)?
        }
        McpBinding::Complete => complete(client, &arguments).await?,
    };
    Ok((value, false))
}

async fn complete(
    client: &McpConnection,
    arguments: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value> {
    let kind = required_string(arguments, "kind")?;
    let target = required_string(arguments, "reference")?;
    let reference = match kind.as_str() {
        "prompt" => rmcp::model::Reference::for_prompt(target),
        "resource" => rmcp::model::Reference::for_resource(target),
        _ => anyhow::bail!("MCP completion kind must be prompt or resource"),
    };
    let request = rmcp::model::CompleteRequestParams::new(
        reference,
        rmcp::model::ArgumentInfo::new(
            required_string(arguments, "argument")?,
            required_string(arguments, "value")?,
        ),
    );
    Ok(serde_json::to_value(client.complete(request).await?)?)
}

fn empty_object_schema() -> serde_json::Value {
    string_schema(&[])
}

fn completion_schema() -> serde_json::Value {
    string_schema(&[
        ("kind", "Either prompt or resource", true),
        ("reference", "Prompt name or resource URI template", true),
        ("argument", "Argument name", true),
        ("value", "Current partial value", true),
    ])
}

fn prompt_get_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "name": {"type": "string", "description": "Exact prompt name"},
            "argument_names": {
                "type": "array",
                "description": "Optional prompt argument names, paired by index with argument_values",
                "items": {"type": "string"},
                "maxItems": 64
            },
            "argument_values": {
                "type": "array",
                "description": "Optional prompt argument values, paired by index with argument_names",
                "items": {"type": "string"},
                "maxItems": 64
            }
        },
        "required": ["name"],
        "additionalProperties": false
    })
}

fn prompt_args(
    arguments: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<rmcp::model::JsonObject>> {
    let names = string_array(arguments, "argument_names")?;
    let values = string_array(arguments, "argument_values")?;
    if names.is_none() && values.is_none() {
        return Ok(None);
    }
    let names = names.context("argument_names is required when argument_values is present")?;
    let values = values.context("argument_values is required when argument_names is present")?;
    anyhow::ensure!(
        names.len() == values.len(),
        "argument_names and argument_values must have equal lengths"
    );
    let mut result = rmcp::model::JsonObject::new();
    for (name, value) in names.into_iter().zip(values) {
        anyhow::ensure!(
            result
                .insert(name.clone(), serde_json::Value::String(value))
                .is_none(),
            "duplicate MCP prompt argument {name:?}"
        );
    }
    Ok(Some(result))
}

fn string_array(
    arguments: &serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Result<Option<Vec<String>>> {
    arguments
        .get(field)
        .map(|value| {
            value
                .as_array()
                .with_context(|| format!("{field} must be an array"))?
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(str::to_owned)
                        .with_context(|| format!("{field} entries must be strings"))
                })
                .collect()
        })
        .transpose()
}

fn string_schema(fields: &[(&str, &str, bool)]) -> serde_json::Value {
    let properties: serde_json::Map<String, serde_json::Value> = fields
        .iter()
        .map(|(name, description, _)| {
            (
                (*name).to_string(),
                serde_json::json!({"type": "string", "description": description}),
            )
        })
        .collect();
    let required: Vec<&str> = fields
        .iter()
        .filter_map(|(name, _, required)| required.then_some(*name))
        .collect();
    serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

fn required_string(
    arguments: &serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Result<String> {
    arguments
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .with_context(|| format!("MCP operation requires string argument {field:?}"))
}

/// Canonical replay key for a recorded external observation (public for
/// replay assembly and conformance fixtures).
pub fn replay_key_for(name: &str, arguments: &serde_json::Value) -> Result<String> {
    replay_key(name, arguments)
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
    fn construction_is_lazy() {
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
    fn namespace_is_safe() {
        assert_eq!(namespace("docs", "find").unwrap(), "mcp.docs.find");
        assert!(namespace("docs", "bad\nname").is_err());
    }

    #[test]
    fn replay_keys_are_canonical() {
        let left = serde_json::json!({"a": 1, "b": 2});
        let right: serde_json::Value = serde_json::from_str("{\"b\":2,\"a\":1}").unwrap();
        assert_eq!(
            replay_key("mcp.s.t", &left).unwrap(),
            replay_key("mcp.s.t", &right).unwrap()
        );
    }

    #[test]
    fn prompt_args_pair() {
        let valid = serde_json::json!({
            "argument_names": ["topic"],
            "argument_values": ["MCP"]
        });
        let parsed = prompt_args(valid.as_object().unwrap()).unwrap().unwrap();
        assert_eq!(parsed["topic"], "MCP");

        let invalid = serde_json::json!({
            "argument_names": ["topic"],
            "argument_values": []
        });
        assert!(prompt_args(invalid.as_object().unwrap()).is_err());
    }
}
