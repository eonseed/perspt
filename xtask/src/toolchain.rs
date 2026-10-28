//! External tool resolution for documentation and asset pipelines.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Result};

/// Locate an executable on `PATH` (or as a direct path) and verify `--version` succeeds.
pub(crate) fn require(program: &str, purpose: &str, hint: &str) -> Result<PathBuf> {
    let search = std::env::var_os("PATH").unwrap_or_default();
    let Some(path) = resolve(program, &search) else {
        bail!("{}", missing(program, purpose, hint));
    };
    let Ok(version) = Command::new(&path).arg("--version").output() else {
        bail!("{}", missing(program, purpose, hint));
    };
    if !version.status.success() {
        bail!("{}", missing(program, purpose, hint));
    }
    Ok(path)
}

pub(crate) fn resolve(program: &str, search: &OsStr) -> Option<PathBuf> {
    let path = Path::new(program);
    if path.components().count() > 1 {
        return path.is_file().then(|| path.to_path_buf());
    }
    let executable = format!("{program}{}", std::env::consts::EXE_SUFFIX);
    std::env::split_paths(search)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(&executable))
        .find(|candidate| candidate.is_file())
}

fn missing(program: &str, purpose: &str, hint: &str) -> String {
    format!(
        "-- MISSING PUBLICATION TOOL [{program}.missing] --------------------\n\n\
         Could not execute `{program}`.\n\n{purpose}\n\nHint: {hint}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_tool_reports_structured_hint() {
        assert!(resolve("typst", OsStr::new("")).is_none());
        let msg = missing(
            "typst",
            "Typst renders SVG diagrams and PDF books.",
            "Install https://typst.app/ and verify `typst --version`.",
        );
        assert!(msg.contains("[typst.missing]"));
        assert!(msg.contains("Hint:"));
    }
}
