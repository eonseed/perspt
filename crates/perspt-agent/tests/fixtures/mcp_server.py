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

PROTOCOL_VERSION = "2025-11-25"

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


def respond(request_id, result):
    sys.stdout.write(
        json.dumps({"jsonrpc": "2.0", "id": request_id, "result": result}) + "\n"
    )
    sys.stdout.flush()


def main():
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        request = json.loads(line)
        method = request.get("method")
        request_id = request.get("id")
        if method == "initialize":
            respond(
                request_id,
                {
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "fixture", "version": "0"},
                },
            )
        elif method == "notifications/initialized":
            continue
        elif method == "tools/list":
            respond(request_id, {"tools": TOOLS})
        elif method == "tools/call":
            params = request.get("params", {})
            name = params.get("name")
            arguments = params.get("arguments", {})
            if name == "echo":
                respond(
                    request_id,
                    {
                        "content": [
                            {"type": "text", "text": "echo: " + arguments.get("text", "")}
                        ],
                        "isError": False,
                    },
                )
            else:
                respond(
                    request_id,
                    {
                        "content": [{"type": "text", "text": "unexpected call"}],
                        "isError": True,
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
