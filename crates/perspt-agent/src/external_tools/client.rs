//! Modern MCP 2026-07-28 client built on the official Rust SDK.

#![allow(deprecated)]

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, CancelledNotificationParam,
    ClientCapabilities, ClientInfo, ConstString, CreateMessageRequestParams, CreateMessageResult,
    CustomNotification, CustomRequest, CustomResult, ElicitRequestParams, ElicitResult,
    ElicitationCapability, FormElicitationCapability, GetPromptRequestParams, GetPromptResult,
    GetTaskParams, Implementation, ListRootsResult, LoggingMessageNotificationParam,
    ProgressNotificationParam, ProtocolVersion, ReadResourceRequestParams, ReadResourceResult,
    ResourceUpdatedNotificationParam, Root, SamplingCapability, ServerNotification,
    SubscriptionFilter, SubscriptionsAcknowledgedNotificationParams, TaskPayload,
    UrlElicitationCapability, TASKS_EXTENSION_ID,
};
use rmcp::service::{NotificationContext, RequestContext, RoleClient, RunningService};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::{StreamableHttpClientTransport, TokioChildProcess};
use rmcp::{ClientHandler, ClientLifecycleMode, ClientServiceExt, ErrorData as McpError};
use tokio::io::AsyncReadExt;

use perspt_core::{ExternalToolConfig, ExternalToolTransport};
use perspt_sdk::{
    Conversation, GenerationOptions, Message, ModelId, ModelTransport, ProviderToolCall,
    ToolChoicePolicy, ToolSpec, TurnOutput,
};

pub const MCP_PROTOCOL_VERSION: &str = "2026-07-28";

/// Product-owned model service for server-initiated MCP sampling.
#[async_trait]
pub trait McpSamplingProvider: Send + Sync {
    async fn create_message(
        &self,
        server_id: &str,
        request: CreateMessageRequestParams,
    ) -> Result<CreateMessageResult>;
}

/// Product-owned interaction service for server-initiated MCP elicitation.
#[async_trait]
pub trait McpElicitationProvider: Send + Sync {
    async fn elicit(&self, server_id: &str, request: ElicitRequestParams) -> Result<ElicitResult>;
}

/// Non-interactive agent policy: acknowledge the protocol request and decline
/// it immediately instead of hanging a run that has no attached input UI.
#[derive(Debug, Default)]
pub struct DecliningMcpElicitationProvider;

