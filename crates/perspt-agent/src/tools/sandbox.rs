use super::*;

// =============================================================================
// PSP-5 Phase 6: Sandbox workspace helpers
// =============================================================================

/// Create a sandbox workspace for provisional verification.
///
/// Copies key project files into a session-scoped temporary directory so
/// speculative verification does not pollute committed workspace state.
/// Returns the path to the sandbox root.
pub fn create_sandbox(
    working_dir: &Path,
    session_id: &str,
    branch_id: &str,
) -> std::io::Result<PathBuf> {
    let sandbox_root = working_dir
        .join(".perspt")
        .join("sandboxes")
        .join(session_id)
        .join(branch_id);

    fs::create_dir_all(&sandbox_root)?;

    log::debug!("Created sandbox workspace at {}", sandbox_root.display());

    Ok(sandbox_root)
}

/// Seed a sandbox with plugin-identified project manifests (Cargo.toml,
/// pyproject.toml, etc.) so that build/test commands can find them.
///
/// Walks the workspace looking for each plugin's `key_files()` and copies
/// any that exist into the sandbox at the same relative path.
pub fn seed_sandbox_manifests(
    working_dir: &Path,
    sandbox_dir: &Path,
    plugins: &[&str],
) -> std::io::Result<()> {
    let registry = perspt_core::plugin::PluginRegistry::new();
    let mut seeded = Vec::new();

    for plugin_name in plugins {
        if let Some(plugin) = registry.get(plugin_name) {
            for key_file in plugin.key_files() {
                // Check workspace root
                if working_dir.join(key_file).exists() {
                    copy_to_sandbox(working_dir, sandbox_dir, key_file)?;
                    seeded.push(key_file.to_string());
                }
                // Also walk up to two levels of subdirectories
                // (e.g. crates/*/Cargo.toml, packages/*/package.json)
                if let Ok(entries) = fs::read_dir(working_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() && path.file_name().is_none_or(|n| n != ".perspt") {
                            // Level 1: e.g. crates/Cargo.toml (unlikely but check)
                            let sub_key = path.join(key_file);
                            if sub_key.exists() {
                                let rel = sub_key
                                    .strip_prefix(working_dir)
                                    .unwrap_or(&sub_key)
                                    .to_string_lossy()
                                    .to_string();
                                let _ = copy_to_sandbox(working_dir, sandbox_dir, &rel);
                                seeded.push(rel);
                            }
                            // Level 2: e.g. crates/cfd-core/Cargo.toml
                            if let Ok(sub_entries) = fs::read_dir(&path) {
                                for sub_entry in sub_entries.flatten() {
                                    let sub_path = sub_entry.path();
                                    if sub_path.is_dir() {
                                        let deep_key = sub_path.join(key_file);
                                        if deep_key.exists() {
                                            let rel = deep_key
                                                .strip_prefix(working_dir)
                                                .unwrap_or(&deep_key)
                                                .to_string_lossy()
                                                .to_string();
                                            let _ = copy_to_sandbox(working_dir, sandbox_dir, &rel);
                                            seeded.push(rel);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if !seeded.is_empty() {
        log::debug!("Seeded sandbox with manifests: {}", seeded.join(", "));
    }

    // For Rust workspaces: ensure every workspace member in the sandbox has
    // at minimum a valid Cargo.toml + source target, so commands like
    // `cargo add -p <crate>` can resolve the workspace graph.
    if plugins.contains(&"rust") {
        ensure_rust_workspace_members_in_sandbox(working_dir, sandbox_dir);
    }

    // For Python projects: symlink .venv and seed src/<pkg>/ so uv run
    // commands work immediately in the sandbox.
    if plugins.contains(&"python") {
        seed_python_sandbox(working_dir, sandbox_dir);
    }

    Ok(())
}

/// Ensure all Cargo workspace members in a sandbox have valid Cargo.toml +
/// source target stubs.  Without this, `cargo add -p X` (or any cargo
/// command) fails with "failed to load manifest for workspace member Y"
/// because the sandbox only gets the current node's files but the root
/// Cargo.toml references ALL members.
fn ensure_rust_workspace_members_in_sandbox(working_dir: &Path, sandbox_dir: &Path) {
    let cargo_toml = sandbox_dir.join("Cargo.toml");
    let content = match fs::read_to_string(&cargo_toml) {
        Ok(c) => c,
        Err(_) => return,
    };
    let members = parse_workspace_members(&content);

    for member in &members {
        let member_dir = sandbox_dir.join(member);
        let member_cargo = member_dir.join("Cargo.toml");

        // Try to copy from main workspace first (preserves any real content)
        let src_cargo = working_dir.join(member).join("Cargo.toml");
        if src_cargo.exists() && !member_cargo.exists() {
            let _ = fs::create_dir_all(&member_dir);
            let _ = fs::copy(&src_cargo, &member_cargo);
        }

        // Create a stub Cargo.toml if still missing
        if !member_cargo.exists() {
            let _ = fs::create_dir_all(&member_dir);
            let name = member.rsplit('/').next().unwrap_or(member);
            let stub = format!(
                "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
                name
            );
            let _ = fs::write(&member_cargo, &stub);
        }

        // Ensure at least one source target exists (src/lib.rs or src/main.rs)
        let src_dir = member_dir.join("src");
        let has_lib = src_dir.join("lib.rs").exists();
        let has_main = src_dir.join("main.rs").exists();
        if !has_lib && !has_main {
            let _ = fs::create_dir_all(&src_dir);
            // Try copying from main workspace
            let ws_lib = working_dir.join(member).join("src").join("lib.rs");
            let ws_main = working_dir.join(member).join("src").join("main.rs");
            if ws_lib.exists() {
                let _ = fs::copy(&ws_lib, src_dir.join("lib.rs"));
            } else if ws_main.exists() {
                let _ = fs::copy(&ws_main, src_dir.join("main.rs"));
            } else {
                // Create minimal stub so cargo doesn't complain about missing targets
                let _ = fs::write(
                    src_dir.join("lib.rs"),
                    "// stub — will be replaced by agent\n",
                );
            }
        }
    }

    if !members.is_empty() {
        log::debug!(
            "Ensured {} workspace member(s) have valid stubs in sandbox",
            members.len()
        );
    }
}

/// Quick parse of a root Cargo.toml's `[workspace] members` array.
fn parse_workspace_members(content: &str) -> Vec<String> {
    let mut in_workspace = false;
    let mut members: Vec<String> = Vec::new();
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') {
            in_workspace = line == "[workspace]";
            continue;
        }
        if in_workspace && line.starts_with("members") {
            if let Some((_, value)) = line.split_once('=') {
                let raw = value.trim();
                if raw.starts_with('[') {
                    let inner = raw.trim_start_matches('[').trim_end_matches(']');
                    for item in inner.split(',') {
                        let member = item.trim().trim_matches('"').trim_matches('\'');
                        if !member.is_empty() {
                            members.push(member.to_string());
                        }
                    }
                }
            }
        }
    }
    members
}

/// Seed a Python project sandbox with the workspace `.venv/` (via symlink)
/// and the `src/<pkg>/` package directory tree so that `uv run` commands
/// work immediately without a full re-sync.
fn seed_python_sandbox(working_dir: &Path, sandbox_dir: &Path) {
    // Symlink .venv/ so uv run reuses the workspace venv instead of
    // recreating one per sandbox (saves ~2-3s per node).
    let workspace_venv = working_dir.join(".venv");
    let sandbox_venv = sandbox_dir.join(".venv");
    if workspace_venv.is_dir() && !sandbox_venv.exists() {
        #[cfg(unix)]
        {
            if let Err(e) = std::os::unix::fs::symlink(&workspace_venv, &sandbox_venv) {
                log::debug!("Could not symlink .venv into sandbox: {}", e);
            } else {
                log::debug!("Symlinked .venv into sandbox");
            }
        }
        #[cfg(not(unix))]
        {
            // On Windows, symlinks require elevated privileges; skip the
            // optimisation — uv will auto-create a venv when needed.
            log::debug!("Skipping .venv symlink on non-Unix platform");
        }
    }

    // Seed ancillary files that `uv add` / `uv sync` need when building
    // the project inside the sandbox.  In particular, `uv init` generates
    // `readme = "README.md"` in pyproject.toml, so the sandbox build fails
    // with "failed to open file README.md" if we don't copy it.
    for ancillary in &["README.md", "README.rst", "README", ".python-version"] {
        let src = working_dir.join(ancillary);
        if src.is_file() {
            let dst = sandbox_dir.join(ancillary);
            if !dst.exists() {
                let _ = fs::copy(&src, &dst);
            }
        }
    }

    // Copy the src/<pkg>/ directory tree so imports resolve.  We walk one
    // level under src/ looking for Python packages (__init__.py present).
    let workspace_src = working_dir.join("src");
    if workspace_src.is_dir() {
        if let Ok(entries) = fs::read_dir(&workspace_src) {
            for entry in entries.flatten() {
                let pkg_dir = entry.path();
                if pkg_dir.is_dir() && pkg_dir.join("__init__.py").exists() {
                    // Recursively copy all .py files from this package
                    if let Err(e) = copy_dir_to_sandbox(working_dir, sandbox_dir, &pkg_dir) {
                        log::debug!(
                            "Could not seed src/{} into sandbox: {}",
                            entry.file_name().to_string_lossy(),
                            e
                        );
                    }
                }
            }
        }
    }

    // Also copy conftest.py / tests/ directory if present (needed for pytest)
    for extra in &["conftest.py", "tests"] {
        let src = working_dir.join(extra);
        if src.is_file() {
            let rel = extra.to_string();
            let _ = copy_to_sandbox(working_dir, sandbox_dir, &rel);
        } else if src.is_dir() {
            let _ = copy_dir_to_sandbox(working_dir, sandbox_dir, &src);
        }
    }
}

/// Recursively copy a directory from workspace into sandbox, preserving
/// relative paths.  Skips `.venv`, `__pycache__`, and bytecode files.
fn copy_dir_to_sandbox(
    working_dir: &Path,
    sandbox_dir: &Path,
    src_dir: &Path,
) -> std::io::Result<()> {
    const SKIP: &[&str] = &[".venv", "__pycache__", ".mypy_cache", ".pytest_cache"];
    for entry in fs::read_dir(src_dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if path.is_dir() {
            if SKIP.iter().any(|s| *s == &*name_str) {
                continue;
            }
            copy_dir_to_sandbox(working_dir, sandbox_dir, &path)?;
        } else if !name_str.ends_with(".pyc") {
            if let Ok(rel) = path.strip_prefix(working_dir) {
                let rel_str = rel.to_string_lossy().to_string();
                copy_to_sandbox(working_dir, sandbox_dir, &rel_str)?;
            }
        }
    }
    Ok(())
}

/// Clean up a specific sandbox workspace.
pub fn cleanup_sandbox(sandbox_dir: &Path) -> std::io::Result<()> {
    if sandbox_dir.exists() {
        fs::remove_dir_all(sandbox_dir)?;
        log::debug!("Cleaned up sandbox at {}", sandbox_dir.display());
    }
    Ok(())
}

/// Clean up all sandbox workspaces for a session.
pub fn cleanup_session_sandboxes(working_dir: &Path, session_id: &str) -> std::io::Result<()> {
    let session_sandbox = working_dir
        .join(".perspt")
        .join("sandboxes")
        .join(session_id);

    if session_sandbox.exists() {
        fs::remove_dir_all(&session_sandbox)?;
        log::debug!("Cleaned up all sandboxes for session {}", session_id);
    }
    Ok(())
}

/// Copy a file from the workspace into a sandbox, preserving relative paths.
pub fn copy_to_sandbox(
    working_dir: &Path,
    sandbox_dir: &Path,
    relative_path: &str,
) -> std::io::Result<()> {
    let src = working_dir.join(relative_path);
    let dst = sandbox_dir.join(relative_path);

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }

    if src.exists() {
        fs::copy(&src, &dst)?;
    }
    Ok(())
}

/// Copy a file from a sandbox back to the live workspace, preserving relative paths.
pub fn copy_from_sandbox(
    sandbox_dir: &Path,
    working_dir: &Path,
    relative_path: &str,
) -> std::io::Result<()> {
    let src = sandbox_dir.join(relative_path);
    let dst = working_dir.join(relative_path);

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }

    if src.exists() {
        fs::copy(&src, &dst)?;
    }
    Ok(())
}

/// List all files in a sandbox directory as workspace-relative paths.
pub fn list_sandbox_files(sandbox_dir: &Path) -> std::io::Result<Vec<String>> {
    let mut files = Vec::new();
    if !sandbox_dir.exists() {
        return Ok(files);
    }
    /// Directories that should never be exported from sandbox back to
    /// workspace — virtual-environments, bytecode caches, build artifacts.
    const SKIP_DIRS: &[&str] = &[
        ".venv",
        "__pycache__",
        "node_modules",
        ".mypy_cache",
        ".pytest_cache",
        ".ruff_cache",
    ];
    fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) -> std::io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if SKIP_DIRS.iter().any(|s| *s == &*name_str) {
                    continue;
                }
                walk(&path, base, out)?;
            } else if let Ok(rel) = path.strip_prefix(base) {
                let normalized = rel
                    .components()
                    .map(|component| component.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                // Skip bytecode / lock artifacts that shouldn't transfer
                if !normalized.ends_with(".pyc") {
                    out.push(normalized);
                }
            }
        }
        Ok(())
    }
    walk(sandbox_dir, sandbox_dir, &mut files)?;
    Ok(files)
}
