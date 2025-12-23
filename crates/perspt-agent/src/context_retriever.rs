//! Context Retriever
//!
//! Uses the grep crate (ripgrep library) for fast code search across the workspace.
//! Provides context retrieval for LLM prompts while respecting token budgets.

use anyhow::Result;
use grep::regex::RegexMatcher;
use grep::searcher::sinks::UTF8;
use grep::searcher::Searcher;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

/// A search hit from grep
#[derive(Debug, Clone)]
pub struct SearchHit {
    /// File path (relative to workspace)
    pub file: PathBuf,
    /// Line number (1-indexed)
    pub line: u32,
    /// Content of the matching line
    pub content: String,
    /// Column where match starts (0-indexed)
    pub column: Option<usize>,
}

/// Context retriever for gathering relevant code context
pub struct ContextRetriever {
    /// Workspace root directory
    working_dir: PathBuf,
    /// Maximum bytes to read per file
    max_file_bytes: usize,
    /// Maximum total context bytes
    max_context_bytes: usize,
}

impl ContextRetriever {
    /// Create a new context retriever
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            working_dir,
            max_file_bytes: 50 * 1024,     // 50KB per file
            max_context_bytes: 100 * 1024, // 100KB total
        }
    }

    /// Set max bytes per file
    pub fn with_max_file_bytes(mut self, bytes: usize) -> Self {
        self.max_file_bytes = bytes;
        self
    }

    /// Set max total context bytes
    pub fn with_max_context_bytes(mut self, bytes: usize) -> Self {
        self.max_context_bytes = bytes;
        self
    }

    /// Search for a pattern in the workspace using ripgrep
    /// Respects .gitignore and common ignore patterns
    pub fn search(&self, pattern: &str, max_results: usize) -> Vec<SearchHit> {
        let mut hits = Vec::new();

        // Create regex matcher
        let matcher = match RegexMatcher::new(pattern) {
            Ok(m) => m,
            Err(e) => {
                log::warn!("Invalid search pattern '{}': {}", pattern, e);
                return hits;
            }
        };

        // Walk workspace respecting .gitignore
        let walker = WalkBuilder::new(&self.working_dir)
            .hidden(true) // Skip hidden files
            .git_ignore(true) // Respect .gitignore
            .git_global(true) // Respect global gitignore
            .git_exclude(true) // Respect .git/info/exclude
            .build();

        let mut searcher = Searcher::new();

        for entry in walker.flatten() {
            if hits.len() >= max_results {
                break;
            }

            let path = entry.path();

            // Only search files
            if !path.is_file() {
                continue;
            }

            // Skip binary files by extension
            if Self::is_binary_extension(path) {
                continue;
            }

            // Search the file
            let _ = searcher.search_path(
                &matcher,
                path,
                UTF8(|line_num, line| {
                    if hits.len() < max_results {
                        let relative_path = path
                            .strip_prefix(&self.working_dir)
                            .unwrap_or(path)
                            .to_path_buf();

                        hits.push(SearchHit {
                            file: relative_path,
                            line: line_num as u32,
                            content: line.trim_end().to_string(),
                            column: None,
                        });
                    }
                    Ok(hits.len() < max_results)
                }),
            );
        }

        hits
    }

    /// Read a file with truncation if it exceeds max bytes
    pub fn read_file_truncated(&self, path: &Path) -> Result<String> {
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.working_dir.join(path)
        };

        let content = std::fs::read_to_string(&full_path)?;

        if content.len() > self.max_file_bytes {
            let truncated = &content[..self.max_file_bytes];
            // Find last newline to avoid cutting mid-line
            let last_newline = truncated.rfind('\n').unwrap_or(self.max_file_bytes);
            Ok(format!(
                "{}\n\n... [truncated, {} more bytes]",
                &content[..last_newline],
                content.len() - last_newline
            ))
        } else {
            Ok(content)
        }
    }

    /// Get context for a task based on its context_files and output_files
    /// Returns a formatted string suitable for LLM prompts
    pub fn get_task_context(&self, context_files: &[PathBuf], output_files: &[PathBuf]) -> String {
        let mut context = String::new();
        let mut remaining_budget = self.max_context_bytes;

        // Add context files (files to read for understanding)
        if !context_files.is_empty() {
            context.push_str("## Context Files (for reference)\n\n");
            for file in context_files {
                if remaining_budget == 0 {
                    break;
                }
                if let Ok(content) = self.read_file_truncated(file) {
                    let section = format!("### {}\n```\n{}\n```\n\n", file.display(), content);
                    if section.len() <= remaining_budget {
                        remaining_budget -= section.len();
                        context.push_str(&section);
                    }
                }
            }
        }

        // Add output files (files to modify - show current state)
        if !output_files.is_empty() {
            context.push_str("## Target Files (to modify)\n\n");
            for file in output_files {
                if remaining_budget == 0 {
                    break;
                }
                let full_path = self.working_dir.join(file);
                if full_path.exists() {
                    if let Ok(content) = self.read_file_truncated(file) {
                        let section = format!(
                            "### {} (current content)\n```\n{}\n```\n\n",
                            file.display(),
                            content
                        );
                        if section.len() <= remaining_budget {
                            remaining_budget -= section.len();
                            context.push_str(&section);
                        }
                    }
                } else {
                    context.push_str(&format!("### {} (new file)\n\n", file.display()));
                }
            }
        }

        context
    }

    /// Search for relevant code based on a query (e.g., function name, class name)
    /// Returns formatted context for LLM
    pub fn search_for_context(&self, query: &str, max_results: usize) -> String {
        let hits = self.search(query, max_results);

        if hits.is_empty() {
            return String::new();
        }

        let mut context = format!("## Related Code (search: '{}')\n\n", query);

        for hit in &hits {
            context.push_str(&format!(
                "- **{}:{}**: `{}`\n",
                hit.file.display(),
                hit.line,
                hit.content.trim()
            ));
        }
        context.push('\n');

        context
    }

    /// Check if a file extension indicates a binary file
    fn is_binary_extension(path: &Path) -> bool {
        match path.extension().and_then(|e| e.to_str()) {
            Some(ext) => matches!(
                ext.to_lowercase().as_str(),
                "png"
                    | "jpg"
                    | "jpeg"
                    | "gif"
                    | "bmp"
                    | "ico"
                    | "webp"
                    | "pdf"
                    | "doc"
                    | "docx"
                    | "xls"
                    | "xlsx"
                    | "ppt"
                    | "pptx"
                    | "zip"
                    | "tar"
                    | "gz"
                    | "bz2"
                    | "7z"
                    | "rar"
                    | "exe"
                    | "dll"
                    | "so"
                    | "dylib"
                    | "a"
                    | "wasm"
                    | "o"
                    | "obj"
                    | "pyc"
                    | "pyo"
                    | "class"
                    | "db"
                    | "sqlite"
                    | "sqlite3"
            ),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_search_finds_pattern() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.py");
        fs::write(&file_path, "def hello_world():\n    print('Hello')\n").unwrap();

        let retriever = ContextRetriever::new(dir.path().to_path_buf());
        let hits = retriever.search("hello_world", 10);

        assert_eq!(hits.len(), 1);
        assert!(hits[0].content.contains("def hello_world"));
    }

    #[test]
    fn test_read_file_truncated() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("large.txt");
        let content = "line\n".repeat(10000); // ~50KB
        fs::write(&file_path, &content).unwrap();

        let retriever = ContextRetriever::new(dir.path().to_path_buf()).with_max_file_bytes(1000);

        let result = retriever.read_file_truncated(&file_path).unwrap();
        assert!(result.contains("truncated"));
        assert!(result.len() < 2000); // Should be truncated + message
    }
}