#[async_trait]
impl McpElicitationProvider for DecliningMcpElicitationProvider {
    async fn elicit(
        &self,
        _server_id: &str,
        _request: ElicitRequestParams,
    ) -> Result<ElicitResult> {
        Ok(ElicitResult::new(rmcp::model::ElicitationAction::Decline))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpElicitationAction {
    Accept,
    Decline,
    Cancel,
}

#[derive(Debug, Clone)]
pub struct McpPendingElicitation {
    pub id: u64,
    pub server_id: String,
    pub request: serde_json::Value,
}

struct ElicitationBrokerInner {
    next_id: AtomicU64,
    pending: Mutex<HashMap<u64, tokio::sync::oneshot::Sender<ElicitResult>>>,
    requests: tokio::sync::mpsc::UnboundedSender<McpPendingElicitation>,
}

/// A product-neutral interactive elicitation bridge. The protocol task waits
/// on a one-shot response while a TUI or SDK host drains requests and answers
/// them explicitly.
#[derive(Clone)]
pub struct McpElicitationBroker {
    inner: Arc<ElicitationBrokerInner>,
    receiver: Arc<Mutex<tokio::sync::mpsc::UnboundedReceiver<McpPendingElicitation>>>,
}

impl Default for McpElicitationBroker {
    fn default() -> Self {
        Self::new()
    }
}

impl McpElicitationBroker {
    pub fn new() -> Self {
        let (requests, receiver) = tokio::sync::mpsc::unbounded_channel();
        Self {
            inner: Arc::new(ElicitationBrokerInner {
                next_id: AtomicU64::new(1),
                pending: Mutex::new(HashMap::new()),
                requests,
            }),
            receiver: Arc::new(Mutex::new(receiver)),
        }
    }

    pub fn try_next(&self) -> Option<McpPendingElicitation> {
        self.receiver
            .lock()
            .expect("MCP elicitation receiver poisoned")
            .try_recv()
            .ok()
    }

    pub fn respond(
        &self,
        id: u64,
        action: McpElicitationAction,
        content: Option<serde_json::Value>,
    ) -> Result<()> {
        let sender = self
            .inner
            .pending
            .lock()
            .expect("MCP elicitation map poisoned")
            .remove(&id)
            .with_context(|| format!("unknown or already answered MCP elicitation {id}"))?;
        let action = match action {
            McpElicitationAction::Accept => rmcp::model::ElicitationAction::Accept,
            McpElicitationAction::Decline => rmcp::model::ElicitationAction::Decline,
            McpElicitationAction::Cancel => rmcp::model::ElicitationAction::Cancel,
        };
        let mut response = ElicitResult::new(action);
        if let Some(content) = content {
            response = response.with_content(content);
        }
        sender
            .send(response)
            .map_err(|_| anyhow::anyhow!("MCP elicitation requester disconnected"))
    }
}

#[async_trait]
impl McpElicitationProvider for McpElicitationBroker {
    async fn elicit(&self, server_id: &str, request: ElicitRequestParams) -> Result<ElicitResult> {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let request_value = serde_json::to_value(&request)?;
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.inner
            .pending
            .lock()
            .expect("MCP elicitation map poisoned")
            .insert(id, sender);
        if self
            .inner
            .requests
            .send(McpPendingElicitation {
                id,
                server_id: server_id.to_string(),
                request: request_value,
            })
            .is_err()
        {
            self.inner
                .pending
                .lock()
                .expect("MCP elicitation map poisoned")
                .remove(&id);
            anyhow::bail!("MCP elicitation UI is unavailable");
        }
        receiver
            .await
            .context("MCP elicitation was cancelled because the UI closed")
    }
}

#[derive(Clone, Default)]
pub struct McpClientServices {
    pub sampling: Option<Arc<dyn McpSamplingProvider>>,
    pub elicitation: Option<Arc<dyn McpElicitationProvider>>,
}

/// Sampling adapter used by the governed agent. The requesting server may
/// suggest a model, but Perspt keeps route selection local and uses the model
/// already selected for the product lifecycle.
pub struct ModelTransportSamplingProvider {
    transport: Arc<dyn ModelTransport>,
    model: ModelId,
}

impl ModelTransportSamplingProvider {
    pub fn new(transport: Arc<dyn ModelTransport>, model: ModelId) -> Self {
        Self { transport, model }
    }
}

#[async_trait]
impl McpSamplingProvider for ModelTransportSamplingProvider {
    async fn create_message(
        &self,
        _server_id: &str,
        request: CreateMessageRequestParams,
    ) -> Result<CreateMessageResult> {
        request.validate().map_err(|error| anyhow::anyhow!(error))?;
        anyhow::ensure!(
            request.include_context.is_none()
                || request.include_context == Some(rmcp::model::ContextInclusion::None),
            "MCP sampling includeContext is not advertised by Perspt"
        );
        let generation = GenerationOptions {
            max_tokens: Some(request.max_tokens),
            temperature: request.temperature,
            stop_sequences: request.stop_sequences.clone().unwrap_or_default(),
        };
        let mut conversation = request
            .system_prompt
            .as_deref()
            .map(Conversation::with_system)
            .unwrap_or_default();
        for message in request.messages {
            append_sample(&mut conversation, message)?;
        }
        let tools = request
            .tools
            .unwrap_or_default()
            .into_iter()
            .map(|tool| ToolSpec {
                name: tool.name.into_owned(),
                description: tool
                    .description
                    .map(|description| description.into_owned())
                    .unwrap_or_default(),
                schema: serde_json::Value::Object(tool.input_schema.as_ref().clone()),
                strict: false,
            })
            .collect::<Vec<_>>();
        let choice = request
            .tool_choice
            .and_then(|choice| choice.mode)
            .map(|mode| match mode {
                rmcp::model::ToolChoiceMode::Required => ToolChoicePolicy::Required,
                rmcp::model::ToolChoiceMode::None => ToolChoicePolicy::None,
                _ => ToolChoicePolicy::Auto,
            })
            .unwrap_or(ToolChoicePolicy::Auto);
        let output = self
            .transport
            .chat_turn_with_options(&self.model, &conversation, &tools, choice, generation)
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        sampling_result(output, self.model.to_string())
    }
}

fn append_sample(
    conversation: &mut Conversation,
    message: rmcp::model::SamplingMessage,
) -> Result<()> {
    let mut texts = Vec::new();
    let mut calls = Vec::new();
    let mut results = Vec::new();
    for content in message.content.into_vec() {
        match content {
            rmcp::model::SamplingMessageContentBlock::Text(text) => texts.push(text.text),
            rmcp::model::SamplingMessageContentBlock::ToolUse(tool) => {
                calls.push(ProviderToolCall {
                    call_id: tool.id,
                    name: tool.name,
                    arguments: serde_json::Value::Object(tool.input),
                });
            }
            rmcp::model::SamplingMessageContentBlock::ToolResult(result) => {
                results.push((result.tool_use_id, serde_json::to_string(&result.content)?));
            }
            rmcp::model::SamplingMessageContentBlock::Image(_)
            | rmcp::model::SamplingMessageContentBlock::Audio(_) => {
                anyhow::bail!(
                    "the Perspt model transport does not accept MCP media sampling content"
                )
            }
            _ => anyhow::bail!("unknown MCP sampling content block"),
        }
    }
    if !texts.is_empty() {
        conversation.push(match message.role {
            rmcp::model::Role::User => Message::User {
                content: texts.join("\n"),
            },
            rmcp::model::Role::Assistant => Message::Assistant {
                content: texts.join("\n"),
            },
        });
    }
    if !calls.is_empty() {
        conversation.push_tool_calls(calls);
    }
    for (call_id, content) in results {
        conversation.push_tool_response(call_id, content);
    }
    Ok(())
}

fn sampling_result(output: TurnOutput, model: String) -> Result<CreateMessageResult> {
    let result = match output {
        TurnOutput::Text(text) => {
            CreateMessageResult::new(rmcp::model::SamplingMessage::assistant_text(text), model)
                .with_stop_reason(CreateMessageResult::STOP_REASON_END_TURN)
        }
        TurnOutput::ToolCalls(calls) => {
            let contents = calls
                .into_iter()
                .map(|call| {
                    let input = call.arguments.as_object().cloned().with_context(|| {
                        format!(
                            "sampling tool call {:?} arguments were not an object",
                            call.name
                        )
                    })?;
                    Ok(rmcp::model::SamplingMessageContentBlock::tool_use(
                        call.call_id,
                        call.name,
                        input,
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            CreateMessageResult::new(
                rmcp::model::SamplingMessage::new_multiple(rmcp::model::Role::Assistant, contents),
                model,
            )
            .with_stop_reason(CreateMessageResult::STOP_REASON_TOOL_USE)
        }
    };
    result.validate().map_err(anyhow::Error::msg)?;
    Ok(result)
}

impl std::fmt::Debug for McpClientServices {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpClientServices")
            .field("sampling", &self.sampling.is_some())
            .field("elicitation", &self.elicitation.is_some())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub enum McpServerEvent {
    Progress(serde_json::Value),
    Log(serde_json::Value),
    ResourceUpdated(serde_json::Value),
    ResourcesChanged,
    ToolsChanged,
    PromptsChanged,
    TaskChanged(serde_json::Value),
    Cancelled(serde_json::Value),
    SubscriptionAcknowledged(serde_json::Value),
    CustomNotification {
        method: String,
        params: Option<serde_json::Value>,
    },
    SubscriptionEnded(String),
}

#[derive(Clone)]
struct PersptClientHandler {
    server_id: String,
    info: ClientInfo,
    roots: Vec<Root>,
    max_sampling_tokens: u32,
    services: McpClientServices,
    events: tokio::sync::mpsc::UnboundedSender<McpServerEvent>,
}

impl PersptClientHandler {
    fn emit<T: serde::Serialize>(
        &self,
        constructor: impl FnOnce(serde_json::Value) -> McpServerEvent,
        value: &T,
    ) {
        if let Ok(value) = serde_json::to_value(value) {
            let _ = self.events.send(constructor(value));
        }
    }

    async fn task_inputs(
        &self,
        requests: rmcp::model::InputRequests,
    ) -> Result<rmcp::model::InputResponses> {
        let mut responses = rmcp::model::InputResponses::new();
        for (key, request) in requests {
            let value = match request {
                rmcp::model::InputRequest::CreateMessage(request) => {
                    anyhow::ensure!(
                        request.params.max_tokens <= self.max_sampling_tokens,
                        "task sampling request exceeded local token cap"
                    );
                    let provider = self
                        .services
                        .sampling
                        .as_ref()
                        .context("task requested sampling but sampling is disabled")?;
                    serde_json::to_value(
                        provider
                            .create_message(&self.server_id, request.params)
                            .await?,
                    )?
                }
                rmcp::model::InputRequest::Elicitation(request) => {
                    let provider = self
                        .services
                        .elicitation
                        .as_ref()
                        .context("task requested elicitation but elicitation is disabled")?;
                    serde_json::to_value(provider.elicit(&self.server_id, request.params).await?)?
                }
                rmcp::model::InputRequest::ListRoots(_) => {
                    serde_json::to_value(ListRootsResult::new(self.roots.clone()))?
                }
                _ => anyhow::bail!("MCP task requested an unknown input capability"),
            };
            responses.insert(key, value);
        }
        Ok(responses)
    }
}

impl ClientHandler for PersptClientHandler {
    async fn create_message(
        &self,
        request: CreateMessageRequestParams,
        _context: RequestContext<RoleClient>,
    ) -> std::result::Result<CreateMessageResult, McpError> {
        if request.max_tokens > self.max_sampling_tokens {
            return Err(McpError::invalid_params(
                format!(
                    "sampling request maxTokens {} exceeds local cap {}",
                    request.max_tokens, self.max_sampling_tokens
                ),
                None,
            ));
        }
        let provider = self.services.sampling.as_ref().ok_or_else(|| {
            McpError::internal_error("MCP sampling provider is unavailable", None)
        })?;
        provider
            .create_message(&self.server_id, request)
            .await
            .map_err(|error| McpError::internal_error(error.to_string(), None))
    }

    async fn list_roots(
        &self,
        _context: RequestContext<RoleClient>,
    ) -> std::result::Result<ListRootsResult, McpError> {
        Ok(ListRootsResult::new(self.roots.clone()))
    }

    async fn create_elicitation(
        &self,
        request: ElicitRequestParams,
        _context: RequestContext<RoleClient>,
    ) -> std::result::Result<ElicitResult, McpError> {
        let provider = self.services.elicitation.as_ref().ok_or_else(|| {
            McpError::method_not_found::<rmcp::model::ElicitationCreateRequestMethod>()
        })?;
        provider
            .elicit(&self.server_id, request)
            .await
            .map_err(|error| McpError::internal_error(error.to_string(), None))
    }

    async fn on_custom_request(
        &self,
        request: CustomRequest,
        context: RequestContext<RoleClient>,
    ) -> std::result::Result<CustomResult, McpError> {
        // rmcp may preserve SEP-2577-deprecated sampling as a custom request
        // under the 2026 lifecycle. Route that current wire method through the
        // same typed handler without negotiating another protocol version.
        if request.method == rmcp::model::CreateMessageRequestMethod::VALUE {
            let params = serde_json::from_value(request.params.unwrap_or_default())
                .map_err(|error| McpError::invalid_params(error.to_string(), None))?;
            let result = self.create_message(params, context).await?;
            return serde_json::to_value(result)
                .map(CustomResult::new)
                .map_err(|error| McpError::internal_error(error.to_string(), None));
        }
        Err(McpError::new(
            rmcp::model::ErrorCode::METHOD_NOT_FOUND,
            request.method,
            None,
        ))
    }

    async fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        self.emit(McpServerEvent::Progress, &params);
    }

    async fn on_cancelled(
        &self,
        params: CancelledNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        self.emit(McpServerEvent::Cancelled, &params);
    }

    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        self.emit(McpServerEvent::Log, &params);
    }

    async fn on_resource_updated(
        &self,
        params: ResourceUpdatedNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        self.emit(McpServerEvent::ResourceUpdated, &params);
    }

    async fn on_resource_list_changed(&self, _context: NotificationContext<RoleClient>) {
        let _ = self.events.send(McpServerEvent::ResourcesChanged);
    }

    async fn on_tool_list_changed(&self, _context: NotificationContext<RoleClient>) {
        let _ = self.events.send(McpServerEvent::ToolsChanged);
    }

    async fn on_prompt_list_changed(&self, _context: NotificationContext<RoleClient>) {
        let _ = self.events.send(McpServerEvent::PromptsChanged);
    }

    async fn on_task_status(
        &self,
        params: rmcp::model::TaskStatusNotificationParams,
        _context: NotificationContext<RoleClient>,
    ) {
        self.emit(McpServerEvent::TaskChanged, &params);
    }

    async fn on_subscriptions_acknowledged(
        &self,
        params: SubscriptionsAcknowledgedNotificationParams,
        _context: NotificationContext<RoleClient>,
    ) {
        self.emit(McpServerEvent::SubscriptionAcknowledged, &params);
    }

    async fn on_custom_notification(
        &self,
        notification: CustomNotification,
        _context: NotificationContext<RoleClient>,
    ) {
        let _ = self.events.send(McpServerEvent::CustomNotification {
            method: notification.method,
            params: notification.params,
        });
    }

    fn get_info(&self) -> ClientInfo {
        self.info.clone()
    }
}

pub struct McpConnection {
    client: RunningService<RoleClient, PersptClientHandler>,
    events: tokio::sync::mpsc::UnboundedReceiver<McpServerEvent>,
    subscription: Option<tokio::task::JoinHandle<()>>,
    stderr: Arc<Mutex<Vec<u8>>>,
    timeout: Duration,
    max_result_bytes: usize,
    tasks: bool,
    task_timeout: Duration,
}

impl std::fmt::Debug for McpConnection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpConnection")
            .field("peer", &self.client.peer_info())
            .finish_non_exhaustive()
    }
}

impl McpConnection {
    pub async fn connect(config: &ExternalToolConfig, services: McpClientServices) -> Result<Self> {
        anyhow::ensure!(
            !config.sampling || services.sampling.is_some(),
            "MCP server {:?} enables sampling but this product installed no sampling provider",
            config.id
        );
        anyhow::ensure!(
            !config.elicitation || services.elicitation.is_some(),
            "MCP server {:?} enables elicitation but this product installed no interactive provider",
            config.id
        );

        let roots = roots_from(config);
        let (event_tx, events) = tokio::sync::mpsc::unbounded_channel();
        let handler = PersptClientHandler {
            server_id: config.id.clone(),
            info: client_info(config, !roots.is_empty()),
            roots,
            max_sampling_tokens: config.max_sampling_tokens,
            services,
            events: event_tx.clone(),
        };
        let stderr = Arc::new(Mutex::new(Vec::new()));
        let client = match config.transport {
            ExternalToolTransport::Stdio => {
                connect_stdio(config, handler.clone(), stderr.clone()).await?
            }
            ExternalToolTransport::StreamableHttp => connect_http(config, handler).await?,
        };

        let negotiated = client
            .peer_info()
            .context("MCP discover omitted peer information")?;
        anyhow::ensure!(
            negotiated.protocol_version == ProtocolVersion::V_2026_07_28,
            "MCP server did not negotiate required protocol {MCP_PROTOCOL_VERSION}"
        );
        let tasks = config.tasks
            && negotiated
                .capabilities
                .extensions
                .as_ref()
                .is_some_and(|extensions| extensions.contains_key(TASKS_EXTENSION_ID));

        let mut connection = Self {
            client,
            events,
            subscription: None,
            stderr,
            timeout: Duration::from_millis(config.timeout_ms),
            max_result_bytes: config.max_result_bytes,
            tasks,
            task_timeout: Duration::from_millis(config.max_task_wait_ms),
        };
        if config.subscriptions {
            connection
                .start_subscription(&config.resource_subscriptions, event_tx)
                .await?;
        }
        Ok(connection)
    }

    pub fn peer_info(&self) -> Option<Arc<rmcp::model::ServerPeerInfo>> {
        self.client.peer_info()
    }

    pub fn drain_events(&mut self) -> Vec<McpServerEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.events.try_recv() {
            events.push(event);
        }
        events
    }

    pub async fn list_tools(&self) -> Result<Vec<rmcp::model::Tool>> {
        self.bounded(self.client.list_all_tools()).await
    }

    pub async fn list_resources(&self) -> Result<Vec<rmcp::model::Resource>> {
        self.bounded(self.client.list_all_resources()).await
    }

    pub async fn list_resource_templates(&self) -> Result<Vec<rmcp::model::ResourceTemplate>> {
        self.bounded(self.client.list_all_resource_templates())
            .await
    }

    pub async fn read_resource(&self, uri: String) -> Result<ReadResourceResult> {
        self.bounded(
            self.client
                .read_resource(ReadResourceRequestParams::new(uri)),
        )
        .await
    }

    pub async fn list_prompts(&self) -> Result<Vec<rmcp::model::Prompt>> {
        self.bounded(self.client.list_all_prompts()).await
    }

    pub async fn get_prompt(
        &self,
        name: String,
        arguments: Option<rmcp::model::JsonObject>,
    ) -> Result<GetPromptResult> {
        let mut request = GetPromptRequestParams::new(name);
        if let Some(arguments) = arguments {
            request = request.with_arguments(arguments);
        }
        self.bounded(self.client.get_prompt(request)).await
    }

    pub async fn complete(
        &self,
        request: rmcp::model::CompleteRequestParams,
    ) -> Result<rmcp::model::CompleteResult> {
        self.bounded(self.client.complete(request)).await
    }

    pub async fn set_log_level(&self, level: rmcp::model::LoggingLevel) -> Result<()> {
        self.request(
            self.client
                .set_level(rmcp::model::SetLevelRequestParams::new(level)),
        )
        .await
    }

    pub async fn call_tool(
        &self,
        name: String,
        arguments: rmcp::model::JsonObject,
    ) -> Result<CallToolResult> {
        let mut request = CallToolRequestParams::new(name).with_arguments(arguments);
        if !self.tasks {
            return self.bounded(self.client.call_tool(request)).await;
        }
        let handler = self.client.service().clone();
        let mut state_only_rounds = 0u32;
        for _ in 0..rmcp::model::DEFAULT_MRTR_MAX_ROUNDS {
            match self
                .request(self.client.call_tool_once(request.clone()))
                .await?
            {
                CallToolResponse::Complete(result) => return self.ensure_size(result),
                CallToolResponse::InputRequired(input) => {
                    let had_requests = input
                        .input_requests
                        .as_ref()
                        .is_some_and(|requests| !requests.is_empty());
                    anyhow::ensure!(
                        had_requests || input.request_state.is_some(),
                        "MCP MRTR response contained neither requests nor state"
                    );
                    let responses = handler
                        .task_inputs(input.input_requests.unwrap_or_default())
                        .await?;
                    request.input_responses = (!responses.is_empty()).then_some(responses);
                    request.request_state = input.request_state;
                    if had_requests {
                        state_only_rounds = 0;
                    } else {
                        let delay = 25u64.saturating_mul(1u64 << state_only_rounds.min(7));
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                        state_only_rounds += 1;
                    }
                }
                CallToolResponse::Task(task) => {
                    let task_id = task.task.task_id;
                    return match tokio::time::timeout(
                        self.task_timeout,
                        self.wait_for_task(task_id.clone()),
                    )
                    .await
                    {
                        Ok(result) => result,
                        Err(_) => {
                            let _ = self
                                .request(
                                    self.client
                                        .cancel_task(rmcp::model::CancelTaskParams::new(task_id)),
                                )
                                .await;
                            anyhow::bail!("MCP asynchronous task exceeded max_task_wait_ms")
                        }
                    };
                }
                _ => anyhow::bail!("MCP server returned an unknown tools/call response"),
            }
        }
        anyhow::bail!("MCP tool exceeded the bounded MRTR round limit")
    }

    async fn wait_for_task(&self, task_id: String) -> Result<CallToolResult> {
        let handler = self.client.service().clone();
        loop {
            let state = self
                .request(self.client.get_task(GetTaskParams::new(task_id.clone())))
                .await?;
            let delay = state
                .task
                .task
                .poll_interval_ms
                .unwrap_or(500)
                .clamp(50, 30_000);
            match state.task.payload {
                TaskPayload::Working => tokio::time::sleep(Duration::from_millis(delay)).await,
                TaskPayload::InputRequired { input_requests } => {
                    let responses = handler.task_inputs(input_requests).await?;
                    self.request(self.client.update_task(rmcp::model::UpdateTaskParams::new(
                        task_id.clone(),
                        responses,
                    )))
                    .await?;
                }
                TaskPayload::Completed { result } => {
                    return self
                        .ensure_size(serde_json::from_value(serde_json::Value::Object(result))?);
                }
                TaskPayload::Failed { error } => {
                    anyhow::bail!("MCP task failed: {}", serde_json::Value::Object(error))
                }
                TaskPayload::Cancelled => anyhow::bail!("MCP task was cancelled"),
                _ => anyhow::bail!("MCP task returned an unknown state"),
            }
        }
    }

    async fn start_subscription(
        &mut self,
        resources: &[String],
        event_tx: tokio::sync::mpsc::UnboundedSender<McpServerEvent>,
    ) -> Result<()> {
        let capabilities = &self
            .client
            .peer_info()
            .context("MCP peer information unavailable")?
            .capabilities;
        let requested = SubscriptionFilter::builder()
            .tools_list_changed()
            .prompts_list_changed()
            .resources_list_changed()
            .resource_subscriptions(resources.iter().cloned())
            .build();
        let supported = requested.supported_by(capabilities);
        if supported == SubscriptionFilter::default() {
            return Ok(());
        }
        let mut subscription = self.request(self.client.listen(supported)).await?;
        self.subscription = Some(tokio::spawn(async move {
            loop {
                match subscription.next().await {
                    Ok(Some(notification)) => emit_subscription(&event_tx, notification),
                    Ok(None) => {
                        let _ = event_tx.send(McpServerEvent::SubscriptionEnded(format!(
                            "{:?}",
                            subscription.end()
                        )));
                        break;
                    }
                    Err(error) => {
                        let _ = event_tx.send(McpServerEvent::SubscriptionEnded(error.to_string()));
                        break;
                    }
                }
            }
        }));
        Ok(())
    }

    async fn request<T, E>(
        &self,
        future: impl std::future::Future<Output = std::result::Result<T, E>>,
    ) -> Result<T>
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        tokio::time::timeout(self.timeout, future)
            .await
            .context("MCP request timed out")?
            .map_err(anyhow::Error::new)
    }

    async fn bounded<T, E>(
        &self,
        future: impl std::future::Future<Output = std::result::Result<T, E>>,
    ) -> Result<T>
    where
        T: serde::Serialize,
        E: std::error::Error + Send + Sync + 'static,
    {
        let value = self.request(future).await?;
        self.ensure_size(value)
    }

    fn ensure_size<T: serde::Serialize>(&self, value: T) -> Result<T> {
        anyhow::ensure!(
            serde_json::to_vec(&value)?.len() <= self.max_result_bytes,
            "MCP result exceeded byte cap"
        );
        Ok(value)
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(subscription) = self.subscription.take() {
            subscription.abort();
        }
        self.client
            .close_with_timeout(Duration::from_secs(3))
            .await
            .context("joining MCP service")?;
        Ok(())
    }

    pub fn stderr_tail(&self) -> Vec<u8> {
        self.stderr.lock().expect("stderr capture poisoned").clone()
    }
}

fn roots_from(config: &ExternalToolConfig) -> Vec<Root> {
    config
        .roots
        .iter()
        .map(|root| match &root.name {
            Some(name) => Root::new(root.uri.clone()).with_name(name.clone()),
            None => Root::new(root.uri.clone()),
        })
        .collect()
}

fn client_info(config: &ExternalToolConfig, has_roots: bool) -> ClientInfo {
    let mut capabilities = ClientCapabilities::default();
    if has_roots {
        let mut roots = rmcp::model::RootsCapabilities::default();
        roots.list_changed = Some(false);
        capabilities.roots = Some(roots);
    }
    if config.sampling {
        let mut sampling = SamplingCapability::default();
        sampling.tools = Some(Default::default());
        capabilities.sampling = Some(sampling);
    }
    if config.elicitation {
        capabilities.elicitation = Some(
            ElicitationCapability::new()
                .with_form(FormElicitationCapability::new().with_schema_validation(false))
                .with_url(UrlElicitationCapability::new()),
        );
    }
    if config.tasks {
        capabilities
            .extensions
            .get_or_insert_default()
            .insert(TASKS_EXTENSION_ID.to_string(), Default::default());
    }
    ClientInfo::new(
        capabilities,
        Implementation::new("perspt", env!("CARGO_PKG_VERSION")),
    )
    .with_protocol_version(ProtocolVersion::V_2026_07_28)
}

fn discover_mode() -> ClientLifecycleMode {
    ClientLifecycleMode::Discover {
        preferred_versions: vec![ProtocolVersion::V_2026_07_28],
    }
}

async fn connect_stdio(
    config: &ExternalToolConfig,
    handler: PersptClientHandler,
    stderr: Arc<Mutex<Vec<u8>>>,
) -> Result<RunningService<RoleClient, PersptClientHandler>> {
    let (program, arguments) = config
        .command
        .split_first()
        .context("stdio MCP command is empty")?;
    let mut command = tokio::process::Command::new(program);
    command.args(arguments).env_clear().kill_on_drop(true);
    if let Some(path) = std::env::var_os("PATH") {
        command.env("PATH", path);
    }
    #[cfg(windows)]
    if let Some(system_root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", system_root);
    }
    for (destination, source) in &config.env_from_env {
        let value = std::env::var(source)
            .with_context(|| format!("stdio MCP environment source {source:?} is unavailable"))?;
        command.env(destination, value);
    }
    let (transport, stderr_pipe) = TokioChildProcess::builder(command)
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("starting stdio MCP server {:?}", config.id))?;
    if let Some(stderr_pipe) = stderr_pipe {
        capture_stderr(stderr_pipe, stderr, config.max_stderr_bytes);
    }
    Ok(tokio::time::timeout(
        Duration::from_millis(config.timeout_ms),
        handler.serve_with_lifecycle(transport, discover_mode()),
    )
    .await
    .context("MCP server/discover timed out")??)
}

