//! HTTP-backed session manager.
//!
//! Parallel to [`crate::manager::CliManager`], but instead of spawning
//! a CLI subprocess and parsing its stdout, this manager calls an
//! [`kangnam_router::LlmProviderDyn`] over HTTP and maps its
//! [`kangnam_router::LlmStreamEvent`] stream onto the same
//! [`crate::types::UnifiedMessage`] events the rest of the SDK
//! consumes. Hosts can wire either manager (or both) into the same
//! [`crate::sink::AgentEventSink`] — chat-rpc / chat-server don't
//! care which one produced the events.
//!
//! ## State model
//!
//! Unlike CLI sessions, HTTP turns are stateless on the server side:
//! every `send_message` call loads the conversation history from a
//! [`kangnam_chat_core::Storage`] backend, calls the provider, streams the
//! response, and persists user + assistant turns. There is no
//! long-lived child process to kill, no stdin pipe to write to. The
//! "session" is just a conversation id + the provider config.
//!
//! ## Why use the Storage trait
//!
//! Local-CLI providers carry their own conversation context (Claude
//! Code's `--session-id`, Codex's history-in-prompt). HTTP providers
//! have no equivalent — every request must include the full message
//! list. So this manager owns history reconstruction itself, via the
//! pluggable [`kangnam_chat_core::Storage`] trait introduced in chat-core
//! v0.2: SQLite for desktop hosts, Postgres for server hosts.

use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use kangnam_chat_core::{NewMessage, Storage};
use kangnam_router::{
    ChatMessage, LlmProviderDyn, LlmStreamEvent,
    context::{ContextWindowBudget, compact_messages_for_window},
};

use crate::sink::AgentEventSink;
use crate::types::{TokenUsage, UnifiedMessage};

/// Errors surfaced by [`HttpSessionManager`].
///
/// String-typed for parity with [`crate::manager::CliManager`] —
/// chat-rpc wraps both into the same `JsonRpcError::internal`.
pub type HttpError = String;

/// Lookup of conversation history → provider call → unified events.
///
/// Cheap to clone (`Arc` inside). Construct once at app startup,
/// share across request handlers.
#[derive(Clone)]
pub struct HttpSessionManager {
    storage: Arc<dyn Storage>,
    provider_factory: Arc<dyn ProviderFactory>,
    /// Optional override of the system prompt sent on every turn.
    /// Hosts that need per-conversation prompts should subclass via
    /// composition and pre-pend their own `system` message.
    system_prompt: String,
    context_window_budget: Option<ContextWindowBudget>,
}

impl HttpSessionManager {
    /// Build a manager that resolves the provider lazily through
    /// `factory`. The factory is consulted once per `send_message`
    /// call so hosts can hot-swap providers (e.g. an admin LLM
    /// config UI) without bouncing the manager.
    pub fn new(storage: Arc<dyn Storage>, provider_factory: Arc<dyn ProviderFactory>) -> Self {
        Self {
            storage,
            provider_factory,
            system_prompt: String::new(),
            context_window_budget: None,
        }
    }

    /// Override the system prompt sent on every turn. Defaults to empty.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Enable deterministic context compaction for HTTP-backed chat sessions.
    ///
    /// This path reconstructs full history from storage on every turn; when
    /// that estimated request is too large, older turns are folded into one
    /// summary message before the provider call.
    pub fn with_context_window_tokens(mut self, max_context_tokens: usize) -> Self {
        self.context_window_budget = Some(ContextWindowBudget::new(max_context_tokens));
        self
    }

    /// Enable context compaction with explicit budget knobs.
    pub fn with_context_window_budget(mut self, budget: ContextWindowBudget) -> Self {
        self.context_window_budget = Some(budget);
        self
    }

    /// Open a session.
    ///
    /// Lazily ensures the conversation row exists in storage and
    /// emits a `SessionInit` event. Idempotent — safe to call again
    /// for the same id (e.g. after WS reconnect). The `provider_hint`
    /// is recorded on the conversation row so list views can show
    /// which backend the chat is using.
    pub async fn start_session(
        &self,
        session_id: &str,
        provider_hint: &str,
        sink: Arc<dyn AgentEventSink>,
    ) -> Result<(), HttpError> {
        self.storage
            .ensure_conversation(session_id, provider_hint)
            .await
            .map_err(|e| format!("storage error: {e}"))?;
        sink.emit_message(UnifiedMessage::SessionInit {
            session_id: session_id.to_string(),
        });
        Ok(())
    }

