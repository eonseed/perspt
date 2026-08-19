//! Prompt-matrix mechanism checks (PSP-10 Gate Y, Phase 5).
//!
//! The five inline actor prompts moved verbatim into base sections; these
//! golden asserts prove the composed rendering is byte-identical to the
//! pre-move literals. A text change is always a visible diff here.

use perspt_core::prompts::{PlatformPromptLibrary, PlatformStage};

/// Compile a stage for the live single-slot dialect and return the one
/// transport-facing system text (the byte-identity surface).
fn wire_text(stage: &PlatformStage) -> String {
    let route = perspt_sdk::prompt::PromptRoute {
        adapter: "genai".into(),
        family: perspt_sdk::ModelFamily::Other("test".into()),
        exact_model: None,
    };
    stage
        .compile(
            &route,
            &perspt_sdk::prompt::ModelDialect::genai_single_slot_v1(),
            &perspt_sdk::prompt::tool_surface_hash(&[]),
        )
        .unwrap()
        .system_text()
}

fn digest(stage: &PlatformStage) -> String {
    let route = perspt_sdk::prompt::PromptRoute {
        adapter: "genai".into(),
        family: perspt_sdk::ModelFamily::Other("test".into()),
        exact_model: None,
    };
    stage
        .compile(
            &route,
            &perspt_sdk::prompt::ModelDialect::genai_single_slot_v1(),
            &perspt_sdk::prompt::tool_surface_hash(&[]),
        )
        .unwrap()
        .program_digest
}

/// The exact pre-move literals, frozen at migration time.
const WORKER: &str =
    "You are a governed coding agent. Propose tool calls; every effect is mediated.";
const ARCHITECT: &str = "You are a planning architect. Decompose the task into independent \
     work-graph nodes ONLY when parts genuinely touch disjoint files. \
     Call update_graph exactly once; its `revision` argument is JSON: \
     {\"nodes\":[{\"node_id\":str,\"goal\":str,\"output_targets\":[str]}],\
     \"edges\":[[src,dst]]}. Declare output_targets precisely; a node \
     without them serializes against everything. Prefer one node when \
     in doubt.";
const EXPLORER: &str = "You are a read-only repository explorer. Inspect with the provided \
     tools, then answer. You cannot modify anything.";
const ADJUDICATOR: &str = "You are a conjunctive coding validator with no tools or authority. \
     Review only the realized diff. Return strict JSON: \
     {\"pass\":bool,\"reason\":string}. Reject uncertainty; do not \
     propose edits.";
const SUMMARIZER: &str = "Summarize the deterministic repository map for a coding worker. \
     You have no tools and no authority. Do not claim facts absent \
     from the map.";

/// The revision shape's single source of truth (mirrors the constant the
/// architect passes; asserted equal in perspt-agent's tests).
const REVISION_SHAPE: &str =
    "{\"nodes\":[{\"node_id\":str,\"goal\":str,\"output_targets\":[str]}],\"edges\":[[src,dst]]}";

#[test]
fn composed_renderings_are_byte_identical_to_the_literals() {
    assert_eq!(
        wire_text(
            &PlatformPromptLibrary::session_bootstrap("coding", &Default::default()).unwrap()
        ),
        WORKER
    );
    assert_eq!(
        wire_text(&PlatformPromptLibrary::graph_plan(REVISION_SHAPE, &Default::default()).unwrap()),
        ARCHITECT
    );
    assert_eq!(
        wire_text(&PlatformPromptLibrary::repository_explore(&Default::default()).unwrap()),
        EXPLORER
    );
    assert_eq!(
        wire_text(&PlatformPromptLibrary::adjudicate(&Default::default()).unwrap()),
        ADJUDICATOR
    );
    assert_eq!(
        wire_text(&PlatformPromptLibrary::evidence_summarize(&Default::default()).unwrap()),
        SUMMARIZER
    );
}

