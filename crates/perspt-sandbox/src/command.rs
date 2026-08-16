//! Sandboxed Command Execution
//!
//! Provides a trait and implementation for executing commands with sandboxing.

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const DEFAULT_OUTPUT_LIMIT: usize = 8 * 1024 * 1024;

/// Filesystem authority granted to a process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemAccess {
    /// The process may inspect files but may not modify the workspace.
    ReadOnly,
    /// Writes are confined to the declared workspace root.
    WorkspaceWrite,
}

/// Whether absence of an OS isolation backend is a hard error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationMode {
    Required,
    BestEffort,
}

/// Auditable process policy. Network and filesystem authority are separate;
/// neither is inferred from a command name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessPolicy {
    pub workspace_root: PathBuf,
    pub filesystem: FilesystemAccess,
    pub allow_network: bool,
    pub timeout: Duration,
    pub output_limit: usize,
    pub isolation: IsolationMode,
    pub environment: BTreeMap<String, String>,
}

impl ProcessPolicy {
    pub fn inspection(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            filesystem: FilesystemAccess::ReadOnly,
            allow_network: false,
            timeout: Duration::from_secs(60),
            output_limit: DEFAULT_OUTPUT_LIMIT,
            isolation: IsolationMode::Required,
            environment: minimal_environment(),
        }
    }

    pub fn candidate_mutation(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            filesystem: FilesystemAccess::WorkspaceWrite,
            ..Self::inspection(workspace_root)
        }
    }

    pub fn with_network(mut self, allow: bool) -> Self {
        self.allow_network = allow;
        self
    }

    pub fn best_effort(mut self) -> Self {
        self.isolation = IsolationMode::BestEffort;
        self
    }
}

/// Process runner backed by the host's OS sandbox (`sandbox-exec` on macOS,
/// `bwrap` on Linux). It fails closed when isolation is required but absent.
#[derive(Debug, Clone)]
pub struct ProcessSandbox {
    program: String,
    args: Vec<String>,
    policy: ProcessPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedProcess {
    pub program: String,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
    pub environment: BTreeMap<String, String>,
    pub isolated: bool,
}

impl ProcessSandbox {
    pub fn new(
        program: impl Into<String>,
        args: Vec<String>,
        policy: ProcessPolicy,
    ) -> Result<Self> {
        let root = policy
            .workspace_root
            .canonicalize()
            .with_context(|| format!("canonicalizing {}", policy.workspace_root.display()))?;
        let mut policy = policy;
        policy.workspace_root = root;
        Ok(Self {
            program: program.into(),
            args,
            policy,
        })
    }

    pub fn prepare_invocation(&self) -> Result<PreparedProcess> {
        let (program, args, isolated) = platform_command(&self.program, &self.args, &self.policy);
        if !isolated && self.policy.isolation == IsolationMode::Required {
            anyhow::bail!("no supported OS process sandbox is available");
        }
        Ok(PreparedProcess {
            program,
            args,
            working_dir: self.policy.workspace_root.clone(),
            environment: self.policy.environment.clone(),
            isolated,
        })
    }

    fn prepared(&self) -> Result<BasicSandbox> {
        let PreparedProcess {
            program,
            args,
            working_dir,
            environment,
            ..
        } = self.prepare_invocation()?;
        let mut command = BasicSandbox::new(program, args)
            .with_working_dir(working_dir.to_string_lossy().into_owned())
            .with_timeout(self.policy.timeout)
            .with_output_limit(self.policy.output_limit)
            .with_clean_environment();
        for (key, value) in environment {
            command = command.with_environment(key, value);
        }
        Ok(command)
    }
}

impl SandboxedCommand for ProcessSandbox {
    fn execute(&self) -> Result<CommandResult> {
        self.prepared()?.execute()
    }

    fn display(&self) -> String {
        BasicSandbox::new(self.program.clone(), self.args.clone()).display()
    }

    fn is_read_only(&self) -> bool {
        self.policy.filesystem == FilesystemAccess::ReadOnly
    }
}

/// Result of a sandboxed command execution
#[derive(Debug, Clone)]
pub struct CommandResult {
    /// Standard output
    pub stdout: String,
    /// Standard error output
    pub stderr: String,
    /// Exit status
    pub exit_code: Option<i32>,
    /// Whether the command timed out
    pub timed_out: bool,
    /// Execution duration
    pub duration: Duration,
}

impl CommandResult {
    /// Check if the command succeeded
    pub fn success(&self) -> bool {
        self.exit_code == Some(0) && !self.timed_out
    }
}

/// Trait for sandboxed command execution
///
/// This trait abstracts command execution to allow different sandboxing
/// implementations (basic, Docker, Landlock, etc.)
pub trait SandboxedCommand: Send + Sync {
    /// Execute the command and return the result
    fn execute(&self) -> Result<CommandResult>;

