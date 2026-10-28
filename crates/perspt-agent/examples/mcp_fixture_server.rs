//! Conformance fixture: minimal newline-delimited JSON-RPC MCP server in Rust.
//!
//! Declares three baseline tools (`echo`, `shell`, `fetch`) plus optional
//! `--complete` tools (`client_roundtrip`, `async_task`) and `--legacy`
//! protocol version negotiation for `tests/mc_external.rs`.

use std::io::{self, BufRead, Write};

use serde_json::{json, Value};

const PROTOCOL_VERSION: &str = "2026-07-28";
const LEGACY_VERSION: &str = "2025-11-25";

struct ServerMode {
    supported_version: &'static str,
    complete: bool,
}

fn parse_mode() -> ServerMode {
    let args: Vec<String> = std::env::args().collect();
    let legacy = args.iter().any(|arg| arg == "--legacy");
    let complete = args.iter().any(|arg| arg == "--complete");
    ServerMode {
        supported_version: if legacy {
            LEGACY_VERSION
        } else {
            PROTOCOL_VERSION
        },
        complete,
    }
}

fn tool_list(complete: bool) -> Value {
    let mut tools = vec![
        json!({
            "name": "echo",
            "description": "Echo the provided text back",
            "inputSchema": {
                "type": "object",
                "properties": {"text": {"type": "string", "description": "Text to echo"}},
                "required": ["text"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "shell",
            "description": "Run an arbitrary command (over-privileged on purpose)",
            "inputSchema": {
                "type": "object",
                "properties": {"command": {"type": "string", "description": "Command"}},
                "required": ["command"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "fetch",
            "description": "Fetch a URL (network effect the session withholds)",
            "inputSchema": {
                "type": "object",
                "properties": {"url": {"type": "string", "description": "URL"}},
                "required": ["url"],
                "additionalProperties": false
            }
        }),
    ];
    if complete {
        tools.push(json!({
            "name": "client_roundtrip",
            "description": "Exercise roots, sampling, and elicitation",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false}
        }));
        tools.push(json!({
            "name": "async_task",
            "description": "Return a completed asynchronous task",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false}
        }));
    }
    Value::Array(tools)
}

fn respond(request_id: &Value, result: Value) {
    let mut out = io::stdout().lock();
    let _ = writeln!(
        out,
        "{}",
        json!({"jsonrpc": "2.0", "id": request_id, "result": result})
    );
    let _ = out.flush();
}

fn respond_error(request_id: &Value, code: i64, message: &str) {
    let mut out = io::stdout().lock();
    let _ = writeln!(
        out,
        "{}",
        json!({"jsonrpc": "2.0", "id": request_id, "error": {"code": code, "message": message}})
    );
    let _ = out.flush();
}

fn client_request(
    lines: &mut impl Iterator<Item = io::Result<String>>,
    request_id: &str,
    method: &str,
    params: Option<Value>,
) -> Value {
    let mut msg = json!({"jsonrpc": "2.0", "id": request_id, "method": method});
    if let Some(p) = params {
        msg["params"] = p;
    }
    {
        let mut out = io::stdout().lock();
        let _ = writeln!(out, "{msg}");
        let _ = out.flush();
    }
    for line in lines.flatten() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(resp) = serde_json::from_str::<Value>(trimmed) {
            if resp.get("id").and_then(Value::as_str) == Some(request_id) {
                return resp;
            }
        }
    }
    Value::Null
}

fn handle_client_roundtrip(
    lines: &mut impl Iterator<Item = io::Result<String>>,
    request_id: &Value,
) {
    let sample = client_request(
        lines,
        "sample-1",
        "sampling/createMessage",
        Some(json!({
            "messages": [{"role": "user", "content": {"type": "text", "text": "sample"}}],
            "maxTokens": 16,
            "temperature": 0.0,
            "stopSequences": ["stop"]
        })),
    );
    let roots = client_request(lines, "roots-1", "roots/list", None);
    let elicited = client_request(
        lines,
        "elicit-1",
        "elicitation/create",
        Some(json!({
            "mode": "form",
            "message": "Confirm the fixture",
            "requestedSchema": {
                "type": "object",
                "properties": {"confirmed": {"type": "boolean"}},
                "required": ["confirmed"]
            }
        })),
    );
    let root_ok =
        roots.pointer("/result/roots/0/uri").and_then(Value::as_str) == Some("file:///workspace");
    let sample_ok =
        sample.pointer("/result/model").and_then(Value::as_str) == Some("fixture-model");
    let elicit_ok = elicited.pointer("/result/action").and_then(Value::as_str) == Some("accept");
    let ok = root_ok && sample_ok && elicit_ok;
    let text = if ok {
        "client roundtrip ok".to_string()
    } else {
        json!({"roots": roots, "sample": sample, "elicited": elicited}).to_string()
    };
    respond(
        request_id,
        json!({
            "resultType": "complete",
            "content": [{"type": "text", "text": text}],
            "isError": !ok
        }),
    );
}

fn handle_tool_call(
    lines: &mut impl Iterator<Item = io::Result<String>>,
    mode: &ServerMode,
    request_id: &Value,
    params: &Value,
) {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    match name {
        "echo" => {
            let text = args.get("text").and_then(Value::as_str).unwrap_or("");
            respond(
                request_id,
                json!({
                    "resultType": "complete",
                    "content": [{"type": "text", "text": format!("echo: {text}")}],
                    "isError": false
                }),
            );
        }
        "client_roundtrip" if mode.complete => handle_client_roundtrip(lines, request_id),
        "async_task" if mode.complete => respond(
            request_id,
            json!({
                "resultType": "task",
                "taskId": "task-1",
                "status": "working",
                "createdAt": "2026-08-21T00:00:00Z",
                "lastUpdatedAt": "2026-08-21T00:00:00Z",
                "ttlMs": 60000,
                "pollIntervalMs": 1
            }),
        ),
        _ => respond(
            request_id,
            json!({
                "resultType": "complete",
                "content": [{"type": "text", "text": "unexpected call"}],
                "isError": true
            }),
        ),
    }
}

fn handle_resource_method(method: &str, request_id: &Value, params: &Value) -> bool {
    match method {
        "resources/list" => respond(
            request_id,
            json!({
                "resultType": "complete",
                "resources": [
                    {"uri": "file:///fixture.txt", "name": "Fixture", "mimeType": "text/plain"}
                ],
                "ttlMs": 0,
                "cacheScope": "private"
            }),
        ),
        "resources/templates/list" => respond(
            request_id,
            json!({
                "resultType": "complete",
                "resourceTemplates": [
                    {"uriTemplate": "file:///fixtures/{name}", "name": "Fixture template"}
                ],
                "ttlMs": 0,
                "cacheScope": "private"
            }),
        ),
        "resources/read" => {
            let uri = params.get("uri").and_then(Value::as_str).unwrap_or("");
            respond(
                request_id,
                json!({"contents": [{"uri": uri, "mimeType": "text/plain", "text": "fixture resource"}]}),
            );
        }
        _ => return false,
    }
    true
}

fn handle_context_method(method: &str, request_id: &Value, params: &Value) -> bool {
    if handle_resource_method(method, request_id, params) {
        return true;
    }
    match method {
        "prompts/list" => respond(
            request_id,
            json!({
                "resultType": "complete",
                "prompts": [{"name": "review", "description": "Review input"}],
                "ttlMs": 0,
                "cacheScope": "private"
            }),
        ),
        "prompts/get" => {
            let topic = params
                .pointer("/arguments/topic")
                .and_then(Value::as_str)
                .unwrap_or("fixture");
            respond(
                request_id,
                json!({
                    "description": "Fixture prompt",
                    "messages": [
                        {"role": "user", "content": {"type": "text", "text": format!("review {topic}")}}
                    ]
                }),
            );
        }
        "completion/complete" => respond(
            request_id,
            json!({"completion": {"values": ["fixture"], "total": 1, "hasMore": false}}),
        ),
        "tasks/get" => respond(
            request_id,
            json!({
                "resultType": "complete",
                "taskId": "task-1",
                "status": "completed",
                "createdAt": "2026-08-21T00:00:00Z",
                "lastUpdatedAt": "2026-08-21T00:00:01Z",
                "ttlMs": 60000,
                "result": {"content": [{"type": "text", "text": "task complete"}], "isError": false}
            }),
        ),
        _ => return false,
    }
    true
}

fn dispatch_request(
    lines: &mut impl Iterator<Item = io::Result<String>>,
    mode: &ServerMode,
    request: &Value,
) {
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let Some(request_id) = request.get("id") else {
        return;
    };
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
    match method {
        "server/discover" => {
            let capabilities = if mode.complete {
                json!({
                    "tools": {},
                    "resources": {"subscribe": true, "listChanged": true},
                    "prompts": {"listChanged": true},
                    "completions": {},
                    "extensions": {"io.modelcontextprotocol/tasks": {}}
                })
            } else {
                json!({"tools": {}})
            };
            respond(
                request_id,
                json!({
                    "resultType": "complete",
                    "supportedVersions": [mode.supported_version],
                    "capabilities": capabilities,
                    "ttlMs": 0,
                    "cacheScope": "private"
                }),
            );
        }
        "tools/list" => respond(
            request_id,
            json!({
                "resultType": "complete",
                "tools": tool_list(mode.complete),
                "ttlMs": 0,
                "cacheScope": "private"
            }),
        ),
        "tools/call" => handle_tool_call(lines, mode, request_id, &params),
        _ if mode.complete && handle_context_method(method, request_id, &params) => {}
        _ => respond_error(request_id, -32601, "method not found"),
    }
}

fn main() {
    let mode = parse_mode();
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    while let Some(Ok(line)) = lines.next() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(request) = serde_json::from_str::<Value>(trimmed) {
            dispatch_request(&mut lines, &mode, &request);
        }
    }
}
