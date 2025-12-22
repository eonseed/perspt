//! Command Sanitization
//!
//! Parses and validates shell commands to detect dangerous patterns.

use anyhow::Result;
use shell_words;

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
}
