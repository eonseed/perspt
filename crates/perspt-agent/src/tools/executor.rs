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
}

impl AgentTools {
    /// Create new agent tools instance
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
    /// Read a file's contents
    pub(crate) fn read_file(&self, call: &ToolCall) -> ToolResult {
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
    pub(crate) fn search_code(&self, call: &ToolCall) -> ToolResult {
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
    /// List files in a directory
    pub(crate) fn list_files(&self, call: &ToolCall) -> ToolResult {
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

    /// Create or replace a whole file, creating parent directories as needed
    pub(crate) fn write_file(&self, call: &ToolCall) -> ToolResult {
        let path = match call.arguments.get("path") {
            Some(p) => self.resolve_path(p),
            None => {
                return ToolResult::failure("write_file", "Missing 'path' argument".to_string())
            }
        };
        let content = match call.arguments.get("content") {
            Some(c) => c,
            None => {
                return ToolResult::failure("write_file", "Missing 'content' argument".to_string())
            }
        };
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                return ToolResult::failure(
                    "write_file",
                    format!("Failed to create directories: {}", e),
                );
            }
        }
        match fs::write(&path, content) {
            Ok(_) => ToolResult::success("write_file", format!("Successfully wrote {:?}", path)),
            Err(e) => {
                ToolResult::failure("write_file", format!("Failed to write {:?}: {}", path, e))
            }
        }
    }

    /// Delete a file from the workspace
    pub(crate) fn delete_file(&self, call: &ToolCall) -> ToolResult {
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
    pub(crate) fn move_file(&self, call: &ToolCall) -> ToolResult {
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
