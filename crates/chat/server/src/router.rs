use std::sync::Arc;

use axum::{routing::get, Router};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use crate::{mcp, ws, ServerConfig, ServerContext};

pub(crate) fn create_router(ctx: Arc<ServerContext>, config: &ServerConfig) -> Router {
    let mcp_state = Arc::new(mcp::McpState {
        ctx: ctx.clone(),
        server_name: config.server_name.clone(),
        server_version: config.server_version.clone(),
        tool_namespace: config.mcp_tool_namespace.clone(),
    });

    let mut router = Router::new()
        .route("/ws", get(ws::ws_handler))
        .route(
            "/mcp",
            get(mcp::mcp_sse_handler).post(mcp::mcp_handler),
        )
        .layer(CorsLayer::permissive())
        .with_state(RouterState { ctx, mcp: mcp_state });

    if let Some(ref dir) = config.static_dir {
        router = router.nest_service("/", ServeDir::new(dir));
    }

    router
}

#[derive(Clone)]
pub(crate) struct RouterState {
    pub ctx: Arc<ServerContext>,
    pub mcp: Arc<mcp::McpState>,
}

impl axum::extract::FromRef<RouterState> for Arc<ServerContext> {
    fn from_ref(input: &RouterState) -> Self {
        input.ctx.clone()
    }
}

impl axum::extract::FromRef<RouterState> for Arc<mcp::McpState> {
    fn from_ref(input: &RouterState) -> Self {
        input.mcp.clone()
    }
}
