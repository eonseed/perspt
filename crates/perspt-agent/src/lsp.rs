//! Native LSP Client
//!
//! Lightweight JSON-RPC client for Language Server Protocol communication.
//! Provides the "Sensor Architecture" for SRBN stability monitoring.

use anyhow::{Context, Result};
use lsp_types::{Diagnostic, DiagnosticSeverity, InitializeParams, InitializeResult};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, Mutex};

/// LSP Client for real-time diagnostics and symbol information
pub struct LspClient {
    /// Server stdin writer
    stdin: Option<Arc<Mutex<ChildStdin>>>,
    /// Request ID counter
    request_id: AtomicU64,
    /// Pending requests (ID -> Sender)
    pending_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>>,
    /// Cached diagnostics per file
    diagnostics: Arc<Mutex<HashMap<String, Vec<Diagnostic>>>>,
    /// Server name (e.g., "rust-analyzer", "pyright")
    server_name: String,
    /// Keep track of the process to kill it on drop
    process: Option<Child>,
    /// Whether the server is initialized
    initialized: bool,
}

impl LspClient {
    /// Create a new LSP client (not connected)
    pub fn new(server_name: &str) -> Self {
        Self {
            stdin: None,
            request_id: AtomicU64::new(1),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            diagnostics: Arc::new(Mutex::new(HashMap::new())),
            server_name: server_name.to_string(),
            process: None,
            initialized: false,
        }
    }

    /// Get the command for a known language server
    fn get_server_command(server_name: &str) -> Option<(&'static str, Vec<&'static str>)> {
        match server_name {
            "rust-analyzer" => Some(("rust-analyzer", vec![])),
            "pyright" => Some(("pyright-langserver", vec!["--stdio"])),
            // ty is installed via uv, so use uvx to run it
            "ty" => Some(("uvx", vec!["ty", "server"])),
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

        let mut child = Command::new(cmd)
            .args(&args)
            .current_dir(workspace_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context(format!("Failed to start {}", cmd))?;

        let stdin = child.stdin.take().context("No stdin")?;
        let stdout = child.stdout.take().context("No stdout")?;
        let stderr = child.stderr.take().context("No stderr")?;

        // Handle stderr logging in background
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                log::debug!("[LSP stderr] {}", line);
            }
        });

        // Handle stdout message loop
        let pending_requests = self.pending_requests.clone();
        let diagnostics = self.diagnostics.clone();

        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            loop {
                // Read headers
                let mut content_length = 0;
                loop {
                    let mut line = String::new();
                    match reader.read_line(&mut line).await {
                        Ok(0) => return, // EOF
                        Ok(_) => {
                            if line == "\r\n" {
                                break;
                            }
                            if line.starts_with("Content-Length:") {
                                if let Ok(len) = line
                                    .trim_start_matches("Content-Length:")
                                    .trim()
                                    .parse::<usize>()
                                {
                                    content_length = len;
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Error reading LSP header: {}", e);
                            return;
                        }
                    }
                }

                if content_length == 0 {
                    continue;
                }

                // Read body
                let mut body = vec![0u8; content_length];
                match reader.read_exact(&mut body).await {
                    Ok(_) => {
                        if let Ok(value) = serde_json::from_slice::<Value>(&body) {
                            Self::handle_message(value, &pending_requests, &diagnostics).await;
                        }
                    }
                    Err(e) => {
                        log::error!("Error reading LSP body: {}", e);
                        return;
                    }
                }
            }
        });

        self.stdin = Some(Arc::new(Mutex::new(stdin)));
        self.process = Some(child);
        self.initialized = false;

        // Send initialize request
        self.initialize(workspace_root).await?;

