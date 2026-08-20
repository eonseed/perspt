//! The base tool catalog (PSP-9 system 5).
//!
//! Domain-neutral entries mapping onto the `EffectKind` variants PSP-8
//! declares. `sed_replace` and `awk_filter` are deliberately absent: their
//! argument contracts lack the exact-match and state-witness preconditions
//! governed mutation requires, and `edit_file` / `apply_diff` cover their
//! uses. They are removed rather than retained through an internal bypass,
//! because this PSP preserves no unmediated effect path.

use super::entry::{ToolEntry, ToolOrigin};
use super::footprint::{AccessMode, FootprintSpec, ResourceSelector};
use crate::capability::{EffectKind, RiskClass};

/// Shorthand: an object schema from `(name, type, description, required)`.
fn schema(fields: &[(&str, &str, &str, bool)]) -> serde_json::Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for (name, kind, description, is_required) in fields {
        properties.insert(
            (*name).to_string(),
            serde_json::json!({"type": kind, "description": description}),
        );
        if *is_required {
            required.push(serde_json::Value::String((*name).to_string()));
        }
    }
    serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

fn path_read() -> FootprintSpec {
    FootprintSpec::new(vec![ResourceSelector::PathArgument {
        field: "path".into(),
        access: AccessMode::Read,
    }])
}

fn path_write() -> FootprintSpec {
    FootprintSpec::new(vec![ResourceSelector::PathArgument {
        field: "path".into(),
        access: AccessMode::Write,
    }])
}

fn entry(
    name: &str,
    description: &str,
    effect: EffectKind,
    risk: RiskClass,
    arg_schema: serde_json::Value,
    footprint: FootprintSpec,
    durable: bool,
) -> ToolEntry {
    ToolEntry {
        name: name.into(),
        description: description.into(),
        discovery_summary: String::new(),
        description_templates: None,
        effect,
        risk,
        schema: arg_schema,
        footprint,
        proposal_bindings: Vec::new(),
        durable,
        origin: ToolOrigin::Builtin,
        hot: false,
    }
}

/// Host-side tool-surface entries (deferred discovery and composition).
fn surface_entries() -> Vec<ToolEntry> {
    vec![
        entry(
            "tool_search",
            "Find governed tools by capability, name, and description; matching \
             schemas become available on the next turn",
            EffectKind::ToolSearch,
            RiskClass::Low,
            schema(&[
                ("query", "string", "Capability or operation to find", true),
                ("limit", "integer", "Maximum matches, from 1 to 12", false),
            ]),
            FootprintSpec::default(),
            false,
        )
        .hot(),
        entry(
            "tool_program",
            "Run a bounded pure Starlark program that returns nested tool \
             proposals; each nested call is mediated separately",
            EffectKind::ToolProgram,
            RiskClass::Low,
            schema(&[(
                "source",
                "string",
                "Starlark whose final expression is main and whose main() returns call JSON",
                true,
            )]),
            FootprintSpec::default(),
            false,
        )
        .hot(),
    ]
    .into_iter()
    .chain(recall_entries())
    .collect()
}

/// Host-side retrieval of evicted context and stored artifacts.
fn recall_entries() -> Vec<ToolEntry> {
    vec![
        entry(
            "context_recall",
            "Restore evicted context pages by page id (from an eviction note \
             or the page index), by path, or by typed diagnostic/test key; \
             returns the pages' original content",
            EffectKind::DataRead,
            RiskClass::Low,
            schema(&[
                (
                    "page_id",
                    "string",
                    "Content-addressed page id or its 16-hex prefix",
                    false,
                ),
                ("path", "string", "A path a tool call named", false),
                ("diagnostic_id", "string", "A typed diagnostic id", false),
                ("test_id", "string", "A typed test id", false),
                ("symbol", "string", "A typed symbol name", false),
                (
                    "provenance_key",
                    "string",
                    "A provenance key (the page's content address)",
                    false,
                ),
            ]),
            FootprintSpec::opaque(),
            false,
        )
        .hot(),
        entry(
            "read_artifact",
            "Read a window of a stored artifact by the artifact:<handle> shown in a \
             truncated-output note; returns bytes offset..offset+limit with a \
             continuation hint",
            EffectKind::DataRead,
            RiskClass::Low,
            schema(&[
                (
                    "handle",
                    "string",
                    "The content handle from a [full output: artifact:…] note",
                    true,
                ),
                (
                    "offset",
                    "integer",
                    "0-based byte offset (default 0)",
                    false,
                ),
                ("limit", "integer", "Bytes to return, capped at 7168", false),
            ]),
            FootprintSpec::opaque(),
            false,
        )
        .hot(),
    ]
}

