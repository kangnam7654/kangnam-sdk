//! Pure-Rust port of open-design's `@open-design/contracts` package.
//!
//! Wire-compatible with the upstream TypeScript zod schemas — the JSON
//! shapes match the production daemon ↔ web boundary, so a Rust daemon
//! can talk to the open-design web client byte-for-byte.
//!
//! ## Modules
//!
//! | Module | Upstream | Covered |
//! |--------|----------|---------|
//! | [`common`] | `src/common.ts` | `OkResponse`, `IdResponse`, `BoundedJsonConstraints`, `LIVE_ARTIFACT_BOUNDED_JSON_CONSTRAINTS` |
//! | [`errors`] | `src/errors.ts` | `ApiErrorCode` (49 variants), `ApiError`, `ApiErrorResponse`, `ApiValidationIssue`, `SseErrorPayload`, helpers |
//! | [`tasks`] | `src/tasks.ts` | `TaskState` (6 variants), `TaskStatus` |
//! | [`sse`] | `src/sse/common.ts` + `src/sse/chat.ts` + `src/sse/proxy.ts` | `SseEvent<P>`, `DaemonAgentPayload` enum, chat/proxy SSE event variants |
//!
//! ## REST endpoints — fully ported
//!
//! All 11 upstream `src/api/*.ts` files have a Rust port in [`api`]:
//! [`api::app_config`], [`api::artifacts`], [`api::chat`],
//! [`api::comments`], [`api::connectors`], [`api::files`],
//! [`api::live_artifacts`], [`api::projects`], [`api::proxy`],
//! [`api::registry`], [`api::version`].
//!
//! ## Not yet ported
//!
//! - `src/critique.ts` — debate config + 5-role panelist enums (zod-heavy
//!   refinements). Will need a custom validator pass.
//! - `src/prompts/system.ts` — already implemented separately by
//!   `kangnam-design-prompt` (Rust port of the system-prompt composer).
//! - `src/examples.ts` — fixture data, not boundary types.
//!
//! ## Wire compatibility notes
//!
//! - All enum codes serialize as the upstream string slug (e.g.
//!   `ApiErrorCode::BadRequest` → `"BAD_REQUEST"`). Deserialization is
//!   strict: unknown codes fail rather than fall back to a catch-all.
//! - `serde(rename_all = "camelCase")` on most structs to match the JS
//!   convention (`taskId`, `requestId`, `runId`).
//! - Optional fields use `#[serde(skip_serializing_if = "Option::is_none")]`
//!   so the emitted JSON omits them — matching the TS interface (where
//!   `field?: T` produces `{}` not `{ field: undefined }`).

pub mod api;
pub mod common;
pub mod errors;
pub mod serde_helpers;
pub mod sse;
pub mod tasks;

pub use common::{
    BoundedJsonConstraints, IdResponse, OkResponse, LIVE_ARTIFACT_BOUNDED_JSON_CONSTRAINTS,
};
pub use errors::{
    create_api_error, create_api_error_response, ApiError, ApiErrorCode, ApiErrorResponse,
    ApiValidationErrorDetails, ApiValidationIssue, SseErrorPayload,
};
pub use sse::{
    ChatSseEndPayload, ChatSseEvent, ChatSseStartPayload, DaemonAgentPayload,
    LiveArtifactRefreshSsePayload, LiveArtifactRefreshSsePhase, LiveArtifactSseAction,
    LiveArtifactSsePayload, ProxySseEvent, SseEvent, ToolResultPayload, ToolUsePayload,
    UsagePayload, CHAT_SSE_PROTOCOL_VERSION, PROXY_SSE_PROTOCOL_VERSION,
};
pub use tasks::{TaskState, TaskStatus, TASK_STATES};
