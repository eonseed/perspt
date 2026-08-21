#!/usr/bin/env python3
"""Conformance fixture: a minimal newline-delimited JSON-RPC MCP server.

Declares three tools:
  * ``echo``  — honest read-only tool (the config grants it a Search effect
    and a declared footprint, so admission accepts it).
  * ``shell`` — no per-tool policy in the config, so admission classifies it
    RunShell + opaque and rejects it (the session grants no RunShell).
  * ``fetch`` — the config declares NetworkFetch, which the session withholds,
    so admission rejects it (registration never mints authority).
"""

import json
import sys

PROTOCOL_VERSION = "2026-07-28"
SUPPORTED_VERSION = "2025-11-25" if "--legacy" in sys.argv else PROTOCOL_VERSION
COMPLETE = "--complete" in sys.argv

TOOLS = [
    {
        "name": "echo",
        "description": "Echo the provided text back",
        "inputSchema": {
            "type": "object",
            "properties": {"text": {"type": "string", "description": "Text to echo"}},
            "required": ["text"],
            "additionalProperties": False,
        },
    },
    {
        "name": "shell",
        "description": "Run an arbitrary command (over-privileged on purpose)",
        "inputSchema": {
            "type": "object",
            "properties": {"command": {"type": "string", "description": "Command"}},
            "required": ["command"],
            "additionalProperties": False,
        },
    },
    {
        "name": "fetch",
        "description": "Fetch a URL (network effect the session withholds)",
        "inputSchema": {
            "type": "object",
            "properties": {"url": {"type": "string", "description": "URL"}},
            "required": ["url"],
            "additionalProperties": False,
        },
    },
]

if COMPLETE:
    TOOLS.append(
        {
            "name": "client_roundtrip",
            "description": "Exercise roots, sampling, and elicitation",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": False,
            },
        }
    )
    TOOLS.append(
        {
            "name": "async_task",
            "description": "Return a completed asynchronous task",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": False,
            },
        }
    )


def respond(request_id, result):
    sys.stdout.write(
        json.dumps({"jsonrpc": "2.0", "id": request_id, "result": result}) + "\n"
    )
    sys.stdout.flush()


def client_request(request_id, method, params=None):
    message = {"jsonrpc": "2.0", "id": request_id, "method": method}
    if params is not None:
        message["params"] = params
    sys.stdout.write(json.dumps(message) + "\n")
    sys.stdout.flush()
    while True:
        response = json.loads(sys.stdin.readline())
        if response.get("id") == request_id:
            return response


