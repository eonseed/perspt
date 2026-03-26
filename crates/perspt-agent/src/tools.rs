//! Agent Tooling
//!
//! Tools available to agents for interacting with the workspace.
//! Implements: read_file, search_code, apply_patch, run_command

use diffy::{apply, Patch};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as AsyncCommand;

/// Tool result from agent execution
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub tool_name: String,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

impl ToolResult {
    pub fn success(tool_name: &str, output: String) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            success: true,
            output,
            error: None,
        }
    }

    pub fn failure(tool_name: &str, error: String) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            success: false,
            output: String::new(),
            error: Some(error),
        }
    }
}

/// Tool call request from LLM
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
    pub arguments: HashMap<String, String>,
}

/// Agent tools for workspace interaction
pub struct AgentTools {
    /// Working directory (sandbox root)
    working_dir: PathBuf,
    /// Whether to require user approval for commands
    require_approval: bool,
    /// Event sender for streaming output
    event_sender: Option<perspt_core::events::channel::EventSender>,
}

impl AgentTools {
    /// Create new agent tools instance
    pub fn new(working_dir: PathBuf, require_approval: bool) -> Self {
        Self {
            working_dir,
            require_approval,
            event_sender: None,
        }
    }

    /// Set event sender for streaming output
    pub fn set_event_sender(&mut self, sender: perspt_core::events::channel::EventSender) {
        self.event_sender = Some(sender);
    }

    /// Execute a tool call
    pub async fn execute(&self, call: &ToolCall) -> ToolResult {
        match call.name.as_str() {
            "read_file" => self.read_file(call),
            "search_code" => self.search_code(call),
            "apply_patch" => self.apply_patch(call),
            "run_command" => self.run_command(call).await,
            "list_files" => self.list_files(call),
            "write_file" => self.write_file(call),
            "apply_diff" => self.apply_diff(call),
            // Power Tools (OS-level)
            "sed_replace" => self.sed_replace(call),
            "awk_filter" => self.awk_filter(call),
            "diff_files" => self.diff_files(call),
            _ => ToolResult::failure(&call.name, format!("Unknown tool: {}", call.name)),
        }
    }

    /// Read a file's contents
    fn read_file(&self, call: &ToolCall) -> ToolResult {
        let path = match call.arguments.get("path") {
            Some(p) => self.resolve_path(p),
            None => return ToolResult::failure("read_file", "Missing 'path' argument".to_string()),
        };

        match fs::read_to_string(&path) {
            Ok(content) => ToolResult::success("read_file", content),
            Err(e) => ToolResult::failure("read_file", format!("Failed to read {:?}: {}", path, e)),
        }
    }

