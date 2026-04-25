pub mod dispatcher;
pub mod handlers;

// JSON-RPC types are owned by the `chat-core` crate. Re-exported here under
// the original `crate::rpc::types` path so existing imports keep working.
pub use chat_core::json_rpc as types;
