use super::*;

/// Default `read_file` window when the model passes no `limit`.
const READ_DEFAULT_LIMIT: usize = 300;
/// Individual lines longer than this are truncated with an ellipsis.
const READ_MAX_LINE_CHARS: usize = 2000;
/// Search output is capped at this many lines before the omission note.
const GREP_MAX_LINES: usize = 200;
/// Largest honored `-C` context value for a search.
const GREP_MAX_CONTEXT: usize = 10;

/// Default listing window for `glob` and `list_files`.
const LIST_DEFAULT_LIMIT: usize = 100;

/// A 0-based (offset, limit) listing window.
fn parse_window(call: &ToolCall) -> Result<(usize, usize), String> {
    let offset = match call.arguments.get("offset") {
        None => 0usize,
        Some(raw) => raw
            .trim()
            .parse::<usize>()
            .map_err(|_| format!("'offset' must be a non-negative integer, got {raw:?}"))?,
    };
    let limit = match call.arguments.get("limit") {
        None => LIST_DEFAULT_LIMIT,
        Some(raw) => match raw.trim().parse::<usize>() {
            Ok(0) | Err(_) => return Err(format!("'limit' must be at least 1, got {raw:?}")),
            Ok(v) => v,
        },
    };
    Ok((offset, limit))
}

/// Render a bounded window of `entries` with a total header and, when the
/// window ends early, a continuation hint naming the next offset.
fn paged_listing(label: &str, entries: &[String], (offset, limit): (usize, usize)) -> String {
    let total = entries.len();
    if total == 0 {
        return format!("{label}: 0 entries");
    }
    if offset >= total {
        return format!("{label}: {total} entries total; offset {offset} is past the end");
    }
    let end = total.min(offset + limit);
    let mut output = format!("{label}: entries {offset}-{} of {total}\n", end - 1);
    output.push_str(&entries[offset..end].join("\n"));
    if end < total {
        output.push_str(&format!(
            "\n[{} more entries; continue with offset={end}]",
            total - end
        ));
    }
    output
}

/// Keep the first `GREP_MAX_LINES` output lines plus an omission note.
fn capped_search_output(stdout: &[u8]) -> String {
    let text = String::from_utf8_lossy(stdout);
    let lines: Vec<&str> = text.lines().collect();
    let mut kept: Vec<String> = lines
        .iter()
        .take(GREP_MAX_LINES)
        .map(|l| l.to_string())
        .collect();
    if lines.len() > GREP_MAX_LINES {
        kept.push(format!(
            "[{} more lines omitted; narrow the query, pass a file path, or lower context]",
            lines.len() - GREP_MAX_LINES
        ));
    }
    kept.join("\n")
}

