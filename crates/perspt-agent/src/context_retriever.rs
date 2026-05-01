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

    // =========================================================================
    // PSP-5 Phase 3: Context Provenance & Structural Digests
    // =========================================================================

    /// PSP-5 Phase 3: Build a restriction map for a node
    ///
    /// The restriction map defines the context boundary: what files, digests,
    /// and summaries a node is allowed to see. Built from the ownership manifest,
    /// task graph, and parent scope.
    pub fn build_restriction_map(
        &self,
        node: &perspt_core::types::SRBNNode,
        manifest: &perspt_core::types::OwnershipManifest,
    ) -> perspt_core::types::RestrictionMap {
        let mut map = perspt_core::types::RestrictionMap::for_node(node.node_id.clone());

        // Add files owned by this node
        let owned = manifest.files_owned_by(&node.node_id);
        map.owned_files = owned.iter().map(|s| s.to_string()).collect();

        // Add output targets (node's primary files)
        for target in &node.output_targets {
            let path_str = target.to_string_lossy().to_string();
            if !map.owned_files.contains(&path_str) {
                map.owned_files.push(path_str);
            }
        }

        // Add context files as sealed interfaces (read-only dependencies)
        for ctx_file in &node.context_files {
            map.sealed_interfaces
                .push(ctx_file.to_string_lossy().to_string());
        }

        // Apply budget from retriever limits
        map.budget = perspt_core::types::ContextBudget {
            byte_limit: self.max_context_bytes,
            file_count_limit: 20,
        };

        map
    }

    /// PSP-5 Phase 3: Assemble a reproducible context package for a node
    ///
    /// Builds a complete, bounded context package from the restriction map.
    /// Prioritizes: owned files (full content) > sealed interfaces (digest or content) > summaries.
    pub fn assemble_context_package(
        &self,
        node: &perspt_core::types::SRBNNode,
        restriction_map: &perspt_core::types::RestrictionMap,
    ) -> perspt_core::types::ContextPackage {
        let mut package = perspt_core::types::ContextPackage::new(node.node_id.clone());
        package.restriction_map = restriction_map.clone();

        // 1. Include owned files in full (highest priority — node needs these)
        for file_path in &restriction_map.owned_files {
            let full_path = self.working_dir.join(file_path);
            if full_path.exists() {
                if let Ok(content) = self.read_file_truncated(&full_path) {
                    if !package.add_file(file_path, content) {
                        log::warn!(
                            "Budget exceeded adding owned file '{}' for node '{}'",
                            file_path,
                            node.node_id
                        );
                        break;
                    }
                }
            }
        }

        // 2. Include sealed interfaces (prefer digest if budget is tight)
        for iface_path in &restriction_map.sealed_interfaces {
            let full_path = self.working_dir.join(iface_path);
            if full_path.exists() {
                // Try to include full content if budget allows
                if let Ok(content) = self.read_file_truncated(&full_path) {
                    if !package.add_file(iface_path, content) {
                        // Budget exceeded — compute digest instead
                        if let Ok(raw) = std::fs::read(&full_path) {
                            let digest = perspt_core::types::StructuralDigest::from_content(
                                &node.node_id,
                                iface_path,
                                perspt_core::types::ArtifactKind::InterfaceSeal,
                                &raw,
                            );
                            package.add_structural_digest(digest);
                        }
                    }
                }
            }
        }

        // 3. Include any pre-existing structural digests from the restriction map
        for digest in &restriction_map.structural_digests {
            package.add_structural_digest(digest.clone());
        }

        // 4. Include summary digests
        for summary in &restriction_map.summary_digests {
            package.add_summary_digest(summary.clone());
        }

        package
    }

    /// PSP-5 Phase 3: Compute a structural digest for a file
    pub fn compute_structural_digest(
        &self,
        path: &str,
        artifact_kind: perspt_core::types::ArtifactKind,
        source_node_id: &str,
    ) -> Result<perspt_core::types::StructuralDigest> {
        let full_path = self.working_dir.join(path);
        let content = std::fs::read(&full_path)?;
        Ok(perspt_core::types::StructuralDigest::from_content(
            source_node_id,
            path,
            artifact_kind,
            &content,
        ))
    }

    /// PSP-5 Phase 3: Format a context package as text for LLM prompts
    pub fn format_context_package(&self, package: &perspt_core::types::ContextPackage) -> String {
        let mut context = String::new();

        // Owned/included files
        if !package.included_files.is_empty() {
            context.push_str("## Context Files\n\n");
            for (path, content) in &package.included_files {
                context.push_str(&format!("### {}\n```\n{}\n```\n\n", path, content));
            }
        }

        // Structural digests (compact representation)
        if !package.structural_digests.is_empty() {
            context.push_str("## Structural Dependencies (digests)\n\n");
            for digest in &package.structural_digests {
                context.push_str(&format!(
                    "- {} ({}) from node '{}' [hash: {:02x}{:02x}..]\n",
                    digest.source_path,
                    digest.artifact_kind,
                    digest.source_node_id,
                    digest.hash[0],
                    digest.hash[1],
                ));
            }
            context.push('\n');
        }

        // Summary digests
        if !package.summary_digests.is_empty() {
            context.push_str("## Advisory Summaries\n\n");
            for summary in &package.summary_digests {
                context.push_str(&format!(
                    "### {} (from {})\n{}\n\n",
                    summary.digest_id, summary.source_node_id, summary.summary_text
                ));
            }
        }

        if package.budget_exceeded {
            context.push_str(
                "\n> Note: Context budget was exceeded. Some files replaced with structural digests.\n",
            );
        }

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

    /// PSP-5 Phase 3: Validate a persisted provenance record against the current workspace.
    ///
    /// Parses structural digest references from the provenance record and checks
    /// whether the referenced source files still exist on disk. Returns a list
    /// of missing file paths — empty means no drift detected.
    pub fn validate_provenance_record(
        &self,
        record: &perspt_store::ContextProvenanceRecord,
    ) -> Vec<String> {
        let mut missing = Vec::new();

        // Parse structural_hashes JSON: entries have format "digest_id:hex_hash"
        // where digest_id is "source_node_id:source_path:artifact_kind".
        if let Ok(entries) = serde_json::from_str::<Vec<String>>(&record.structural_hashes) {
            for entry in &entries {
                // Entry format: "source_node_id:source_path:artifact_kind:hex_hash"
                // Split and extract source_path (second segment)
                let parts: Vec<&str> = entry.splitn(4, ':').collect();
                if parts.len() >= 3 {
                    // parts[0] = source_node_id, parts[1] = source_path,
                    // parts[2..] = artifact_kind:hex_hash
                    let source_path = parts[1];
                    let full_path = self.working_dir.join(source_path);
                    if !full_path.exists() {
                        missing.push(source_path.to_string());
                    }
                }
            }
        }

        missing
    }

    // =========================================================================
    // PSP-5: Project Summary for Existing-Project Context
    // =========================================================================

    /// Gather a structured project summary for injection into sheafification prompts.
    ///
    /// Returns a formatted string describing: detected language plugins,
    /// dependency manifests, entry points, test locations, and build system.
    /// Uses the plugin registry and file-system inspection; zero LLM calls.
    pub fn get_project_summary(&self) -> String {
        let registry = perspt_core::plugin::PluginRegistry::new();
        let detected = registry.detect_all(&self.working_dir);

        if detected.is_empty() {
            return String::new();
        }

        let mut summary = String::from("## Existing Project Summary\n\n");

        for plugin in &detected {
            summary.push_str(&format!("**Language/Plugin:** {}\n", plugin.name()));
        }
        summary.push('\n');

        // Dependency manifests
        let manifest_candidates = [
            "Cargo.toml",
            "pyproject.toml",
            "setup.py",
            "requirements.txt",
            "package.json",
            "uv.lock",
            "Cargo.lock",
            "poetry.lock",
        ];
        let mut found_manifests = Vec::new();
        for candidate in &manifest_candidates {
            if self.working_dir.join(candidate).exists() {
                found_manifests.push(*candidate);
            }
        }
        if !found_manifests.is_empty() {
            summary.push_str(&format!(
                "**Dependency manifests:** {}\n",
                found_manifests.join(", ")
            ));
        }

        // Entry points
        let entry_candidates = [
            "src/main.rs",
            "src/lib.rs",
            "src/main.py",
            "main.py",
            "app.py",
            "__main__.py",
            "src/index.ts",
            "src/index.js",
            "index.ts",
            "index.js",
        ];
        let mut found_entries = Vec::new();
        for candidate in &entry_candidates {
            if self.working_dir.join(candidate).exists() {
                found_entries.push(*candidate);
            }
        }
        if !found_entries.is_empty() {
            summary.push_str(&format!("**Entry points:** {}\n", found_entries.join(", ")));
        }

        // Test locations
        let test_candidates = ["tests/", "test/", "src/tests/", "tests.py", "test_*.py"];
        let mut found_tests = Vec::new();
        for candidate in &test_candidates {
            if self.working_dir.join(candidate).exists() {
                found_tests.push(*candidate);
            }
        }
        if !found_tests.is_empty() {
            summary.push_str(&format!("**Test locations:** {}\n", found_tests.join(", ")));
        }

        // Read key manifest content (truncated) for context
        for manifest in &found_manifests {
            if let Ok(content) = self.read_file_truncated(Path::new(manifest)) {
                // Only include first 2KB of each manifest
                let truncated = if content.len() > 2048 {
                    format!("{}...\n[truncated]", &content[..2048])
                } else {
                    content
                };
                summary.push_str(&format!("\n### {}\n```\n{}\n```\n", manifest, truncated));
            }
        }

        summary
    }

    /// Gather bounded evidence for the architect: API seams, module
    /// boundaries, test-to-source mapping, and dependency hotspots.
    ///
    /// Returns a markdown section suitable for injection into the architect
    /// prompt so it respects existing architecture rather than re-scaffolding.
    pub fn gather_architect_evidence(&self) -> String {
        let mut sections: Vec<String> = Vec::new();

        // 1. Public API seams: scan for exported symbols
        let api_hits = self.search(r"pub\s+(fn|struct|trait|enum|type|mod)\b", 30);
        if !api_hits.is_empty() {
            let mut lines: Vec<String> = Vec::new();
            for hit in &api_hits {
                let rel = hit
                    .file
                    .strip_prefix(&self.working_dir)
                    .unwrap_or(&hit.file);
                lines.push(format!(
                    "- `{}` L{}: {}",
                    rel.display(),
                    hit.line,
                    hit.content.trim()
                ));
            }
            sections.push(format!(
                "### API Seams (public symbols)\n{}",
                lines.join("\n")
            ));
        }

        // 2. Module boundaries: look for `mod` declarations
        let mod_hits = self.search(r"^pub\s+mod\s+\w+", 20);
        if !mod_hits.is_empty() {
            let mut lines: Vec<String> = Vec::new();
            for hit in &mod_hits {
                let rel = hit
                    .file
                    .strip_prefix(&self.working_dir)
                    .unwrap_or(&hit.file);
                lines.push(format!(
                    "- `{}` L{}: {}",
                    rel.display(),
                    hit.line,
                    hit.content.trim()
                ));
            }
            sections.push(format!("### Module Boundaries\n{}", lines.join("\n")));
        }

        // 3. Test layout: map test files to source modules
        let test_hits = self.search(r"#\[test\]|#\[cfg\(test\)\]|def test_|class Test", 20);
        if !test_hits.is_empty() {
            let mut test_files: Vec<String> = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for hit in &test_hits {
                let rel = hit
                    .file
                    .strip_prefix(&self.working_dir)
                    .unwrap_or(&hit.file);
                let key = rel.display().to_string();
                if seen.insert(key.clone()) {
                    test_files.push(format!("- `{}`", key));
                }
            }
            sections.push(format!(
                "### Test Layout\nFiles containing tests:\n{}",
                test_files.join("\n")
            ));
        }

        // 4. Dependency hotspots: most-imported modules
        let import_hits = self.search(r"^use |^from \w+ import|^import |require\(", 40);
        if !import_hits.is_empty() {
            let mut counts: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            for hit in &import_hits {
                let rel = hit
                    .file
                    .strip_prefix(&self.working_dir)
                    .unwrap_or(&hit.file);
                *counts.entry(rel.display().to_string()).or_insert(0) += 1;
            }
            let mut sorted: Vec<_> = counts.into_iter().collect();
            sorted.sort_by_key(|b| std::cmp::Reverse(b.1));
            let top: Vec<String> = sorted
                .iter()
                .take(10)
                .map(|(f, c)| format!("- `{}`: {} import statements", f, c))
                .collect();
            sections.push(format!(
                "### Dependency Hotspots (files with most imports)\n{}",
                top.join("\n")
            ));
        }

        if sections.is_empty() {
            String::new()
        } else {
            format!("## Architect Evidence\n\n{}\n", sections.join("\n\n"))
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

    // =========================================================================
    // PSP-5 Phase 3: Restriction Maps & Context Packages
    // =========================================================================

    #[test]
    fn test_build_restriction_map() {
        let dir = tempdir().unwrap();
        let retriever = ContextRetriever::new(dir.path().to_path_buf());

        let mut node = perspt_core::types::SRBNNode::new(
            "node_1".to_string(),
            "test goal".to_string(),
            perspt_core::types::ModelTier::Actuator,
        );
        node.output_targets = vec![std::path::PathBuf::from("src/main.rs")];
        node.context_files = vec![std::path::PathBuf::from("src/lib.rs")];

        let mut manifest = perspt_core::types::OwnershipManifest::new();
        manifest.assign(
            "src/main.rs",
            "node_1",
            "rust",
            perspt_core::types::NodeClass::Implementation,
        );
        manifest.assign(
            "src/utils.rs",
            "node_1",
            "rust",
            perspt_core::types::NodeClass::Implementation,
        );

        let map = retriever.build_restriction_map(&node, &manifest);

        assert_eq!(map.node_id, "node_1");
        // Owned files: src/main.rs (from output_targets) + src/utils.rs (from manifest)
        assert!(map.owned_files.contains(&"src/main.rs".to_string()));
        assert!(map.owned_files.contains(&"src/utils.rs".to_string()));
        // Sealed interfaces: src/lib.rs (from context_files)
        assert_eq!(map.sealed_interfaces, vec!["src/lib.rs".to_string()]);
    }

    #[test]
    fn test_assemble_context_package_with_files() {
        let dir = tempdir().unwrap();
        // Create a file that the node owns
        let src_dir = dir.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("main.rs"), "fn main() {}").unwrap();

        let retriever = ContextRetriever::new(dir.path().to_path_buf());

        let node = perspt_core::types::SRBNNode::new(
            "node_1".to_string(),
            "test goal".to_string(),
            perspt_core::types::ModelTier::Actuator,
        );

        let mut map = perspt_core::types::RestrictionMap::for_node("node_1".to_string());
        map.owned_files.push("src/main.rs".to_string());
        map.budget.byte_limit = 10 * 1024; // 10KB

        let package = retriever.assemble_context_package(&node, &map);

        assert_eq!(package.node_id, "node_1");
        assert!(package.included_files.contains_key("src/main.rs"));
        assert!(!package.budget_exceeded);
        assert!(package.total_bytes > 0);
    }

    #[test]
    fn test_assemble_context_package_budget_exceeded() {
        let dir = tempdir().unwrap();
        let src_dir = dir.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        // Create a file larger than the budget
        fs::write(src_dir.join("big.rs"), "x".repeat(500)).unwrap();

        let retriever = ContextRetriever::new(dir.path().to_path_buf());

        let node = perspt_core::types::SRBNNode::new(
            "node_1".to_string(),
            "test goal".to_string(),
            perspt_core::types::ModelTier::Actuator,
        );

        let mut map = perspt_core::types::RestrictionMap::for_node("node_1".to_string());
        map.owned_files.push("src/big.rs".to_string());
        map.budget.byte_limit = 100; // Very small budget

        let package = retriever.assemble_context_package(&node, &map);
        assert!(package.budget_exceeded);
    }

    #[test]
    fn test_format_context_package_empty() {
        let retriever = ContextRetriever::new(PathBuf::from("."));
        let package = perspt_core::types::ContextPackage::new("node_1".to_string());

        let formatted = retriever.format_context_package(&package);
        assert!(formatted.is_empty());
    }

    #[test]
    fn test_format_context_package_with_files() {
        let retriever = ContextRetriever::new(PathBuf::from("."));
        let mut package = perspt_core::types::ContextPackage::new("node_1".to_string());
        package.add_file("src/main.rs", "fn main() {}".to_string());

        let formatted = retriever.format_context_package(&package);
        assert!(formatted.contains("## Context Files"));
        assert!(formatted.contains("src/main.rs"));
        assert!(formatted.contains("fn main() {}"));
    }

    #[test]
    fn test_compute_structural_digest() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.rs"), "fn test() {}").unwrap();

        let retriever = ContextRetriever::new(dir.path().to_path_buf());
        let digest = retriever
            .compute_structural_digest(
                "test.rs",
                perspt_core::types::ArtifactKind::Signature,
                "node_1",
            )
            .unwrap();

        assert_eq!(digest.source_node_id, "node_1");
        assert_eq!(digest.source_path, "test.rs");
        assert_ne!(digest.hash, [0u8; 32]);
    }

    #[test]
    fn test_gather_architect_evidence_rust_project() {
        let dir = tempdir().unwrap();
        // Create a small Rust-like project
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(
            dir.path().join("src/lib.rs"),
            "pub mod math;\npub mod utils;\n\nuse crate::math::add;\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("src/math.rs"),
            "pub fn add(a: i32, b: i32) -> i32 { a + b }\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn test_add() { assert_eq!(super::add(1, 2), 3); }\n}\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("src/utils.rs"),
            "pub struct Config { pub name: String }\n",
        )
        .unwrap();

        let retriever = ContextRetriever::new(dir.path().to_path_buf());
        let evidence = retriever.gather_architect_evidence();

        assert!(
            evidence.contains("Architect Evidence"),
            "Should produce an evidence section"
        );
        assert!(evidence.contains("API Seams"), "Should find public symbols");
        assert!(
            evidence.contains("pub fn add") || evidence.contains("pub mod math"),
            "Should list at least one public API"
        );
    }

    #[test]
    fn test_gather_architect_evidence_empty_dir() {
        let dir = tempdir().unwrap();
        let retriever = ContextRetriever::new(dir.path().to_path_buf());
        let evidence = retriever.gather_architect_evidence();
        assert!(
            evidence.is_empty(),
            "Empty projects should produce no evidence"
        );
    }
}
