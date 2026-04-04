//! Build script for perspt-dashboard.
//!
//! Compiles Tailwind CSS + DaisyUI from `input.css` into `static/dashboard.css`
//! using `npx @tailwindcss/cli`. Falls back gracefully with a human-readable
//! message when Node.js / npm / npx are not available.

use std::path::Path;
use std::process::Command;

fn main() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let crate_dir = Path::new(&crate_dir);

    // Tell Cargo when to re-run this script
    println!("cargo:rerun-if-changed=input.css");
    println!("cargo:rerun-if-changed=package.json");
    println!("cargo:rerun-if-changed=templates/");

    // ── 1. Check npx is available ────────────────────────────────────────
    let npx = if cfg!(target_os = "windows") {
        "npx.cmd"
    } else {
        "npx"
    };
    match Command::new(npx).arg("--version").output() {
        Ok(o) if o.status.success() => {}
        _ => {
            println!(
                "cargo:warning=\n\
                 ╔══════════════════════════════════════════════════════════════╗\n\
                 ║  npx not found — Tailwind CSS will NOT be compiled.        ║\n\
                 ║                                                            ║\n\
                 ║  The dashboard will render without styles.                 ║\n\
                 ║  To fix: install Node.js ≥ 18 (https://nodejs.org)        ║\n\
                 ║  then re-run `cargo build`.                               ║\n\
                 ╚══════════════════════════════════════════════════════════════╝"
            );
            return;
        }
    }

    // ── 2. npm install (only when node_modules is missing) ───────────────
    let node_modules = crate_dir.join("node_modules");
    if !node_modules.exists() {
        let npm = if cfg!(target_os = "windows") {
            "npm.cmd"
        } else {
            "npm"
        };
        println!("cargo:warning=Installing dashboard CSS dependencies (npm install)…");
        match Command::new(npm)
            .arg("install")
            .current_dir(crate_dir)
            .output()
        {
            Ok(o) if o.status.success() => {}
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                println!(
                    "cargo:warning=npm install failed (exit {}):\n{}",
                    o.status,
                    first_lines(&stderr, 5)
                );
                println!("cargo:warning=Dashboard will use the fallback stylesheet.");
                return;
            }
            Err(e) => {
                println!("cargo:warning=Could not run npm install: {e}");
                println!("cargo:warning=Dashboard will use the fallback stylesheet.");
                return;
            }
        }
    }

    // ── 3. Compile Tailwind CSS ──────────────────────────────────────────
    match Command::new(npx)
        .args([
            "@tailwindcss/cli",
            "-i",
            "input.css",
            "-o",
            "static/dashboard.css",
            "--minify",
        ])
        .current_dir(crate_dir)
        .output()
    {
        Ok(o) if o.status.success() => {
            // Report the size so developers can eyeball it
            if let Ok(meta) = std::fs::metadata(crate_dir.join("static/dashboard.css")) {
                let kb = meta.len() / 1024;
                println!("cargo:warning=Tailwind CSS compiled → static/dashboard.css ({kb} KB)");
            }
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            println!(
                "cargo:warning=Tailwind CSS compilation failed (exit {}):\n{}",
                o.status,
                first_lines(&stderr, 8)
            );
            println!("cargo:warning=Dashboard will use the fallback stylesheet.");
        }
        Err(e) => {
            println!("cargo:warning=Could not run npx @tailwindcss/cli: {e}");
            println!("cargo:warning=Dashboard will use the fallback stylesheet.");
        }
    }
}

/// Return at most `n` lines from a string, for compact cargo warnings.
fn first_lines(s: &str, n: usize) -> String {
    s.lines().take(n).collect::<Vec<_>>().join("\n")
}
