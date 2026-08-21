//! Governed verifier sandbox: compiler/test/lint processes run inside an
//! OS process sandbox with a deny-network profile and a read allow-list
//! (toolchain roots, no credentials). Extracted from the candidate so the
//! sandbox policy has one home.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use std::time::Duration;

use anyhow::{Context, Result};
use perspt_coding::LanguageId;
use tokio::process::Command;

/// Outcome of one governed verifier run.
pub(crate) struct VerifierExecution {
    pub(crate) success: bool,
    pub(crate) output: String,
}

/// Per-stage wall-clock limits for governed verifier processes
/// (`[verification] stage_timeout_secs` plus per-stage overrides).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifierTimeouts {
    pub default_secs: u64,
    pub syntax: Option<u64>,
    pub build: Option<u64>,
    pub test: Option<u64>,
    pub lint: Option<u64>,
    pub format: Option<u64>,
}

impl Default for VerifierTimeouts {
    fn default() -> Self {
        Self {
            default_secs: 180,
            syntax: None,
            build: None,
            test: None,
            lint: None,
            format: None,
        }
    }
}

impl VerifierTimeouts {
    /// The limit for one stage; `None` means an interactive or untyped run.
    pub fn for_stage(&self, stage: Option<perspt_core::plugin::VerifierStage>) -> Duration {
        use perspt_core::plugin::VerifierStage;
        let secs = match stage {
            Some(VerifierStage::SyntaxCheck) => self.syntax,
            Some(VerifierStage::Build) => self.build,
            Some(VerifierStage::Test) => self.test,
            Some(VerifierStage::Lint) => self.lint,
            Some(VerifierStage::Format) => self.format,
            None => None,
        };
        Duration::from_secs(secs.unwrap_or(self.default_secs).max(1))
    }
}

pub(crate) struct VerifierJob {
    pub(crate) plugin: String,
    pub(crate) adapter_id: LanguageId,
    pub(crate) stage: perspt_core::plugin::VerifierStage,
    pub(crate) command: String,
    pub(crate) root: PathBuf,
}

pub(crate) async fn run_governed_verifier(
    root: PathBuf,
    command: String,
    allow_unisolated: bool,
    target_suffix: String,
    extra_env: Vec<(String, String)>,
    timeout: Duration,
) -> Result<VerifierExecution> {
    let tmp = root.join(".perspt-tmp").join(&target_suffix);
    let target = root.join(".perspt-target").join(&target_suffix);
    std::fs::create_dir_all(&tmp)?;
    std::fs::create_dir_all(&target)?;
    let read_roots = verifier_read_roots(&root, &extra_env);
    let mut process = if allow_unisolated {
        host_shell(&root, &command)
    } else {
        isolated_command(&root, &command, &read_roots)?
    };
    // Toolchain caches must live inside the writable overlay: the sandbox
    // denies network and writes outside the candidate, so `uv` gets a
    // per-run cache dir (mirroring CARGO_TARGET_DIR) and stays offline —
    // verifiers run against the project's already-synced environment.
    let uv_cache = root.join(".perspt-tmp").join("uv-cache");
    let isolated_home = root.join(".perspt-home");
    std::fs::create_dir_all(&uv_cache)?;
    std::fs::create_dir_all(&isolated_home)?;
    let original_home = std::env::var_os("HOME").map(PathBuf::from);
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| original_home.as_ref().map(|home| home.join(".cargo")));
    let rustup_home = std::env::var_os("RUSTUP_HOME")
        .map(PathBuf::from)
        .or_else(|| original_home.as_ref().map(|home| home.join(".rustup")));
    process
        .env_clear()
        .envs(verifier_environment())
        .env("HOME", &isolated_home)
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_TARGET_DIR", target)
        .env("UV_CACHE_DIR", uv_cache)
        .env("UV_OFFLINE", "1")
        .env("TMPDIR", tmp)
        .env("TEMP", root.join(".perspt-tmp").join(&target_suffix))
        .env("TMP", root.join(".perspt-tmp").join(&target_suffix))
        .envs(extra_env)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(path) = cargo_home.filter(|path| path.is_dir()) {
        process.env("CARGO_HOME", path);
    }
    if let Some(path) = rustup_home.filter(|path| path.is_dir()) {
        process.env("RUSTUP_HOME", path);
    }
    let output = tokio::time::timeout(timeout, process.output())
        .await
        .with_context(|| {
            format!(
                "governed verifier exceeded its {} second limit \
                 ([verification] stage_timeout_secs raises it)",
                timeout.as_secs()
            )
        })??;
    Ok(VerifierExecution {
        success: output.status.success(),
        output: format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    })
}

