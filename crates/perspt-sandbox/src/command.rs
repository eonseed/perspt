//! Sandboxed Command Execution
//!
//! Provides a trait and implementation for executing commands with sandboxing.

use anyhow::Result;
use std::process::{Command, Stdio};
use std::time::Duration;

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
}

impl BasicSandbox {
    /// Create a new basic sandbox
    pub fn new(program: String, args: Vec<String>) -> Self {
        Self {
            program,
            args,
            working_dir: None,
            timeout: Some(Duration::from_secs(60)), // Default 60s timeout
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

        if let Some(ref dir) = self.working_dir {
            cmd.current_dir(dir);
        }

        let output = cmd.output()?;
        let duration = start.elapsed();

        // Check timeout (basic implementation - doesn't actually kill on timeout)
        let timed_out = self.timeout.is_some_and(|t| duration > t);

        Ok(CommandResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code(),
            timed_out,
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
}
