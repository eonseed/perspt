//! Source discovery.
//!
//! The PSP code check rules govern **Rust sources only**. Documentation is deliberately
//! out of scope: `docs/` holds PSPs and the Sphinx book, where a 3,500-line
//! specification is correct rather than a violation.

use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use walkdir::WalkDir;

/// Directories never descended into, at any depth.
const EXCLUDED_DIRS: [&str; 5] = ["target", "node_modules", "docs", ".git", ".cargo"];

/// One Rust source file, read once and shared by every rule.
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// Repository-relative path with `/` separators, as it appears in reports.
    pub rel: PathBuf,
    /// File contents.
    pub text: String,
}

impl SourceFile {
    /// Physical line count, matching `wc -l` for newline-terminated files.
    pub fn line_count(&self) -> usize {
        self.text.lines().count()
    }

    /// The file's lines, 0-indexed.
    pub fn lines(&self) -> Vec<&str> {
        self.text.lines().collect()
    }
}

/// Find every in-scope Rust source under `root`, sorted for stable reports.
pub fn discover(root: &Path) -> Result<Vec<SourceFile>> {
    let mut found = Vec::new();
    let walker = WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| !is_excluded_dir(entry.path(), root));

    for entry in walker {
        let entry = entry.context("walking the repository")?;
        let path = entry.path();
        if !entry.file_type().is_file() || path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let rel = path.strip_prefix(root).unwrap_or(path).to_path_buf();
        let text =
            fs::read_to_string(path).with_context(|| format!("reading {}", rel.display()))?;
        found.push(SourceFile {
            rel: normalize(&rel),
            text,
        });
    }

    found.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(found)
}

/// Whether `path` is a directory the scanner must not descend into.
fn is_excluded_dir(path: &Path, root: &Path) -> bool {
    if path == root {
        return false;
    }
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    path.is_dir() && EXCLUDED_DIRS.contains(&name)
}

/// Render a path with `/` separators so reports and baselines match on Windows.
fn normalize(path: &Path) -> PathBuf {
    let joined = path
        .components()
        .filter_map(|c| match c {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    PathBuf::from(joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_count_matches_wc_semantics() {
        let file = SourceFile {
            rel: PathBuf::from("a.rs"),
            text: "one\ntwo\nthree\n".to_string(),
        };
        assert_eq!(file.line_count(), 3);
    }

    #[test]
    fn documentation_directories_are_out_of_scope() {
        let root = Path::new("/repo");
        assert!(EXCLUDED_DIRS.contains(&"docs"));
        assert!(!is_excluded_dir(root, root));
    }

    #[test]
    fn paths_are_reported_with_forward_slashes() {
        let mixed: PathBuf = ["crates", "perspt-sdk", "src", "lib.rs"].iter().collect();
        assert_eq!(
            normalize(&mixed),
            PathBuf::from("crates/perspt-sdk/src/lib.rs")
        );
    }
}