/// The worker's domain is a variable now — the research domain gets its own
/// wording instead of the historic hardcoded "coding".
#[test]
fn the_worker_domain_is_no_longer_hardcoded() {
    let research =
        PlatformPromptLibrary::session_bootstrap("research", &Default::default()).unwrap();
    assert_eq!(
        wire_text(&research),
        "You are a governed research agent. Propose tool calls; every effect is mediated."
    );
}

/// Composition, provenance, and digests are deterministic.
#[test]
fn composition_is_deterministic_with_full_provenance() {
    let first = PlatformPromptLibrary::session_bootstrap("coding", &Default::default()).unwrap();
    let second = PlatformPromptLibrary::session_bootstrap("coding", &Default::default()).unwrap();
    assert_eq!(digest(&first), digest(&second));
    assert_eq!(first.sections.len(), 2);
    assert!(first
        .sections
        .iter()
        .all(|section| section.content_hash.starts_with("sha256:")));
    // A changed variable changes the digest.
    let other = PlatformPromptLibrary::session_bootstrap("research", &Default::default()).unwrap();
    assert_ne!(digest(&first), digest(&other));
}

/// The committed manifest digest covers every section.
#[test]
fn the_manifest_covers_all_ten_sections() {
    let manifest = PlatformPromptLibrary::manifest();
    assert_eq!(manifest.entries.len(), 10);
    assert!(manifest.digest().starts_with("sha256:"));
    assert_eq!(
        manifest.digest(),
        PlatformPromptLibrary::manifest().digest()
    );
}

/// No production actor builds its system prompt from an inline literal any
/// more: the five migrated call sites compose from the section library.
/// Probes and tests are exempt (spec test plan, Phase 5).
#[test]
fn no_production_system_prompt_literal_remains() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("perspt-agent/src");
    let mut offenders = Vec::new();
    scan(&root, &mut offenders);
    assert!(offenders.is_empty(), "inline system prompts: {offenders:?}");
}

fn scan(dir: &std::path::Path, offenders: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan(&path, offenders);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        // The capability probe is an exempt diagnostic surface.
        if path.ends_with("probe.rs") {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        // Strip in-module test blocks: literals in tests are exempt.
        let production = source
            .split("#[cfg(test)]")
            .next()
            .unwrap_or_default()
            .to_string();
        if production.contains("Conversation::with_system(\"") {
            offenders.push(path.display().to_string());
        }
    }
}

/// System 25 live substitution: a validated replacement body composes in
/// place of the base section (same schema, its own content hash), renders
/// with the base section's typed values, and refuses unknown ids and
/// undeclared placeholders.
#[test]
fn a_bundle_override_substitutes_live_under_the_compiled_schema() {
    let mut overrides = perspt_core::prompts::SectionOverrides::default();
    overrides
        .insert_replacement(
            "session_bootstrap/role",
            "You are an experimental governed {{domain_id}} agent.",
        )
        .unwrap();
    let stage = PlatformPromptLibrary::session_bootstrap("coding", &overrides).unwrap();
    assert!(stage.sections[0]
        .content
        .contains("experimental governed coding agent"));
    let base = PlatformPromptLibrary::session_bootstrap("coding", &Default::default()).unwrap();
    assert_ne!(
        stage.sections[0].content_hash, base.sections[0].content_hash,
        "the replacement carries its own content hash"
    );
    assert_eq!(
        stage.sections[1].content, base.sections[1].content,
        "sections without an override stay byte-identical"
    );

    let mut unknown = perspt_core::prompts::SectionOverrides::default();
    assert!(
        unknown
            .insert_replacement("branch_correct/role", "x")
            .is_err(),
        "non-platform sections refuse live substitution"
    );

    let mut bad = perspt_core::prompts::SectionOverrides::default();
    bad.insert_replacement("session_bootstrap/role", "hello {{undeclared}}")
        .unwrap();
    assert!(
        PlatformPromptLibrary::session_bootstrap("coding", &bad).is_err(),
        "an undeclared placeholder fails closed at render"
    );
}