    /// Search for code patterns using grep
    fn search_code(&self, call: &ToolCall) -> ToolResult {
        let query = match call.arguments.get("query") {
            Some(q) => q,
            None => {
                return ToolResult::failure("search_code", "Missing 'query' argument".to_string())
            }
        };

        let path = call
            .arguments
            .get("path")
            .map(|p| self.resolve_path(p))
            .unwrap_or_else(|| self.working_dir.clone());

        // Use ripgrep if available, fallback to grep
        let output = Command::new("rg")
            .args(["--json", "-n", query])
            .current_dir(&path)
            .output()
            .or_else(|_| {
                Command::new("grep")
                    .args(["-rn", query, "."])
                    .current_dir(&path)
                    .output()
            });

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                ToolResult::success("search_code", stdout)
            }
            Err(e) => ToolResult::failure("search_code", format!("Search failed: {}", e)),
        }
    }

    /// Apply a patch to a file
    fn apply_patch(&self, call: &ToolCall) -> ToolResult {
        let path = match call.arguments.get("path") {
            Some(p) => self.resolve_path(p),
            None => {
                return ToolResult::failure("apply_patch", "Missing 'path' argument".to_string())
            }
        };

        let content = match call.arguments.get("content") {
            Some(c) => c,
            None => {
                return ToolResult::failure("apply_patch", "Missing 'content' argument".to_string())
            }
        };

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                return ToolResult::failure(
                    "apply_patch",
                    format!("Failed to create directories: {}", e),
                );
            }
        }

        match fs::write(&path, content) {
            Ok(_) => ToolResult::success("apply_patch", format!("Successfully wrote {:?}", path)),
            Err(e) => {
                ToolResult::failure("apply_patch", format!("Failed to write {:?}: {}", path, e))
            }
        }
    }

    /// Apply a unified diff patch to a file
    fn apply_diff(&self, call: &ToolCall) -> ToolResult {
        let path = match call.arguments.get("path") {
            Some(p) => self.resolve_path(p),
            None => {
                return ToolResult::failure("apply_diff", "Missing 'path' argument".to_string())
            }
        };

        let diff_content = match call.arguments.get("diff") {
            Some(c) => c,
            None => {
                return ToolResult::failure("apply_diff", "Missing 'diff' argument".to_string())
            }
        };

        // Read original file
        let original = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                // If file doesn't exist, we can't patch it.
                // (Unless it's a new file creation patch, but diffy usually assumes base text)
                return ToolResult::failure(
                    "apply_diff",
                    format!("Failed to read base file {:?}: {}", path, e),
                );
            }
        };

        // Parse patch
        let patch = match Patch::from_str(diff_content) {
            Ok(p) => p,
            Err(e) => {
                return ToolResult::failure("apply_diff", format!("Failed to parse diff: {}", e));
            }
        };

        // Apply patch
        match apply(&original, &patch) {
            Ok(patched) => match fs::write(&path, patched) {
                Ok(_) => {
                    ToolResult::success("apply_diff", format!("Successfully patched {:?}", path))
                }
                Err(e) => ToolResult::failure(
                    "apply_diff",
                    format!("Failed to write patched file: {}", e),
                ),
            },
            Err(e) => ToolResult::failure("apply_diff", format!("Failed to apply patch: {}", e)),
        }
    }

    /// Run a shell command (requires approval unless auto-approve is set)
    async fn run_command(&self, call: &ToolCall) -> ToolResult {
        let cmd_str = match call.arguments.get("command") {
            Some(c) => c,
            None => {
                return ToolResult::failure("run_command", "Missing 'command' argument".to_string())
            }
        };

        // PSP-5 Phase 4: Sanitize command through policy before execution
        match perspt_policy::sanitize_command(cmd_str) {
            Ok(sr) if sr.rejected => {
                return ToolResult::failure(
                    "run_command",
                    format!(
                        "Command rejected by policy: {}",
                        sr.rejection_reason
                            .unwrap_or_else(|| "unknown reason".to_string())
                    ),
                );
            }
            Ok(sr) => {
                for warning in &sr.warnings {
                    log::warn!("Command policy warning: {}", warning);
                }
            }
            Err(e) => {
                return ToolResult::failure(
                    "run_command",
                    format!("Command sanitization failed: {}", e),
                );
            }
        }

        // Validate workspace bounds
        if let Err(e) = perspt_policy::validate_workspace_bound(cmd_str, &self.working_dir) {
            return ToolResult::failure("run_command", format!("Command rejected: {}", e));
        }

        if self.require_approval {
            log::info!("Command requires approval: {}", cmd_str);
        }

        let mut child = match AsyncCommand::new("sh")
            .args(["-c", cmd_str])
            .current_dir(&self.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => return ToolResult::failure("run_command", format!("Failed to spawn: {}", e)),
        };

        let stdout = child.stdout.take().expect("Failed to open stdout");
        let stderr = child.stderr.take().expect("Failed to open stderr");
        let sender = self.event_sender.clone();

        let stdout_handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            let mut output = String::new();
            while let Ok(Some(line)) = reader.next_line().await {
                if let Some(ref s) = sender {
                    let _ = s.send(perspt_core::AgentEvent::Log(line.clone()));
                }
                output.push_str(&line);
                output.push('\n');
            }
            output
        });

        let sender_err = self.event_sender.clone();
        let stderr_handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            let mut output = String::new();
            while let Ok(Some(line)) = reader.next_line().await {
                if let Some(ref s) = sender_err {
                    let _ = s.send(perspt_core::AgentEvent::Log(format!("ERR: {}", line)));
                }
                output.push_str(&line);
                output.push('\n');
            }
            output
        });

        let status = match child.wait().await {
            Ok(s) => s,
            Err(e) => return ToolResult::failure("run_command", format!("Failed to wait: {}", e)),
        };

        let stdout_str = stdout_handle.await.unwrap_or_default();
        let stderr_str = stderr_handle.await.unwrap_or_default();

        if status.success() {
            ToolResult::success("run_command", stdout_str)
        } else {
            ToolResult::failure(
                "run_command",
                format!("Exit code: {:?}\n{}", status.code(), stderr_str),
            )
        }
    }

    /// List files in a directory
    fn list_files(&self, call: &ToolCall) -> ToolResult {
        let path = call
            .arguments
            .get("path")
            .map(|p| self.resolve_path(p))
            .unwrap_or_else(|| self.working_dir.clone());

        match fs::read_dir(&path) {
            Ok(entries) => {
                let files: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                            format!("{}/", name)
                        } else {
                            name
                        }
                    })
                    .collect();
                ToolResult::success("list_files", files.join("\n"))
            }
            Err(e) => {
                ToolResult::failure("list_files", format!("Failed to list {:?}: {}", path, e))
            }
        }
    }

    /// Write content to a file
    fn write_file(&self, call: &ToolCall) -> ToolResult {
        // Alias for apply_patch with different semantics
        self.apply_patch(call)
    }

    /// Resolve a path relative to working directory
    fn resolve_path(&self, path: &str) -> PathBuf {
        let p = Path::new(path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.working_dir.join(p)
        }
    }

    // =========================================================================
    // Power Tools (OS-level operations)
    // =========================================================================

    /// Replace text in a file using sed-like pattern matching
    fn sed_replace(&self, call: &ToolCall) -> ToolResult {
        let path = match call.arguments.get("path") {
            Some(p) => self.resolve_path(p),
            None => {
                return ToolResult::failure("sed_replace", "Missing 'path' argument".to_string())
            }
        };

        let pattern = match call.arguments.get("pattern") {
            Some(p) => p,
            None => {
                return ToolResult::failure("sed_replace", "Missing 'pattern' argument".to_string())
            }
        };

        let replacement = match call.arguments.get("replacement") {
            Some(r) => r,
            None => {
                return ToolResult::failure(
                    "sed_replace",
                    "Missing 'replacement' argument".to_string(),
                )
            }
        };

        // Read file, perform replacement, write back
        match fs::read_to_string(&path) {
            Ok(content) => {
                let new_content = content.replace(pattern, replacement);
                match fs::write(&path, &new_content) {
                    Ok(_) => ToolResult::success(
                        "sed_replace",
                        format!(
                            "Replaced '{}' with '{}' in {:?}",
                            pattern, replacement, path
                        ),
                    ),
                    Err(e) => ToolResult::failure("sed_replace", format!("Failed to write: {}", e)),
                }
            }
            Err(e) => {
                ToolResult::failure("sed_replace", format!("Failed to read {:?}: {}", path, e))
            }
        }
    }

    /// Filter file content using awk-like field selection
    fn awk_filter(&self, call: &ToolCall) -> ToolResult {
        let path = match call.arguments.get("path") {
            Some(p) => self.resolve_path(p),
            None => {
                return ToolResult::failure("awk_filter", "Missing 'path' argument".to_string())
            }
        };

        let filter = match call.arguments.get("filter") {
            Some(f) => f,
            None => {
                return ToolResult::failure("awk_filter", "Missing 'filter' argument".to_string())
            }
        };

        // Use awk command for filtering
        let output = Command::new("awk").arg(filter).arg(&path).output();

        match output {
            Ok(out) => {
                if out.status.success() {
                    ToolResult::success(
                        "awk_filter",
                        String::from_utf8_lossy(&out.stdout).to_string(),
                    )
                } else {
                    ToolResult::failure(
                        "awk_filter",
                        String::from_utf8_lossy(&out.stderr).to_string(),
                    )
                }
            }
            Err(e) => ToolResult::failure("awk_filter", format!("Failed to run awk: {}", e)),
        }
    }

    /// Show differences between two files
    fn diff_files(&self, call: &ToolCall) -> ToolResult {
        let file1 = match call.arguments.get("file1") {
            Some(p) => self.resolve_path(p),
            None => {
                return ToolResult::failure("diff_files", "Missing 'file1' argument".to_string())
            }
        };

        let file2 = match call.arguments.get("file2") {
            Some(p) => self.resolve_path(p),
            None => {
                return ToolResult::failure("diff_files", "Missing 'file2' argument".to_string())
            }
        };

        // Use diff command
        let output = Command::new("diff")
            .args([
                "--unified",
                &file1.to_string_lossy(),
                &file2.to_string_lossy(),
            ])
            .output();

        match output {
            Ok(out) => {
                // diff exits with 0 if files are same, 1 if different, 2 if error
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                if stdout.is_empty() {
                    ToolResult::success("diff_files", "Files are identical".to_string())
                } else {
                    ToolResult::success("diff_files", stdout)
                }
            }
            Err(e) => ToolResult::failure("diff_files", format!("Failed to run diff: {}", e)),
        }
    }
}