    /// Run one turn end-to-end.
    ///
    /// 1. Persist the user message and (if title is still default)
    ///    auto-title the conversation.
    /// 2. Reconstruct the full message history from storage.
    /// 3. Stream the provider response, mapping each
    ///    [`LlmStreamEvent`] to a [`UnifiedMessage`] on the sink.
    /// 4. On `End`, persist the accumulated assistant text and emit
    ///    `TurnEnd` with token usage.
    ///
    /// Returns once the stream terminates (after `TurnEnd` or
    /// `Error` is emitted). Callers that need fire-and-forget
    /// semantics should `tokio::spawn` this.
    pub async fn send_message(
        &self,
        session_id: &str,
        message: &str,
        sink: Arc<dyn AgentEventSink>,
    ) -> Result<(), HttpError> {
        // 1. Persist user turn first so a crash mid-stream still
        //    leaves a recoverable conversation. Auto-title is
        //    best-effort — failures don't block the call.
        self.storage
            .add_message(session_id, NewMessage::user(message))
            .await
            .map_err(|e| format!("storage error: {e}"))?;
        let _ = self.storage.auto_title_if_needed(session_id, message).await;

        // 2. Load history. Already includes the user turn we just
        //    inserted, so the provider sees it as the last entry.
        let history = self
            .storage
            .get_messages(session_id)
            .await
            .map_err(|e| format!("storage error: {e}"))?;

        let mut chat_messages: Vec<ChatMessage> = history
            .iter()
            .filter_map(|m| match m.role.as_str() {
                "user" => Some(ChatMessage::user(m.content.clone())),
                "assistant" => Some(ChatMessage::assistant(m.content.clone())),
                // system / tool roles are not currently round-tripped
                // through the trait; the http path is for plain chat.
                _ => None,
            })
            .collect();
        let resolved_budget = match &self.context_window_budget {
            Some(budget) => Some(budget.clone()),
            None => self.provider_factory.context_window_budget().await,
        };
        if let Some(budget) = &resolved_budget {
            let compacted =
                compact_messages_for_window(&self.system_prompt, &chat_messages, budget);
            chat_messages = compacted.messages;
        }

        // 3. Provider call. The factory may fail (missing API key,
        //    misconfigured model) — surface as Error event then bail.
        let provider = match self.provider_factory.create() {
            Ok(p) => p,
            Err(e) => {
                let msg = format!("provider init failed: {e}");
                sink.emit_message(UnifiedMessage::Error {
                    message: msg.clone(),
                });
                return Err(msg);
            }
        };

        let mut stream = provider.chat_stream_dyn(
            &self.system_prompt,
            &chat_messages,
            &serde_json::Value::Null,
        );

        let mut buffer = String::new();
        while let Some(event) = stream.next().await {
            match event {
                LlmStreamEvent::Delta { text } => {
                    buffer.push_str(&text);
                    sink.emit_message(UnifiedMessage::TextDelta { text });
                }
                LlmStreamEvent::ToolCall { call } => {
                    sink.emit_message(UnifiedMessage::ToolUseStart {
                        id: call.id,
                        name: call.name,
                        input: call.arguments,
                    });
                }
                LlmStreamEvent::Error { message } => {
                    sink.emit_message(UnifiedMessage::Error {
                        message: message.clone(),
                    });
                    return Err(message);
                }
                LlmStreamEvent::End { total } => {
                    // Provider's accumulated text is authoritative if
                    // we somehow missed deltas (HTTP retries, etc.).
                    if buffer.is_empty() && !total.rendered_text.is_empty() {
                        buffer.push_str(&total.rendered_text);
                    }
                    let usage = match (total.input_tokens, total.output_tokens) {
                        (Some(i), Some(o)) => Some(TokenUsage {
                            input_tokens: u64::from(i),
                            output_tokens: u64::from(o),
                        }),
                        _ => None,
                    };
                    if !buffer.is_empty() {
                        if let Err(e) = self
                            .storage
                            .add_message(session_id, NewMessage::assistant(&buffer))
                            .await
                        {
                            // Persistence failure is loud — emit so the
                            // UI shows it, but don't replace the End
                            // event; turn still completed for the user.
                            sink.emit_message(UnifiedMessage::Error {
                                message: format!("persist assistant turn failed: {e}"),
                            });
                        }
                    }
                    sink.emit_message(UnifiedMessage::TurnEnd { usage });
                    return Ok(());
                }
                // `LlmStreamEvent` is `#[non_exhaustive]`. Newer
                // variants (Thinking, Usage in v0.5+) are intentionally
                // dropped — the unified-event vocabulary used by the
                // rest of the SDK doesn't have a slot for them yet.
                _ => {}
            }
        }

        // Stream ended without `End` — treat as a clean cutoff.
        // Persist whatever we accumulated so the conversation isn't
        // lost.
        if !buffer.is_empty() {
            let _ = self
                .storage
                .add_message(session_id, NewMessage::assistant(&buffer))
                .await;
        }
        sink.emit_message(UnifiedMessage::TurnEnd { usage: None });
        Ok(())
    }

