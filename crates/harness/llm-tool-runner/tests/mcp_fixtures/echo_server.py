#!/usr/bin/env python3
"""Tiny stdio MCP server fixture — echoes a single canned tool.

Reads line-delimited JSON-RPC from stdin, writes responses to stdout.
Implements just enough of the MCP spec for the bridge's stdio
transport tests:

- `initialize` → returns {protocolVersion, serverInfo}
- `tools/list` → returns one `echo` tool that takes a `text` arg
- `tools/call` (name=echo) → returns {content: [{type: text, text: <input>}]}
- anything else → JSON-RPC error -32601 (method not found)

Stays alive until stdin closes. Used by `tests/mcp_stdio.rs`.

Apache-2.0 license — original Rust-side test fixture, no third-party
imports beyond the Python stdlib.
"""

import json
import sys


def respond(req_id, *, result=None, error=None):
    msg = {"jsonrpc": "2.0", "id": req_id}
    if error is not None:
        msg["error"] = error
    else:
        msg["result"] = result
    sys.stdout.write(json.dumps(msg) + "\n")
    sys.stdout.flush()


def main():
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            req = json.loads(line)
        except json.JSONDecodeError:
            continue
        method = req.get("method", "")
        req_id = req.get("id", "")
        params = req.get("params") or {}

        if method == "initialize":
            respond(
                req_id,
                result={
                    "protocolVersion": "2024-11-05",
                    "serverInfo": {"name": "echo-fixture", "version": "0.1.0"},
                    "capabilities": {"tools": {}},
                },
            )
        elif method == "tools/list":
            respond(
                req_id,
                result={
                    "tools": [
                        {
                            "name": "echo",
                            "description": "Echo the input text verbatim.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {"text": {"type": "string"}},
                                "required": ["text"],
                            },
                        }
                    ]
                },
            )
        elif method == "tools/call":
            name = params.get("name")
            args = params.get("arguments") or {}
            if name == "echo":
                text = str(args.get("text", ""))
                respond(
                    req_id,
                    result={
                        "content": [{"type": "text", "text": text}],
                    },
                )
            else:
                respond(
                    req_id,
                    error={"code": -32602, "message": f"unknown tool '{name}'"},
                )
        else:
            respond(
                req_id,
                error={"code": -32601, "message": f"method '{method}' not implemented"},
            )


if __name__ == "__main__":
    main()
