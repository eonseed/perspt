//! Prompt-matrix mechanism checks (PSP-10 Gate Y, Phase 5).
//!
//! The five inline actor prompts moved verbatim into base sections; these
//! golden asserts prove the composed rendering is byte-identical to the
//! pre-move literals. A text change is always a visible diff here.

use perspt_core::prompts::PlatformPromptLibrary;

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
        PlatformPromptLibrary::session_bootstrap("coding")
            .unwrap()
            .text,
        WORKER
    );
    assert_eq!(
        PlatformPromptLibrary::graph_plan(REVISION_SHAPE)
            .unwrap()
            .text,
        ARCHITECT
    );
    assert_eq!(
        PlatformPromptLibrary::repository_explore().unwrap().text,
        EXPLORER
    );
    assert_eq!(
        PlatformPromptLibrary::adjudicate().unwrap().text,
        ADJUDICATOR
    );
    assert_eq!(
        PlatformPromptLibrary::evidence_summarize().unwrap().text,
        SUMMARIZER
    );
}

/// The worker's domain is a variable now — the research domain gets its own
/// wording instead of the historic hardcoded "coding".
#[test]
fn the_worker_domain_is_no_longer_hardcoded() {
    let research = PlatformPromptLibrary::session_bootstrap("research").unwrap();
    assert_eq!(
        research.text,
        "You are a governed research agent. Propose tool calls; every effect is mediated."
    );
}

/// Composition, provenance, and digests are deterministic.
#[test]
fn composition_is_deterministic_with_full_provenance() {
    let first = PlatformPromptLibrary::session_bootstrap("coding").unwrap();
    let second = PlatformPromptLibrary::session_bootstrap("coding").unwrap();
    assert_eq!(first, second);
    assert_eq!(first.sections.len(), 2);
    assert!(first
        .sections
        .iter()
        .all(|section| section.content_hash.starts_with("sha256:")));
    // A changed variable changes the digest.
    let other = PlatformPromptLibrary::session_bootstrap("research").unwrap();
    assert_ne!(first.digest, other.digest);
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