def main():
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        request = json.loads(line)
        method = request.get("method")
        request_id = request.get("id")
        if method == "server/discover":
            respond(
                request_id,
                {
                    "resultType": "complete",
                    "supportedVersions": [SUPPORTED_VERSION],
                    "capabilities": (
                        {
                            "tools": {},
                            "resources": {"subscribe": True, "listChanged": True},
                            "prompts": {"listChanged": True},
                            "completions": {},
                            "extensions": {"io.modelcontextprotocol/tasks": {}},
                        }
                        if COMPLETE
                        else {"tools": {}}
                    ),
                    "ttlMs": 0,
                    "cacheScope": "private",
                },
            )
        elif method == "tools/list":
            respond(
                request_id,
                {
                    "resultType": "complete",
                    "tools": TOOLS,
                    "ttlMs": 0,
                    "cacheScope": "private",
                },
            )
        elif method == "tools/call":
            params = request.get("params", {})
            name = params.get("name")
            arguments = params.get("arguments", {})
            if name == "echo":
                respond(
                    request_id,
                    {
                        "resultType": "complete",
                        "content": [
                            {"type": "text", "text": "echo: " + arguments.get("text", "")}
                        ],
                        "isError": False,
                    },
                )
            elif name == "client_roundtrip" and COMPLETE:
                sample = client_request(
                    "sample-1",
                    "sampling/createMessage",
                    {
                        "messages": [
                            {"role": "user", "content": {"type": "text", "text": "sample"}}
                        ],
                        "maxTokens": 16,
                        "temperature": 0.0,
                        "stopSequences": ["stop"],
                    },
                )
                roots = client_request("roots-1", "roots/list")
                elicited = client_request(
                    "elicit-1",
                    "elicitation/create",
                    {
                        "mode": "form",
                        "message": "Confirm the fixture",
                        "requestedSchema": {
                            "type": "object",
                            "properties": {"confirmed": {"type": "boolean"}},
                            "required": ["confirmed"],
                        },
                    },
                )
                ok = (
                    roots.get("result", {}).get("roots", [{}])[0].get("uri")
                    == "file:///workspace"
                    and sample.get("result", {}).get("model") == "fixture-model"
                    and elicited.get("result", {}).get("action") == "accept"
                )
                respond(
                    request_id,
                    {
                        "resultType": "complete",
                        "content": [
                            {
                                "type": "text",
                                "text": (
                                    "client roundtrip ok"
                                    if ok
                                    else json.dumps(
                                        {"roots": roots, "sample": sample, "elicited": elicited}
                                    )
                                ),
                            }
                        ],
                        "isError": not ok,
                    },
                )
            elif name == "async_task" and COMPLETE:
                respond(
                    request_id,
                    {
                        "resultType": "task",
                        "taskId": "task-1",
                        "status": "working",
                        "createdAt": "2026-08-21T00:00:00Z",
                        "lastUpdatedAt": "2026-08-21T00:00:00Z",
                        "ttlMs": 60000,
                        "pollIntervalMs": 1,
                    },
                )
            else:
                respond(
                    request_id,
                    {
                        "resultType": "complete",
                        "content": [{"type": "text", "text": "unexpected call"}],
                        "isError": True,
                    },
                )
        elif method == "resources/list" and COMPLETE:
            respond(
                request_id,
                {
                    "resultType": "complete",
                    "resources": [
                        {"uri": "file:///fixture.txt", "name": "Fixture", "mimeType": "text/plain"}
                    ],
                    "ttlMs": 0,
                    "cacheScope": "private",
                },
            )
        elif method == "resources/templates/list" and COMPLETE:
            respond(
                request_id,
                {
                    "resultType": "complete",
                    "resourceTemplates": [
                        {
                            "uriTemplate": "file:///fixtures/{name}",
                            "name": "Fixture template",
                        }
                    ],
                    "ttlMs": 0,
                    "cacheScope": "private",
                },
            )
        elif method == "resources/read" and COMPLETE:
            respond(
                request_id,
                {
                    "contents": [
                        {
                            "uri": request.get("params", {}).get("uri", ""),
                            "mimeType": "text/plain",
                            "text": "fixture resource",
                        }
                    ]
                },
            )
        elif method == "prompts/list" and COMPLETE:
            respond(
                request_id,
                {
                    "resultType": "complete",
                    "prompts": [{"name": "review", "description": "Review input"}],
                    "ttlMs": 0,
                    "cacheScope": "private",
                },
            )
        elif method == "prompts/get" and COMPLETE:
            prompt_arguments = params.get("arguments", {})
            topic = prompt_arguments.get("topic", "fixture")
            respond(
                request_id,
                {
                    "description": "Fixture prompt",
                    "messages": [
                        {
                            "role": "user",
                            "content": {"type": "text", "text": "review " + topic},
                        }
                    ],
                },
            )
        elif method == "completion/complete" and COMPLETE:
            respond(
                request_id,
                {"completion": {"values": ["fixture"], "total": 1, "hasMore": False}},
            )
        elif method == "tasks/get" and COMPLETE:
            respond(
                request_id,
                {
                    "resultType": "complete",
                    "taskId": "task-1",
                    "status": "completed",
                    "createdAt": "2026-08-21T00:00:00Z",
                    "lastUpdatedAt": "2026-08-21T00:00:01Z",
                    "ttlMs": 60000,
                    "result": {
                        "content": [{"type": "text", "text": "task complete"}],
                        "isError": False,
                    },
                },
            )
        elif request_id is not None:
            sys.stdout.write(
                json.dumps(
                    {
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "error": {"code": -32601, "message": "method not found"},
                    }
                )
                + "\n"
            )
            sys.stdout.flush()


if __name__ == "__main__":
    main()
