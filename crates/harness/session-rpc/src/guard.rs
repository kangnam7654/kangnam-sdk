//! Pluggable pre-message check.
//!
//! Hosts that bill per turn, enforce rate limits, or refuse messages
//! based on application-level state implement [`MessageGuard`] and
//! plug it into [`crate::DispatchContext::guard`]. The dispatcher
//! invokes it before forwarding `cli.sendMessage` to the manager.
//! On `Err`, the call is rejected with a structured JSON-RPC error
//! and never reaches the LLM provider.
//!
//! ## Why this lives in session-rpc, not session-server
//!
//! Tauri / standalone-binary hosts that don't use the WebSocket
//! server still want billing and rate-limit enforcement. Putting the
//! trait on the dispatcher means every transport gets it for free.
//! The companion `AuthHook` is in session-server because it only fires
//! during HTTP/WS upgrade — irrelevant for in-process transports.

use async_trait::async_trait;

use kangnam_harness_session_core::json_rpc::JsonRpcError;

/// Reasons a guard can reject a message.
///
/// Each variant maps to a distinct JSON-RPC error code so clients can
/// branch on `code` instead of parsing strings. The `data` field on
/// the resulting `JsonRpcError` carries structured detail
/// (`{required, balance}` for insufficient funds, etc.) so frontends
/// can render specific UI (top-up CTA, retry-after toast, etc.).
#[derive(Debug, Clone)]
pub enum GuardError {
    /// Guard requires authentication and the dispatcher had no user
    /// id (auth hook not configured, or session pre-dates auth).
    Unauthorized(String),
    /// User lacks the credit / point / token balance required for
    /// this turn. `required` and `balance` are returned in the
    /// JSON-RPC `error.data` so the client can render the gap.
    InsufficientFunds {
        required: i64,
        balance: i64,
        /// Human-readable summary; client may display verbatim.
        message: String,
    },
    /// Per-user / per-session rate limit hit.
    RateLimited(String),
    /// Catch-all for host-specific failures (DB error during charge,
    /// downstream service down, etc.). Mapped to JSON-RPC internal.
    Other(String),
}

impl GuardError {
    pub const CODE_UNAUTHORIZED: i32 = -32003;
    pub const CODE_INSUFFICIENT_FUNDS: i32 = -32004;
    pub const CODE_RATE_LIMITED: i32 = -32007;
}

impl From<GuardError> for JsonRpcError {
    fn from(e: GuardError) -> Self {
        match e {
            GuardError::Unauthorized(msg) => JsonRpcError {
                code: GuardError::CODE_UNAUTHORIZED,
                message: format!("unauthorized: {msg}"),
                data: None,
            },
            GuardError::InsufficientFunds {
                required,
                balance,
                message,
            } => JsonRpcError {
                code: GuardError::CODE_INSUFFICIENT_FUNDS,
                message,
                data: Some(serde_json::json!({
                    "required": required,
                    "balance": balance,
                })),
            },
            GuardError::RateLimited(msg) => JsonRpcError {
                code: GuardError::CODE_RATE_LIMITED,
                message: format!("rate limited: {msg}"),
                data: None,
            },
            GuardError::Other(msg) => JsonRpcError::internal(&msg),
        }
    }
}

/// Pre-message hook. Implement on a struct that captures whatever
/// host state you need (a DB pool, a billing client, etc.) and
/// register with [`crate::DispatchContext::guard`].
///
/// The hook is invoked for `cli.sendMessage` only — start/stop and
/// permission responses pass through unchecked. Authentication is
/// the WS-upgrade layer's job (`kangnam_harness_session_server::AuthHook`); by the time
/// a request reaches the guard the user is already known.
#[async_trait]
pub trait MessageGuard: Send + Sync {
    /// Decide whether to allow this turn. Return `Ok(())` to forward,
    /// `Err(GuardError)` to reject.
    ///
    /// `user_id` is `None` when the host has no auth hook — in that
    /// case guards that need a caller identity should return
    /// [`GuardError::Unauthorized`] explicitly.
    async fn check(
        &self,
        user_id: Option<&str>,
        session_id: &str,
        message: &str,
    ) -> Result<(), GuardError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insufficient_funds_carries_data() {
        let e = GuardError::InsufficientFunds {
            required: 10,
            balance: 3,
            message: "10P required, you have 3P".into(),
        };
        let rpc: JsonRpcError = e.into();
        assert_eq!(rpc.code, GuardError::CODE_INSUFFICIENT_FUNDS);
        let data = rpc.data.expect("data");
        assert_eq!(data["required"], 10);
        assert_eq!(data["balance"], 3);
    }

    #[test]
    fn unauthorized_maps_to_auth_code() {
        let rpc: JsonRpcError = GuardError::Unauthorized("missing token".into()).into();
        assert_eq!(rpc.code, GuardError::CODE_UNAUTHORIZED);
        assert!(rpc.message.contains("missing token"));
    }

    #[test]
    fn rate_limited_maps_to_rate_code() {
        let rpc: JsonRpcError = GuardError::RateLimited("60s".into()).into();
        assert_eq!(rpc.code, GuardError::CODE_RATE_LIMITED);
    }

    #[test]
    fn other_falls_back_to_internal() {
        let rpc: JsonRpcError = GuardError::Other("db down".into()).into();
        assert_eq!(rpc.code, -32603);
    }
}
