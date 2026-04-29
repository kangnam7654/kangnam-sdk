//! Core streaming client abstraction.
//!
//! An [`AiClient`] turns a text prompt (plus optional [`AiAttachment`]s) into
//! a boxed `Stream` of [`AiChunk`] values. Provider implementations live in
//! the `providers` module and wrap underlying CLIs (Gemini CLI, Claude CLI)
//! or HTTP APIs (LM Studio / OpenAI-compatible).

use std::path::PathBuf;

use futures::stream::BoxStream;
use thiserror::Error;

/// One unit of streaming output from a model.
///
/// A well-behaved provider yields zero or more [`AiChunk::Delta`] values
/// containing incremental text, followed by exactly one
/// [`AiChunk::Done { full_text }`] carrying the concatenated output.
/// Consumers that only care about the final answer can ignore the deltas
/// and wait for `Done`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiChunk {
    Delta(String),
    Done { full_text: String },
}

/// Failures surfaced from an [`AiClient`] stream.
///
/// - `Process`: OS-level / transport problem (spawn failed, non-zero exit,
///   HTTP non-2xx, connection closed).
/// - `Protocol`: the provider produced output we couldn't parse (malformed
///   JSON, unexpected envelope shape, etc.).
#[derive(Debug, Error)]
pub enum AiError {
    #[error("ai process failed: {0}")]
    Process(String),
    #[error("ai protocol error: {0}")]
    Protocol(String),
}

/// A file (typically an image) attached to a prompt.
///
/// The `label` is a human-readable description that gets rendered into the
/// prompt alongside the path reference (e.g. `"deck.pdf (1/5 페이지)"` or
/// `"inspo.png"`). This gives the model the *source* context it would
/// otherwise be missing from the generic `page-001.png` filenames produced
/// during rasterization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiAttachment {
    pub path: PathBuf,
    pub mime_type: String,
    pub label: String,
}

impl AiAttachment {
    /// Build an image attachment. If `label` is empty, the filename
    /// component of `path` is used as a fallback so callers that forget to
    /// pass a label still get something marginally useful instead of a
    /// blank.
    pub fn image(
        path: impl Into<PathBuf>,
        mime_type: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        let path = path.into();
        let label_in = label.into();
        let label = if label_in.is_empty() {
            path.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string()
        } else {
            label_in
        };
        Self {
            path,
            mime_type: mime_type.into(),
            label,
        }
    }
}

/// Streaming AI client. Every provider implements this trait.
///
/// `complete` is synchronous (no `async fn`) but returns a `BoxStream`, so
/// the call site can `.next().await` chunks as they arrive. This keeps the
/// trait object-safe without needing `#[async_trait]`.
pub trait AiClient: Send + Sync {
    fn complete(
        &self,
        prompt: String,
        attachments: Vec<AiAttachment>,
    ) -> BoxStream<'static, Result<AiChunk, AiError>>;
}
