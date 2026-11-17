# Kangnam Client

A desktop client (Tauri 2 + React) that puts a chat-style GUI in front of locally installed coding-agent CLIs such as Claude Code and Codex CLI.

[한국어 문서 (Korean)](docs/README_ko.md)

> **Source-visible, not open source.** This repository is public for transparency and portfolio review only. See [License](#license).

---

## For reviewers: what this is

**Problem.** Coding-agent CLIs are capable, but they are terminal-first. This project explores running those CLIs as local subprocesses and showing their streamed output, tool activity and permission prompts in a desktop chat UI.

**Where it lives.** Links to `kangnam7654/kangnam-client` redirect to this repository (`kangnam-sdk`), which is a monorepo. The client is one part of it:

- `apps/kangnam-client/` — Tauri 2 desktop shell (Rust)
- `src/renderer/` — React front end
- `crates/harness/session-*` — shared Rust crates the client delegates to (CLI adapters, JSON-RPC handlers, Axum server)
- Other `crates/` (router, design, fortune, pii-crypto) are separate workspace members, not part of this README's scope.

**Existing release.** GitHub lists [v0.3.2](https://github.com/kangnam7654/kangnam-sdk/releases/tag/v0.3.2) with macOS, Windows and Linux build assets. That tag points to the source revision described here; the asset filenames say 0.3.1, matching the desktop app's [`tauri.conf.json`](apps/kangnam-client/tauri.conf.json). This documentation pass did not install or run those binaries. A public download is not permission to use or redistribute it; see [License](#license).

**What you can see in the source today**

| Area | Where to look |
|------|---------------|
| App entry: starts a local Axum server on a background thread (port `3001`, or `KANGNAM_PORT`), then builds the Tauri window, tray and Tauri commands | [`apps/kangnam-client/src/lib.rs`](apps/kangnam-client/src/lib.rs) |
| App state: SQLite connection, CLI manager with Claude Code and Codex adapters registered, broadcast channels, pending-permission map | [`apps/kangnam-client/src/state.rs`](apps/kangnam-client/src/state.rs) |
| Server boot: hands app state to the shared session server (local-only, no auth hook) | [`apps/kangnam-client/src/server/mod.rs`](apps/kangnam-client/src/server/mod.rs) |
| Shared server: binds `127.0.0.1`, serves JSON-RPC over WebSocket at `/ws` and an `/mcp` endpoint used for Claude Code permission prompts | [`crates/harness/session-server/src/lib.rs`](crates/harness/session-server/src/lib.rs) |
| Message persistence: accumulates assistant text and saves it at turn end | [`crates/harness/session-server/src/saver.rs`](crates/harness/session-server/src/saver.rs) |
| CLI adapters: spawn `claude` / `codex` and parse their streamed output | [`claude.rs`](crates/harness/session-agent/src/adapters/claude.rs), [`codex.rs`](crates/harness/session-agent/src/adapters/codex.rs) |
| Front end: layout with Chat / Studio / Project / Hub views; auto-detects an installed CLI provider on startup | [`src/renderer/App.tsx`](src/renderer/App.tsx) |
| Front-end transport: WebSocket client to `ws://localhost:<port>/ws` | [`src/renderer/lib/cli-api.ts`](src/renderer/lib/cli-api.ts) |
| Build and window config | [`apps/kangnam-client/tauri.conf.json`](apps/kangnam-client/tauri.conf.json) |

**Verification boundary.** The frontend type check, tests and production frontend build ran locally; the Tauri desktop app, CLI-provider sessions, MCP calls and Rust workspace tests did not. Earlier versions of this README listed modules that are no longer in this tree; source and frontend checks alone do not prove desktop runtime behavior.

**Limits visible in source** (not based on a runtime test):

- The shared message saver writes to the *most recent* conversation, not the conversation associated with a CLI session; concurrent sessions can therefore attach text to the wrong conversation ([`saver.rs`](crates/harness/session-server/src/saver.rs)).
- Settings are still served through Tauri commands rather than the JSON-RPC layer ([`lib.rs`](apps/kangnam-client/src/lib.rs)).
- Design-mode question-form / preview hooks are left as `None` on the desktop host until the front end wires that flow ([`server/mod.rs`](apps/kangnam-client/src/server/mod.rs)).
- The new-project panel has no regular UI entry yet; it is only reachable through a development hook ([`App.tsx`](src/renderer/App.tsx)).

---

## Architecture

```
React front end <── JSON-RPC over WebSocket ──> Axum server (in-process, 127.0.0.1) <──> CLI subprocess
 src/renderer        ws://localhost:3001/ws      crates/harness/session-server          claude / codex
        │
        └── Tauri commands (settings, conversations, skills, MCP config, agents, ...) ──> apps/kangnam-client
```

Conversations and other app data are stored in a local SQLite database (`rusqlite`, bundled) under the per-user config/data directory (`kangnam-client/data/`), per [`state.rs`](apps/kangnam-client/src/state.rs).

---

## Tech stack

Taken from [`package.json`](package.json), [`Cargo.toml`](Cargo.toml) and [`apps/kangnam-client/Cargo.toml`](apps/kangnam-client/Cargo.toml):

| Layer | Technology |
|-------|-----------|
| Desktop | Tauri 2 (Rust, Cargo workspace edition 2024, `rust-version` 1.85) |
| Local server | Axum 0.8 with WebSocket, Tokio |
| Front end | React 19, Vite 7, Tailwind CSS 4, Zustand 5 |
| Storage | rusqlite (bundled SQLite) |
| Package manager | pnpm (lockfile and CI use pnpm 9) |

---

## Development reference (owner / authorized contributors)

These commands are listed for the copyright holder and anyone with separate written permission. They are **not** a grant of permission to clone, run, modify or distribute this software; see [License](#license). They follow [`package.json`](package.json), [`tauri.conf.json`](apps/kangnam-client/tauri.conf.json) and the [CI](.github/workflows/ci.yml) / [release](.github/workflows/release.yml) workflows. The frontend checks above were run; the full Tauri and Rust commands below remain source-defined references.

**Prerequisites:** Node.js (CI uses 20, release builds use 22), pnpm 9, a Rust toolchain ≥ 1.85, and the [Tauri 2 platform prerequisites](https://v2.tauri.app/start/prerequisites/). On Ubuntu, CI installs `libwebkit2gtk-4.1-dev libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`. Using a CLI provider additionally requires that CLI (`claude` or `codex`) to be installed and signed in on its own terms.

From the repository root:

```bash
pnpm install --frozen-lockfile   # as in CI
pnpm tauri:dev                   # tauri dev; beforeDevCommand runs Vite, devUrl is :5173
pnpm tauri:build                 # tauri build; runs the Vite production build first
pnpm typecheck                   # tsc --noEmit -p tsconfig.web.json
pnpm test                        # vitest run (front end)
cargo test --workspace --locked  # Rust tests, as in CI
```

Because the Cargo workspace root is the repository root, Tauri bundles are expected under `target/release/bundle/` (or `target/<target-triple>/release/bundle/` when `--target` is passed, as the release workflow does). This path follows from the workspace layout and has not been checked against a fresh build here.

---

## License

**Proprietary — All Rights Reserved.** Copyright (c) 2026 Kangnam Kim.

This repository is source-visible for transparency, portfolio and personal-archive purposes only. It is **not open source**. Public visibility does not grant any right to use, copy, run, modify or distribute the code; those require prior written permission from the copyright holder. The [LICENSE](LICENSE) file is authoritative, including over any license field in package metadata. For licensing inquiries, contact kangnam7654@naver.com.
