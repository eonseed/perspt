//! MCP conformance (Gates K, L, U): admission against real transports.
//!
//! One fixture server declares an honest read-only tool, an over-privileged
//! tool, and a network tool the session withholds. On both transports the
//! same triple must hold: honest admitted and executable, over-privileged
//! rejected, withheld rejected — and a replay lifecycle answers from
//! recorded observations without reconnecting.

#![allow(deprecated)]

use std::collections::BTreeMap;
use std::collections::VecDeque;

use perspt_agent::{
    ExternalToolResult, ExternalToolRuntime, McpClientServices, McpElicitationAction,
    McpElicitationBroker, McpSamplingProvider,
};
use perspt_core::{
    Config, ExternalToolConfig, ExternalToolMode, ExternalToolPolicy, ExternalToolTransport,
};
use perspt_sdk::{AccessMode, ActorId, Capability, EffectKind, FootprintSpec, ResourceSelector};

fn fixture_server_path() -> String {
    format!(
        "{}/tests/fixtures/mcp_server.py",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn echo_policy() -> ExternalToolPolicy {
    ExternalToolPolicy {
        effect: Some(EffectKind::Search),
        risk: Some(perspt_sdk::RiskClass::Low),
        footprint: Some(FootprintSpec::new(vec![ResourceSelector::ScopedArgument {
            family: "mcp-fixture".into(),
            field: "text".into(),
            access: AccessMode::Read,
        }])),
        proposal_bindings: Vec::new(),
    }
}

fn fetch_policy() -> ExternalToolPolicy {
    ExternalToolPolicy {
        effect: Some(EffectKind::NetworkFetch),
        risk: Some(perspt_sdk::RiskClass::High),
        footprint: Some(FootprintSpec::opaque()),
        proposal_bindings: Vec::new(),
    }
}

fn stdio_config() -> Config {
    let mut tools = BTreeMap::new();
    tools.insert("echo".to_string(), echo_policy());
    tools.insert("fetch".to_string(), fetch_policy());
    // "shell" gets no policy on purpose: undeclared classifies RunShell.
    Config {
        external_tools: vec![ExternalToolConfig {
            id: "fixture".into(),
            transport: ExternalToolTransport::Stdio,
            command: vec!["python3".into(), fixture_server_path()],
            tools,
            ..ExternalToolConfig::default()
        }],
        ..Config::default()
    }
}

fn session_capability() -> Capability {
    Capability::new(
        ActorId::new("session"),
        vec![
            EffectKind::ReadFile,
            EffectKind::DataRead,
            EffectKind::Search,
            EffectKind::List,
        ],
    )
}

struct FixtureSampler;

#[test]
fn sampling_shape_is_current() {
    let params = serde_json::json!({
        "method": "sampling/createMessage",
        "params": {
            "messages": [
                {"role": "user", "content": {"type": "text", "text": "sample"}}
            ],
            "maxTokens": 16,
            "temperature": 0.0,
            "stopSequences": ["stop"]
        }
    });
    let request: rmcp::model::ServerRequest = serde_json::from_value(params.clone()).unwrap();
    assert!(matches!(
        request,
        rmcp::model::ServerRequest::CreateMessageRequest(_)
    ));
    let message: rmcp::model::ServerJsonRpcMessage = serde_json::from_value(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "sample-1",
        "method": params["method"],
        "params": params["params"],
    }))
    .unwrap();
    assert!(matches!(
        message,
        rmcp::model::ServerJsonRpcMessage::Request(request)
            if matches!(request.request, rmcp::model::ServerRequest::CreateMessageRequest(_))
    ));
}

#[async_trait::async_trait]
impl McpSamplingProvider for FixtureSampler {
    async fn create_message(
        &self,
        _server_id: &str,
        request: rmcp::model::CreateMessageRequestParams,
    ) -> anyhow::Result<rmcp::model::CreateMessageResult> {
        assert_eq!(request.max_tokens, 16);
        assert_eq!(request.stop_sequences, Some(vec!["stop".to_string()]));
        Ok(rmcp::model::CreateMessageResult::new(
            rmcp::model::SamplingMessage::assistant_text("sampled"),
            "fixture-model".to_string(),
        ))
    }
}