/// The read-only half of the base catalog.
fn read_entries() -> Vec<ToolEntry> {
    let mut entries = surface_entries();
    entries.extend([
        entry(
            "read_file",
            "Read a file's contents with optional offset/limit windows; returns line-numbered text",
            EffectKind::ReadFile,
            RiskClass::Low,
            schema(&[
                ("path", "string", "Workspace-relative file path", true),
                ("offset", "integer", "1-based first line to read", false),
                ("limit", "integer", "Maximum number of lines", false),
            ]),
            path_read(),
            false,
        )
        .hot(),
        entry(
            "list_files",
            "List files in a directory, respecting ignore files",
            EffectKind::List,
            RiskClass::Low,
            schema(&[
                (
                    "path",
                    "string",
                    "Directory to list (default workspace root)",
                    false,
                ),
                ("offset", "integer", "0-based entry to continue from", false),
                ("limit", "integer", "Maximum entries to return", false),
            ]),
            path_read(),
            false,
        ),
    ]);
    entries.extend(search_read_entries());
    entries
}

/// The pattern-search half of the read catalog.
fn search_read_entries() -> Vec<ToolEntry> {
    vec![
        entry(
            "glob",
            "Match files by glob pattern, sorted by modification time",
            EffectKind::Search,
            RiskClass::Low,
            schema(&[
                ("pattern", "string", "Glob pattern, e.g. src/**/*.rs", true),
                ("offset", "integer", "0-based match to continue from", false),
                ("limit", "integer", "Maximum matches to return", false),
            ]),
            FootprintSpec::default(),
            false,
        )
        .hot(),
        entry(
            "grep",
            "Search file contents by regex with optional path filter and context lines",
            EffectKind::Search,
            RiskClass::Low,
            schema(&[
                ("query", "string", "Regex to search for", true),
                ("path", "string", "Directory or file to search in", false),
                (
                    "context",
                    "integer",
                    "Context lines around each match",
                    false,
                ),
            ]),
            path_read(),
            false,
        )
        .hot(),
    ]
}

/// Language-intelligence read entries.
fn intel_entries() -> Vec<ToolEntry> {
    vec![entry(
        "lsp_query",
        "Query language intelligence: definitions, references, hover, diagnostics",
        EffectKind::LspQuery,
        RiskClass::Low,
        schema(&[
            (
                "kind",
                "string",
                "definition | references | hover | diagnostics",
                true,
            ),
            ("path", "string", "File the query targets", true),
            ("symbol", "string", "Symbol name, when applicable", false),
        ]),
        path_read(),
        false,
    )]
}

/// Repository and escalation entries.
fn context_entries() -> Vec<ToolEntry> {
    vec![
        entry(
            "git_read",
            "Read repository state: status, diff, log, show",
            EffectKind::GitRead,
            RiskClass::Low,
            schema(&[
                ("subcommand", "string", "status | diff | log | show", true),
                ("args", "string", "Additional read-only arguments", false),
            ]),
            FootprintSpec::default(),
            false,
        ),
        entry(
            "ask_user",
            "Escalate to the user for approval, a capability grant, or a decision",
            EffectKind::AskUser,
            RiskClass::Low,
            schema(&[("question", "string", "The question or request", true)]),
            FootprintSpec::default(),
            false,
        )
        .hot(),
    ]
}