/// Get tool definitions for LLM function calling
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "read_file".to_string(),
            description: "Read the contents of a file".to_string(),
            parameters: vec![ToolParameter {
                name: "path".to_string(),
                description: "Path to the file to read".to_string(),
                required: true,
            }],
        },
        ToolDefinition {
            name: "search_code".to_string(),
            description: "Search for code patterns in the workspace using grep/ripgrep".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "query".to_string(),
                    description: "Search pattern (regex supported)".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "path".to_string(),
                    description: "Directory to search in (default: working directory)".to_string(),
                    required: false,
                },
            ],
        },
        ToolDefinition {
            name: "apply_patch".to_string(),
            description: "Write or replace file contents".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "path".to_string(),
                    description: "Path to the file to write".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "content".to_string(),
                    description: "New file contents".to_string(),
                    required: true,
                },
            ],
        },
        ToolDefinition {
            name: "apply_diff".to_string(),
            description: "Apply a Unified Diff patch to a file".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "path".to_string(),
                    description: "Path to the file to patch".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "diff".to_string(),
                    description: "Unified Diff content".to_string(),
                    required: true,
                },
            ],
        },
        ToolDefinition {
            name: "run_command".to_string(),
            description: "Execute a shell command in the working directory".to_string(),
            parameters: vec![ToolParameter {
                name: "command".to_string(),
                description: "Shell command to execute".to_string(),
                required: true,
            }],
        },
        ToolDefinition {
            name: "list_files".to_string(),
            description: "List files in a directory".to_string(),
            parameters: vec![ToolParameter {
                name: "path".to_string(),
                description: "Directory path (default: working directory)".to_string(),
                required: false,
            }],
        },
        // Power Tools
        ToolDefinition {
            name: "sed_replace".to_string(),
            description: "Replace text in a file using sed-like pattern matching".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "path".to_string(),
                    description: "Path to the file".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "pattern".to_string(),
                    description: "Search pattern".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "replacement".to_string(),
                    description: "Replacement text".to_string(),
                    required: true,
                },
            ],
        },
        ToolDefinition {
            name: "awk_filter".to_string(),
            description: "Filter file content using awk-like field selection".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "path".to_string(),
                    description: "Path to the file".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "filter".to_string(),
                    description: "Awk filter expression (e.g., '$1 == \"error\"')".to_string(),
                    required: true,
                },
            ],
        },
        ToolDefinition {
            name: "diff_files".to_string(),
            description: "Show differences between two files".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "file1".to_string(),
                    description: "First file path".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "file2".to_string(),
                    description: "Second file path".to_string(),
                    required: true,
                },
            ],
        },
    ]
}