        Ok(())
    }

    /// Handle incoming LSP message
    async fn handle_message(
        msg: Value,
        pending_requests: &Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>>,
        diagnostics: &Arc<Mutex<HashMap<String, Vec<Diagnostic>>>>,
    ) {
        if let Some(id) = msg.get("id").and_then(|id| id.as_u64()) {
            // It's a response
            let mut pending = pending_requests.lock().await;
            if let Some(tx) = pending.remove(&id) {
                if let Some(error) = msg.get("error") {
                    let _ = tx.send(Err(anyhow::anyhow!("LSP error: {}", error)));
                } else if let Some(result) = msg.get("result") {
                    let _ = tx.send(Ok(result.clone()));
                } else {
                    let _ = tx.send(Ok(Value::Null));
                }
            }
        } else if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
            // It's a notification
            if method == "textDocument/publishDiagnostics" {
                if let Some(params) = msg.get("params") {
                    if let (Some(uri), Some(diags)) = (
                        params.get("uri").and_then(|u| u.as_str()),
                        params.get("diagnostics").and_then(|d| {
                            serde_json::from_value::<Vec<Diagnostic>>(d.clone()).ok()
                        }),
                    ) {
                        // Normalize URI to file path
                        let path = uri.trim_start_matches("file://");

                        // For ty/lsp, path might be absolute or relative, ensure we match broadly
                        // Store exactly what we got for now
                        let mut diag_map = diagnostics.lock().await;

                        // Also try to simplify the key to just filename for easier lookup
                        // This assumes unique filenames which is a simplification but useful
                        if let Some(filename) = Path::new(path).file_name() {
                            let filename_str = filename.to_string_lossy().to_string();
                            diag_map.insert(filename_str, diags.clone());
                        }

                        diag_map.insert(path.to_string(), diags);
                        log::info!("Updated diagnostics for {}", path);
                    }
                }
            }
        }
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
        let (tx, rx) = oneshot::channel();

        // Register pending request
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(id, tx);
        }

        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });

        if let Err(e) = self.write_message(&request).await {
            // Cleanup on error
            let mut pending = self.pending_requests.lock().await;
            pending.remove(&id);
            return Err(e);
        }

        // Wait for response
        let result = rx.await??;
        Ok(serde_json::from_value(result)?)
    }

    /// Send a JSON-RPC notification
    async fn send_notification(&mut self, method: &str, params: Value) -> Result<()> {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });

        self.write_message(&notification).await
    }

    /// Write a message to the server stdin
    async fn write_message(&mut self, msg: &Value) -> Result<()> {
        let content = serde_json::to_string(msg)?;
        let message = format!("Content-Length: {}\r\n\r\n{}", content.len(), content);

        if let Some(ref stdin_arc) = self.stdin {
            let mut stdin = stdin_arc.lock().await;
            stdin.write_all(message.as_bytes()).await?;
            stdin.flush().await?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("LSP stdin not available"))
        }
    }

    /// Get diagnostics for a file
    /// Get diagnostics for a file
    pub async fn get_diagnostics(&self, path: &str) -> Vec<Diagnostic> {
        let map = self.diagnostics.lock().await;

        // Collect cached diagnostics from any matching key
        let mut cached = Vec::new();

        // 1. Exact match
        if let Some(diags) = map.get(path) {
            cached = diags.clone();
        }
        // 2. Clean path (no file://)
        else if let Some(diags) = map.get(path.trim_start_matches("file://")) {
            cached = diags.clone();
        }
        // 3. URI format
        else if !path.starts_with("file://") {
            let uri = format!("file://{}", path);
            if let Some(diags) = map.get(&uri) {
                cached = diags.clone();
            }
        }

        // 4. Filename fallback (only if still empty/not found)
        if cached.is_empty() {
            if let Some(filename) = Path::new(path).file_name() {
                let filename_str = filename.to_string_lossy();
                if let Some(diags) = map.get(filename_str.as_ref()) {
                    cached = diags.clone();
                }
            }
        }

        // If we found diagnostics, verify they aren't empty?
        // Actually, valid code has empty diagnostics.
        // But for 'ty', we want to be paranoid if it reports nothing.
        if !cached.is_empty() {
            return cached;
        }

        // Trust but Verify: If cached is empty (meaning LSP says "clean" or "unknown"),
        // AND we are using `ty`, double-check with CLI.
        // This is crucial for stability monitoring to avoid false negatives from async races.
        if self.server_name == "ty" {
            // Drop lock before await
            drop(map);
            return self.run_type_check(path).await;
        }

        Vec::new()
    }

    /// Run ty check CLI to get diagnostics for a file
    async fn run_type_check(&self, path: &str) -> Vec<Diagnostic> {
        use std::process::Command;

        log::debug!("Running ty check on: {}", path);

        // ty check doesn't support JSON output yet, so we parse the default output
        let output = Command::new("uvx").args(["ty", "check", path]).output();

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                log::debug!("ty check status: {}", output.status);
                if !stdout.is_empty() {
                    log::debug!("ty check stdout: {}", stdout);
                }
                if !stderr.is_empty() {
                    log::debug!("ty check stderr: {}", stderr);
                }

                // Parse the output (ty outputs diagnostics to stderr in text format, but we check both)
                let combined = format!("{}\n{}", stdout, stderr);
                self.parse_ty_output(&combined, path)
            }
            Err(e) => {
                log::warn!("Failed to run ty check: {}", e);
                Vec::new()
            }
        }
    }

    /// Parse ty check text output into diagnostics
    fn parse_ty_output(&self, output: &str, _path: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Look for lines like:
        // error[invalid-return-type]: Return type does not match returned value
        // ---> main.py:7:12

        let lines: Vec<&str> = output.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];

            // Check for error/warning prefix
            if line.contains("error") || line.contains("warning") {
                // Heuristic parsing for severity
                let severity = if line.contains("error") {
                    Some(DiagnosticSeverity::ERROR)
                } else if line.contains("warning") {
                    Some(DiagnosticSeverity::WARNING)
                } else {
                    Some(DiagnosticSeverity::INFORMATION)
                };

                // Extract message
                // Try to find the message part after "error:" or "error[...]:"
                let message = if let Some(idx) = line.find("]: ") {
                    line[idx + 3..].to_string()
                } else if let Some(idx) = line.find(": ") {
                    line[idx + 2..].to_string()
                } else {
                    line.to_string()
                };

                // Look for location in next lines
                let mut line_num = 0;
                let mut col_num = 0;

                // Scan up to 3 following lines for location
                for j in 1..4 {
                    if i + j < lines.len() {
                        let next_line = lines[i + j];
                        if next_line.trim().starts_with("-->") {
                            // Parse:   --> main.py:7:12
                            // or: --> main.py:7:12
                            if let Some(parts) = next_line.split("-->").nth(1) {
                                let parts: Vec<&str> = parts.trim().split(':').collect();
                                if parts.len() >= 3 {
                                    // parts[0] is filename
                                    line_num = parts[1].parse().unwrap_or(0);
                                    col_num = parts[2].parse().unwrap_or(0);
                                }
                            }
                            break;
                        }
                    }
                }

                diagnostics.push(Diagnostic {
                    range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: if line_num > 0 { line_num - 1 } else { 0 },
                            character: if col_num > 0 { col_num - 1 } else { 0 },
                        },
                        end: lsp_types::Position {
                            line: if line_num > 0 { line_num - 1 } else { 0 },
                            character: if col_num > 0 { col_num } else { 1 },
                        },
                    },
                    severity,
                    message,
                    ..Default::default()
                });
            }

            i += 1;
        }

        if !diagnostics.is_empty() {
            log::info!("ty check found {} diagnostics", diagnostics.len());
        }

        diagnostics
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

    /// Notify language server that a file was opened
    pub async fn did_open(&mut self, path: &std::path::Path, content: &str) -> Result<()> {
        if !self.is_ready() {
            return Ok(());
        }

        let uri = format!("file://{}", path.display());
        let language_id = match path.extension().and_then(|e| e.to_str()) {
            Some("py") => "python",
            Some("rs") => "rust",
            Some("js") => "javascript",
            Some("ts") => "typescript",
            Some("go") => "go",
            _ => "python", // Default to python for now instead of plaintext
        };

        self.send_notification(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": language_id,
                    "version": 1,
                    "text": content
                }
            }),
        )
        .await
    }

    /// Notify language server that a file changed
    pub async fn did_change(
        &mut self,
        path: &std::path::Path,
        content: &str,
        version: i32,
    ) -> Result<()> {
        if !self.is_ready() {
            return Ok(());
        }

        let uri = format!("file://{}", path.display());

        self.send_notification(
            "textDocument/didChange",
            json!({
                "textDocument": {
                    "uri": uri,
                    "version": version
                },
                "contentChanges": [{
                    "text": content
                }]
            }),
        )
        .await
    }

    /// Shutdown the language server
    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(ref mut process) = self.process {
            let _ = process.kill().await;
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