async fn assert_admission_triple(runtime: &mut ExternalToolRuntime) {
    let admitted = runtime.discover_server("fixture").await.unwrap();
    let names: Vec<&str> = admitted.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(
        names,
        ["mcp.fixture.echo"],
        "only the honest read-only tool is admitted: {names:?}"
    );
    let rejected = runtime.admission_rejections("fixture");
    assert_eq!(rejected.len(), 2);
    assert!(rejected
        .iter()
        .any(|item| item.remote_tool == "shell" && item.reason.contains("RunShell")));
    assert!(rejected
        .iter()
        .any(|item| item.remote_tool == "fetch" && item.reason.contains("NetworkFetch")));
    let entry = &admitted[0];
    assert!(
        entry.durable,
        "external calls are bracketed external effects"
    );
    assert!(
        entry.description.starts_with("[untrusted MCP description]"),
        "descriptions are untrusted observations"
    );

    let result = runtime
        .call("mcp.fixture.echo", serde_json::json!({"text": "governed"}))
        .await
        .unwrap();
    assert!(!result.is_error);
    assert!(result.content.to_string().contains("echo: governed"));

    // Rejected tools are simply not callable.
    let rejected = runtime
        .call("mcp.fixture.shell", serde_json::json!({"command": "id"}))
        .await;
    assert!(rejected.is_err());
}

#[tokio::test]
async fn stdio_admission_and_replay() {
    let config = stdio_config();
    let mut runtime = ExternalToolRuntime::from_config(
        &config,
        ExternalToolMode::Agent,
        vec![session_capability()],
    )
    .unwrap();
    assert_admission_triple(&mut runtime).await;
    let entries = runtime.admitted_entries();
    runtime.shutdown().await.unwrap();

    // Replay: recorded observations answer; the server is never respawned.
    let mut results: BTreeMap<String, VecDeque<ExternalToolResult>> = BTreeMap::new();
    results.insert(
        perspt_agent::external_tools::replay_key_for(
            "mcp.fixture.echo",
            &serde_json::json!({"text": "governed"}),
        )
        .unwrap(),
        VecDeque::from([ExternalToolResult {
            content: serde_json::json!({"content": [{"type": "text", "text": "echo: governed"}]}),
            is_error: false,
            replayed: false,
        }]),
    );
    let mut replay = ExternalToolRuntime::replay(ExternalToolMode::Agent, entries, results);
    let replayed = replay
        .call("mcp.fixture.echo", serde_json::json!({"text": "governed"}))
        .await
        .unwrap();
    assert!(replayed.replayed, "replay must answer from the record");
    let dry = replay
        .call("mcp.fixture.echo", serde_json::json!({"text": "governed"}))
        .await;
    assert!(dry.is_err(), "replay refuses to invent unrecorded results");
}

#[tokio::test]
async fn legacy_server_is_rejected() {
    let mut config = stdio_config();
    config.external_tools[0].command.push("--legacy".into());
    let mut runtime = ExternalToolRuntime::from_config(
        &config,
        ExternalToolMode::Agent,
        vec![session_capability()],
    )
    .unwrap();
    let error = runtime.discover_server("fixture").await.unwrap_err();
    let message = format!("{error:#}");
    assert!(
        message.contains("2026-07-28") || message.contains("compatible protocol"),
        "{message}"
    );
}

#[tokio::test]
async fn context_and_client_requests() {
    let config = complete_config();
    let broker = McpElicitationBroker::new();
    let mut runtime = complete_runtime(&config, &broker).await;
    let answer = answer_elicitation(broker);
    let roundtrip = runtime
        .call("mcp.fixture.client_roundtrip", serde_json::json!({}))
        .await
        .unwrap();
    answer.await.unwrap();
    assert!(!roundtrip.is_error, "{}", roundtrip.content);
    assert_context_ops(&mut runtime).await;
    runtime.shutdown().await.unwrap();
}

fn complete_config() -> Config {
    let mut config = stdio_config();
    let server = &mut config.external_tools[0];
    server.command.push("--complete".into());
    server.roots.push(perspt_core::McpRootConfig {
        uri: "file:///workspace".into(),
        name: Some("Workspace".into()),
    });
    server.sampling = true;
    server.elicitation = true;
    server.subscriptions = false;
    server
        .tools
        .insert("client_roundtrip".into(), read_policy());
    server.tools.insert("async_task".into(), read_policy());
    config
}

