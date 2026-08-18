//! The `perspt prompts` command family (PSP-10 system 28).
//!
//! Everything here is read-only over compiled section libraries, external
//! bundle directories, and the session ledger — the one exception is
//! `manifest`, which regenerates a library's committed `manifest.toml`
//! explicitly (a normal build never edits the tree).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use perspt_core::prompts::PlatformPromptLibrary;
use perspt_sdk::prompt::SectionTemplate;

/// Every compiled section with its owner, for list/render/lint.
fn compiled_sections() -> Vec<(&'static str, &'static str, SectionTemplate)> {
    let mut sections = Vec::new();
    for (stage, templates) in [
        (
            "session_bootstrap",
            perspt_core::prompts::session_bootstrap::templates(),
        ),
        ("graph_plan", perspt_core::prompts::graph_plan::templates()),
        (
            "repository_explore",
            perspt_core::prompts::repository_explore::templates(),
        ),
        ("adjudicate", perspt_core::prompts::adjudicate::templates()),
        (
            "evidence_summarize",
            perspt_core::prompts::evidence_summarize::templates(),
        ),
    ] {
        for template in templates {
            sections.push(("perspt-core", stage, template));
        }
    }
    for template in perspt_coding::prompts::branch_correct::templates() {
        sections.push(("perspt-coding", "branch_correct", template));
    }
    sections
}

/// `perspt prompts list`: every section's id, version, stage, role,
/// required/priority, and content hash.
pub fn list() -> Result<()> {
    println!(
        "{:<40} {:>3}  {:<20} {:<6} {:<8} hash",
        "id", "ver", "stage", "role", "req/prio"
    );
    for (owner, _stage, template) in compiled_sections() {
        let schema = &template.schema;
        let req = if schema.required {
            "req".to_string()
        } else {
            format!("p{}", schema.priority)
        };
        println!(
            "{:<40} {:>3}  {:<20} {:<6} {:<8} {}  [{owner}]",
            schema.id.0,
            schema.version.0,
            schema.id.0.split('/').next().unwrap_or(""),
            match schema.role {
                perspt_sdk::prompt::PromptMessageRole::System => "system",
                perspt_sdk::prompt::PromptMessageRole::User => "user",
            },
            req,
            &template.content_hash[..21.min(template.content_hash.len())],
        );
    }
    Ok(())
}

/// `perspt prompts render <stage>`: compose a stage with fixture
/// variables and print the composed text, provenance, and digest.
pub fn render(stage: &str) -> Result<()> {
    let composed = match stage {
        "session_bootstrap" => PlatformPromptLibrary::session_bootstrap("coding"),
        "graph_plan" => PlatformPromptLibrary::graph_plan("{\"nodes\":[…],\"edges\":[…]}"),
        "repository_explore" => PlatformPromptLibrary::repository_explore(),
        "adjudicate" => PlatformPromptLibrary::adjudicate(),
        "evidence_summarize" => PlatformPromptLibrary::evidence_summarize(),
        other => anyhow::bail!(
            "unknown stage {other:?}; renderable stages: session_bootstrap, graph_plan, \
             repository_explore, adjudicate, evidence_summarize"
        ),
    }
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("stage: {stage}");
    println!("digest: {}", composed.digest);
    println!("sections:");
    for section in &composed.sections {
        println!(
            "  {} v{} {}",
            section.id.0, section.version.0, section.content_hash
        );
    }
    println!("---\n{}", composed.text);
    Ok(())
}

/// `perspt prompts lint [--bundle <dir>]`: run the codegen validation list
/// over an external bundle directory (or report the built-ins healthy).
pub fn lint(bundle: Option<&Path>) -> Result<()> {
    let Some(bundle) = bundle else {
        println!(
            "built-in sections are compile-time validated; pass --bundle <dir> to lint a \
             replacement bundle"
        );
        return Ok(());
    };
    let known: Vec<(String, SectionTemplate)> = compiled_sections()
        .into_iter()
        .map(|(_, _, template)| (template.schema.id.0.clone(), template))
        .collect();
    let mut checked = 0usize;
    for entry in std::fs::read_dir(bundle).context("reading bundle directory")? {
        let stage_dir = entry?.path();
        if !stage_dir.is_dir() {
            continue;
        }
        for file in std::fs::read_dir(&stage_dir)? {
            let path = file?.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }
            let raw = std::fs::read_to_string(&path)?;
            let (matter, body) =
                perspt_prompt_macros::parse_section_file(&path.display().to_string(), &raw)
                    .map_err(|error| anyhow::anyhow!("{error}"))?;
            let Some((_, template)) = known.iter().find(|(id, _)| *id == matter.id) else {
                anyhow::bail!(
                    "{}: section id {:?} is not a known compiled section — a bundle may \
                     replace known sections only (PSP-10 system 25)",
                    path.display(),
                    matter.id
                );
            };
            let body = body.strip_suffix('\n').unwrap_or(&body);
            perspt_prompt_macros::validate_section_body(&template.schema, body)
                .map_err(|error| anyhow::anyhow!("{error}"))?;
            checked += 1;
            println!("ok: {} replaces {}", path.display(), matter.id);
        }
    }
    anyhow::ensure!(checked > 0, "bundle contains no section files");
    println!("{checked} replacement section(s) valid");
    Ok(())
}