    /// Get the command string for display
    fn display(&self) -> String;

    /// Check if the command is read-only (no side effects)
    fn is_read_only(&self) -> bool;
}

/// Basic sandboxed command wrapper
///
/// Phase 1 implementation: Executes commands directly but with
/// output capture and timeout support.
pub struct BasicSandbox {
    /// The program to execute
    program: String,
    /// Command arguments
    args: Vec<String>,
    /// Working directory
    working_dir: Option<String>,
    /// Timeout for execution
    timeout: Option<Duration>,
    output_limit: usize,
    inherit_environment: bool,
    environment: BTreeMap<String, String>,
}

impl BasicSandbox {
    /// Create a new basic sandbox
    pub fn new(program: String, args: Vec<String>) -> Self {
        Self {
            program,
            args,
            working_dir: None,
            timeout: Some(Duration::from_secs(60)), // Default 60s timeout
            output_limit: DEFAULT_OUTPUT_LIMIT,
            inherit_environment: true,
            environment: BTreeMap::new(),
        }
    }

    /// Set the working directory
    pub fn with_working_dir(mut self, dir: String) -> Self {
        self.working_dir = Some(dir);
        self
    }

    /// Set the timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn with_output_limit(mut self, bytes: usize) -> Self {
        self.output_limit = bytes.max(1);
        self
    }

    pub fn with_clean_environment(mut self) -> Self {
        self.inherit_environment = false;
        self
    }

    pub fn with_environment(mut self, key: String, value: String) -> Self {
        self.environment.insert(key, value);
        self
    }

    /// Parse a command string into program and args
    pub fn from_command_string(cmd: &str) -> Result<Self> {
        let parts = shell_words::split(cmd)?;
        if parts.is_empty() {
            anyhow::bail!("Empty command");
        }

        Ok(Self::new(parts[0].clone(), parts[1..].to_vec()))
    }
}