fn read_policy() -> ExternalToolPolicy {
    ExternalToolPolicy {
        effect: Some(EffectKind::Search),
        risk: Some(perspt_sdk::RiskClass::Low),
        footprint: Some(FootprintSpec::default()),
        proposal_bindings: Vec::new(),
    }
}

async fn complete_runtime(config: &Config, broker: &McpElicitationBroker) -> ExternalToolRuntime {
    let mut runtime = ExternalToolRuntime::from_config(
        config,
        ExternalToolMode::Agent,
        vec![session_capability()],
    )
    .unwrap();
    runtime.set_client_services(McpClientServices {
        sampling: Some(std::sync::Arc::new(FixtureSampler)),
        elicitation: Some(std::sync::Arc::new(broker.clone())),
    });
    let entries = runtime.discover_server("fixture").await.unwrap();
    assert_complete_ops(&entries);
    runtime
}

fn assert_complete_ops(entries: &[perspt_sdk::ToolEntry]) {
    let names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
    for expected in [
        "mcp.fixture.client_roundtrip",
        "mcp.fixture.async_task",
        "mcp.fixture._perspt_resources_list",
        "mcp.fixture._perspt_resource_templates_list",
        "mcp.fixture._perspt_resource_read",
        "mcp.fixture._perspt_prompts_list",
        "mcp.fixture._perspt_prompt_get",
        "mcp.fixture._perspt_complete",
    ] {
        assert!(names.contains(&expected), "missing {expected}: {names:?}");
    }
}

fn answer_elicitation(broker: McpElicitationBroker) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            if let Some(request) = broker.try_next() {
                broker
                    .respond(
                        request.id,
                        McpElicitationAction::Accept,
                        Some(serde_json::json!({"confirmed": true})),
                    )
                    .unwrap();
                return;
            }
            tokio::task::yield_now().await;
        }
    })
}

async fn assert_context_ops(runtime: &mut ExternalToolRuntime) {
    let resource = runtime
        .call(
            "mcp.fixture._perspt_resource_read",
            serde_json::json!({"uri": "file:///fixture.txt"}),
        )
        .await
        .unwrap();
    assert!(resource.content.to_string().contains("fixture resource"));
    let prompt = runtime
        .call(
            "mcp.fixture._perspt_prompt_get",
            serde_json::json!({
                "name": "review",
                "argument_names": ["topic"],
                "argument_values": ["fixture"]
            }),
        )
        .await
        .unwrap();
    assert!(prompt.content.to_string().contains("review fixture"));
    let completion = runtime
        .call(
            "mcp.fixture._perspt_complete",
            serde_json::json!({
                "kind": "prompt",
                "reference": "review",
                "argument": "topic",
                "value": "fix"
            }),
        )
        .await
        .unwrap();
    assert!(completion.content.to_string().contains("fixture"));
    let task = runtime
        .call("mcp.fixture.async_task", serde_json::json!({}))
        .await
        .unwrap();
    assert!(task.content.to_string().contains("task complete"));
}

#[tokio::test]
async fn chat_discovery_diagnostics() {
    let mut config = stdio_config();
    config.external_tools[0].modes = vec![ExternalToolMode::Chat];
    let (session, notices) = perspt_agent::external_tools::chat::ChatToolSession::from_config(
        &config,
        std::sync::Arc::new(perspt_core::GenAIProvider::new().unwrap()),
        "fixture-model",
    )
    .await
    .unwrap()
    .expect("chat-enabled server");

    assert_eq!(session.tool_names(), ["mcp.fixture.echo"]);
    assert!(notices
        .iter()
        .any(|notice| notice.contains("1 tool(s) admitted, 2 rejected")));
    assert!(notices
        .iter()
        .any(|notice| notice.contains("fixture.shell rejected")));
}

