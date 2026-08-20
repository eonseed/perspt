//! Deterministic, read-only repository orientation for the PSP-9 runtime.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use anyhow::Result;
use ignore::WalkBuilder;
use perspt_sdk::ProjectMap;
use serde::{Deserialize, Serialize};

/// The deterministic repository map plus its provenance witnesses — the
/// seed of `SearchContext` (PSP-10: `ProjectMap` re-homed into
/// `perspt_sdk::search`; the removed `ExplorationReport` surface collapses
/// to exactly what the runtime consumes).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceMap {
    pub project_map: ProjectMap,
    /// Content hashes of the observed inputs, for provenance.
    pub input_witnesses: Vec<String>,
}

const MAX_PACKAGE_ROOTS: usize = 512;
const MAX_ENTRY_POINTS: usize = 64;
const MAX_RISK_HOTSPOTS: usize = 64;
const MAX_MANIFEST_WITNESSES: usize = 512;

/// Build a bounded project map from filesystem metadata and streamed
/// manifest hashes. This phase has no mutation capability and is safe to
/// run in parallel with other read-only preparation.
pub fn map_workspace(root: &Path) -> Result<WorkspaceMap> {
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
        entry_points: scan.entry_points.into_iter().collect(),
        risk_hotspots: scan.risk_hotspots.into_iter().collect(),
    };
    Ok(WorkspaceMap {
        project_map: map,
        input_witnesses: scan
            .witnesses
            .into_iter()
            .map(|(path, hash)| format!("{path}:{hash}"))
            .collect(),
    })
}

fn include_entry(root: &Path, path: &Path) -> bool {
    if path == root {
        return true;
    }
    let relative = path.strip_prefix(root).unwrap_or(path);
    !relative.components().any(|component| {
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
    witnesses: BTreeMap<String, String>,
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
                bounded_insert(
                    &mut self.package_roots,
                    relative
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .to_string_lossy()
                        .replace('\\', "/"),
                    MAX_PACKAGE_ROOTS,
                );
                if let Ok(hash) = hash_file(path) {
                    bounded_insert_map(
                        &mut self.witnesses,
                        rendered.clone(),
                        hash,
                        MAX_MANIFEST_WITNESSES,
                    );
                }
            }
            if matches!(
                name,
                "main.rs" | "main.py" | "main.ts" | "main.js" | "lib.rs"
            ) || name.starts_with("app.")
            {
                bounded_insert(&mut self.entry_points, rendered.clone(), MAX_ENTRY_POINTS);
            }
        }
        if rendered.contains("migrations/")
            || rendered.contains(".github/workflows/")
            || rendered.contains("auth")
            || rendered.contains("security")
            || rendered.ends_with("Cargo.lock")
            || rendered.ends_with("package-lock.json")
        {
            bounded_insert(&mut self.risk_hotspots, rendered, MAX_RISK_HOTSPOTS);
        }
    }
}

/// Retain the lexicographically smallest `cap` values independent of
/// traversal order. The whole repository is observed, while summary memory
/// remains bounded and deterministic.
fn bounded_insert(values: &mut BTreeSet<String>, value: String, cap: usize) {
    values.insert(value);
    if values.len() > cap {
        values.pop_last();
    }
}

fn bounded_insert_map(
    values: &mut BTreeMap<String, String>,
    key: String,
    value: String,
    cap: usize,
) {
    values.insert(key, value);
    if values.len() > cap {
        values.pop_last();
    }
}

/// SHA-256 a manifest as a stream. Repository orientation never needs to
/// materialize even an accidentally enormous manifest in memory.
fn hash_file(path: &Path) -> std::io::Result<String> {
    use sha2::Digest;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut hex, "{byte:02x}").expect("writing into a string cannot fail");
    }
    Ok(hex)
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

    #[test]
    fn bounded_summaries_are_independent_of_traversal_order() {
        let values: Vec<String> = (0..600).map(|index| format!("crate-{index:04}")).collect();
        let mut forward = BTreeSet::new();
        let mut reverse = BTreeSet::new();
        for value in &values {
            bounded_insert(&mut forward, value.clone(), MAX_PACKAGE_ROOTS);
        }
        for value in values.iter().rev() {
            bounded_insert(&mut reverse, value.clone(), MAX_PACKAGE_ROOTS);
        }
        assert_eq!(forward, reverse);
        assert_eq!(forward.len(), MAX_PACKAGE_ROOTS);
        assert!(forward.contains("crate-0000"));
        assert!(!forward.contains("crate-0599"));
    }

    #[test]
    fn streamed_manifest_witness_matches_the_content_hash() {
        let root = tempfile::tempdir().unwrap();
        let mut manifest = b"[package]\nname='large'\n#".to_vec();
        manifest.extend(std::iter::repeat_n(b'x', 200_000));
        let path = root.path().join("Cargo.toml");
        std::fs::write(&path, &manifest).unwrap();
        assert_eq!(
            hash_file(&path).unwrap(),
            perspt_sdk::ledger::content_hash(&manifest)
        );
    }
}