/// The mutating half of the base catalog.
fn write_entries() -> Vec<ToolEntry> {
    vec![
        entry(
            "write_file",
            "Create or replace a whole file",
            EffectKind::WriteArtifact,
            RiskClass::Medium,
            schema(&[
                ("path", "string", "Workspace-relative file path", true),
                ("content", "string", "Complete new file contents", true),
            ]),
            path_write(),
            false,
        ),
        entry(
            "edit_file",
            "Exact-string replace with a uniqueness check; fails closed on ambiguity",
            EffectKind::ApplyPatch,
            RiskClass::Medium,
            schema(&[
                ("path", "string", "File to edit", true),
                (
                    "old_string",
                    "string",
                    "Exact text to replace (must be unique)",
                    true,
                ),
                ("new_string", "string", "Replacement text", true),
            ]),
            path_write(),
            false,
        )
        .hot(),
        entry(
            "apply_diff",
            "Apply a unified diff to a file",
            EffectKind::ApplyPatch,
            RiskClass::Medium,
            schema(&[
                ("path", "string", "File to patch", true),
                ("diff", "string", "Unified diff content", true),
            ]),
            path_write(),
            false,
        )
        .hot(),
    ]
}

/// File relocation entries.
fn relocation_entries() -> Vec<ToolEntry> {
    vec![
        entry(
            "move_file",
            "Move or rename a file",
            EffectKind::MoveFile,
            RiskClass::Medium,
            schema(&[
                ("path", "string", "Source path", true),
                ("to", "string", "Destination path", true),
            ]),
            FootprintSpec::new(vec![
                ResourceSelector::PathArgument {
                    field: "path".into(),
                    access: AccessMode::Write,
                },
                ResourceSelector::PathArgument {
                    field: "to".into(),
                    access: AccessMode::Write,
                },
            ]),
            false,
        ),
        entry(
            "delete_file",
            "Delete a file",
            EffectKind::DeleteFile,
            RiskClass::Medium,
            schema(&[("path", "string", "File to delete", true)]),
            path_write(),
            false,
        ),
    ]
}

/// Verifier and command entries; footprints are opaque because their touched
/// state depends on the workspace, so they serialize rather than race.
fn command_entries() -> Vec<ToolEntry> {
    vec![
        entry(
            "exec",
            "Run one shell-free, read-only OS program in the workspace sandbox; \
             use rg, git diff/status/log/show, awk, sed -n, or coreutils. For \
             tests use run_test, for builds run_build, for formatting \
             run_formatter — exec denies them",
            EffectKind::Search,
            RiskClass::Low,
            schema(&[(
                "command",
                "string",
                "One direct program invocation; pipes, redirects, expansion, and \
                 mutation commands are denied",
                true,
            )]),
            FootprintSpec::opaque(),
            false,
        )
        .hot(),
        entry(
            "run_test",
            "Run the domain's declared test command; output is parsed into residuals",
            EffectKind::RunTest,
            RiskClass::Medium,
            schema(&[("filter", "string", "Optional test name filter", false)]),
            FootprintSpec::opaque(),
            false,
        )
        .hot(),
        entry(
            "run_build",
            "Run the domain's declared build command",
            EffectKind::RunBuild,
            RiskClass::Medium,
            schema(&[]),
            FootprintSpec::opaque(),
            false,
        )
        .hot(),
        entry(
            "run_formatter",
            "Run the domain's declared formatter",
            EffectKind::RunFormatter,
            RiskClass::Medium,
            schema(&[]),
            FootprintSpec::opaque(),
            false,
        )
        .hot(),
        entry(
            "run_repo_script",
            "Run a script declared in the project profile",
            EffectKind::RunRepoScript,
            RiskClass::Medium,
            schema(&[("name", "string", "Declared script name", true)]),
            FootprintSpec::opaque(),
            false,
        ),
        dependency_entry(),
    ]
}

