use super::*;

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
            "edit_file" => self.edit_file(call),
            "glob" => self.glob(call),
            "git_read" => self.git_read(call),
            "delete_file" => self.delete_file(call),
            "move_file" => self.move_file(call),
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
    pub(crate) fn apply_diff(&self, call: &ToolCall) -> ToolResult {
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

    /// Exact-string replace with a uniqueness check (PSP-9 system 5).
    ///
    /// Fails closed on ambiguity: zero matches means the model edited a
    /// stale view; more than one means the anchor is not unique. Both are
    /// returned as errors the harness converts into directed corrections.
    pub(crate) fn edit_file(&self, call: &ToolCall) -> ToolResult {
        let (path, old, new) = match (
            call.arguments.get("path"),
            call.arguments.get("old_string"),
            call.arguments.get("new_string"),
        ) {
            (Some(p), Some(o), Some(n)) => (self.resolve_path(p), o, n),
            _ => {
                return ToolResult::failure(
                    "edit_file",
                    "Missing required arguments: path, old_string, new_string".to_string(),
                )
            }
        };
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                return ToolResult::failure("edit_file", format!("Failed to read {path:?}: {e}"))
            }
        };
        match content.matches(old.as_str()).count() {
            0 => ToolResult::failure(
                "edit_file",
                "old_string not found; re-read the file — it may have changed".to_string(),
            ),
            1 => {
                let updated = content.replacen(old.as_str(), new, 1);
                match fs::write(&path, updated) {
                    Ok(_) => ToolResult::success("edit_file", format!("Edited {path:?}")),
                    Err(e) => {
                        ToolResult::failure("edit_file", format!("Failed to write {path:?}: {e}"))
                    }
                }
            }
            n => ToolResult::failure(
                "edit_file",
                format!("old_string matches {n} locations; provide a unique anchor"),
            ),
        }
    }

    /// Match files by glob pattern, newest first.
    pub(crate) fn glob(&self, call: &ToolCall) -> ToolResult {
        let Some(pattern) = call.arguments.get("pattern") else {
            return ToolResult::failure("glob", "Missing 'pattern' argument".to_string());
        };
        let mut matches: Vec<(std::time::SystemTime, String)> = Vec::new();
        collect_glob_matches(&self.working_dir, &self.working_dir, pattern, &mut matches);
        matches.sort_by_key(|(mtime, _)| std::cmp::Reverse(*mtime));
        let listing: Vec<String> = matches.into_iter().map(|(_, p)| p).collect();
        ToolResult::success("glob", listing.join("\n"))
    }

    /// Read repository state through git's read-only subcommands.
    pub(crate) fn git_read(&self, call: &ToolCall) -> ToolResult {
        let Some(subcommand) = call.arguments.get("subcommand") else {
            return ToolResult::failure("git_read", "Missing 'subcommand' argument".to_string());
        };
        if !matches!(subcommand.as_str(), "status" | "diff" | "log" | "show") {
            return ToolResult::failure(
                "git_read",
                format!(
                    "Subcommand {subcommand:?} is not read-only; allowed: status, diff, log, show"
                ),
            );
        }
        let mut args = vec![subcommand.clone()];
        if let Some(extra) = call.arguments.get("args") {
            args.extend(extra.split_whitespace().map(str::to_string));
        }
        match Command::new("git")
            .args(&args)
            .current_dir(&self.working_dir)
            .output()
        {
            Ok(out) if out.status.success() => {
                ToolResult::success("git_read", String::from_utf8_lossy(&out.stdout).to_string())
            }
            Ok(out) => {
                ToolResult::failure("git_read", String::from_utf8_lossy(&out.stderr).to_string())
            }
            Err(e) => ToolResult::failure("git_read", format!("git failed to start: {e}")),
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

        // Honor explicit working_dir from the caller (e.g. sandbox path),
        // falling back to self.working_dir (the main workspace).
        let effective_dir = call
            .arguments
            .get("working_dir")
            .map(PathBuf::from)
            .filter(|d| d.is_dir())
            .unwrap_or_else(|| self.working_dir.clone());

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

        self.spawn_and_stream(cmd_str, &effective_dir).await
    }

    /// Spawn a sanitized shell command and stream its output as log events.
    async fn spawn_and_stream(&self, cmd_str: &str, effective_dir: &Path) -> ToolResult {
        let mut child = match AsyncCommand::new("sh")
            .args(["-c", cmd_str])
            .current_dir(effective_dir)
            .env_remove("VIRTUAL_ENV")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => return ToolResult::failure("run_command", format!("Failed to spawn: {}", e)),
        };

        let stdout = child.stdout.take().expect("Failed to open stdout");
        let stderr = child.stderr.take().expect("Failed to open stderr");
        let stdout_handle = stream_lines(
            BufReader::new(stdout).lines(),
            self.event_sender.clone(),
            "",
        );
        let stderr_handle = stream_lines(
            BufReader::new(stderr).lines(),
            self.event_sender.clone(),
            "ERR: ",
        );

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

    /// Delete a file from the workspace
    fn delete_file(&self, call: &ToolCall) -> ToolResult {
        let path = match call.arguments.get("path") {
            Some(p) => self.resolve_path(p),
            None => {
                return ToolResult::failure("delete_file", "Missing 'path' argument".to_string())
            }
        };

        if !path.exists() {
            return ToolResult::success(
                "delete_file",
                format!("Path does not exist, nothing to delete: {:?}", path),
            );
        }

        if path.is_dir() {
            return ToolResult::failure(
                "delete_file",
                format!(
                    "Cannot delete directory {:?}; only files are supported",
                    path
                ),
            );
        }

        match std::fs::remove_file(&path) {
            Ok(()) => ToolResult::success("delete_file", format!("Deleted {:?}", path)),
            Err(e) => {
                ToolResult::failure("delete_file", format!("Failed to delete {:?}: {}", path, e))
            }
        }
    }

    /// Move/rename a file within the workspace
    fn move_file(&self, call: &ToolCall) -> ToolResult {
        let from = match call
            .arguments
            .get("path")
            .or_else(|| call.arguments.get("from"))
        {
            Some(p) => self.resolve_path(p),
            None => return ToolResult::failure("move_file", "Missing 'path' argument".to_string()),
        };
        let to = match call.arguments.get("to") {
            Some(p) => self.resolve_path(p),
            None => return ToolResult::failure("move_file", "Missing 'to' argument".to_string()),
        };

        if !from.exists() {
            return ToolResult::failure(
                "move_file",
                format!("Source path does not exist: {:?}", from),
            );
        }

        // Ensure destination parent directory exists
        if let Some(parent) = to.parent() {
            if !parent.exists() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    return ToolResult::failure(
                        "move_file",
                        format!("Failed to create destination directory {:?}: {}", parent, e),
                    );
                }
            }
        }

        match std::fs::rename(&from, &to) {
            Ok(()) => ToolResult::success("move_file", format!("Moved {:?} -> {:?}", from, to)),
            Err(e) => ToolResult::failure(
                "move_file",
                format!("Failed to move {:?} -> {:?}: {}", from, to, e),
            ),
        }
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

/// Read lines from a child pipe, forwarding each as a log event.
fn stream_lines<R>(
    mut reader: tokio::io::Lines<BufReader<R>>,
    sender: Option<perspt_core::events::channel::EventSender>,
    prefix: &'static str,
) -> tokio::task::JoinHandle<String>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut output = String::new();
        while let Ok(Some(line)) = reader.next_line().await {
            if let Some(ref s) = sender {
                let _ = s.send(perspt_core::AgentEvent::Log(format!("{prefix}{line}")));
            }
            output.push_str(&line);
            output.push('\n');
        }
        output
    })
}

/// Walk the workspace collecting files whose relative path matches `pattern`
/// (via the plan-validation glob), with modification times for sorting.
fn collect_glob_matches(
    root: &Path,
    dir: &Path,
    pattern: &str,
    matches: &mut Vec<(std::time::SystemTime, String)>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }
        if path.is_dir() {
            collect_glob_matches(root, &path, pattern, matches);
        } else if let Ok(rel) = path.strip_prefix(root) {
            let rel = rel.to_string_lossy().replace('\\', "/");
            if perspt_core::types::glob_matches(pattern, &rel) {
                let mtime = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                matches.push((mtime, rel));
            }
        }
    }
}
