//! Compile the coding domain's prompt section library (PSP-10 system 23).
//! Same discipline as perspt-core: a malformed section fails this build,
//! and the committed manifest is validated, never edited.

use perspt_prompt_macros::{compile_prompt_dir, validate_manifest, StageDecl};

fn main() {
    println!("cargo:rerun-if-changed=prompts");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("prompts");
    let stages = [StageDecl::new("branch_correct", "\n")];
    let generated = match compile_prompt_dir(&root, &stages) {
        Ok(generated) => generated,
        Err(error) => {
            eprintln!("prompt section error: {error}");
            std::process::exit(1);
        }
    };
    if let Err(error) = validate_manifest(&root.join("manifest.toml"), &generated.sections) {
        eprintln!("prompt manifest error: {error}");
        std::process::exit(1);
    }
    let out = std::path::Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR"))
        .join("prompt_sections.rs");
    std::fs::write(&out, generated.rust_source).expect("write generated prompt sections");
}
