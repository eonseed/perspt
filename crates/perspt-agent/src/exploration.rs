//! Deterministic, read-only repository orientation for the PSP-9 runtime.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use ignore::WalkBuilder;
use perspt_sdk::{ExplorationReport, GraphHint, ProjectMap};

const MAX_FILES: usize = 5_000;

/// Build a bounded project map from filesystem metadata and small manifest
/// hashes. This phase has no mutation capability and is safe to run in
/// parallel with other read-only preparation.
pub fn map_workspace(root: &Path) -> Result<ExplorationReport> {
    let mut scan = WorkspaceScan::default();
    let mut walker = WalkBuilder::new(root);
    let walk_root = root.to_path_buf();
    walker
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .max_depth(Some(12))
        .filter_entry(move |entry| include_entry(&walk_root, entry.path()));
    for entry in walker
        .build()
        .flatten()
        .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
        .take(MAX_FILES)
    {
        scan.observe(root, entry.path());
    }

    let mut languages: Vec<String> = scan
        .language_counts
        .into_iter()
        .map(|(language, count)| format!("{language}:{count}"))
        .collect();
    languages.sort();
    let map = ProjectMap {
        languages,
        package_roots: scan.package_roots.into_iter().collect(),
        build_systems: scan.build_systems.into_iter().collect(),
        entry_points: scan.entry_points.into_iter().take(64).collect(),
        risk_hotspots: scan.risk_hotspots.into_iter().take(64).collect(),
    };
    let mut report = ExplorationReport::new(map);
    report.deterministically_backed = true;
    report.input_witnesses = scan.witnesses;
    report.graph_hints.push(GraphHint {
        goal: "implement the requested change against the mapped package roots".into(),
        suggested_outputs: Vec::new(),
        rationale: "deterministic repository orientation; output scope remains proposal-bound"
            .into(),
    });
    Ok(report)
}

fn include_entry(root: &Path, path: &Path) -> bool {
    if path == root {
        return true;
    }
    !path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some(
                ".git"
                    | ".perspt"
                    | ".venv"
                    | ".pytest_cache"
                    | ".ruff_cache"
                    | ".mypy_cache"
                    | "__pycache__"
                    | "node_modules"
                    | "target"
            )
        )
    })
}

#[derive(Default)]
struct WorkspaceScan {
    language_counts: BTreeMap<&'static str, usize>,
    package_roots: BTreeSet<String>,
    build_systems: BTreeSet<String>,
    entry_points: BTreeSet<String>,
    risk_hotspots: BTreeSet<String>,
    witnesses: Vec<String>,
}

impl WorkspaceScan {
    fn observe(&mut self, root: &Path, path: &Path) {
        let relative = path.strip_prefix(root).unwrap_or(path);
        let rendered = relative.to_string_lossy().replace('\\', "/");
        if let Some(language) = language_for(path) {
            *self.language_counts.entry(language).or_default() += 1;
        }
        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            if let Some(system) = build_system(name) {
                self.build_systems.insert(system.to_string());
                self.package_roots.insert(
                    relative
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
                if let Ok(bytes) = std::fs::read(path) {
                    self.witnesses.push(format!(
                        "{}:{}",
                        rendered,
                        perspt_sdk::ledger::content_hash(&bytes)
                    ));
                }
            }
            if matches!(
                name,
                "main.rs" | "main.py" | "main.ts" | "main.js" | "lib.rs"
            ) || name.starts_with("app.")
            {
                self.entry_points.insert(rendered.clone());
            }
        }
        if rendered.contains("migrations/")
            || rendered.contains(".github/workflows/")
            || rendered.contains("auth")
            || rendered.contains("security")
            || rendered.ends_with("Cargo.lock")
            || rendered.ends_with("package-lock.json")
        {
            self.risk_hotspots.insert(rendered);
        }
    }
}

fn language_for(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "rs" => Some("rust"),
        "py" => Some("python"),
        "ts" | "tsx" => Some("typescript"),
        "js" | "jsx" => Some("javascript"),
        "go" => Some("go"),
        "java" => Some("java"),
        "kt" | "kts" => Some("kotlin"),
        "c" | "h" => Some("c"),
        "cc" | "cpp" | "hpp" => Some("cpp"),
        "zig" => Some("zig"),
        "rb" => Some("ruby"),
        _ => None,
    }
}

fn build_system(name: &str) -> Option<&'static str> {
    match name {
        "Cargo.toml" => Some("cargo"),
        "pyproject.toml" => Some("python-pyproject"),
        "package.json" => Some("node"),
        "go.mod" => Some("go-modules"),
        "pom.xml" => Some("maven"),
        "build.gradle" | "build.gradle.kts" => Some("gradle"),
        "build.zig" => Some("zig-build"),
        "Makefile" => Some("make"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_map_is_deterministic_and_manifest_backed() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src")).unwrap();
        std::fs::write(root.path().join("Cargo.toml"), "[package]\nname='x'\n").unwrap();
        std::fs::write(root.path().join("src/main.rs"), "fn main() {}\n").unwrap();
        let left = map_workspace(root.path()).unwrap();
        let right = map_workspace(root.path()).unwrap();
        assert_eq!(left.project_map, right.project_map);
        assert_eq!(left.input_witnesses, right.input_witnesses);
        assert!(left.deterministically_backed);
        assert_eq!(left.project_map.build_systems, ["cargo"]);
    }

    #[test]
    fn project_map_excludes_installed_environments_and_build_outputs() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src")).unwrap();
        std::fs::create_dir_all(root.path().join(".venv/lib/site-packages/pkg")).unwrap();
        std::fs::create_dir_all(root.path().join("target/generated")).unwrap();
        std::fs::write(root.path().join("pyproject.toml"), "[project]\nname='x'\n").unwrap();
        std::fs::write(root.path().join("src/main.py"), "print('x')\n").unwrap();
        std::fs::write(
            root.path().join(".venv/lib/site-packages/pkg/main.py"),
            "installed = True\n",
        )
        .unwrap();
        std::fs::write(
            root.path().join("target/generated/main.py"),
            "generated = True\n",
        )
        .unwrap();

        let report = map_workspace(root.path()).unwrap();
        assert_eq!(report.project_map.languages, ["python:1"]);
        assert_eq!(report.project_map.entry_points, ["src/main.py"]);
    }
}