async fn connect_http(
    config: &ExternalToolConfig,
    handler: PersptClientHandler,
) -> Result<RunningService<RoleClient, PersptClientHandler>> {
    let uri = config.url.as_deref().context("MCP HTTP URL missing")?;
    validate_http_uri(uri)?;
    let mut headers = HashMap::new();
    for (destination, source) in &config.headers_from_env {
        let value = std::env::var(source)
            .with_context(|| format!("MCP HTTP header source {source:?} is unavailable"))?;
        headers.insert(
            reqwest::header::HeaderName::from_bytes(destination.as_bytes())?,
            reqwest::header::HeaderValue::from_str(&value)?,
        );
    }
    let builder = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none());
    let builder = if is_loopback_uri(uri)? {
        builder.no_proxy()
    } else {
        builder
    };
    let transport = StreamableHttpClientTransport::with_client(
        builder.build()?,
        StreamableHttpClientTransportConfig::with_uri(uri)
            .custom_headers(headers)
            .max_sse_event_size(config.max_result_bytes)
            .reinit_on_expired_session(false),
    );
    Ok(tokio::time::timeout(
        Duration::from_millis(config.timeout_ms),
        handler.serve_with_lifecycle(transport, discover_mode()),
    )
    .await
    .context("MCP server/discover timed out")??)
}