/// The governed dependency-mutation entry (Gate J).
fn dependency_entry() -> ToolEntry {
    entry(
        "mutate_dependencies",
        "Add, remove, or update dependencies via the domain package manager",
        EffectKind::MutateDependencies,
        RiskClass::High,
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "add | remove | update"
                },
                "packages": {
                    "type": "array",
                    "description": "Package names (optionally versioned)",
                    "items": {"type": "string"},
                    "maxItems": 16
                },
                "dev": {
                    "type": "boolean",
                    "description": "Add as a development dependency"
                }
            },
            "required": ["action", "packages"],
            "additionalProperties": false
        }),
        FootprintSpec::new(vec![
            ResourceSelector::Literal {
                resource: crate::scheduler::Resource::Manifest("workspace".into()),
                access: AccessMode::Write,
            },
            ResourceSelector::Literal {
                resource: crate::scheduler::Resource::Lockfile("workspace".into()),
                access: AccessMode::Write,
            },
            ResourceSelector::Literal {
                resource: crate::scheduler::Resource::Toolchain("workspace".into()),
                access: AccessMode::Read,
            },
        ]),
        true,
    )
}

/// High-risk command and network entries.
fn high_risk_entries() -> Vec<ToolEntry> {
    vec![
        entry(
            "run_shell",
            "Run a shell command; requires an explicit RunShell capability",
            EffectKind::RunShell,
            RiskClass::High,
            schema(&[("command", "string", "The command line", true)]),
            FootprintSpec::opaque(),
            true,
        ),
        entry(
            "git_write",
            "Stage and commit; never push or hard-reset by default",
            EffectKind::GitWrite,
            RiskClass::High,
            schema(&[
                ("subcommand", "string", "add | commit", true),
                ("args", "string", "Arguments", false),
            ]),
            FootprintSpec::opaque(),
            true,
        ),
        entry(
            "fetch_url",
            "Fetch a URL; defaults to ask, domain allow-lists permitted",
            EffectKind::NetworkFetch,
            RiskClass::High,
            schema(&[("url", "string", "The URL to fetch", true)]),
            FootprintSpec::opaque(),
            true,
        ),
    ]
}

/// Privileged and delegation entries.
fn privileged_entries() -> Vec<ToolEntry> {
    vec![
        entry(
            "update_graph",
            "Split, add, or retire work-graph nodes under PSP-8 revision rules",
            EffectKind::UpdateGraph,
            RiskClass::Critical,
            schema(&[(
                "revision",
                "string",
                "Serialized graph revision request",
                true,
            )]),
            FootprintSpec::new(vec![ResourceSelector::Literal {
                resource: crate::scheduler::Resource::WorkGraph,
                access: AccessMode::Write,
            }]),
            true,
        ),
        entry(
            "spawn_agent",
            "Delegate to a child agent; the child capability MUST attenuate (Theorem 1)",
            EffectKind::SpawnAgent,
            RiskClass::Critical,
            schema(&[
                ("role", "string", "explorer | worker | reviewer", true),
                ("goal", "string", "The child's goal", true),
            ]),
            FootprintSpec::opaque(),
            true,
        ),
    ]
}

/// Every builtin entry.
pub fn base_entries() -> Vec<ToolEntry> {
    let mut entries = read_entries();
    entries.extend(intel_entries());
    entries.extend(context_entries());
    entries.extend(write_entries());
    entries.extend(relocation_entries());
    entries.extend(command_entries());
    entries.extend(high_risk_entries());
    entries.extend(privileged_entries());
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_base_entry_validates_at_assembly() {
        for entry in base_entries() {
            entry
                .validate()
                .unwrap_or_else(|e| panic!("{}: {e}", entry.name));
        }
    }

    #[test]
    fn the_governed_catalog_omits_sed_and_awk() {
        let names: Vec<String> = base_entries().into_iter().map(|e| e.name).collect();
        assert!(!names.contains(&"sed_replace".to_string()));
        assert!(!names.contains(&"awk_filter".to_string()));
        assert!(names.contains(&"edit_file".to_string()));
    }

    #[test]
    fn privileged_effects_are_critical_risk() {
        for entry in base_entries() {
            if entry.effect.is_privileged() {
                assert_eq!(entry.risk, RiskClass::Critical, "{}", entry.name);
            }
        }
    }

    #[test]
    fn read_only_entries_never_mark_durable() {
        for entry in base_entries() {
            if entry.effect.is_read_only() {
                assert!(!entry.durable, "{}", entry.name);
            }
        }
    }
}
