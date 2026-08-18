//! Compile the platform prompt section library (PSP-10 system 23).
//!
//! Sections live as reviewed markdown under `prompts/`; a malformed edit
//! fails this build with an error naming the file — never a session. The
//! generated typed structs land in `OUT_DIR` and are included by
//! `src/prompts.rs`. A committed `manifest.toml`, generated explicitly by
//! `perspt prompts manifest`, is validated here; a normal build never
//! edits the source tree.

use perspt_prompt_macros::{compile_prompt_dir, validate_manifest, StageDecl};

fn main() {
    println!("cargo:rerun-if-changed=prompts");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("prompts");
    // Migrated single-line literals join with " "; the separator is a
    // compilation input (PSP-10 resolved decision 6: byte-identity first).
    let stages = [
        StageDecl::new("session_bootstrap", " "),
        StageDecl::new("graph_plan", " "),
        StageDecl::new("repository_explore", " "),
        StageDecl::new("adjudicate", " "),
        StageDecl::new("evidence_summarize", " "),
    ];
    let generated = match compile_prompt_dir(&root, &stages) {
        Ok(generated) => generated,
        Err(error) => {
            eprintln!("prompt section error: {error}");
            std::process::exit(1);
        }
    };
    let manifest = root.join("manifest.toml");
    if let Err(error) = validate_manifest(&manifest, &generated.sections) {
        eprintln!("prompt manifest error: {error}");
        std::process::exit(1);
    }
    let out = std::path::Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR"))
        .join("prompt_sections.rs");
    std::fs::write(&out, generated.rust_source).expect("write generated prompt sections");
}