fn validate_http_uri(uri: &str) -> Result<()> {
    let url = reqwest::Url::parse(uri)?;
    let loopback = is_loopback_uri(uri)?;
    anyhow::ensure!(
        url.scheme() == "https" || (url.scheme() == "http" && loopback),
        "MCP Streamable HTTP requires HTTPS except for explicit loopback fixtures"
    );
    Ok(())
}

fn is_loopback_uri(uri: &str) -> Result<bool> {
    Ok(reqwest::Url::parse(uri)?
        .host_str()
        .is_some_and(|host| matches!(host, "localhost" | "127.0.0.1" | "[::1]" | "::1")))
}

fn capture_stderr(
    mut pipe: tokio::process::ChildStderr,
    captured: Arc<Mutex<Vec<u8>>>,
    maximum: usize,
) {
    tokio::spawn(async move {
        let mut chunk = [0u8; 4096];
        while let Ok(count) = pipe.read(&mut chunk).await {
            if count == 0 {
                break;
            }
            let mut output = captured.lock().expect("stderr capture poisoned");
            output.extend_from_slice(&chunk[..count]);
            if output.len() > maximum {
                let excess = output.len() - maximum;
                output.drain(..excess);
            }
        }
    });
}

fn emit_subscription(
    sender: &tokio::sync::mpsc::UnboundedSender<McpServerEvent>,
    notification: ServerNotification,
) {
    let event = match notification {
        ServerNotification::ToolListChangedNotification(_) => McpServerEvent::ToolsChanged,
        ServerNotification::PromptListChangedNotification(_) => McpServerEvent::PromptsChanged,
        ServerNotification::ResourceListChangedNotification(_) => McpServerEvent::ResourcesChanged,
        ServerNotification::ResourceUpdatedNotification(notification) => {
            McpServerEvent::ResourceUpdated(
                serde_json::to_value(notification.params).unwrap_or_default(),
            )
        }
        _ => return,
    };
    let _ = sender.send(event);
}
