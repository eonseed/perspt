//! Canonical path resolution for artifact paths.
//!
//! Provides a single normalization function that all path consumers share:
//! bundle validation, ownership manifest lookups, sandbox copy, policy checks,
//! and commit reconciliation.  This ensures that `src/main.rs`, `./src/main.rs`,
//! `src/../src/main.rs`, and `src/./main.rs` all resolve to the same identity.
//!
//! Paths are always workspace-relative.  Absolute paths and traversals that
//! escape the workspace root are rejected.

use std::path::{Component, PathBuf};

/// Errors returned by path normalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    /// The path is empty after normalization.
    Empty,
    /// The path is absolute (starts with `/` or a drive letter).
    Absolute(String),
    /// The path escapes the workspace root via `..` traversal.
    Escapes(String),
    /// The path contains a null byte or other invalid component.
    Invalid(String),
}

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathError::Empty => write!(f, "path is empty"),
            PathError::Absolute(p) => write!(f, "path is absolute: '{}'", p),
            PathError::Escapes(p) => write!(f, "path escapes workspace root: '{}'", p),
            PathError::Invalid(p) => write!(f, "path contains invalid components: '{}'", p),
        }
    }
}

impl std::error::Error for PathError {}

/// Normalize a workspace-relative artifact path to its canonical form.
///
/// Resolves `.` and `..` components, strips redundant separators, and
/// converts backslashes to forward slashes.  The result is a clean
/// relative path suitable for use as a map key and file identity.
///
/// # Errors
///
/// Returns `PathError` if the path is empty, absolute, or escapes the
/// workspace root (net `..` depth goes below zero).
///
/// # Examples
///
/// ```
/// use perspt_core::path::normalize_artifact_path;
///
/// assert_eq!(normalize_artifact_path("src/main.rs").unwrap(), "src/main.rs");
/// assert_eq!(normalize_artifact_path("./src/main.rs").unwrap(), "src/main.rs");
/// assert_eq!(normalize_artifact_path("src/../src/main.rs").unwrap(), "src/main.rs");
/// assert_eq!(normalize_artifact_path("src/./main.rs").unwrap(), "src/main.rs");
/// assert!(normalize_artifact_path("../escape.rs").is_err());
/// assert!(normalize_artifact_path("/absolute/path").is_err());
/// ```
pub fn normalize_artifact_path(raw: &str) -> Result<String, PathError> {
    if raw.is_empty() {
        return Err(PathError::Empty);
    }

    // Null bytes are never valid in paths
    if raw.contains('\0') {
        return Err(PathError::Invalid(raw.to_string()));
    }

    // PSP-7: Strip surrounding backticks, quotes, and markdown formatting
    // that LLMs often wrap around file paths.
    let stripped = raw
        .trim()
        .trim_matches('`')
        .trim_matches('"')
        .trim_matches('\'')
        .trim_start_matches("**")
        .trim_end_matches("**")
        .trim();

    if stripped.is_empty() {
        return Err(PathError::Empty);
    }

    // Normalize backslashes before parsing
    let normalized = stripped.replace('\\', "/");
    let p = std::path::Path::new(&normalized);

    // Reject absolute paths early
    if p.is_absolute() || normalized.starts_with('/') {
        return Err(PathError::Absolute(raw.to_string()));
    }

    // Windows drive prefix check
    let bytes = normalized.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return Err(PathError::Absolute(raw.to_string()));
    }

    // Resolve components, tracking depth to detect escapes
    let mut components: Vec<String> = Vec::new();
    let mut depth: i32 = 0;

    for component in p.components() {
        match component {
            Component::Normal(s) => {
                let s = s.to_string_lossy().to_string();
                components.push(s);
                depth += 1;
            }
            Component::ParentDir => {
                if depth <= 0 {
                    return Err(PathError::Escapes(raw.to_string()));
                }
                components.pop();
                depth -= 1;
            }
            Component::CurDir => {
                // Skip `.` components
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(PathError::Absolute(raw.to_string()));
            }
        }
    }

    let result: PathBuf = components.iter().collect();
    let result_str = result.to_string_lossy().to_string();

    // Normalize to forward slashes in the output
    let result_str = result_str.replace('\\', "/");

    if result_str.is_empty() {
        return Err(PathError::Empty);
    }

    Ok(result_str)
}

