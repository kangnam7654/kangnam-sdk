//! `McpAgentTool` — adapter that exposes one MCP-server-advertised
//! tool through the [`AgentTool`] trait so it slots straight into a
//! [`crate::LlmAgent`] tool registry.

use std::marker::PhantomData;

use async_trait::async_trait;
use serde_json::Value;

use kangnam_harness_runtime::{AgentTool, ToolCtx, ToolResult};

use super::client::McpClient;

/// One tool from an MCP server, wired up as an `AgentTool<C>`.
///
/// The adapter is generic over the harness capability bundle `C` so
/// it composes with any `LlmAgent<C>`. The capability bundle is not
/// used during execution — MCP tools have access only to whatever
/// resources the server itself exposes — but the `PhantomData<C>`
/// makes the type parameter explicit at the call site.
///
/// Constructed via [`crate::LlmAgent::with_mcp_server_stdio`] or by
/// listing tools manually:
///
/// ```ignore
/// let client = McpClient::new_stdio("npx", &["-y", "@modelcontextprotocol/server-everything"], default()).await?;
/// for tool in client.list_tools().await? {
///     let adapter = McpAgentTool::new(client.clone(), tool);
///     agent = agent.with_boxed_tool(Arc::new(adapter), description);
/// }
/// ```
pub struct McpAgentTool<C = kangnam_harness_runtime::DefaultCapabilities> {
    client: McpClient,
    tool_name: String,
    description: String,
    input_schema: Value,
    _marker: PhantomData<fn(&C)>,
}

impl<C> McpAgentTool<C> {
    pub fn new(client: McpClient, tool: super::types::McpTool) -> Self {
        let description = tool.description.unwrap_or_default();
        Self {
            client,
            tool_name: tool.name,
            description,
            input_schema: tool.input_schema,
            _marker: PhantomData,
        }
    }

    /// The model-facing description, useful when registering the tool
    /// with [`crate::LlmAgent::with_boxed_tool`].
    pub fn description(&self) -> &str {
        &self.description
    }
}

#[async_trait]
impl<C: Send + Sync + 'static> AgentTool<C> for McpAgentTool<C> {
    fn name(&self) -> &str {
        &self.tool_name
    }

    fn parameters(&self) -> Value {
        self.input_schema.clone()
    }

    async fn execute(&self, params: Value, _ctx: &ToolCtx<C>) -> ToolResult {
        match self.client.call_tool(&self.tool_name, params).await {
            Ok(result) => {
                let text = result.flatten_text();
                if result.is_error {
                    ToolResult::Failed { error: text }
                } else {
                    ToolResult::Success {
                        content: Value::String(text),
                    }
                }
            }
            Err(e) => ToolResult::Failed {
                error: format!("MCP call_tool failed: {e}"),
            },
        }
    }
}
