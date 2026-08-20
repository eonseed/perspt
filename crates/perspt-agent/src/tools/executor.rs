use super::*;

/// Default `read_file` window when the model passes no `limit`.
const READ_DEFAULT_LIMIT: usize = 300;
/// Largest honored `limit` for one read window; bigger requests clamp so a
/// single call can never stage an unbounded window in memory.
const READ_MAX_LIMIT: usize = 2000;
/// Individual lines longer than this are truncated with an ellipsis.
const READ_MAX_LINE_CHARS: usize = 2000;
/// Bytes retained per line while streaming (the char cap applies after
/// decoding); the rest of an overlong line is counted, never buffered.
const READ_MAX_LINE_BYTES: usize = 4 * READ_MAX_LINE_CHARS + 4;
/// Search output is capped at this many lines before the omission note.
const GREP_MAX_LINES: usize = 200;
/// Largest honored `-C` context value for a search.
const GREP_MAX_CONTEXT: usize = 10;

/// Default listing window for `glob` and `list_files`.
const LIST_DEFAULT_LIMIT: usize = 100;
const LIST_MAX_LIMIT: usize = 1000;
const LIST_MAX_ENTRIES: usize = 10_000;
const GLOB_MAX_MATCHES: usize = 10_000;

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
            Ok(v) => v.min(LIST_MAX_LIMIT),
        },
    };
    Ok((offset, limit))
}

/// Options that can make an ostensibly read-only Git subcommand write a
/// file, invoke configured external code, or read arbitrary host paths.
fn unsafe_git_read_arg(argument: &str) -> bool {
    [
        "--output",
        "--ext-diff",
        "--textconv",
        "--no-index",
        "--pathspec-from-file",
    ]
    .iter()
    .any(|forbidden| argument == *forbidden || argument.starts_with(&format!("{forbidden}=")))
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

/// Largest stdout volume retained from a search child; past it the child
/// is killed and the result marked truncated, so one pathological match
/// (a multi-gigabyte line) cannot balloon host memory.
const GREP_MAX_STDOUT_BYTES: usize = 2 * 1024 * 1024;
const GREP_MAX_STDERR_BYTES: usize = 64 * 1024;
const GREP_PROCESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// A search child's bounded result.
#[derive(Debug)]
struct SearchOutput {
    /// Exit status success, or a deliberate truncation kill (matches were
    /// flowing when the cap hit — that is a successful, capped search).
    success: bool,
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    truncated: bool,
}

/// Drain one child pipe completely while retaining at most `cap` bytes.
/// Draining continues after the cap so the child cannot deadlock on a full
/// pipe while the parent waits for it to exit.
fn drain_bounded(
    mut pipe: impl std::io::Read,
    cap: usize,
    cap_signal: Option<std::sync::mpsc::Sender<()>>,
) -> std::io::Result<(Vec<u8>, bool)> {
    let mut retained = Vec::new();
    let mut truncated = false;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = pipe.read(&mut buf)?;
        if n == 0 {
            break;
        }
        let room = cap.saturating_sub(retained.len());
        retained.extend_from_slice(&buf[..n.min(room)]);
        if n > room && !truncated {
            truncated = true;
            if let Some(sender) = &cap_signal {
                let _ = sender.send(());
            }
        }
    }
    Ok((retained, truncated))
}

/// Run a child with concurrent bounded drains for stdout and stderr. The
/// child is killed at the stdout cap or deadline; neither pipe can block the
/// other, and unbounded stderr is discarded rather than retained.
fn bounded_command_output(mut command: Command) -> std::io::Result<SearchOutput> {
    bounded_command_output_with_timeout(&mut command, GREP_PROCESS_TIMEOUT)
}

