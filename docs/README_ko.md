# Kangnam Client

로컬에 설치된 코딩 에이전트 CLI(Claude Code, Codex CLI 등) 앞에 채팅형 GUI를 두는 데스크톱 클라이언트입니다 (Tauri 2 + React).

[English](../README.md)

> **소스 공개(source-visible)이며 오픈소스가 아닙니다.** 이 저장소는 투명성과 포트폴리오 검토 목적으로만 공개되어 있습니다. [라이선스](#라이선스)를 참고하세요.

---

## 리뷰어를 위한 안내

**문제의식.** 코딩 에이전트 CLI는 강력하지만 터미널 중심입니다. 이 프로젝트는 이런 CLI를 로컬 서브프로세스로 실행하고, 스트리밍 출력·도구 활동·권한 요청을 데스크톱 채팅 UI로 보여주는 방식을 다룹니다.

**위치.** `kangnam7654/kangnam-client` 링크는 이 저장소(`kangnam-sdk`)로 리다이렉트되며, 이 저장소는 모노레포입니다. 클라이언트는 그 일부입니다.

- `apps/kangnam-client/` — Tauri 2 데스크톱 셸 (Rust)
- `src/renderer/` — React 프론트엔드
- `crates/harness/session-*` — 클라이언트가 위임하는 공용 Rust 크레이트 (CLI 어댑터, JSON-RPC 핸들러, Axum 서버)
- 그 밖의 `crates/` (router, design, fortune, pii-crypto)는 별도 워크스페이스 멤버이며 이 문서의 범위가 아닙니다.

**기존 릴리스.** GitHub에 [v0.3.2](https://github.com/kangnam7654/kangnam-sdk/releases/tag/v0.3.2)와 macOS·Windows·Linux 빌드 파일이 공개되어 있습니다. 태그는 여기서 설명하는 소스 리비전을 가리킵니다. 파일 이름의 0.3.1은 데스크톱 앱의 [`tauri.conf.json`](../apps/kangnam-client/tauri.conf.json) 버전과 일치합니다. 이번 작업에서 바이너리를 설치하거나 실행하지는 않았습니다. 파일을 내려받을 수 있다는 사실이 사용·재배포 허가는 아닙니다 ([라이선스](#라이선스) 참고).

**현재 소스에서 확인할 수 있는 것**

| 영역 | 위치 |
|------|------|
| 앱 진입점: 백그라운드 스레드에서 로컬 Axum 서버를 띄운 뒤(포트 `3001`, 또는 `KANGNAM_PORT`) Tauri 창·트레이·Tauri 커맨드를 구성 | [`apps/kangnam-client/src/lib.rs`](../apps/kangnam-client/src/lib.rs) |
| 앱 상태: SQLite 연결, Claude Code·Codex 어댑터가 등록된 CLI 매니저, broadcast 채널, 권한 요청 대기 맵 | [`apps/kangnam-client/src/state.rs`](../apps/kangnam-client/src/state.rs) |
| 서버 기동: 앱 상태를 공용 세션 서버에 전달 (로컬 전용, 인증 훅 없음) | [`apps/kangnam-client/src/server/mod.rs`](../apps/kangnam-client/src/server/mod.rs) |
| 공용 서버: `127.0.0.1`에 바인딩, `/ws`에서 WebSocket 기반 JSON-RPC 제공, Claude Code 권한 요청용 `/mcp` 엔드포인트 | [`crates/harness/session-server/src/lib.rs`](../crates/harness/session-server/src/lib.rs) |
| 메시지 저장: 응답 텍스트를 모았다가 턴 종료 시 저장 | [`crates/harness/session-server/src/saver.rs`](../crates/harness/session-server/src/saver.rs) |
| CLI 어댑터: `claude` / `codex`를 실행하고 스트리밍 출력을 파싱 | [`claude.rs`](../crates/harness/session-agent/src/adapters/claude.rs), [`codex.rs`](../crates/harness/session-agent/src/adapters/codex.rs) |
| 프론트엔드: Chat / Studio / Project / Hub 뷰 레이아웃, 시작 시 설치된 CLI 프로바이더 자동 감지 | [`src/renderer/App.tsx`](../src/renderer/App.tsx) |
| 프론트엔드 전송 계층: `ws://localhost:<port>/ws` WebSocket 클라이언트 | [`src/renderer/lib/cli-api.ts`](../src/renderer/lib/cli-api.ts) |
| 빌드·창 설정 | [`apps/kangnam-client/tauri.conf.json`](../apps/kangnam-client/tauri.conf.json) |

**검증 범위.** 프론트엔드 타입 검사·테스트·프로덕션 빌드는 로컬에서 실행했습니다. Tauri 데스크톱 앱, CLI 프로바이더 세션, MCP 호출, Rust 워크스페이스 테스트는 실행하지 않았습니다. 이전 README에는 현재 트리에 없는 모듈이 나열되어 있었으며, 소스와 프론트엔드 검사만으로 데스크톱 런타임 동작이 입증되지는 않습니다.

**소스에서 확인한 한계** (실행 관찰이 아닌 코드 기준):

- 공용 메시지 저장기는 CLI 세션에 연결된 대화가 아니라 *가장 최근 대화*에 응답을 기록합니다. 세션이 겹치면 다른 대화에 저장될 수 있습니다 ([`saver.rs`](../crates/harness/session-server/src/saver.rs)).
- 설정은 아직 JSON-RPC가 아닌 Tauri 커맨드로 처리됩니다 ([`lib.rs`](../apps/kangnam-client/src/lib.rs)).
- 디자인 모드의 question-form / preview 훅은 프론트엔드 연결 전까지 데스크톱 호스트에서 `None`으로 남아 있습니다 ([`server/mod.rs`](../apps/kangnam-client/src/server/mod.rs)).
- 새 프로젝트 패널은 아직 정식 UI 진입점이 없고 개발용 훅으로만 열 수 있습니다 ([`App.tsx`](../src/renderer/App.tsx)).

---

## 아키텍처

```
React 프론트엔드 <── WebSocket 기반 JSON-RPC ──> Axum 서버 (프로세스 내, 127.0.0.1) <──> CLI 서브프로세스
 src/renderer         ws://localhost:3001/ws       crates/harness/session-server           claude / codex
        │
        └── Tauri 커맨드 (설정, 대화, 스킬, MCP 설정, 에이전트 등) ──> apps/kangnam-client
```

대화 등 앱 데이터는 사용자별 config/data 디렉터리 아래(`kangnam-client/data/`)의 로컬 SQLite 데이터베이스(`rusqlite`, 번들)에 저장됩니다 ([`state.rs`](../apps/kangnam-client/src/state.rs) 기준).

---

## 기술 스택

[`package.json`](../package.json), [`Cargo.toml`](../Cargo.toml), [`apps/kangnam-client/Cargo.toml`](../apps/kangnam-client/Cargo.toml) 기준:

| 레이어 | 기술 |
|--------|------|
| 데스크톱 | Tauri 2 (Rust, Cargo 워크스페이스 edition 2024, `rust-version` 1.85) |
| 로컬 서버 | Axum 0.8 (WebSocket), Tokio |
| 프론트엔드 | React 19, Vite 7, Tailwind CSS 4, Zustand 5 |
| 저장소 | rusqlite (번들 SQLite) |
| 패키지 매니저 | pnpm (lockfile과 CI가 pnpm 9 사용) |

---

## 개발 참고 (저작권자 / 허가받은 기여자용)

아래 명령은 저작권자와 별도 서면 허가를 받은 사람을 위한 참고용입니다. 이 소프트웨어를 클론·실행·수정·배포할 권리를 부여하는 것이 **아닙니다** ([라이선스](#라이선스) 참고). [`package.json`](../package.json), [`tauri.conf.json`](../apps/kangnam-client/tauri.conf.json), [CI](../.github/workflows/ci.yml) / [릴리스](../.github/workflows/release.yml) 워크플로를 기준으로 합니다. 위의 프론트엔드 검사는 실행했으나 전체 Tauri·Rust 명령은 소스 기준 참고용입니다.

**전제조건:** Node.js (CI는 20, 릴리스 빌드는 22 사용), pnpm 9, Rust 1.85 이상, [Tauri 2 플랫폼 요구사항](https://v2.tauri.app/start/prerequisites/). Ubuntu에서 CI는 `libwebkit2gtk-4.1-dev libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`를 설치합니다. CLI 프로바이더를 쓰려면 해당 CLI(`claude` 또는 `codex`)가 각자의 조건에 따라 설치·로그인되어 있어야 합니다.

저장소 루트에서:

```bash
pnpm install --frozen-lockfile   # CI와 동일
pnpm tauri:dev                   # tauri dev; beforeDevCommand가 Vite 실행, devUrl은 :5173
pnpm tauri:build                 # tauri build; Vite 프로덕션 빌드를 먼저 실행
pnpm typecheck                   # tsc --noEmit -p tsconfig.web.json
pnpm test                        # vitest run (프론트엔드)
cargo test --workspace --locked  # Rust 테스트, CI와 동일
```

Cargo 워크스페이스 루트가 저장소 루트이므로 Tauri 번들은 `target/release/bundle/`(릴리스 워크플로처럼 `--target`을 넘기면 `target/<target-triple>/release/bundle/`) 아래에 생성될 것으로 예상됩니다. 이 경로는 워크스페이스 구조에서 추론한 것이며 새 빌드로 확인하지는 않았습니다.

---

## 라이선스

**독점 소프트웨어 — All Rights Reserved.** Copyright (c) 2026 Kangnam Kim.

이 저장소는 투명성, 포트폴리오, 개인 아카이브 목적으로만 소스를 공개합니다. **오픈소스가 아닙니다.** 공개되어 있다는 사실만으로 코드를 사용·복제·실행·수정·배포할 권리가 생기지 않으며, 이를 위해서는 저작권자의 사전 서면 허가가 필요합니다. 패키지 메타데이터의 라이선스 필드를 포함해 모든 경우에 [LICENSE](../LICENSE) 파일이 우선합니다. 라이선스 문의: kangnam7654@naver.com
