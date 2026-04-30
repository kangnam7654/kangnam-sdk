//! Streaming `<artifact>` parser + `<question-form>` JSON schema + P0/P1
//! lint + sandboxed srcdoc HTML wrapper.
//!
//! This crate sits between the model's text stream and the chat-server's
//! WebSocket out-channel: SSE / stream-json deltas feed [`ArtifactParser`],
//! which emits typed [`ArtifactEvent`]s the host can forward as new
//! `UnifiedMessage::Artifact{Start,Delta,End}` and `QuestionFormPosted`
//! variants.
//!
//! Distilled from open-design's [`src/artifacts/parser.ts`][p] and
//! [`src/artifacts/question-form.ts`][qf] (Apache-2.0). Sandboxed iframe
//! wrapper logic mirrors open-design `runtime/srcdoc.ts` and open-codesign
//! `packages/runtime/src/index.ts`.
//!
//! [p]: https://github.com/nexu-io/open-design/blob/main/src/artifacts/parser.ts
//! [qf]: https://github.com/nexu-io/open-design/blob/main/src/artifacts/question-form.ts

pub mod parser;
pub mod question_form;
pub mod lint;
pub mod srcdoc;

pub use parser::{ArtifactEvent, ArtifactKind, ArtifactParser};
pub use question_form::{
    parse_question_form, FormResponse, QuestionForm, QuestionFormError, QuestionType,
};
pub use lint::{lint_artifact, LintFinding, LintSeverity};
pub use srcdoc::{wrap_srcdoc, SrcdocOpts};
