//! Fixture materialization and the hidden-oracle verification harness.
//!
//! The hidden suite runs in a fresh copy of the post-run fixture with the
//! pristine build manifests restored, agent-added test configuration
//! removed, the withheld `hidden/` tree overlaid on top, and a freshly
//! synced Python environment — so a weakened visible test, a rewritten
//! manifest, or a poisoned `.venv` cannot subvert the oracle.

use std::path::Path;
use std::process::Command;

use anyhow::Context as _;

use crate::Task;

/// Directories never copied into the hidden-verification tree.
pub(crate) const SKIP_DIRS: &[&str] = &[
    "target",
    ".venv",
    ".perspt",
    ".perspt-target",
    ".perspt-tmp",
    ".perspt-home",
    ".pytest_cache",
    ".ruff_cache",
    "__pycache__",
    "node_modules",
];

/// Build-manifest names restored from the pristine fixture before the
/// oracle runs. An agent may edit them while solving the task, but the
/// hidden suite is judged under the fixture's own harness declaration.
const PRISTINE_MANIFESTS: &[&str] = &["Cargo.toml", "pyproject.toml"];

/// Test-configuration files an agent could add to redirect or silence the
/// oracle; removed unless the pristine fixture or the hidden overlay
/// ships one at the same relative path.
const AGENT_TEST_CONFIG: &[&str] = &["conftest.py", "pytest.ini", "tox.ini", "setup.cfg"];

pub(crate) fn copy_tree(source: &Path, destination: &Path) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if SKIP_DIRS.contains(&name.as_str()) || name == "eval-ledger.db" {
            continue;
        }
        let target = destination.join(&name);
        if entry.file_type()?.is_dir() {
            std::fs::create_dir_all(&target)?;
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn visit_tree(
    root: &Path,
    relative: &Path,
    visit: &mut dyn FnMut(&Path, &Path) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(root.join(relative))? {
        let entry = entry?;
        let name = entry.file_name();
        if SKIP_DIRS.contains(&name.to_string_lossy().as_ref()) {
            continue;
        }
        let child = relative.join(&name);
        if entry.file_type()?.is_dir() {
            visit_tree(root, &child, visit)?;
        } else {
            visit(&child, &entry.path())?;
        }
    }
    Ok(())
}

fn restore_pristine_manifests(pristine: &Path, verify: &Path) -> anyhow::Result<()> {
    visit_tree(pristine, Path::new(""), &mut |relative, source| {
        let name = source.file_name().and_then(|name| name.to_str());
        if name.is_some_and(|name| PRISTINE_MANIFESTS.contains(&name)) {
            let target = verify.join(relative);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(source, target)?;
        }
        Ok(())
    })
}

fn purge_agent_test_config(pristine: &Path, hidden: &Path, verify: &Path) -> anyhow::Result<()> {
    let mut planted = Vec::new();
    visit_tree(verify, Path::new(""), &mut |relative, path| {
        let name = path.file_name().and_then(|name| name.to_str());
        if name.is_some_and(|name| AGENT_TEST_CONFIG.contains(&name))
            && !pristine.join(relative).is_file()
            && !hidden.join(relative).is_file()
        {
            planted.push(path.to_path_buf());
        }
        Ok(())
    })?;
    for path in planted {
        std::fs::remove_file(&path)
            .with_context(|| format!("remove planted {}", path.display()))?;
    }
    Ok(())
}

/// The hidden suite: a fresh copy of the post-run fixture with pristine
/// manifests restored, planted test configuration removed, the withheld
/// `hidden/` tree overlaid on top (overwriting any weakened visible
/// test), and a fresh `.venv`, run identically for every arm. `Ok(pass)`
/// is a completed oracle verdict; `Err` is an infrastructure failure and
/// must be reported as one, never as a scored test failure.
pub(crate) fn hidden_verdict(fixture: &Path, task: &Task) -> anyhow::Result<bool> {
    let verify = tempfile::tempdir().context("hidden verification tempdir")?;
    copy_tree(fixture, verify.path()).context("copy post-run fixture")?;
    restore_pristine_manifests(&task.fixture_dir, verify.path())?;
    purge_agent_test_config(&task.fixture_dir, &task.hidden_dir, verify.path())?;
    if task.hidden_dir.is_dir() {
        copy_tree(&task.hidden_dir, verify.path()).context("overlay hidden oracle")?;
    }
    prepare_python_env(verify.path())?;
    let check = &task.spec.hidden_check;
    let output = Command::new(&check[0])
        .args(&check[1..])
        .current_dir(verify.path())
        .env("CARGO_INCREMENTAL", "0")
        .output()
        .with_context(|| format!("spawn hidden check {check:?}"))?;
    Ok(output.status.success())
}

/// The synced environment is part of the harness, not the agent's job:
/// verification is offline (`uv run --no-sync`), so pytest must already
/// be importable from a `.venv` the agent never touched.
pub(crate) fn prepare_python_env(dir: &Path) -> anyhow::Result<()> {
    if !dir.join("pyproject.toml").is_file() {
        return Ok(());
    }
    for args in [vec!["venv", "-q"], vec!["pip", "install", "-q", "pytest"]] {
        let status = Command::new("uv")
            .args(&args)
            .current_dir(dir)
            .status()
            .with_context(|| format!("spawn uv {args:?}"))?;
        anyhow::ensure!(status.success(), "fixture env setup failed: uv {args:?}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_oracle_tree_restores_manifests_and_drops_planted_test_config() {
        let pristine = tempfile::tempdir().unwrap();
        let hidden = tempfile::tempdir().unwrap();
        let verify = tempfile::tempdir().unwrap();
        std::fs::write(pristine.path().join("Cargo.toml"), "[package]\n").unwrap();
        std::fs::create_dir_all(pristine.path().join("nested")).unwrap();
        std::fs::write(pristine.path().join("nested/pyproject.toml"), "[project]\n").unwrap();
        std::fs::write(pristine.path().join("conftest.py"), "# fixture-owned\n").unwrap();
        // The post-run tree carries a weakened manifest and planted config.
        std::fs::write(
            verify.path().join("Cargo.toml"),
            "[package]\nautotests = false\n",
        )
        .unwrap();
        std::fs::create_dir_all(verify.path().join("nested")).unwrap();
        std::fs::write(
            verify.path().join("nested/pyproject.toml"),
            "[tool.pytest]\n",
        )
        .unwrap();
        std::fs::write(verify.path().join("conftest.py"), "# fixture-owned\n").unwrap();
        std::fs::write(
            verify.path().join("pytest.ini"),
            "[pytest]\naddopts = --no-header\n",
        )
        .unwrap();
        std::fs::write(
            verify.path().join("nested/conftest.py"),
            "collect_ignore = ['x']\n",
        )
        .unwrap();

        restore_pristine_manifests(pristine.path(), verify.path()).unwrap();
        purge_agent_test_config(pristine.path(), hidden.path(), verify.path()).unwrap();

        let manifest = std::fs::read_to_string(verify.path().join("Cargo.toml")).unwrap();
        assert_eq!(manifest, "[package]\n");
        let nested = std::fs::read_to_string(verify.path().join("nested/pyproject.toml")).unwrap();
        assert_eq!(nested, "[project]\n");
        assert!(verify.path().join("conftest.py").is_file());
        assert!(!verify.path().join("pytest.ini").exists());
        assert!(!verify.path().join("nested/conftest.py").exists());
    }
}