/// Parse a 1-based positive integer line argument, defaulting when absent.
fn parse_line_argument(call: &ToolCall, name: &str, default: usize) -> Result<usize, String> {
    match call.arguments.get(name) {
        None => Ok(default),
        Some(raw) => match raw.trim().parse::<usize>() {
            Ok(0) => Err(format!("'{name}' must be at least 1")),
            Ok(v) => Ok(v),
            Err(_) => Err(format!("'{name}' must be a positive integer, got {raw:?}")),
        },
    }
}

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
    /// Read a windowed slice of a file as line-numbered text.
    ///
    /// Honors the catalog's `offset` (1-based first line) and `limit`
    /// arguments and always reports the total line count plus the offset
    /// that continues the read, so large files stay navigable page by page.
    pub(crate) fn read_file(&self, call: &ToolCall) -> ToolResult {
        let Some(raw_path) = call.arguments.get("path") else {
            return ToolResult::failure("read_file", "Missing 'path' argument".to_string());
        };
        let path = self.resolve_path(raw_path);
        let offset = match parse_line_argument(call, "offset", 1) {
            Ok(v) => v,
            Err(e) => return ToolResult::failure("read_file", e),
        };
        let limit = match parse_line_argument(call, "limit", READ_DEFAULT_LIMIT) {
            Ok(v) => v,
            Err(e) => return ToolResult::failure("read_file", e),
        };

        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(e) => {
                return ToolResult::failure(
                    "read_file",
                    format!("Failed to read {:?}: {}", path, e),
                )
            }
        };
        let lines: Vec<&str> = content.lines().collect();
        let total = lines.len();
        if offset > total {
            return ToolResult::success(
                "read_file",
                format!("file {raw_path}: {total} lines total; offset {offset} is past the end"),
            );
        }
        let end = total.min(offset - 1 + limit);
        let mut output = format!("file {raw_path}: lines {offset}-{end} of {total}\n");
        for (index, line) in lines[offset - 1..end].iter().enumerate() {
            let rendered = if line.chars().count() > READ_MAX_LINE_CHARS {
                let cut: String = line.chars().take(READ_MAX_LINE_CHARS).collect();
                format!("{cut}…")
            } else {
                (*line).to_string()
            };
            output.push_str(&format!("{:>6}\t{rendered}\n", offset + index));
        }
        if end < total {
            output.push_str(&format!(
                "[{} more lines; continue with offset={}]",
                total - end,
                end + 1
            ));
        }
        ToolResult::success("read_file", output)
    }

    /// Search for code patterns with ripgrep (plain-text grep fallback).
    ///
    /// Runs from the workspace root against a relative target so both file
    /// and directory paths work, honors the catalog's `context` argument,
    /// and caps the returned lines so a broad query stays bounded.
    pub(crate) fn search_code(&self, call: &ToolCall) -> ToolResult {
        let query = match call.arguments.get("query") {
            Some(q) => q,
            None => {
                return ToolResult::failure("search_code", "Missing 'query' argument".to_string())
            }
        };
        let target = call
            .arguments
            .get("path")
            .cloned()
            .unwrap_or_else(|| ".".to_string());
        if !self.resolve_path(&target).exists() {
            return ToolResult::failure(
                "search_code",
                format!("Search target does not exist: {target}"),
            );
        }
        let context = match call.arguments.get("context") {
            None => 0usize,
            Some(raw) => match raw.trim().parse::<usize>() {
                Ok(n) => n.min(GREP_MAX_CONTEXT),
                Err(_) => {
                    return ToolResult::failure(
                        "search_code",
                        format!("'context' must be a non-negative integer, got {raw:?}"),
                    )
                }
            },
        };
        match self.spawn_search(query, &target, context) {
            Ok(out) if out.status.code() == Some(1) && out.stdout.is_empty() => {
                ToolResult::success(
                    "search_code",
                    format!("no matches for {query:?} in {target}"),
                )
            }
            Ok(out) if out.status.success() || out.status.code() == Some(1) => {
                ToolResult::success("search_code", capped_search_output(&out.stdout))
            }
            Ok(out) => ToolResult::failure(
                "search_code",
                format!("Search failed: {}", String::from_utf8_lossy(&out.stderr)),
            ),
            Err(e) => ToolResult::failure("search_code", format!("Search failed: {}", e)),
        }
    }

    /// Run ripgrep (plain grep fallback) from the workspace root against a
    /// relative target so file and directory paths both work.
    fn spawn_search(
        &self,
        query: &str,
        target: &str,
        context: usize,
    ) -> std::io::Result<std::process::Output> {
        let context_arg = format!("-C{context}");
        let mut rg_args = vec!["-n", "--no-heading", "--color", "never", "-H"];
        if context > 0 {
            rg_args.push(&context_arg);
        }
        rg_args.extend(["--max-count", "50", "--", query, target]);
        Command::new("rg")
            .args(&rg_args)
            .current_dir(&self.working_dir)
            .output()
            .or_else(|_| {
                let mut grep_args = vec!["-rn"];
                if context > 0 {
                    grep_args.push(&context_arg);
                }
                grep_args.extend(["--", query, target]);
                Command::new("grep")
                    .args(&grep_args)
                    .current_dir(&self.working_dir)
                    .output()
            })
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

    /// Match files by glob pattern, newest first, in bounded windows.
    pub(crate) fn glob(&self, call: &ToolCall) -> ToolResult {
        let Some(pattern) = call.arguments.get("pattern") else {
            return ToolResult::failure("glob", "Missing 'pattern' argument".to_string());
        };
        let window = match parse_window(call) {
            Ok(w) => w,
            Err(e) => return ToolResult::failure("glob", e),
        };
        let mut matches: Vec<(std::time::SystemTime, String)> = Vec::new();
        collect_glob_matches(&self.working_dir, &self.working_dir, pattern, &mut matches);
        matches.sort_by_key(|(mtime, _)| std::cmp::Reverse(*mtime));
        let listing: Vec<String> = matches.into_iter().map(|(_, p)| p).collect();
        ToolResult::success(
            "glob",
            paged_listing(&format!("glob {pattern}"), &listing, window),
        )
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
    /// List a directory's entries, honoring ignore files, in bounded windows.
    pub(crate) fn list_files(&self, call: &ToolCall) -> ToolResult {
        let label = call
            .arguments
            .get("path")
            .cloned()
            .unwrap_or_else(|| ".".to_string());
        let path = call
            .arguments
            .get("path")
            .map(|p| self.resolve_path(p))
            .unwrap_or_else(|| self.working_dir.clone());
        let window = match parse_window(call) {
            Ok(w) => w,
            Err(e) => return ToolResult::failure("list_files", e),
        };
        if !path.is_dir() {
            return ToolResult::failure(
                "list_files",
                format!("Failed to list {:?}: not a directory", path),
            );
        }

        let mut files: Vec<String> = ignore::WalkBuilder::new(&path)
            .max_depth(Some(1))
            .standard_filters(true)
            .require_git(false)
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| e.depth() > 0)
            .map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    format!("{}/", name)
                } else {
                    name
                }
            })
            .collect();
        files.sort();
        ToolResult::success(
            "list_files",
            paged_listing(&format!("list_files {label}"), &files, window),
        )
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
