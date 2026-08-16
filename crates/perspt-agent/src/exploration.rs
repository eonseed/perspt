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
    let mut language_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut package_roots = BTreeSet::new();
    let mut build_systems = BTreeSet::new();
    let mut entry_points = BTreeSet::new();
    let mut risk_hotspots = BTreeSet::new();
    let mut witnesses = Vec::new();

    for entry in WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .max_depth(Some(12))
        .build()
        .flatten()
        .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
        .take(MAX_FILES)
    {
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(path);
        let rendered = relative.to_string_lossy().replace('\\', "/");
        if let Some(language) = language_for(path) {
            *language_counts.entry(language).or_default() += 1;
        }
        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            if let Some(system) = build_system(name) {
                build_systems.insert(system.to_string());
                package_roots.insert(
                    relative
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
                if let Ok(bytes) = std::fs::read(path) {
                    witnesses.push(format!(
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
                entry_points.insert(rendered.clone());
            }
        }
        if rendered.contains("migrations/")
            || rendered.contains(".github/workflows/")
            || rendered.contains("auth")
            || rendered.contains("security")
            || rendered.ends_with("Cargo.lock")
            || rendered.ends_with("package-lock.json")
        {
            risk_hotspots.insert(rendered);
        }
    }

    let mut languages: Vec<String> = language_counts
        .into_iter()
        .map(|(language, count)| format!("{language}:{count}"))
        .collect();
    languages.sort();
    let map = ProjectMap {
        languages,
        package_roots: package_roots.into_iter().collect(),
        build_systems: build_systems.into_iter().collect(),
        entry_points: entry_points.into_iter().take(64).collect(),
        risk_hotspots: risk_hotspots.into_iter().take(64).collect(),
    };
    let mut report = ExplorationReport::new(map);
    report.deterministically_backed = true;
    report.input_witnesses = witnesses;
    report.graph_hints.push(GraphHint {
        goal: "implement the requested change against the mapped package roots".into(),
        suggested_outputs: Vec::new(),
        rationale: "deterministic repository orientation; output scope remains proposal-bound"
            .into(),
    });
    Ok(report)
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
}