/// A minimal loopback Streamable-HTTP wrapper over the same tool set:
/// JSON responses for requests, 202 for notifications, SSE for tools/call.
async fn spawn_http_fixture() -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                loop {
                    let mut buffer = Vec::new();
                    let mut chunk = [0u8; 4096];
                    let body = loop {
                        let Ok(n) = socket.read(&mut chunk).await else {
                            return;
                        };
                        if n == 0 {
                            return;
                        }
                        buffer.extend_from_slice(&chunk[..n]);
                        if let Some(split) =
                            buffer.windows(4).position(|window| window == b"\r\n\r\n")
                        {
                            let header = String::from_utf8_lossy(&buffer[..split]).to_string();
                            let length = header
                                .lines()
                                .find_map(|line| {
                                    line.to_ascii_lowercase()
                                        .strip_prefix("content-length:")
                                        .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                                })
                                .unwrap_or(0);
                            let start = split + 4;
                            while buffer.len() < start + length {
                                let Ok(n) = socket.read(&mut chunk).await else {
                                    return;
                                };
                                if n == 0 {
                                    return;
                                }
                                buffer.extend_from_slice(&chunk[..n]);
                            }
                            break buffer[start..start + length].to_vec();
                        }
                    };
                    let request: serde_json::Value =
                        serde_json::from_slice(&body).unwrap_or_default();
                    let response = http_fixture_response(&request);
                    if socket.write_all(response.as_bytes()).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    format!("http://127.0.0.1:{}/mcp", address.port())
}

fn http_fixture_response(request: &serde_json::Value) -> String {
    let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let id = request.get("id").cloned();
    let Some(id) = id else {
        return "HTTP/1.1 202 Accepted\r\ncontent-length: 0\r\n\r\n".into();
    };
    let result = match method {
        "server/discover" => serde_json::json!({
            "resultType": "complete",
            "supportedVersions": ["2026-07-28"],
            "capabilities": {"tools": {}},
            "ttlMs": 0,
            "cacheScope": "private",
        }),
        "tools/list" => serde_json::json!({
            "resultType": "complete",
            "ttlMs": 0,
            "cacheScope": "private",
            "tools": [
                {"name": "echo", "description": "Echo the provided text back",
                 "inputSchema": {"type": "object",
                   "properties": {"text": {"type": "string", "description": "Text"}},
                   "required": ["text"], "additionalProperties": false}},
                {"name": "shell", "description": "Run anything",
                 "inputSchema": {"type": "object",
                   "properties": {"command": {"type": "string", "description": "Cmd"}},
                   "required": ["command"], "additionalProperties": false}},
                {"name": "fetch", "description": "Fetch a URL",
                 "inputSchema": {"type": "object",
                   "properties": {"url": {"type": "string", "description": "URL"}},
                   "required": ["url"], "additionalProperties": false}},
            ]
        }),
        "tools/call" => {
            let text = request
                .pointer("/params/arguments/text")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let payload = serde_json::json!({
                "jsonrpc": "2.0", "id": id,
                "result": {"resultType": "complete",
                            "content": [{"type": "text", "text": format!("echo: {text}")}],
                            "isError": false},
            });
            // SSE framing exercises the stream path end to end.
            let body = format!("event: message\ndata: {payload}\n\n");
            return format!(
                "HTTP/1.1 200 OK\r\ncontent-type: \
                 text/event-stream\r\ncontent-length: {}\r\n\r\n{}",
                body.len(),
                body
            );
        }
        _ => serde_json::json!({}),
    };
    let payload = serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string();
    format!(
        "HTTP/1.1 200 OK\r\ncontent-type: \
         application/json\r\ncontent-length: {}\r\n\r\n{}",
        payload.len(),
        payload
    )
}

#[tokio::test]
async fn streamable_http_admission_triple() {
    let url = spawn_http_fixture().await;
    let mut tools = BTreeMap::new();
    tools.insert("echo".to_string(), echo_policy());
    tools.insert("fetch".to_string(), fetch_policy());
    let config = Config {
        external_tools: vec![ExternalToolConfig {
            id: "fixture".into(),
            transport: ExternalToolTransport::StreamableHttp,
            url: Some(url),
            tools,
            ..ExternalToolConfig::default()
        }],
        ..Config::default()
    };
    let mut runtime = ExternalToolRuntime::from_config(
        &config,
        ExternalToolMode::Agent,
        vec![session_capability()],
    )
    .unwrap();
    assert_admission_triple(&mut runtime).await;
}
