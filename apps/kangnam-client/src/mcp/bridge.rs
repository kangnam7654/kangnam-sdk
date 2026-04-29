#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

use super::types::*;

type PendingMap = Arc<Mutex<HashMap<String, oneshot::Sender<Result<serde_json::Value, String>>>>>;

/// JSON-RPC bridge to the MCP sidecar process
pub struct McpBridge {
    process: Mutex<Option<Child>>,
    pending: PendingMap,
}

#[derive(Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    method: String,
    params: serde_json::Value,
    id: String,
}

#[derive(Deserialize)]
struct JsonRpcResponse {
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
    id: String,
}

#[derive(Deserialize)]
struct JsonRpcError {
    message: String,
}

impl McpBridge {
    pub fn new() -> Self {
        Self {
            process: Mutex::new(None),
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Start the sidecar process
    pub fn start(&self) -> Result<(), String> {
        let sidecar_path = Self::find_sidecar()?;

        let mut child = Command::new(&sidecar_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("Failed to start MCP sidecar: {e}"))?;

        let stdout = child.stdout.take().ok_or("No stdout from sidecar")?;

        let pending = Arc::clone(&self.pending);

        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let line = match line {
                    Ok(l) => l,
                    Err(_) => break,
                };
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let resp: JsonRpcResponse = match serde_json::from_str(trimmed) {
                    Ok(r) => r,
                    Err(_) => continue,
                };

                if let Some(tx) = pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&resp.id) {
                    if let Some(err) = resp.error {
                        let _ = tx.send(Err(err.message));
                    } else {
                        let _ = tx.send(Ok(resp.result.unwrap_or(serde_json::Value::Null)));
                    }
                }
            }
        });

        *self.process.lock().unwrap_or_else(|e| e.into_inner()) = Some(child);
        Ok(())
    }

    /// Send a JSON-RPC request and wait for response
    pub async fn request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        // Auto-start if not running
        {
            let proc = self.process.lock().unwrap_or_else(|e| e.into_inner());
            if proc.is_none() {
                drop(proc);
                self.start()?;
            }
        }

        let id = uuid::Uuid::new_v4().to_string();
        let req = JsonRpcRequest {
            jsonrpc: "2.0",
            method: method.to_string(),
            params,
            id: id.clone(),
        };

        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).insert(id, tx);

        // Write to stdin
        {
            let mut proc = self.process.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(ref mut child) = *proc {
                if let Some(ref mut stdin) = child.stdin {
                    let json = serde_json::to_string(&req).map_err(|e| e.to_string())?;
                    writeln!(stdin, "{json}").map_err(|e| format!("Failed to write to sidecar: {e}"))?;
                    stdin.flush().map_err(|e| format!("Failed to flush: {e}"))?;
                }
            }
        }

        // Wait for response with timeout
        match tokio::time::timeout(tokio::time::Duration::from_secs(30), rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("Sidecar channel closed".to_string()),
            Err(_) => Err("Request timed out after 30s".to_string()),
        }
    }

    pub async fn list_tools(&self) -> Result<Vec<AggregatedTool>, String> {
        let result = self.request("mcp:list-tools", serde_json::json!({})).await?;
        serde_json::from_value(result).map_err(|e| e.to_string())
    }

    pub async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<ToolCallResult, String> {
        let result = self
            .request("mcp:call-tool", serde_json::json!({ "name": name, "arguments": arguments }))
            .await?;
        serde_json::from_value(result).map_err(|e| e.to_string())
    }

    pub async fn server_status(&self) -> Result<Vec<ServerStatus>, String> {
        let result = self.request("mcp:server-status", serde_json::json!({})).await?;
        serde_json::from_value(result).map_err(|e| e.to_string())
    }

    pub async fn load_config(&self) -> Result<serde_json::Value, String> {
        self.request("mcp:load-config", serde_json::json!({})).await
    }

    pub fn stop(&self) {
        if let Some(mut child) = self.process.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = child.kill();
        }
    }

    fn find_sidecar() -> Result<String, String> {
        // Production: compiled binary next to the app
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let binary = dir.join("mcp-bridge");
                if binary.exists() {
                    return Ok(binary.to_string_lossy().to_string());
                }
            }
        }

        // Development: npx tsx to run TypeScript directly
        let workspace_sidecar = std::env::current_dir()
            .unwrap_or_default()
            .join("sidecar/mcp-bridge.ts");
        if workspace_sidecar.exists() {
            return Ok("npx".to_string());
        }

        Err("MCP sidecar not found. Build the sidecar first.".to_string())
    }
}

impl Drop for McpBridge {
    fn drop(&mut self) {
        self.stop();
    }
}