    /// HTTP sessions hold no kernel resources — `stop_session` is a
    /// no-op kept for API parity with [`crate::manager::CliManager`].
    pub async fn stop_session(&self, _session_id: &str) -> Result<(), HttpError> {
        Ok(())
    }
}

/// Provider factory. Called once per `send_message` so hosts can
/// reconfigure the underlying provider (api key rotation, model
/// switch from an admin UI, etc.) without bouncing the manager.
///
/// Implementors typically capture an `Arc<RwLock<Config>>` and
/// resolve it on each call.
#[async_trait]
pub trait ProviderFactory: Send + Sync {
    fn create(&self) -> Result<Box<dyn LlmProviderDyn>, String>;

    async fn context_window_budget(&self) -> Option<ContextWindowBudget> {
        None
    }
}

/// Convenience: pin a single provider config at construction time.
/// Equivalent to calling `kangnam_router::create_provider` once and
/// returning a clone-able handle each turn — but `LlmProviderDyn`
/// isn't `Clone`, so we re-construct via [`StaticConfig`] every call.
pub struct StaticConfig {
    pub provider: String,
    pub api_key: String,
    pub model: String,
    pub base_url: String,
}

#[async_trait]
impl ProviderFactory for StaticConfig {
    fn create(&self) -> Result<Box<dyn LlmProviderDyn>, String> {
        kangnam_router::create_provider(&self.provider, &self.api_key, &self.model, &self.base_url)
            .map_err(|e| e.to_string())
    }

    async fn context_window_budget(&self) -> Option<ContextWindowBudget> {
        kangnam_router::context::resolve_model_context_window_tokens(
            &self.provider,
            &self.api_key,
            &self.model,
            &self.base_url,
        )
        .await
        .ok()
        .flatten()
        .map(ContextWindowBudget::new)
    }
}