fn bounded_command_output_with_timeout(
    command: &mut Command,
    timeout: std::time::Duration,
) -> std::io::Result<SearchOutput> {
    let mut child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let stdout_pipe = child.stdout.take().expect("piped stdout");
    let stderr_pipe = child.stderr.take().expect("piped stderr");
    let (cap_tx, cap_rx) = std::sync::mpsc::channel();
    let stdout_thread =
        std::thread::spawn(move || drain_bounded(stdout_pipe, GREP_MAX_STDOUT_BYTES, Some(cap_tx)));
    let stderr_thread =
        std::thread::spawn(move || drain_bounded(stderr_pipe, GREP_MAX_STDERR_BYTES, None));

    let started = std::time::Instant::now();
    let mut hit_cap = false;
    let mut timed_out = false;
    let status = loop {
        if cap_rx.try_recv().is_ok() {
            hit_cap = true;
            let _ = child.kill();
            break child.wait()?;
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= timeout {
            timed_out = true;
            let _ = child.kill();
            break child.wait()?;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    let (stdout, stdout_truncated) = stdout_thread
        .join()
        .map_err(|_| std::io::Error::other("search stdout reader panicked"))??;
    let (stderr, _stderr_truncated) = stderr_thread
        .join()
        .map_err(|_| std::io::Error::other("search stderr reader panicked"))??;
    if timed_out {
        return Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "search process exceeded 30 second deadline",
        ));
    }
    let truncated = hit_cap || stdout_truncated;
    Ok(SearchOutput {
        success: status.success() || truncated,
        code: status.code(),
        stdout,
        stderr,
        truncated,
    })
}

#[cfg(all(test, unix))]
mod bounded_process_tests {
    use super::*;

    #[test]
    fn drains_large_stderr_without_deadlocking() {
        let mut command = Command::new("sh");
        let script = concat!(
            "i=0; while [ $i -lt 100000 ]; do ",
            "printf 'diagnostic diagnostic diagnostic\\n' 1>&2; ",
            "i=$((i + 1)); done"
        );
        command.args(["-c", script]);
        let output =
            bounded_command_output_with_timeout(&mut command, std::time::Duration::from_secs(5))
                .unwrap();
        assert!(output.success);
        assert_eq!(output.stderr.len(), GREP_MAX_STDERR_BYTES);
    }

    #[test]
    fn kills_a_search_that_never_finishes() {
        let mut command = Command::new("sleep");
        command.arg("5");
        let error =
            bounded_command_output_with_timeout(&mut command, std::time::Duration::from_millis(50))
                .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    }
}

/// The streamed line window of one `read_file` call.
struct ReadWindow {
    lines: Vec<String>,
    total: usize,
    truncated: bool,
}

/// Stream the requested window: only in-window lines are retained (each
/// capped), lines outside it are counted without buffering — memory stays
/// O(window) whatever the file size. Binary content (NUL bytes) refuses.
fn collect_read_window(
    path: &std::path::Path,
    raw_path: &str,
    offset: usize,
    limit: usize,
) -> Result<ReadWindow, String> {
    let file = fs::File::open(path).map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
    let mut reader = std::io::BufReader::new(file);
    let mut window = ReadWindow {
        lines: Vec::new(),
        total: 0,
        truncated: false,
    };
    loop {
        let wanted = window.total + 1 >= offset && window.lines.len() < limit;
        let cap = if wanted { READ_MAX_LINE_BYTES } else { 0 };
        let Some((bytes, skipped, saw_nul)) = read_bounded_line(&mut reader, cap)
            .map_err(|e| format!("Failed to read {:?}: {}", path, e))?
        else {
            return Ok(window);
        };
        window.total += 1;
        // Binary refusal covers the whole stream: skipped lines and the
        // unretained tails of overlong lines are scanned too.
        if saw_nul {
            return Err(format!("{raw_path} appears to be a binary file"));
        }
        if !wanted {
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        let rendered = match char_cap(&text, READ_MAX_LINE_CHARS) {
            Some(cut) => {
                window.truncated = true;
                let more = skipped + (text.len() - cut.len());
                format!("{cut}…[+{more} bytes on this line]")
            }
            None if skipped > 0 => {
                window.truncated = true;
                format!("{text}…[+{skipped} bytes on this line]")
            }
            None => text.into_owned(),
        };
        window
            .lines
            .push(format!("{:>6}\t{rendered}", window.total));
    }
}

/// Read one line keeping at most `cap` bytes: `(kept, skipped, saw_nul)`
/// where `skipped` counts the bytes beyond the cap (newline excluded) and
/// `saw_nul` reports a NUL anywhere in the line — retained or skipped — so
/// binary detection covers the whole stream, not just kept prefixes.
/// `None` at EOF. The tail of an overlong line is consumed without
/// buffering, so a single multi-gigabyte line costs O(cap) memory.
fn read_bounded_line(
    reader: &mut impl std::io::BufRead,
    cap: usize,
) -> std::io::Result<Option<(Vec<u8>, usize, bool)>> {
    let mut kept = Vec::new();
    let mut skipped = 0usize;
    let mut saw_any = false;
    let mut saw_nul = false;
    loop {
        let buf = reader.fill_buf()?;
        if buf.is_empty() {
            return Ok(saw_any.then_some((kept, skipped, saw_nul)));
        }
        saw_any = true;
        let newline = buf.iter().position(|&byte| byte == b'\n');
        let take = newline.unwrap_or(buf.len());
        saw_nul = saw_nul || buf[..take].contains(&0);
        let keep = take.min(cap.saturating_sub(kept.len()));
        kept.extend_from_slice(&buf[..keep]);
        skipped += take - keep;
        reader.consume(take + usize::from(newline.is_some()));
        if newline.is_some() {
            if skipped == 0 && kept.last() == Some(&b'\r') {
                kept.pop();
            }
            return Ok(Some((kept, skipped, saw_nul)));
        }
    }
}

/// The first `cap` characters when the text is longer, `None` otherwise.
fn char_cap(text: &str, cap: usize) -> Option<&str> {
    let mut indices = text.char_indices();
    match indices.nth(cap) {
        Some((byte_index, _)) => Some(&text[..byte_index]),
        None => None,
    }
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

        let limit = limit.min(READ_MAX_LIMIT);
        let ReadWindow {
            lines: window,
            total,
            truncated: any_truncated,
        } = match collect_read_window(&path, raw_path, offset, limit) {
            Ok(window) => window,
            Err(message) => return ToolResult::failure("read_file", message),
        };
        if offset > total {
            return ToolResult::success(
                "read_file",
                format!("file {raw_path}: {total} lines total; offset {offset} is past the end"),
            );
        }
        let end = offset - 1 + window.len();
        let mut output = format!("file {raw_path}: lines {offset}-{end} of {total}\n");
        output.push_str(&window.join("\n"));
        output.push('\n');
        if any_truncated {
            output.push_str("[overlong lines truncated; grep returns full lines]\n");
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
            Ok(out) if out.code == Some(1) && out.stdout.is_empty() => ToolResult::success(
                "search_code",
                format!("no matches for {query:?} in {target}"),
            ),
            Ok(out) if out.success || out.code == Some(1) => {
                let mut rendered = capped_search_output(&out.stdout);
                if out.truncated {
                    rendered.push_str("\n[search output truncated; narrow the query]");
                }
                ToolResult::success("search_code", rendered)
            }
            Ok(out) => ToolResult::failure(
                "search_code",
                format!("Search failed: {}", String::from_utf8_lossy(&out.stderr)),
            ),
            Err(e) => ToolResult::failure("search_code", format!("Search failed: {}", e)),
        }
    }

    /// Run ripgrep (plain grep fallback) from the workspace root against a
    /// relative target so file and directory paths both work. Output is
    /// read through the bounded runner, so a match on a multi-gigabyte
    /// line cannot materialize the child's whole stdout.
    fn spawn_search(
        &self,
        query: &str,
        target: &str,
        context: usize,
    ) -> std::io::Result<SearchOutput> {
        let context_arg = format!("-C{context}");
        let mut rg_args = vec!["-n", "--no-heading", "--color", "never", "-H"];
        if context > 0 {
            rg_args.push(&context_arg);
        }
        rg_args.extend(["--max-count", "50", "--", query, target]);
        let mut rg = Command::new("rg");
        rg.args(&rg_args).current_dir(&self.working_dir);
        bounded_command_output(rg).or_else(|_| {
            let mut grep_args = vec!["-rn"];
            if context > 0 {
                grep_args.push(&context_arg);
            }
            grep_args.extend(["--", query, target]);
            let mut grep = Command::new("grep");
            grep.args(&grep_args).current_dir(&self.working_dir);
            bounded_command_output(grep)
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
        let (mut matches, total) = collect_glob_matches(&self.working_dir, pattern);
        matches.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
        let retained = matches.len();
        let listing: Vec<String> = matches.into_iter().map(|(_, p)| p).collect();
        let label = if total > retained {
            format!("glob {pattern} (newest {retained} of {total} matches)")
        } else {
            format!("glob {pattern}")
        };
        ToolResult::success("glob", paged_listing(&label, &listing, window))
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
        if let Some(argument) = args.iter().find(|argument| unsafe_git_read_arg(argument)) {
            return ToolResult::failure(
                "git_read",
                format!("Git argument {argument:?} is not permitted for read-only inspection"),
            );
        }
        if subcommand != "status" {
            args.splice(1..1, ["--no-ext-diff".into(), "--no-textconv".into()]);
        }
        let mut command = Command::new("git");
        command
            .args(&args)
            .current_dir(&self.working_dir)
            .env("GIT_PAGER", "cat")
            .env("PAGER", "cat")
            .env_remove("GIT_EXTERNAL_DIFF");
        match bounded_command_output(command) {
            Ok(out) if out.success => {
                let mut rendered = String::from_utf8_lossy(&out.stdout).to_string();
                if out.truncated {
                    rendered.push_str("\n[git output truncated; narrow the revision or path]");
                }
                ToolResult::success("git_read", rendered)
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

        let mut files = std::collections::BTreeSet::new();
        let mut total = 0usize;
        for name in ignore::WalkBuilder::new(&path)
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
        {
            total = total.saturating_add(1);
            files.insert(name);
            if files.len() > LIST_MAX_ENTRIES {
                files.pop_last();
            }
        }
        let retained = files.len();
        let files: Vec<String> = files.into_iter().collect();
        let label = if total > retained {
            format!("list_files {label} (first {retained} of {total} entries)")
        } else {
            format!("list_files {label}")
        };
        ToolResult::success("list_files", paged_listing(&label, &files, window))
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
/// Walk the workspace without following symlinks, retaining only the newest
/// [`GLOB_MAX_MATCHES`] matches while counting the full result set. This
/// keeps glob memory bounded on generated or very large repositories.
fn collect_glob_matches(
    root: &Path,
    pattern: &str,
) -> (Vec<(std::time::SystemTime, String)>, usize) {
    let mut newest: std::collections::BinaryHeap<
        std::cmp::Reverse<(std::time::SystemTime, String)>,
    > = std::collections::BinaryHeap::new();
    let mut total = 0usize;
    for entry in ignore::WalkBuilder::new(root)
        .standard_filters(true)
        .require_git(false)
        .build()
        .flatten()
        .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
    {
        let Ok(relative) = entry.path().strip_prefix(root) else {
            continue;
        };
        let relative = relative.to_string_lossy().replace('\\', "/");
        if !perspt_core::types::glob_matches(pattern, &relative) {
            continue;
        }
        total = total.saturating_add(1);
        let modified = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        newest.push(std::cmp::Reverse((modified, relative)));
        if newest.len() > GLOB_MAX_MATCHES {
            newest.pop();
        }
    }
    (newest.into_iter().map(|entry| entry.0).collect(), total)
}
