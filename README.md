# Kangnam Client

A desktop GUI wrapper for coding agent CLIs (Claude Code, Codex CLI). Run AI coding agents through a structured chat interface instead of the terminal.

[한국어 문서 (Korean)](docs/README_ko.md)

---

## Why This Exists

Coding agent CLIs like Claude Code and Codex CLI are powerful, but they assume terminal fluency. Kangnam Client wraps these CLIs as subprocesses and presents their JSON output in a structured GUI — making them accessible to non-developers.

This is a personal project, not a commercial product.

---

## What It Does

Kangnam Client is a desktop application built with Tauri 2 (Rust backend + system WebView). It spawns coding agent CLIs as subprocesses, parses their JSON stream output, and renders it as a real-time chat UI.

Key capabilities:

- **Multi-CLI support** — Switch between Claude Code and Codex CLI. Adding new CLIs requires only a new adapter implementation.
- **Real-time streaming** — Token-by-token text display, tool call progress, and subagent activity — all streamed in real time.
- **Safety dialogs** — When the CLI requests permission for file edits or command execution, a confirmation dialog appears.
- **Setup wizard** — Detects installed CLIs, helps install missing ones, guides through first-time setup.
- **Conversation persistence** — Messages are saved to a local SQLite database.
- **Web access** — The same UI works in a browser via WebSocket (ws://localhost:3001/ws), no Tauri required.

---

## Prerequisites

### Required Tools

| Tool | Minimum Version | Installation |
|------|----------------|-------------|
| Node.js | 20+ | [nodejs.org](https://nodejs.org) |
| Rust | 1.75+ (edition 2021) | [rustup.rs](https://rustup.rs) |
| npm | 10+ | Included with Node.js |

### Supported CLIs (at least one required)

| CLI | Install | Subscription |
|-----|---------|-------------|
| Claude Code | `npm install -g @anthropic-ai/claude-code` | Claude Max |
| Codex CLI | `npm install -g @openai/codex` | OpenAI subscription |

### Platform-Specific Requirements

| Platform | Required | Install Command |
|----------|----------|----------------|
| macOS | Xcode Command Line Tools | `xcode-select --install` |
| Linux (Debian/Ubuntu) | webkit2gtk, build essentials | `sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev` |
| Windows | WebView2 Runtime | Pre-installed on Windows 10/11. See [Tauri docs](https://v2.tauri.app/start/prerequisites/) if missing. |

---

## Getting Started

### Step 1: Clone and install

```bash
git clone https://github.com/kangnam7654/kangnam-client.git
cd kangnam-client
npm install
```

### Step 2: Run in development mode

```bash
npm run tauri:dev
```

First build compiles all Rust dependencies (300+ crates) and takes 2-5 minutes. Subsequent builds start in seconds.

### Step 3: Build for production

```bash
npm run tauri:build
```

Find the built application in `src-tauri/target/release/bundle/`.

---

## Architecture

```
React Frontend <--> WebSocket (JSON-RPC 2.0) <--> Axum Server <--> CLI Subprocess
                    ws://localhost:3001/ws          (Rust)       stdin/stdout NDJSON
```

- **Frontend** communicates via JSON-RPC 2.0 over WebSocket
- **Axum server** (embedded in Tauri) dispatches RPC calls to handlers
- **CLI Manager** spawns CLI subprocesses and pipes stdin/stdout
- **CliAdapter trait** normalizes different CLI JSON formats into `UnifiedMessage`
- **Broadcast channel** fans out CLI stream events to all connected WebSocket clients
- **Message saver** background task persists conversations to SQLite

---

## Project Structure

```text
src-tauri/                    # Rust backend (Tauri 2)
  src/
    lib.rs                    # Tauri builder + Axum server spawn
    state.rs                  # AppState (DB, CliManager, broadcast)
    cli/                      # CLI subprocess management
      adapter.rs              # CliAdapter trait
      adapters/claude.rs      # Claude Code NDJSON parser
      adapters/codex.rs       # Codex CLI JSONL parser
      manager.rs              # Subprocess lifecycle, stdin/stdout
      registry.rs             # CLI detection and install
      types.rs                # UnifiedMessage enum, CliStatus
    rpc/                      # JSON-RPC 2.0 layer
      dispatcher.rs           # Method routing
      handlers.rs             # RPC method implementations
      types.rs                # Request/Response/Notification types
    server/                   # Axum WebSocket server
      ws.rs                   # WebSocket handler
      router.rs               # HTTP routes
      broadcast.rs            # tokio::broadcast channel
      saver.rs                # Background message persistence
    db/                       # SQLite (conversations, messages)
    commands/                 # Tauri IPC (settings, legacy)
src/renderer/                 # React frontend
  lib/
    rpc/                      # JSON-RPC client + WebSocket transport
    cli-api.ts                # High-level CLI API wrapper
  components/
    chat/                     # Chat UI, message renderer, safety dialog
    setup/                    # CLI setup wizard
    sidebar/                  # Conversation list, agent panel
    settings/                 # Settings panel
  stores/                     # Zustand state management
docs/
  llm/                        # Implementation plan, TODO
```

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop | Tauri 2 (Rust backend + system WebView) |
| Server | Axum (embedded WebSocket server) |
| Protocol | JSON-RPC 2.0 over WebSocket |
| Frontend | React 19 + Vite 7 + Tailwind CSS v4 |
| State | Zustand 5 |
| Database | rusqlite (bundled SQLite) |
| Async I/O | tokio (subprocess, broadcast, server) |

---

## Current Status

Active development. Core backend is functional. UI needs overhaul.

### Done

- CLI subprocess management (spawn, stdin/stdout piping, lifecycle)
- Claude Code adapter (NDJSON stream parsing, bidirectional messaging)
- Codex CLI adapter (JSONL parsing, multi-turn via history prepend)
- Axum WebSocket server + JSON-RPC 2.0 protocol
- Real-time token-by-token streaming
- Conversation/message persistence to SQLite
- Setup wizard (CLI detection + install)
- Permission request / safety dialog flow

### In Progress

- UI overhaul (current UI is rough from the pivot)
- Session-aware message saving (currently saves to most recent conversation)
- Migrate remaining Tauri IPC commands to JSON-RPC

---

## License

**Proprietary — All Rights Reserved.** Copyright (c) 2026 Kangnam Kim.

This repository is source-visible for transparency and personal-archive
purposes only. It is **not open source**. No license is granted to use, copy,
modify, or distribute the code except by the copyright holder. See
[LICENSE](LICENSE) for full terms. For licensing inquiries, contact
kangnam7654@naver.com.