/// Normalize a path for use as a map key (ownership manifest, bundle dedup).
///
/// Thin wrapper around `normalize_artifact_path` that returns `None` on
/// error instead of `Err`.  Callers that want diagnostics should use
/// `normalize_artifact_path` directly.
pub fn normalize_path_key(raw: &str) -> Option<String> {
    normalize_artifact_path(raw).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_relative_path() {
        assert_eq!(
            normalize_artifact_path("src/main.rs").unwrap(),
            "src/main.rs"
        );
    }

    #[test]
    fn test_dot_prefix_stripped() {
        assert_eq!(
            normalize_artifact_path("./src/main.rs").unwrap(),
            "src/main.rs"
        );
    }

    #[test]
    fn test_redundant_parent_resolved() {
        assert_eq!(
            normalize_artifact_path("src/../src/main.rs").unwrap(),
            "src/main.rs"
        );
    }

    #[test]
    fn test_dot_in_middle_stripped() {
        assert_eq!(
            normalize_artifact_path("src/./main.rs").unwrap(),
            "src/main.rs"
        );
    }

    #[test]
    fn test_multiple_slashes_normalized() {
        assert_eq!(
            normalize_artifact_path("src///main.rs").unwrap(),
            "src/main.rs"
        );
    }

    #[test]
    fn test_backslash_normalized() {
        assert_eq!(
            normalize_artifact_path("src\\lib\\mod.rs").unwrap(),
            "src/lib/mod.rs"
        );
    }

    #[test]
    fn test_trailing_slash_preserved_as_dir() {
        // A trailing slash results in the directory name
        let r = normalize_artifact_path("src/lib/").unwrap();
        assert_eq!(r, "src/lib");
    }

    #[test]
    fn test_empty_path_rejected() {
        assert_eq!(normalize_artifact_path(""), Err(PathError::Empty));
    }

    #[test]
    fn test_absolute_unix_rejected() {
        assert!(matches!(
            normalize_artifact_path("/etc/passwd"),
            Err(PathError::Absolute(_))
        ));
    }

    #[test]
    fn test_absolute_windows_rejected() {
        assert!(matches!(
            normalize_artifact_path("C:\\Windows\\file.txt"),
            Err(PathError::Absolute(_))
        ));
    }

    #[test]
    fn test_escape_via_dotdot_rejected() {
        assert!(matches!(
            normalize_artifact_path("../escape.rs"),
            Err(PathError::Escapes(_))
        ));
    }

    #[test]
    fn test_deep_escape_rejected() {
        assert!(matches!(
            normalize_artifact_path("a/b/../../../../escape"),
            Err(PathError::Escapes(_))
        ));
    }

    #[test]
    fn test_dotdot_that_stays_inside() {
        assert_eq!(
            normalize_artifact_path("a/b/../c/file.rs").unwrap(),
            "a/c/file.rs"
        );
    }

    #[test]
    fn test_null_byte_rejected() {
        assert!(matches!(
            normalize_artifact_path("src/\0bad.rs"),
            Err(PathError::Invalid(_))
        ));
    }

    #[test]
    fn test_just_dot_is_empty() {
        assert_eq!(normalize_artifact_path("."), Err(PathError::Empty));
    }

    #[test]
    fn test_normalize_path_key_returns_none_on_error() {
        assert!(normalize_path_key("").is_none());
        assert!(normalize_path_key("/abs").is_none());
        assert!(normalize_path_key("../escape").is_none());
    }

    #[test]
    fn test_normalize_path_key_returns_some_on_success() {
        assert_eq!(
            normalize_path_key("./src/main.rs"),
            Some("src/main.rs".into())
        );
    }

    // PSP-7 regression tests

    #[test]
    fn test_backtick_wrapped_path() {
        assert_eq!(
            normalize_artifact_path("`src/main.rs`").unwrap(),
            "src/main.rs"
        );
    }

    #[test]
    fn test_double_quoted_path() {
        assert_eq!(
            normalize_artifact_path("\"src/main.rs\"").unwrap(),
            "src/main.rs"
        );
    }

    #[test]
    fn test_single_quoted_path() {
        assert_eq!(
            normalize_artifact_path("'src/main.rs'").unwrap(),
            "src/main.rs"
        );
    }

    #[test]
    fn test_bold_markdown_path() {
        assert_eq!(
            normalize_artifact_path("**src/main.rs**").unwrap(),
            "src/main.rs"
        );
    }

    #[test]
    fn test_backtick_with_dot_prefix() {
        assert_eq!(
            normalize_artifact_path("`./src/lib.rs`").unwrap(),
            "src/lib.rs"
        );
    }

    #[test]
    fn test_only_backticks_is_empty() {
        assert_eq!(normalize_artifact_path("``"), Err(PathError::Empty));
    }
}
