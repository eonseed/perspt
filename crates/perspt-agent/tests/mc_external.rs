//! MCP conformance (Gates K, L, U): admission against real transports.
//!
//! One fixture server declares an honest read-only tool, an over-privileged
//! tool, and a network tool the session withholds. On both transports the
//! same triple must hold: honest admitted and executable, over-privileged
//! rejected, withheld rejected — and a replay lifecycle answers from
//! recorded observations without reconnecting.

use std::collections::BTreeMap;
use std::collections::VecDeque;

use perspt_agent::{ExternalToolResult, ExternalToolRuntime};
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
        vec![EffectKind::ReadFile, EffectKind::Search, EffectKind::List],
    )
}

async fn assert_admission_triple(runtime: &mut ExternalToolRuntime) {
    let admitted = runtime.discover_server("fixture").await.unwrap();
    let names: Vec<&str> = admitted.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(
        names,
        ["mcp.fixture.echo"],
        "only the honest read-only tool is admitted: {names:?}"
    );
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
async fn stdio_admission_triple_and_replay_without_reinvocation() {
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
        return concat!(
            "HTTP/1.1 202 Accepted\r\nmcp-session-id: fixture-session\r\n",
            "content-length: 0\r\n\r\n"
        )
        .into();
    };
    let result = match method {
        "initialize" => serde_json::json!({
            "protocolVersion": "2025-11-25",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "fixture-http", "version": "0"},
        }),
        "tools/list" => serde_json::json!({
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
                "result": {"content": [{"type": "text", "text": format!("echo: {text}")}],
                            "isError": false},
            });
            // SSE framing exercises the stream path end to end.
            let body = format!("event: message\ndata: {payload}\n\n");
            return format!(
                "HTTP/1.1 200 OK\r\nmcp-session-id: fixture-session\r\ncontent-type: \
                 text/event-stream\r\ncontent-length: {}\r\n\r\n{}",
                body.len(),
                body
            );
        }
        _ => serde_json::json!({}),
    };
    let payload = serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string();
    format!(
        "HTTP/1.1 200 OK\r\nmcp-session-id: fixture-session\r\ncontent-type: \
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