// Allow plain closures so tests can inline a one-liner factory.
#[async_trait]
impl<F> ProviderFactory for F
where
    F: Fn() -> Result<Box<dyn LlmProviderDyn>, String> + Send + Sync,
{
    fn create(&self) -> Result<Box<dyn LlmProviderDyn>, String> {
        self()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::sync::Mutex;

    /// Captures every event for assertions.
    struct VecSink {
        events: Mutex<Vec<UnifiedMessage>>,
    }

    impl VecSink {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                events: Mutex::new(Vec::new()),
            })
        }
        fn drain(&self) -> Vec<UnifiedMessage> {
            std::mem::take(&mut *self.events.lock().unwrap())
        }
    }

    impl AgentEventSink for VecSink {
        fn emit_message(&self, msg: UnifiedMessage) {
            self.events.lock().unwrap().push(msg);
        }
    }

    async fn fresh_storage() -> Arc<dyn Storage> {
        let s = kangnam_chat_core::sqlite_storage::SqliteStorage::in_memory().unwrap();
        s.migrate().await.unwrap();
        Arc::new(s)
    }

    fn dummy_factory() -> Arc<dyn ProviderFactory> {
        Arc::new(|| kangnam_router::create_provider("dummy", "", "", "").map_err(|e| e.to_string()))
    }

    #[derive(Clone, Default)]
    struct RecordingProvider {
        observed: Arc<Mutex<Vec<Vec<ChatMessage>>>>,
    }

    impl RecordingProvider {
        fn observed(&self) -> Vec<Vec<ChatMessage>> {
            self.observed.lock().unwrap().clone()
        }
    }

    impl LlmProviderDyn for RecordingProvider {
        fn render_dyn(
            &self,
            _system_prompt: &str,
            _user_input: &str,
            _result_json: &serde_json::Value,
        ) -> Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<kangnam_router::LlmResponse, kangnam_router::LlmError>,
                    > + Send
                    + '_,
            >,
        > {
            Box::pin(async {
                let mut response = kangnam_router::LlmResponse::default();
                response.rendered_text = "recorded".into();
                Ok(response)
            })
        }

        fn chat_dyn(
            &self,
            _system_prompt: &str,
            messages: &[ChatMessage],
            _result_json: &serde_json::Value,
        ) -> Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<kangnam_router::LlmResponse, kangnam_router::LlmError>,
                    > + Send
                    + '_,
            >,
        > {
            self.observed.lock().unwrap().push(messages.to_vec());
            Box::pin(async {
                let mut response = kangnam_router::LlmResponse::default();
                response.rendered_text = "recorded".into();
                Ok(response)
            })
        }
    }

    #[tokio::test]
    async fn start_session_emits_init_and_creates_row() {
        let storage = fresh_storage().await;
        let mgr = HttpSessionManager::new(storage.clone(), dummy_factory());
        let sink = VecSink::new();

        mgr.start_session("conv-1", "claude", sink.clone())
            .await
            .unwrap();

        let events = sink.drain();
        assert!(
            matches!(events.as_slice(), [UnifiedMessage::SessionInit { session_id }] if session_id == "conv-1")
        );
        assert!(storage.conversation_exists("conv-1").await.unwrap());
    }

    #[tokio::test]
    async fn send_message_persists_user_and_assistant_turns() {
        let storage = fresh_storage().await;
        let mgr = HttpSessionManager::new(storage.clone(), dummy_factory());
        let sink = VecSink::new();

        mgr.start_session("conv-1", "dummy", sink.clone())
            .await
            .unwrap();
        sink.drain(); // discard SessionInit
        mgr.send_message("conv-1", "hello", sink.clone())
            .await
            .unwrap();

        // dummy provider echoes the input; we don't assert exact
        // text (provider-defined) but we DO assert structure.
        let events = sink.drain();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, UnifiedMessage::TextDelta { .. }))
        );
        assert!(matches!(
            events.last(),
            Some(UnifiedMessage::TurnEnd { .. })
        ));

        let messages = storage.get_messages("conv-1").await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "hello");
        assert_eq!(messages[1].role, "assistant");
        assert!(!messages[1].content.is_empty());
    }

    #[tokio::test]
    async fn second_turn_includes_history() {
        let storage = fresh_storage().await;
        let mgr = HttpSessionManager::new(storage.clone(), dummy_factory());
        let sink = VecSink::new();

        mgr.start_session("conv-1", "dummy", sink.clone())
            .await
            .unwrap();
        mgr.send_message("conv-1", "first", sink.clone())
            .await
            .unwrap_or_default();
        mgr.send_message("conv-1", "second", sink.clone())
            .await
            .unwrap_or_default();

        let messages = storage.get_messages("conv-1").await.unwrap();
        // 2 user + 2 assistant
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].content, "first");
        assert_eq!(messages[2].content, "second");
    }

    #[tokio::test]
    async fn context_window_compacts_history_before_http_provider_call() {
        let storage = fresh_storage().await;
        storage.ensure_conversation("conv-1", "test").await.unwrap();
        storage
            .add_message("conv-1", NewMessage::user(&"old user ".repeat(200)))
            .await
            .unwrap();
        storage
            .add_message(
                "conv-1",
                NewMessage::assistant(&"old assistant ".repeat(200)),
            )
            .await
            .unwrap();

        let provider = RecordingProvider::default();
        let observer = provider.clone();
        let factory: Arc<dyn ProviderFactory> = Arc::new(move || {
            let boxed: Box<dyn LlmProviderDyn> = Box::new(provider.clone());
            Ok(boxed)
        });
        let mgr = HttpSessionManager::new(storage.clone(), factory).with_context_window_budget(
            ContextWindowBudget {
                max_context_tokens: 180,
                reserve_output_tokens: 20,
                min_recent_messages: 1,
                max_summary_tokens: 40,
            },
        );
        let sink = VecSink::new();

        mgr.send_message("conv-1", "latest question", sink)
            .await
            .unwrap();

        let observed = observer.observed();
        assert_eq!(observed.len(), 1);
        assert_eq!(observed[0].len(), 2);
        assert!(observed[0][0].text_content().contains("compressed"));
        assert_eq!(observed[0][1].text_content(), "latest question");
    }

    #[tokio::test]
    async fn auto_titles_from_first_user_message() {
        let storage = fresh_storage().await;
        let mgr = HttpSessionManager::new(storage.clone(), dummy_factory());
        let sink = VecSink::new();

        mgr.start_session("conv-1", "dummy", sink.clone())
            .await
            .unwrap();
        mgr.send_message("conv-1", "tell me about saju", sink.clone())
            .await
            .unwrap_or_default();

        let conv = storage.get_conversation("conv-1").await.unwrap().unwrap();
        assert_eq!(conv.title, "tell me about saju");
    }

    #[tokio::test]
    async fn provider_init_failure_emits_error() {
        let storage = fresh_storage().await;
        let bad_factory: Arc<dyn ProviderFactory> =
            Arc::new(|| -> Result<Box<dyn LlmProviderDyn>, String> {
                Err("api key missing".into())
            });
        let mgr = HttpSessionManager::new(storage.clone(), bad_factory);
        let sink = VecSink::new();

        mgr.start_session("conv-1", "dummy", sink.clone())
            .await
            .unwrap();
        sink.drain();
        let result = mgr.send_message("conv-1", "hi", sink.clone()).await;

        assert!(result.is_err());
        let events = sink.drain();
        assert!(matches!(events.last(), Some(UnifiedMessage::Error { .. })));

        // User turn was already persisted before the call failed —
        // the conversation isn't lost.
        let messages = storage.get_messages("conv-1").await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
    }
}
