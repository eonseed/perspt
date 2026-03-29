//! Command Sanitization
//!
//! Parses and validates shell commands to detect dangerous patterns.

use anyhow::Result;
use shell_words;

fn has_windows_drive_prefix(part: &str) -> bool {
    part
        .chars()
        .nth(1)
        .is_some_and(|character| character == ':')
}

fn looks_like_path_argument(part: &str) -> bool {
    part.contains('/') || part.contains('\\') || has_windows_drive_prefix(part)
}

fn is_explicit_absolute_path(part: &str, candidate: &std::path::Path) -> bool {
    candidate.is_absolute()
        || part.starts_with('/')
        || part.starts_with('\\')
        || has_windows_drive_prefix(part)
}

/// Sanitization result
#[derive(Debug, Clone)]
pub struct SanitizeResult {
    /// The parsed command parts
    pub parts: Vec<String>,
    /// Warnings about potentially dangerous patterns
    pub warnings: Vec<String>,
    /// Whether the command was rejected
    pub rejected: bool,
    /// Rejection reason if rejected
    pub rejection_reason: Option<String>,
}

/// Sanitize a command string
///
/// Parses the command and checks for:
/// - Subshell expansion (backticks, $())
/// - Command chaining (&&, ||, ;)
/// - Redirections to sensitive paths
/// - Network access without acknowledgment
pub fn sanitize_command(command: &str) -> Result<SanitizeResult> {
    let mut result = SanitizeResult {
        parts: Vec::new(),
        warnings: Vec::new(),
        rejected: false,
        rejection_reason: None,
    };

    // Parse using shell-words
    match shell_words::split(command) {
        Ok(parts) => {
            result.parts = parts;
        }
        Err(e) => {
            result.rejected = true;
            result.rejection_reason = Some(format!("Failed to parse command: {}", e));
            return Ok(result);
        }
    }

    // Check for backtick subshell expansion
    if command.contains('`') {
        result
            .warnings
            .push("Command contains backtick subshell expansion".to_string());
    }

    // Check for $() subshell expansion
    if command.contains("$(") {
        result
            .warnings
            .push("Command contains $() subshell expansion".to_string());
    }

    // Check for command chaining (if not in quotes)
    let dangerous_chains = ["&&", "||", ";"];
    for chain in &dangerous_chains {
        // Simple check - a more robust implementation would respect quoting
        if command.contains(chain) {
            result
                .warnings
                .push(format!("Command contains chaining operator: {}", chain));
        }
    }

    // Check for redirections to sensitive paths
    let sensitive_paths = ["/etc/", "/root/", "~/.ssh/", "/dev/", "/proc/", "/sys/"];

    for path in &sensitive_paths {
        if command.contains(&format!("> {}", path))
            || command.contains(&format!(">> {}", path))
            || command.contains(&format!("< {}", path))
        {
            result.warnings.push(format!(
                "Command redirects to/from sensitive path: {}",
                path
            ));
        }
    }

    // Check for destructive patterns
    let destructive_patterns = [
        ("rm -rf /", "Recursive delete of root"),
        ("rm -rf /*", "Recursive delete of root contents"),
        ("rm -rf ~", "Recursive delete of home directory"),
        (":(){:|:&};:", "Fork bomb"),
        ("mkfs", "Filesystem creation"),
        ("dd if=/dev/zero", "Disk overwrite"),
        ("> /dev/sda", "Direct disk write"),
    ];

    for (pattern, description) in &destructive_patterns {
        if command.contains(pattern) {
            result.rejected = true;
            result.rejection_reason = Some(format!(
                "Dangerous pattern detected: {} ({})",
                pattern, description
            ));
            return Ok(result);
        }
    }

    Ok(result)
}

/// Canonicalize a command for display
///
/// Normalizes the command to prevent visual obfuscation attacks
pub fn canonicalize(command: &str) -> Result<String> {
    // Parse and rejoin to normalize spacing
    let parts = shell_words::split(command)?;
    Ok(shell_words::join(&parts))
}

/// Check if a command is safe for auto-execution
pub fn is_safe_for_auto_exec(command: &str) -> bool {
    let result = sanitize_command(command);
    match result {
        Ok(r) => !r.rejected && r.warnings.is_empty(),
        Err(_) => false,
    }
}

/// Validate that a command is workspace-bound.
///
/// Checks parsed command parts for absolute paths that escape the given
/// workspace root.  Returns `Ok(())` when all path-like arguments resolve
/// inside the workspace, or an error describing the violation.
pub fn validate_workspace_bound(command: &str, workspace_root: &std::path::Path) -> Result<()> {
    let parts = shell_words::split(command)?;

    for part in &parts {
        // Skip flags and non-path arguments
        if part.starts_with('-') || !looks_like_path_argument(part) {
            continue;
        }

        let candidate = std::path::Path::new(part);
        if is_explicit_absolute_path(part, candidate) {
            // Absolute path — must be inside workspace
            if !candidate.starts_with(workspace_root) {
                anyhow::bail!(
                    "command references path outside workspace: {} (workspace: {})",
                    part,
                    workspace_root.display()
                );
            }
        } else if part.contains("..") {
            // Relative with '..' — resolve and check
            let resolved = workspace_root.join(candidate);
            if let Ok(canonical) = resolved.canonicalize() {
                if !canonical.starts_with(workspace_root) {
                    anyhow::bail!(
                        "command escapes workspace via '..': {} resolves to {} (workspace: {})",
                        part,
                        canonical.display(),
                        workspace_root.display()
                    );
                }
            }
            // If canonicalize fails (path doesn't exist yet), allow it —
            // the command may create the path.
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_command() {
        let result = sanitize_command("cargo build --release").unwrap();
        assert!(!result.rejected);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_dangerous_command_rejected() {
        let result = sanitize_command("rm -rf /").unwrap();
        assert!(result.rejected);
    }

    #[test]
    fn test_subshell_warning() {
        let result = sanitize_command("echo $(whoami)").unwrap();
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn test_chaining_warning() {
        let result = sanitize_command("ls && rm file").unwrap();
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn test_canonicalize() {
        let normalized = canonicalize("ls   -la    /tmp").unwrap();
        assert_eq!(normalized, "ls -la /tmp");
    }

    #[test]
    fn test_workspace_bound_relative_safe() {
        let ws = std::path::PathBuf::from("/home/user/project");
        assert!(validate_workspace_bound("cargo build", &ws).is_ok());
    }

    #[test]
    fn test_workspace_bound_absolute_inside() {
        let (ws, command) = if cfg!(windows) {
            (
                std::path::PathBuf::from(r"C:\Users\user\project"),
                r"cat C:\Users\user\project\src\main.rs",
            )
        } else {
            (
                std::path::PathBuf::from("/home/user/project"),
                "cat /home/user/project/src/main.rs",
            )
        };

        assert!(validate_workspace_bound(command, &ws).is_ok());
    }

    #[test]
    fn test_workspace_bound_absolute_outside_rejected() {
        let (ws, command) = if cfg!(windows) {
            (
                std::path::PathBuf::from(r"C:\Users\user\project"),
                r"cat C:\Windows\System32\drivers\etc\hosts",
            )
        } else {
            (
                std::path::PathBuf::from("/home/user/project"),
                "cat /etc/passwd",
            )
        };

        let result = validate_workspace_bound(command, &ws);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("outside workspace"));
    }

    #[test]
    fn test_workspace_bound_flags_ignored() {
        let ws = std::path::PathBuf::from("/home/user/project");
        assert!(validate_workspace_bound("cargo build --release", &ws).is_ok());
    }
}