/// Validate every configured `[prompts].bundles` directory at session
/// start (PSP-10 system 25): an invalid bundle refuses startup — never a
/// silent fallback. Replacement bodies must satisfy exactly the compiled
/// section schema; live substitution into composed programs arrives with
/// override resolution and is refused until then unless the bundle merely
/// re-states known sections.
pub fn validate_configured_bundles(bundles: &[String]) -> Result<()> {
    for bundle in bundles {
        lint(Some(Path::new(bundle)))
            .with_context(|| format!("validating [prompts] bundle {bundle}"))?;
    }
    Ok(())
}

/// `perspt prompts manifest <crate-dir>`: regenerate a library's committed
/// manifest from its prompt sources. Explicit only — builds never write.
pub fn manifest(prompts_dir: &Path, stages: &[(&str, &str)]) -> Result<()> {
    let declared: Vec<perspt_prompt_macros::StageDecl> = stages
        .iter()
        .map(|(name, separator)| perspt_prompt_macros::StageDecl::new(*name, *separator))
        .collect();
    let generated = perspt_prompt_macros::compile_prompt_dir(prompts_dir, &declared)
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    let mut out = String::from(
        "# Generated by `perspt prompts manifest`; validated at build time.\n\
         # Do not edit by hand — regenerate after any section change.\n",
    );
    for section in &generated.sections {
        out.push_str(&format!(
            "\n[[section]]\nid = \"{}\"\nversion = {}\nstage = \"{}\"\nrole = \"{}\"\n\
             required = {}\npriority = {}\nmax_bytes = {}\nowner = \"generated\"\n\
             content_hash = \"{}\"\n",
            section.schema.id.0,
            section.schema.version.0,
            section.stage,
            section.role_name,
            section.schema.required,
            section.schema.priority,
            section.schema.max_bytes,
            section.content_hash,
        ));
    }
    let path = prompts_dir.join("manifest.toml");
    std::fs::write(&path, out)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// `perspt prompts explain-session <db> <session>`: the programs a session
/// actually compiled, with digests, from the ledger.
pub fn explain_session(db_path: &PathBuf, session_id: &str) -> Result<()> {
    let store = perspt_store::SessionStore::open(db_path)?;
    let rows = store.get_psp9_events(session_id)?;
    let mut programs = 0usize;
    for row in &rows {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&row.event_json) else {
            continue;
        };
        if value.get("kind").and_then(|kind| kind.as_str()) != Some("prompt_program_compiled") {
            continue;
        }
        programs += 1;
        let payload = &value["payload"];
        println!(
            "seq {} stage {} digest {}",
            row.sequence,
            payload
                .get("stage")
                .and_then(|stage| stage.as_str())
                .unwrap_or("?"),
            payload
                .get("digest")
                .and_then(|digest| digest.as_str())
                .unwrap_or("?"),
        );
        if let Some(sections) = payload.get("sections").and_then(|value| value.as_array()) {
            for section in sections {
                println!(
                    "    {} v{} {}",
                    section
                        .pointer("/id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?"),
                    section.get("version").and_then(|v| v.as_u64()).unwrap_or(0),
                    section
                        .get("content_hash")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?"),
                );
            }
        }
    }
    println!("{programs} compiled program(s) recorded");
    Ok(())
}

/// `perspt context explain-turn <db> <session>`: the session's recorded
/// context events — compactions (summary + source pages), infeasibility
/// refusals, and working-set selections.
pub fn explain_context(db_path: &PathBuf, session_id: &str) -> Result<()> {
    let store = perspt_store::SessionStore::open(db_path)?;
    let rows = store.get_psp9_events(session_id)?;
    let mut shown = 0usize;
    for row in &rows {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&row.event_json) else {
            continue;
        };
        if value.get("kind").and_then(|kind| kind.as_str()) != Some("tool_loop") {
            continue;
        }
        let Some(payload) = value.get("payload") else {
            continue;
        };
        let body = payload.get("body").unwrap_or(payload);
        let Some(event) = body.get("event").and_then(|event| event.as_str()) else {
            continue;
        };
        if !event.starts_with("context_") {
            continue;
        }
        shown += 1;
        println!("seq {} {event}: {body}", row.sequence);
    }
    println!("{shown} context event(s) recorded");
    Ok(())
}
