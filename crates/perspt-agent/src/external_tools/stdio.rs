//! Newline-delimited JSON-RPC MCP stdio transport.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use super::protocol::{notification_value, request_value, response_result, McpTransport};
use perspt_core::ExternalToolConfig;

pub struct StdioTransport {
    child: Child,
    stdin: Option<BufWriter<ChildStdin>>,
    stdout: BufReader<ChildStdout>,
    stderr: Arc<Mutex<Vec<u8>>>,
    next_id: u64,
}

impl std::fmt::Debug for StdioTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StdioTransport")
            .field("child_id", &self.child.id())
            .finish_non_exhaustive()
    }
}

impl StdioTransport {
    pub async fn spawn(config: &ExternalToolConfig) -> Result<Self> {
        let (program, arguments) = config
            .command
            .split_first()
            .context("stdio MCP command is empty")?;
        let mut command = Command::new(program);
        command
            .args(arguments)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        for (destination, source) in &config.env_from_env {
            let value = std::env::var(source).with_context(|| {
                format!("stdio MCP environment source {source:?} is unavailable")
            })?;
            command.env(destination, value);
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("starting stdio MCP server {:?}", config.id))?;
        let stdin = child.stdin.take().context("MCP child stdin unavailable")?;
        let stdout = child
            .stdout
            .take()
            .context("MCP child stdout unavailable")?;
        let stderr_pipe = child
            .stderr
            .take()
            .context("MCP child stderr unavailable")?;
        let stderr = Arc::new(Mutex::new(Vec::new()));
        capture_stderr(stderr_pipe, stderr.clone(), config.max_stderr_bytes);
        Ok(Self {
            child,
            stdin: Some(BufWriter::new(stdin)),
            stdout: BufReader::new(stdout),
            stderr,
            next_id: 1,
        })
    }

    async fn write_value(&mut self, value: &serde_json::Value) -> Result<()> {
        let stdin = self.stdin.as_mut().context("MCP stdin is closed")?;
        let mut bytes = serde_json::to_vec(value)?;
        bytes.push(b'\n');
        stdin.write_all(&bytes).await?;
        stdin.flush().await?;
        Ok(())
    }

    async fn read_response(
        &mut self,
        expected_id: u64,
        timeout: Duration,
        max_bytes: usize,
    ) -> Result<serde_json::Value> {
        let read = async {
            let mut received = 0usize;
            loop {
                let mut line = String::new();
                let count = self.stdout.read_line(&mut line).await?;
                anyhow::ensure!(count > 0, "MCP stdio server disconnected");
                received = received.saturating_add(count);
                anyhow::ensure!(
                    received <= max_bytes,
                    "MCP stdio response exceeded byte cap"
                );
                let value: serde_json::Value = serde_json::from_str(&line)
                    .context("MCP server emitted non-protocol stdout")?;
                if value.get("method").is_some() {
                    self.reject_server_request(&value).await?;
                    continue;
                }
                return response_result(value, expected_id);
            }
        };
        tokio::time::timeout(timeout, read)
            .await
            .context("MCP stdio request timed out")?
    }

    async fn reject_server_request(&mut self, value: &serde_json::Value) -> Result<()> {
        let Some(id) = value.get("id") else {
            return Ok(());
        };
        self.write_value(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": -32601,
                "message": "Perspt does not enable server-initiated MCP requests",
            }
        }))
        .await
    }
}

fn capture_stderr(
    mut pipe: tokio::process::ChildStderr,
    captured: Arc<Mutex<Vec<u8>>>,
    maximum: usize,
) {
    tokio::spawn(async move {
        let mut chunk = [0u8; 4096];
        while let Ok(count) = pipe.read(&mut chunk).await {
            if count == 0 {
                break;
            }
            let mut output = captured.lock().expect("stderr capture poisoned");
            output.extend_from_slice(&chunk[..count]);
            if output.len() > maximum {
                let excess = output.len() - maximum;
                output.drain(..excess);
            }
        }
    });
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn request(
        &mut self,
        method: &str,
        params: serde_json::Value,
        timeout: Duration,
        max_bytes: usize,
    ) -> Result<serde_json::Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.write_value(&request_value(id, method, params)).await?;
        let result = self.read_response(id, timeout, max_bytes).await;
        if result.is_err() {
            let _ = self
                .notify(
                    "notifications/cancelled",
                    serde_json::json!({"requestId": id}),
                )
                .await;
        }
        result
    }

    async fn notify(&mut self, method: &str, params: serde_json::Value) -> Result<()> {
        self.write_value(&notification_value(method, params)).await
    }

    async fn shutdown(&mut self) -> Result<()> {
        if let Some(mut stdin) = self.stdin.take() {
            let _ = stdin.shutdown().await;
        }
        if tokio::time::timeout(Duration::from_millis(750), self.child.wait())
            .await
            .is_err()
        {
            self.child.start_kill()?;
            let _ = tokio::time::timeout(Duration::from_secs(2), self.child.wait()).await;
        }
        Ok(())
    }

    fn stderr_tail(&self) -> Vec<u8> {
        self.stderr.lock().expect("stderr capture poisoned").clone()
    }
}