impl SandboxedCommand for BasicSandbox {
    fn execute(&self) -> Result<CommandResult> {
        let start = std::time::Instant::now();

        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if !self.inherit_environment {
            cmd.env_clear();
        }
        cmd.envs(&self.environment);

        if let Some(ref dir) = self.working_dir {
            cmd.current_dir(dir);
        }

        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().context("capturing process stdout")?;
        let stderr = child.stderr.take().context("capturing process stderr")?;
        let limit = self.output_limit;
        let stdout_reader = std::thread::spawn(move || read_bounded(stdout, limit));
        let stderr_reader = std::thread::spawn(move || read_bounded(stderr, limit));

        // Active timeout: poll child with a deadline, kill if exceeded
        if let Some(timeout) = self.timeout {
            let deadline = start + timeout;
            loop {
                match child.try_wait() {
                    Ok(Some(_status)) => {
                        // Process exited normally
                        break;
                    }
                    Ok(None) => {
                        // Still running — check deadline
                        if std::time::Instant::now() >= deadline {
                            // Kill the process
                            let _ = child.kill();
                            let _ = child.wait(); // reap zombie
                            let duration = start.elapsed();
                            let stdout = join_reader(stdout_reader)?;
                            let captured_stderr = join_reader(stderr_reader)?;
                            return Ok(CommandResult {
                                stdout,
                                stderr: format!(
                                    "Process killed after {}s timeout\n{}",
                                    timeout.as_secs(),
                                    captured_stderr
                                ),
                                exit_code: None,
                                timed_out: true,
                                duration,
                            });
                        }
                        // Brief sleep to avoid busy-waiting
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    Err(e) => {
                        return Err(e.into());
                    }
                }
            }
        }

        let status = child.wait()?;
        let duration = start.elapsed();

        Ok(CommandResult {
            stdout: join_reader(stdout_reader)?,
            stderr: join_reader(stderr_reader)?,
            exit_code: status.code(),
            timed_out: false,
            duration,
        })
    }

    fn display(&self) -> String {
        if self.args.is_empty() {
            self.program.clone()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
        }
    }

    fn is_read_only(&self) -> bool {
        // Commands that are generally read-only
        let read_only_programs = [
            "ls",
            "cat",
            "head",
            "tail",
            "grep",
            "find",
            "which",
            "echo",
            "pwd",
            "whoami",
            "date",
            "env",
            "printenv",
            "file",
            "stat",
            "cargo check",
            "cargo build",
            "cargo test",
            "cargo clippy",
            "git status",
            "git log",
            "git diff",
            "git show",
        ];

        let full_cmd = self.display();
        read_only_programs.iter().any(|p| full_cmd.starts_with(p))
    }
}

fn read_bounded(mut reader: impl Read, limit: usize) -> std::io::Result<String> {
    let mut retained = Vec::with_capacity(limit.min(64 * 1024));
    let mut chunk = [0u8; 8192];
    let mut truncated = false;
    loop {
        let count = reader.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        let available = limit.saturating_sub(retained.len());
        retained.extend_from_slice(&chunk[..count.min(available)]);
        truncated |= count > available;
    }
    let mut value = String::from_utf8_lossy(&retained).into_owned();
    if truncated {
        value.push_str("\n[output truncated by sandbox]");
    }
    Ok(value)
}

fn join_reader(reader: std::thread::JoinHandle<std::io::Result<String>>) -> Result<String> {
    reader
        .join()
        .map_err(|_| anyhow::anyhow!("process output reader panicked"))?
        .map_err(Into::into)
}

fn minimal_environment() -> BTreeMap<String, String> {
    let mut environment = BTreeMap::new();
    for key in ["PATH", "LANG", "LC_ALL", "USER"] {
        if let Ok(value) = std::env::var(key) {
            environment.insert(key.to_string(), value);
        }
    }
    environment
}

#[cfg(target_os = "macos")]
fn platform_command(
    program: &str,
    args: &[String],
    policy: &ProcessPolicy,
) -> (String, Vec<String>, bool) {
    if !Path::new("/usr/bin/sandbox-exec").is_file() {
        return (program.to_string(), args.to_vec(), false);
    }
    let root = policy
        .workspace_root
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let write_rule = match policy.filesystem {
        FilesystemAccess::ReadOnly => String::new(),
        FilesystemAccess::WorkspaceWrite => {
            format!("(allow file-write* (subpath \"{root}\"))")
        }
    };
    let network_rule = if policy.allow_network {
        "(allow network*)"
    } else {
        ""
    };
    // Inspection tools run with the user's identity, so a global read grant
    // would expose credentials and unrelated repositories. System roots are
    // needed for executables and dynamic libraries; task data is confined to
    // the candidate root.
    let profile = format!(
        "(version 1) (deny default) (allow process*) (allow sysctl-read) \
         (allow file-read* (subpath \"{root}\")) \
         (allow file-read* (subpath \"/System\")) \
         (allow file-read* (subpath \"/usr\")) \
         (allow file-read* (subpath \"/bin\")) \
         (allow file-read* (subpath \"/sbin\")) \
         (allow file-read* (subpath \"/Library\")) \
         (allow file-read* (subpath \"/opt/homebrew\")) \
         (allow file-read* (subpath \"/private/var/db/dyld\")) \
         (allow file-read* (literal \"/dev/null\")) \
         (allow file-write* (literal \"/dev/null\")) \
         {write_rule} {network_rule}"
    );
    let mut wrapped = vec!["-p".into(), profile, "--".into(), program.into()];
    wrapped.extend_from_slice(args);
    ("/usr/bin/sandbox-exec".into(), wrapped, true)
}

#[cfg(target_os = "linux")]
fn platform_command(
    program: &str,
    args: &[String],
    policy: &ProcessPolicy,
) -> (String, Vec<String>, bool) {
    let Some(bwrap) = ["/usr/bin/bwrap", "/bin/bwrap"]
        .into_iter()
        .find(|path| Path::new(path).is_file())
    else {
        return (program.to_string(), args.to_vec(), false);
    };
    let root = policy.workspace_root.to_string_lossy().into_owned();
    let mut wrapped = vec![
        "--die-with-parent".into(),
        "--new-session".into(),
        "--ro-bind".into(),
        "/".into(),
        "/".into(),
    ];
    // Mask user-secret roots after binding the host read-only. The coding
    // candidate is normally under /tmp and is rebound below.
    for sensitive in ["/home", "/root", "/Users"] {
        if Path::new(sensitive).exists() && !policy.workspace_root.starts_with(sensitive) {
            wrapped.extend(["--tmpfs".into(), sensitive.into()]);
        }
    }
    wrapped.extend(["--tmpfs".into(), "/tmp".into()]);
    if policy.filesystem == FilesystemAccess::WorkspaceWrite {
        wrapped.extend(["--bind".into(), root.clone(), root.clone()]);
    }
    if !policy.allow_network {
        wrapped.push("--unshare-net".into());
    }
    wrapped.extend(["--chdir".into(), root, "--".into(), program.into()]);
    wrapped.extend_from_slice(args);
    (bwrap.into(), wrapped, true)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn platform_command(
    program: &str,
    args: &[String],
    _policy: &ProcessPolicy,
) -> (String, Vec<String>, bool) {
    (program.to_string(), args.to_vec(), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_sandbox_echo() {
        let sandbox = BasicSandbox::new("echo".to_string(), vec!["hello".to_string()]);
        let result = sandbox.execute().unwrap();
        assert!(result.success());
        assert_eq!(result.stdout.trim(), "hello");
    }

    #[test]
    fn test_from_command_string() {
        let sandbox = BasicSandbox::from_command_string("ls -la /tmp").unwrap();
        assert_eq!(sandbox.program, "ls");
        assert_eq!(sandbox.args, vec!["-la", "/tmp"]);
    }

    #[test]
    fn test_display() {
        let sandbox = BasicSandbox::new(
            "cargo".to_string(),
            vec!["build".to_string(), "--release".to_string()],
        );
        assert_eq!(sandbox.display(), "cargo build --release");
    }

    #[test]
    fn test_is_read_only() {
        let sandbox = BasicSandbox::new("ls".to_string(), vec!["-la".to_string()]);
        assert!(sandbox.is_read_only());

        let sandbox = BasicSandbox::new("rm".to_string(), vec!["file.txt".to_string()]);
        assert!(!sandbox.is_read_only());
    }

    // =========================================================================
    // Baseline regression tests — freeze pre-refactor behavior
    // =========================================================================

    #[cfg(unix)]
    #[test]
    fn test_basic_sandbox_with_working_dir() {
        let temp = std::env::temp_dir();
        let sandbox = BasicSandbox::new("pwd".to_string(), vec![])
            .with_working_dir(temp.to_string_lossy().to_string());
        let result = sandbox.execute().unwrap();
        assert!(result.success());
        // The working_dir setting should be respected; the pwd output
        // should resolve to the same directory we specified.
        let output_path = std::path::PathBuf::from(result.stdout.trim());
        let expected = std::fs::canonicalize(&temp).unwrap();
        let actual = std::fs::canonicalize(&output_path).unwrap();
        assert_eq!(
            actual, expected,
            "pwd should match the specified working dir"
        );
    }

    #[cfg(windows)]
    #[test]
    fn test_basic_sandbox_with_working_dir() {
        // Use a uniquely-named subdirectory so we can verify the working dir
        // by name alone, avoiding junction/symlink resolution mismatches
        // (e.g. C:\Users\...\Temp junction → D:\tmp on CI runners).
        let unique = format!("perspt_test_{}", std::process::id());
        let dir = std::env::temp_dir().join(&unique);
        std::fs::create_dir_all(&dir).unwrap();
        let sandbox =
            BasicSandbox::new("cmd".to_string(), vec!["/C".to_string(), "cd".to_string()])
                .with_working_dir(dir.to_string_lossy().to_string());
        let result = sandbox.execute().unwrap();
        let _ = std::fs::remove_dir(&dir);
        assert!(result.success(), "cmd /C cd should succeed");
        let output = result.stdout.trim();
        assert!(
            output.ends_with(&unique),
            "working dir output should end with our unique dir name, got: {output}"
        );
    }

    #[test]
    fn test_basic_sandbox_captures_stderr() {
        let sandbox = BasicSandbox::new(
            "sh".to_string(),
            vec!["-c".to_string(), "echo err >&2".to_string()],
        );
        let result = sandbox.execute().unwrap();
        assert!(result.success());
        assert!(
            result.stderr.contains("err"),
            "stderr should capture error output"
        );
    }

    #[test]
    fn test_basic_sandbox_nonzero_exit() {
        let sandbox = BasicSandbox::new("false".to_string(), vec![]);
        let result = sandbox.execute().unwrap();
        assert!(!result.success());
        assert_eq!(result.exit_code, Some(1));
        assert!(!result.timed_out);
    }

    #[test]
    fn test_basic_sandbox_timeout_fast_command_succeeds() {
        // A fast command with a generous timeout should complete normally.
        let sandbox = BasicSandbox::new("echo".to_string(), vec!["fast".to_string()])
            .with_timeout(Duration::from_secs(60));
        let result = sandbox.execute().unwrap();
        assert!(!result.timed_out);
        assert!(result.success());
    }

    #[test]
    fn test_from_command_string_empty_rejected() {
        let result = BasicSandbox::from_command_string("");
        assert!(result.is_err(), "Empty command should be rejected");
    }

    #[test]
    fn test_from_command_string_with_quotes() {
        let sandbox = BasicSandbox::from_command_string(r#"echo "hello world""#).unwrap();
        assert_eq!(sandbox.program, "echo");
        assert_eq!(sandbox.args, vec!["hello world"]);
    }

    #[test]
    fn test_display_no_args() {
        let sandbox = BasicSandbox::new("pwd".to_string(), vec![]);
        assert_eq!(sandbox.display(), "pwd");
    }

    #[test]
    fn test_is_read_only_compound_commands() {
        // cargo check should be read-only
        let sandbox = BasicSandbox::new("cargo".to_string(), vec!["check".to_string()]);
        assert!(sandbox.is_read_only());

        // cargo test should be read-only
        let sandbox = BasicSandbox::new("cargo".to_string(), vec!["test".to_string()]);
        assert!(sandbox.is_read_only());

        // git status should be read-only
        let sandbox = BasicSandbox::new("git".to_string(), vec!["status".to_string()]);
        assert!(sandbox.is_read_only());

        // git push should NOT be read-only
        let sandbox = BasicSandbox::new("git".to_string(), vec!["push".to_string()]);
        assert!(!sandbox.is_read_only());
    }

    #[test]
    fn test_command_result_duration_nonzero() {
        let sandbox = BasicSandbox::new("echo".to_string(), vec!["hi".to_string()]);
        let result = sandbox.execute().unwrap();
        // Duration should be non-zero (process was actually spawned)
        assert!(result.duration.as_nanos() > 0);
    }

    #[test]
    fn test_active_timeout_kills_process() {
        // Start a long-running sleep and verify the sandbox kills it
        let sandbox = BasicSandbox::new("sleep".to_string(), vec!["30".to_string()])
            .with_timeout(Duration::from_millis(200));

        let result = sandbox.execute().unwrap();
        assert!(
            result.timed_out,
            "Process should have been killed by timeout"
        );
        assert!(!result.success());
        assert!(
            result.duration < Duration::from_secs(5),
            "Should return quickly after kill, not wait 30s"
        );
    }

    #[test]
    fn output_is_drained_but_bounded() {
        let sandbox = BasicSandbox::new(
            "sh".to_string(),
            vec!["-c".to_string(), "printf 1234567890".to_string()],
        )
        .with_output_limit(5);
        let result = sandbox.execute().unwrap();
        assert!(result.success());
        assert!(result.stdout.starts_with("12345"));
        assert!(result.stdout.contains("output truncated"));
    }

    #[test]
    fn process_policy_keeps_network_separate_from_write_authority() {
        let root = std::env::temp_dir();
        let inspection = ProcessPolicy::inspection(&root);
        assert_eq!(inspection.filesystem, FilesystemAccess::ReadOnly);
        assert!(!inspection.allow_network);
        let mutation = ProcessPolicy::candidate_mutation(&root).with_network(true);
        assert_eq!(mutation.filesystem, FilesystemAccess::WorkspaceWrite);
        assert!(mutation.allow_network);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn inspection_profile_does_not_grant_global_host_reads() {
        let root = std::env::temp_dir().join(format!("perspt-sandbox-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let sandbox = ProcessSandbox::new(
            "rg",
            vec!["needle".into(), ".".into()],
            ProcessPolicy::inspection(&root),
        )
        .unwrap();
        let prepared = sandbox.prepare_invocation().unwrap();
        let profile = &prepared.args[1];
        assert!(!profile.contains("(allow file-read*)"));
        assert!(profile.contains(&format!(
            "(allow file-read* (subpath \"{}\"))",
            root.canonicalize().unwrap().display()
        )));
        assert!(!profile.contains("allow network*"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