fn verifier_environment() -> Vec<(String, String)> {
    let portable = [
        "PATH",
        "LANG",
        "LC_ALL",
        "SystemRoot",
        "SYSTEMROOT",
        "WINDIR",
        "COMSPEC",
        "PATHEXT",
    ];
    portable
        .into_iter()
        .chain(native_toolchain_environment())
        .filter_map(|key| std::env::var(key).ok().map(|value| (key.into(), value)))
        .collect()
}

#[cfg(windows)]
fn native_toolchain_environment() -> impl Iterator<Item = &'static str> {
    // Rust's MSVC linker discovery needs the installation roots when a process
    // starts from a deliberately cleared environment. In particular,
    // find-msvc-tools consults ProgramFiles(x86); without it rustc may resolve
    // Git for Windows' unrelated Unix `link.exe` from PATH. Keep this an
    // explicit compiler/SDK allowlist rather than inheriting the host
    // environment wholesale.
    [
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramW6432",
        "VCINSTALLDIR",
        "VSINSTALLDIR",
        "VCToolsInstallDir",
        "VCToolsVersion",
        "VSCMD_ARG_HOST_ARCH",
        "VSCMD_ARG_TGT_ARCH",
        "WindowsSdkDir",
        "WindowsSDKVersion",
        "UniversalCRTSdkDir",
        "UCRTVersion",
        "INCLUDE",
        "LIB",
        "LIBPATH",
    ]
    .into_iter()
}

#[cfg(not(windows))]
fn native_toolchain_environment() -> impl Iterator<Item = &'static str> {
    std::iter::empty()
}

#[cfg(windows)]
fn host_shell(root: &Path, command: &str) -> Command {
    let mut process = Command::new("cmd.exe");
    process
        .args(["/D", "/S", "/C"])
        .arg(command)
        .current_dir(root);
    process
}

#[cfg(not(windows))]
fn host_shell(root: &Path, command: &str) -> Command {
    let mut process = Command::new("/bin/sh");
    process.arg("-c").arg(command).current_dir(root);
    process
}

#[cfg(target_os = "macos")]
fn isolated_command(root: &Path, command: &str, read_roots: &[PathBuf]) -> Result<Command> {
    let profile = macos_sandbox_profile(root, read_roots);
    let mut process = Command::new("/usr/bin/sandbox-exec");
    process
        .arg("-p")
        .arg(profile)
        .arg("/bin/sh")
        .arg("-c")
        .arg(command)
        .current_dir(root);
    Ok(process)
}

