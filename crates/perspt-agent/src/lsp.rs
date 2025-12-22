//! Native LSP Client
//!
//! Lightweight JSON-RPC client for Language Server Protocol communication.
//! Provides the "Sensor Architecture" for SRBN stability monitoring.

use anyhow::{Context, Result};
use lsp_types::{Diagnostic, DiagnosticSeverity, InitializeParams, InitializeResult};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use url::Url;

/// LSP Client for real-time diagnostics and symbol information
pub struct LspClient {
    /// Server process
    process: Option<Child>,
    /// Request ID counter
    request_id: AtomicU64,
    /// Cached diagnostics per file
    diagnostics: Arc<Mutex<HashMap<String, Vec<Diagnostic>>>>,
    /// Server name (e.g., "rust-analyzer", "pyright")
    server_name: String,
    /// Whether the server is initialized
    initialized: bool,
}

impl LspClient {
    /// Create a new LSP client (not connected)
    pub fn new(server_name: &str) -> Self {
        Self {
            process: None,
            request_id: AtomicU64::new(1),
            diagnostics: Arc::new(Mutex::new(HashMap::new())),
            server_name: server_name.to_string(),
            initialized: false,
        }
    }

    /// Get the command for a known language server
    fn get_server_command(server_name: &str) -> Option<(&'static str, Vec<&'static str>)> {
        match server_name {
            "rust-analyzer" => Some(("rust-analyzer", vec![])),
            "pyright" => Some(("pyright-langserver", vec!["--stdio"])),
            "typescript" => Some(("typescript-language-server", vec!["--stdio"])),
            "gopls" => Some(("gopls", vec!["serve"])),
            _ => None,
        }
    }

    /// Start the language server process
    pub async fn start(&mut self, workspace_root: &Path) -> Result<()> {
        let (cmd, args) = Self::get_server_command(&self.server_name)
            .context(format!("Unknown language server: {}", self.server_name))?;

        log::info!("Starting LSP server: {} {:?}", cmd, args);

        let child = Command::new(cmd)
            .args(&args)
            .current_dir(workspace_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context(format!("Failed to start {}", cmd))?;

        self.process = Some(child);
        self.initialized = false;

        // Send initialize request
        self.initialize(workspace_root).await?;

        Ok(())
    }

    /// Send the initialize request
    #[allow(deprecated)]
    async fn initialize(&mut self, workspace_root: &Path) -> Result<InitializeResult> {
        // Build a file:// URI from the path
        let path_str = workspace_root.to_string_lossy();
        #[cfg(target_os = "windows")]
        let uri_string = format!("file:///{}", path_str.replace('\\', "/"));
        #[cfg(not(target_os = "windows"))]
        let uri_string = format!("file://{}", path_str);

        let root_uri: lsp_types::Uri = uri_string
            .parse()
            .map_err(|e| anyhow::anyhow!("Failed to parse URI: {:?}", e))?;

        let params = InitializeParams {
            root_uri: Some(root_uri),
            capabilities: lsp_types::ClientCapabilities::default(),
            ..Default::default()
        };

        let result: InitializeResult = self
            .send_request("initialize", serde_json::to_value(params)?)
            .await?;

        // Send initialized notification
        self.send_notification("initialized", json!({})).await?;
        self.initialized = true;

        log::info!("LSP server initialized: {:?}", result.server_info);
        Ok(result)
    }

    /// Send a JSON-RPC request
    async fn send_request<T: for<'de> Deserialize<'de>>(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<T> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);

        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });

        let content = serde_json::to_string(&request)?;
        let message = format!("Content-Length: {}\r\n\r\n{}", content.len(), content);

        let process = self.process.as_mut().context("LSP server not started")?;
        let stdin = process.stdin.as_mut().context("No stdin")?;
        stdin.write_all(message.as_bytes())?;
        stdin.flush()?;

        // Read response (simplified - real impl would be async)
        let stdout = process.stdout.as_mut().context("No stdout")?;
        let mut reader = BufReader::new(stdout);

        // Read headers
        let mut content_length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            if line == "\r\n" {
                break;
            }
            if line.starts_with("Content-Length:") {
                content_length = line.trim_start_matches("Content-Length:").trim().parse()?;
            }
        }

        // Read body
        let mut body = vec![0u8; content_length];
        reader.read_exact(&mut body)?;

        let response: Value = serde_json::from_slice(&body)?;
        let result = response
            .get("result")
            .cloned()
            .context("No result in response")?;

        Ok(serde_json::from_value(result)?)
    }

    /// Send a JSON-RPC notification
    async fn send_notification(&mut self, method: &str, params: Value) -> Result<()> {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });

        let content = serde_json::to_string(&notification)?;
        let message = format!("Content-Length: {}\r\n\r\n{}", content.len(), content);

        let process = self.process.as_mut().context("LSP server not started")?;
        let stdin = process.stdin.as_mut().context("No stdin")?;
        stdin.write_all(message.as_bytes())?;
        stdin.flush()?;

        Ok(())
    }

    /// Get diagnostics for a file
    pub async fn get_diagnostics(&self, path: &str) -> Vec<Diagnostic> {
        self.diagnostics
            .lock()
            .await
            .get(path)
            .cloned()
            .unwrap_or_default()
    }

    /// Calculate syntactic energy from diagnostics
    ///
    /// V_syn = sum(severity_weight * count)
    /// Error = 1.0, Warning = 0.1, Hint = 0.01
    pub fn calculate_syntactic_energy(diagnostics: &[Diagnostic]) -> f32 {
        diagnostics
            .iter()
            .map(|d| match d.severity {
                Some(DiagnosticSeverity::ERROR) => 1.0,
                Some(DiagnosticSeverity::WARNING) => 0.1,
                Some(DiagnosticSeverity::INFORMATION) => 0.01,
                Some(DiagnosticSeverity::HINT) => 0.001,
                _ => 0.1, // Default for unknown or None
            })
            .sum()
    }

    /// Check if the server is running and initialized
    pub fn is_ready(&self) -> bool {
        self.initialized && self.process.is_some()
    }

    /// Shutdown the language server
    pub async fn shutdown(&mut self) -> Result<()> {
        // Just kill the process - proper shutdown would require refactoring
        // the I/O layer to be async
        if let Some(ref mut process) = self.process {
            let _ = process.kill();
        }
        self.process = None;
        self.initialized = false;
        Ok(())
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        if let Some(ref mut process) = self.process {
            let _ = process.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::Range;

    #[test]
    fn test_syntactic_energy_calculation() {
        let diagnostics = vec![
            Diagnostic {
                range: Range::default(),
                severity: Some(DiagnosticSeverity::ERROR),
                message: "error".to_string(),
                ..Default::default()
            },
            Diagnostic {
                range: Range::default(),
                severity: Some(DiagnosticSeverity::WARNING),
                message: "warning".to_string(),
                ..Default::default()
            },
        ];

        let energy = LspClient::calculate_syntactic_energy(&diagnostics);
        assert!((energy - 1.1).abs() < 0.001);
    }
}
