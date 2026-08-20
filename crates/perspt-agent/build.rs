//! Stamp the runtime build identity: `PERSPT_GIT_SHA` feeds the no-good
//! key's `b` component (Gate AB), so rebuilt runtime code never shares a
//! key with an earlier build. An externally supplied `PERSPT_GIT_SHA`
//! (CI, release packaging) wins; without git (crates.io source builds)
//! the `option_env!` fallback stays `dev`.
//!
//! Rerun triggers must cover how a commit actually lands: on a branch,
//! `.git/HEAD` holds a stable symbolic ref and a new commit updates the
//! referenced loose ref file (or `packed-refs`) instead — watching HEAD
//! alone would let a later build keep the previous SHA.

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=PERSPT_GIT_SHA");
    if std::env::var_os("PERSPT_GIT_SHA").is_some() {
        return;
    }
    let Some(head) = git(&["rev-parse", "--git-path", "HEAD"]) else {
        return;
    };
    // HEAD itself (branch switches, detached checkouts) ...
    println!("cargo:rerun-if-changed={head}");
    // ... the loose ref file HEAD resolves to (ordinary commits) ...
    if let Some(ref_name) = git(&["symbolic-ref", "-q", "HEAD"]) {
        if let Some(ref_path) = git(&["rev-parse", "--git-path", &ref_name]) {
            println!("cargo:rerun-if-changed={ref_path}");
        }
    }
    // ... and packed-refs (gc packs the loose ref away).
    if let Some(packed) = git(&["rev-parse", "--git-path", "packed-refs"]) {
        println!("cargo:rerun-if-changed={packed}");
    }
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
