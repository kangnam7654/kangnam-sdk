# Harness / Router Architecture

Kangnam now treats `kangnam-harness` and `kangnam-router` as the core.

## Core Layers

`kangnam-router` owns provider normalization:

- common provider creation
- OpenAI-compatible gateways
- local subscription providers
- shared request/response and streaming contracts

`kangnam-harness-*` owns agent execution:

- tool, skill, hook, agent, permission, and scope definitions
- runtime capability traits
- suspend/resume interaction primitives
- harness resource persistence
- LLM tool-call loops through `kangnam-harness-llm-tool-runner`

`kangnam-harness-session-*` is optional host infrastructure:

- session-oriented message storage
- CLI/session manager adapters
- JSON-RPC dispatch
- WebSocket and MCP transport for desktop or headless hosts

## Deleted Boundary

The legacy `kangnam-chat-*` crate family is no longer part of the workspace.
Session transport now lives under `crates/harness/session*` so the public Rust
crate names stay aligned with the harness/router architecture.

## Regression Rule

Do not re-add `crates/chat/*` to the workspace. If a future UI needs a
conversation surface, build it as a harness session host or as an application
layer on top of `kangnam-harness-session` and `kangnam-router`.