#[cfg(target_os = "macos")]
fn macos_sandbox_profile(root: &Path, read_roots: &[PathBuf]) -> String {
    let escaped = root.to_string_lossy().replace('"', "\\\"");
    let mut readable = read_roots.to_vec();
    readable.push(root.to_path_buf());
    let users_deny = deny_data_except("/Users", &readable);
    let home_deny = deny_data_except("/home", &readable);
    let root_home_deny = deny_data_except("/root", &readable);
    let private_tmp_deny = deny_data_except("/private/tmp", &readable);
    let user_tmp_deny = deny_data_except("/private/var/folders", &readable);
    let volumes_deny = deny_data_except("/Volumes", &readable);
    let network_deny = deny_data_except("/Network", &readable);
    let extra_reads = read_roots
        .iter()
        .map(|path| {
            format!(
                "(allow file-read* (subpath \"{}\"))",
                path.to_string_lossy()
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "(version 1)\n\
             (deny default)\n\
             (allow process*)\n\
             (allow sysctl-read)\n\
             (allow file-read*)\n\
             {users_deny}\n\
             {home_deny}\n\
             {root_home_deny}\n\
             {private_tmp_deny}\n\
             {user_tmp_deny}\n\
             {volumes_deny}\n\
             {network_deny}\n\
             (allow file-read* (subpath \"{escaped}\"))\n\
             {extra_reads}\n\
             (allow file-write* (literal \"/dev/null\"))\n\
             (allow file-write* (subpath \"{escaped}\"))\n\
             (deny network*)"
    )
}

#[cfg(target_os = "macos")]
fn deny_data_except(parent: &str, allowed: &[PathBuf]) -> String {
    let exceptions = allowed
        .iter()
        .filter(|path| path.starts_with(parent))
        .map(|path| {
            let escaped = path
                .to_string_lossy()
                .replace('\\', "\\\\")
                .replace('"', "\\\"");
            format!("(require-not (subpath \"{escaped}\"))")
        })
        .collect::<Vec<_>>();
    if exceptions.is_empty() {
        format!("(deny file-read-data (subpath \"{parent}\"))")
    } else {
        format!(
            "(deny file-read-data (require-all (subpath \"{parent}\") {}))",
            exceptions.join(" ")
        )
    }
}

#[cfg(target_os = "linux")]
fn isolated_command(root: &Path, command: &str, read_roots: &[PathBuf]) -> Result<Command> {
    let bwrap = ["/usr/bin/bwrap", "/bin/bwrap"]
        .into_iter()
        .find(|path| Path::new(path).is_file())
        .context("bubblewrap is required for governed verifier execution")?;
    let mut process = Command::new(bwrap);
    process.args([
        "--die-with-parent",
        "--unshare-net",
        "--ro-bind",
        "/",
        "/",
        "--dev",
        "/dev",
    ]);
    for sensitive in ["/home", "/root", "/Users"] {
        if Path::new(sensitive).exists() {
            process.arg("--tmpfs").arg(sensitive);
        }
    }
    process.arg("--tmpfs").arg("/tmp");
    for read_root in read_roots {
        if !read_root.starts_with(root) {
            prepare_bwrap_destination(&mut process, read_root);
            process.arg("--ro-bind").arg(read_root).arg(read_root);
        }
    }
    prepare_bwrap_destination(&mut process, root);
    process
        .arg("--bind")
        .arg(root)
        .arg(root)
        .arg("--chdir")
        .arg(root)
        .arg("/bin/sh")
        .arg("-c")
        .arg(command);
    Ok(process)
}

#[cfg(target_os = "linux")]
fn prepare_bwrap_destination(process: &mut Command, destination: &Path) {
    for directory in bwrap_destination_parents(destination) {
        process.arg("--dir").arg(directory);
    }
}

#[cfg(any(test, target_os = "linux"))]
fn bwrap_destination_parents(destination: &Path) -> Vec<PathBuf> {
    let Some(parent) = destination.parent() else {
        return Vec::new();
    };
    let masked_roots = [
        Path::new("/home"),
        Path::new("/root"),
        Path::new("/Users"),
        Path::new("/tmp"),
    ];
    let Some(masked) = masked_roots
        .into_iter()
        .find(|masked| parent.starts_with(masked))
    else {
        return Vec::new();
    };
    let mut missing_parents: Vec<_> = parent
        .ancestors()
        .take_while(|ancestor| *ancestor != masked)
        .map(Path::to_path_buf)
        .collect();
    missing_parents.reverse();
    missing_parents
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn isolated_command(_root: &Path, _command: &str, _read_roots: &[PathBuf]) -> Result<Command> {
    anyhow::bail!("this platform has no registered governed process sandbox")
}

fn verifier_read_roots(root: &Path, extra_env: &[(String, String)]) -> Vec<PathBuf> {
    let mut roots = BTreeSet::new();
    for (_, value) in extra_env {
        let path = PathBuf::from(value);
        if path.exists() {
            roots.insert(path.canonicalize().unwrap_or_else(|_| path.clone()));
            for interpreter in [
                path.join("bin/python"),
                path.join("bin/python3"),
                path.join("Scripts/python.exe"),
            ] {
                if let Ok(target) = interpreter.canonicalize() {
                    if let Some(installation) = target.parent().and_then(Path::parent) {
                        roots.insert(installation.to_path_buf());
                    }
                }
            }
        }
    }
    if let Ok(value) = std::env::var("CARGO_HOME") {
        extend_cargo_read_roots(&mut roots, &PathBuf::from(value));
    }
    if let Ok(value) = std::env::var("RUSTUP_HOME") {
        let path = PathBuf::from(value);
        if path.exists() {
            roots.insert(path.canonicalize().unwrap_or(path));
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        extend_cargo_read_roots(&mut roots, &home.join(".cargo"));
        for relative in [".rustup", ".local/bin"] {
            let path = home.join(relative);
            if path.exists() {
                roots.insert(path.canonicalize().unwrap_or(path));
            }
        }
    }
    if let Ok(path) = std::env::var("PATH") {
        for directory in std::env::split_paths(&path) {
            if directory.exists()
                && !directory.starts_with("/usr")
                && !directory.starts_with("/bin")
            {
                roots.insert(directory.canonicalize().unwrap_or(directory));
            }
        }
    }
    roots.remove(root);
    roots.into_iter().collect()
}

fn extend_cargo_read_roots(roots: &mut BTreeSet<PathBuf>, cargo_home: &Path) {
    // Cargo credentials are intentionally absent. Offline verification needs
    // only executables, configuration, and already-fetched source/index data.
    for relative in ["bin", "config", "config.toml", "git", "registry"] {
        let path = cargo_home.join(relative);
        if path.exists() {
            roots.insert(path.canonicalize().unwrap_or(path));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_profile_only_allows_candidate_and_null_device_writes() {
        let profile = macos_sandbox_profile(
            Path::new("/private/tmp/candidate"),
            &[PathBuf::from("/Users/test/.rustup")],
        );
        assert!(profile.contains("(allow file-write* (literal \"/dev/null\"))"));
        assert!(profile.contains("(allow file-write* (subpath \"/private/tmp/candidate\"))"));
        assert_eq!(profile.matches("allow file-write*").count(), 2);
        assert!(profile
            .lines()
            .any(|line| line.trim() == "(allow file-read*)"));
        assert!(profile.contains("(deny file-read-data (require-all (subpath \"/Users\")"));
        assert!(profile.contains("(deny file-read-data (require-all (subpath \"/private/tmp\")"));
        assert!(profile.contains("/Users/test/.rustup"));
        assert!(profile.contains("(deny network*)"));
    }

    #[cfg(unix)]
    #[test]
    fn verifier_roots_follow_virtualenv_interpreter_symlinks() {
        let root = tempfile::tempdir().unwrap();
        let install = root.path().join("uv-python");
        let venv = root.path().join("project/.venv");
        std::fs::create_dir_all(install.join("bin")).unwrap();
        std::fs::create_dir_all(venv.join("bin")).unwrap();
        std::fs::write(install.join("bin/python3"), "binary").unwrap();
        std::os::unix::fs::symlink(install.join("bin/python3"), venv.join("bin/python3")).unwrap();

        let roots = verifier_read_roots(
            root.path(),
            &[("UV_PROJECT_ENVIRONMENT".into(), venv.display().to_string())],
        );
        assert!(roots.contains(&install.canonicalize().unwrap()));
    }

    #[test]
    fn cargo_verifier_roots_exclude_credentials() {
        let root = tempfile::tempdir().unwrap();
        let cargo_home = root.path().join("cargo-home");
        std::fs::create_dir_all(cargo_home.join("bin")).unwrap();
        std::fs::create_dir_all(cargo_home.join("registry")).unwrap();
        std::fs::write(cargo_home.join("credentials.toml"), "[registry]").unwrap();

        let mut roots = BTreeSet::new();
        extend_cargo_read_roots(&mut roots, &cargo_home);
        assert!(roots.contains(&cargo_home.join("bin").canonicalize().unwrap()));
        assert!(roots.contains(&cargo_home.join("registry").canonicalize().unwrap()));
        assert!(!roots.contains(&cargo_home));
        assert!(!roots.contains(&cargo_home.join("credentials.toml")));
    }

    #[test]
    fn bubblewrap_rebinds_recreate_only_masked_destination_parents() {
        assert_eq!(
            bwrap_destination_parents(Path::new("/home/user/.cargo/registry")),
            [
                PathBuf::from("/home/user"),
                PathBuf::from("/home/user/.cargo")
            ]
        );
        assert!(bwrap_destination_parents(Path::new("/usr/bin/cargo")).is_empty());
    }
}
