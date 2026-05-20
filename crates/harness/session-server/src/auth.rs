//! WebSocket-upgrade authentication hook.
//!
//! Hosts that run the chat server in a multi-tenant environment
//! (anything internet-facing) implement [`AuthHook`] and plug it into
//! [`crate::ServerContext::auth_hook`]. The handshake reads query
//! params + headers from the upgrade request, hands them to the
//! hook, and either:
//!
//! - On `Ok(UserContext)` — completes the upgrade and threads the
//!   resulting `user_id` into every JSON-RPC dispatch on that
//!   socket, where [`kangnam_harness_session_rpc::MessageGuard`] can use it for billing.
//! - On `Err(AuthError)` — refuses the upgrade with `401`.
//!
//! Hosts that don't need auth (desktop Tauri apps, local CLI tools)
//! leave `auth_hook = None` and the handshake passes through with no
//! user context. This preserves the v0.1 behaviour.

use std::collections::HashMap;

use async_trait::async_trait;

/// Identity attached to a socket after a successful authenticate call.
///
/// `user_id` is the only required field — guards downstream key off
/// it. `extensions` is a free-form bag for host-specific data
/// (tenant id, role, claims) that lives for the socket lifetime.
#[derive(Clone, Debug)]
pub struct UserContext {
    pub user_id: String,
    pub extensions: serde_json::Value,
}

impl UserContext {
    /// Construct with just a user id and an empty extensions bag.
    pub fn new(user_id: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            extensions: serde_json::Value::Null,
        }
    }
}

/// Reasons a hook can refuse a connection. The server maps each
/// variant to `401 Unauthorized` regardless — the variant exists
/// so logging and metrics can break down by cause.
#[derive(Debug, Clone)]
pub enum AuthError {
    MissingToken,
    InvalidToken(String),
    Expired,
    /// Host-specific failure (downstream auth service down, etc.).
    Other(String),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::MissingToken => f.write_str("missing token"),
            AuthError::InvalidToken(m) => write!(f, "invalid token: {m}"),
            AuthError::Expired => f.write_str("expired"),
            AuthError::Other(m) => write!(f, "auth error: {m}"),
        }
    }
}

impl std::error::Error for AuthError {}

/// Inputs available to the hook at upgrade time.
///
/// Both maps use lowercase header names / raw query keys as recorded
/// by axum. Borrow-only — the maps live for the duration of the
/// `authenticate` call.
pub struct AuthParams<'a> {
    pub query: &'a HashMap<String, String>,
    pub headers: &'a HashMap<String, String>,
}

/// Async upgrade-time check. The hook is invoked exactly once per
/// socket, before [`axum::extract::WebSocketUpgrade::on_upgrade`].
#[async_trait]
pub trait AuthHook: Send + Sync {
    async fn authenticate(&self, params: AuthParams<'_>) -> Result<UserContext, AuthError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedHook(Result<UserContext, AuthError>);

    #[async_trait]
    impl AuthHook for FixedHook {
        async fn authenticate(&self, _params: AuthParams<'_>) -> Result<UserContext, AuthError> {
            self.0.clone().map_err(|e| e)
        }
    }

    #[tokio::test]
    async fn ok_path_returns_user_context() {
        let hook = FixedHook(Ok(UserContext::new("u-1")));
        let params = AuthParams {
            query: &HashMap::new(),
            headers: &HashMap::new(),
        };
        let uc = hook.authenticate(params).await.unwrap();
        assert_eq!(uc.user_id, "u-1");
    }

    #[tokio::test]
    async fn err_path_propagates_variant() {
        let hook = FixedHook(Err(AuthError::Expired));
        let params = AuthParams {
            query: &HashMap::new(),
            headers: &HashMap::new(),
        };
        let err = hook.authenticate(params).await.unwrap_err();
        assert!(matches!(err, AuthError::Expired));
        assert_eq!(format!("{err}"), "expired");
    }

    #[test]
    fn user_context_new_initializes_extensions_null() {
        let uc = UserContext::new("u-1");
        assert!(uc.extensions.is_null());
    }
}
