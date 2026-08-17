//! Protocol-neutral MCP transport contract and wire helpers.

use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;

pub const MCP_PROTOCOL_VERSION: &str = "2025-11-25";

#[async_trait]
pub trait McpTransport: Send {
    async fn request(
        &mut self,
        method: &str,
        params: serde_json::Value,
        timeout: Duration,
        max_bytes: usize,
    ) -> Result<serde_json::Value>;

    async fn notify(&mut self, method: &str, params: serde_json::Value) -> Result<()>;

    async fn shutdown(&mut self) -> Result<()>;

    fn stderr_tail(&self) -> Vec<u8> {
        Vec::new()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpRemoteTool {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct McpToolPage {
    pub tools: Vec<McpRemoteTool>,
    #[serde(rename = "nextCursor")]
    pub next_cursor: Option<String>,
}

pub fn request_value(id: u64, method: &str, params: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    })
}

pub fn notification_value(method: &str, params: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
    })
}

pub fn response_result(value: serde_json::Value, expected_id: u64) -> Result<serde_json::Value> {
    anyhow::ensure!(
        value.get("jsonrpc").and_then(serde_json::Value::as_str) == Some("2.0"),
        "MCP peer emitted a non-JSON-RPC frame"
    );
    anyhow::ensure!(
        value.get("id").and_then(serde_json::Value::as_u64) == Some(expected_id),
        "MCP peer response id did not match request"
    );
    if let Some(error) = value.get("error") {
        anyhow::bail!("MCP request failed: {error}");
    }
    value
        .get("result")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("MCP response omitted result"))
}