/// Tool definition for LLM function calling
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Vec<ToolParameter>,
}

/// Tool parameter definition
#[derive(Debug, Clone)]
pub struct ToolParameter {
    pub name: String,
    pub description: String,
    pub required: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[tokio::test]
    async fn test_read_file() {
        let dir = temp_dir();
        let test_file = dir.join("test_read.txt");
        fs::write(&test_file, "Hello, World!").unwrap();

        let tools = AgentTools::new(dir.clone(), false);
        let call = ToolCall {
            name: "read_file".to_string(),
            arguments: [("path".to_string(), test_file.to_string_lossy().to_string())]
                .into_iter()
                .collect(),
        };

        let result = tools.execute(&call).await;
        assert!(result.success);
        assert_eq!(result.output, "Hello, World!");
    }

    #[tokio::test]
    async fn test_list_files() {
        let dir = temp_dir();
        let tools = AgentTools::new(dir.clone(), false);
        let call = ToolCall {
            name: "list_files".to_string(),
            arguments: HashMap::new(),
        };

        let result = tools.execute(&call).await;
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_apply_diff_tool() {
        use std::collections::HashMap;
        use std::io::Write;
        let temp_dir = temp_dir();
        let file_path = temp_dir.join("test_diff.txt");
        let mut file = std::fs::File::create(&file_path).unwrap();
        // Explicitly write bytes with unix newlines
        file.write_all(b"Hello world\nThis is a test\n").unwrap();

        let tools = AgentTools::new(temp_dir.clone(), true);

        // Exact string with newlines
        let diff = "--- test_diff.txt\n+++ test_diff.txt\n@@ -1,2 +1,2 @@\n-Hello world\n+Hello diffy\n This is a test\n";

        let mut args = HashMap::new();
        args.insert("path".to_string(), "test_diff.txt".to_string());
        args.insert("diff".to_string(), diff.to_string());

        let call = ToolCall {
            name: "apply_diff".to_string(),
            arguments: args,
        };

        let result = tools.apply_diff(&call);
        assert!(
            result.success,
            "Diff application failed: {:?}",
            result.error
        );

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Hello diffy\nThis is a test\n");
    }
}

// =============================================================================
// PSP-5 Phase 6: Sandbox workspace helpers
// =============================================================================

/// Create a sandbox workspace for provisional verification.
///
/// Copies key project files into a session-scoped temporary directory so
/// speculative verification does not pollute committed workspace state.
/// Returns the path to the sandbox root.
pub fn create_sandbox(
    working_dir: &Path,
    session_id: &str,
    branch_id: &str,
) -> std::io::Result<PathBuf> {
    let sandbox_root = working_dir
        .join(".perspt")
        .join("sandboxes")
        .join(session_id)
        .join(branch_id);

    fs::create_dir_all(&sandbox_root)?;

    log::debug!("Created sandbox workspace at {}", sandbox_root.display());

    Ok(sandbox_root)
}

/// Clean up a specific sandbox workspace.
pub fn cleanup_sandbox(sandbox_dir: &Path) -> std::io::Result<()> {
    if sandbox_dir.exists() {
        fs::remove_dir_all(sandbox_dir)?;
        log::debug!("Cleaned up sandbox at {}", sandbox_dir.display());
    }
    Ok(())
}

/// Clean up all sandbox workspaces for a session.
pub fn cleanup_session_sandboxes(working_dir: &Path, session_id: &str) -> std::io::Result<()> {
    let session_sandbox = working_dir
        .join(".perspt")
        .join("sandboxes")
        .join(session_id);

    if session_sandbox.exists() {
        fs::remove_dir_all(&session_sandbox)?;
        log::debug!("Cleaned up all sandboxes for session {}", session_id);
    }
    Ok(())
}

/// Copy a file from the workspace into a sandbox, preserving relative paths.
pub fn copy_to_sandbox(
    working_dir: &Path,
    sandbox_dir: &Path,
    relative_path: &str,
) -> std::io::Result<()> {
    let src = working_dir.join(relative_path);
    let dst = sandbox_dir.join(relative_path);

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }

    if src.exists() {
        fs::copy(&src, &dst)?;
    }
    Ok(())
}

/// Copy a file from a sandbox back to the live workspace, preserving relative paths.
pub fn copy_from_sandbox(
    sandbox_dir: &Path,
    working_dir: &Path,
    relative_path: &str,
) -> std::io::Result<()> {
    let src = sandbox_dir.join(relative_path);
    let dst = working_dir.join(relative_path);

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }

    if src.exists() {
        fs::copy(&src, &dst)?;
    }
    Ok(())
}

/// List all files in a sandbox directory as workspace-relative paths.
pub fn list_sandbox_files(sandbox_dir: &Path) -> std::io::Result<Vec<String>> {
    let mut files = Vec::new();
    if !sandbox_dir.exists() {
        return Ok(files);
    }
    fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) -> std::io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                walk(&path, base, out)?;
            } else if let Ok(rel) = path.strip_prefix(base) {
                out.push(rel.to_string_lossy().to_string());
            }
        }
        Ok(())
    }
    walk(sandbox_dir, sandbox_dir, &mut files)?;
    Ok(files)
}

#[cfg(test)]
mod sandbox_tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_sandbox() {
        let dir = tempdir().unwrap();
        let sandbox = create_sandbox(dir.path(), "sess1", "branch1").unwrap();
        assert!(sandbox.exists());
        assert!(sandbox.ends_with("sess1/branch1"));
    }

    #[test]
    fn test_cleanup_sandbox() {
        let dir = tempdir().unwrap();
        let sandbox = create_sandbox(dir.path(), "sess1", "branch1").unwrap();
        assert!(sandbox.exists());
        cleanup_sandbox(&sandbox).unwrap();
        assert!(!sandbox.exists());
    }

    #[test]
    fn test_cleanup_session_sandboxes() {
        let dir = tempdir().unwrap();
        create_sandbox(dir.path(), "sess1", "b1").unwrap();
        create_sandbox(dir.path(), "sess1", "b2").unwrap();
        let session_dir = dir.path().join(".perspt").join("sandboxes").join("sess1");
        assert!(session_dir.exists());
        cleanup_session_sandboxes(dir.path(), "sess1").unwrap();
        assert!(!session_dir.exists());
    }

    #[test]
    fn test_copy_to_sandbox() {
        let dir = tempdir().unwrap();
        // Create a source file
        let src_dir = dir.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("main.rs"), "fn main() {}").unwrap();

        let sandbox = create_sandbox(dir.path(), "sess1", "b1").unwrap();
        copy_to_sandbox(dir.path(), &sandbox, "src/main.rs").unwrap();

        let copied = sandbox.join("src/main.rs");
        assert!(copied.exists());
        assert_eq!(fs::read_to_string(copied).unwrap(), "fn main() {}");
    }
}
