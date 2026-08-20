//! Stamp the runtime build identity: `PERSPT_GIT_SHA` feeds the no-good
//! key's `b` component (Gate AB), so rebuilt runtime code never shares a
//! key with an earlier build. An externally supplied `PERSPT_GIT_SHA`
//! (CI, release packaging) wins; without git (crates.io source builds)
//! the `option_env!` fallback stays `dev`.

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=PERSPT_GIT_SHA");
    if std::env::var_os("PERSPT_GIT_SHA").is_some() {
        return;
    }
    let Some(head) = git(&["rev-parse", "--git-path", "HEAD"]) else {
        return;
    };
    println!("cargo:rerun-if-changed={head}");
    if let Some(sha) = git(&["rev-parse", "HEAD"]) {
        println!("cargo:rustc-env=PERSPT_GIT_SHA={sha}");
    }
}

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!value.is_empty()).then_some(value)
}
