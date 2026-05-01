//! PSP-7 §5: Typed prompt compiler.
//!
//! Maps `(PromptIntent, PromptEvidence) → CompiledPrompt` with full provenance
//! tracking.  This module is the single entry-point for all prompt assembly;
//! callers build a [`PromptEvidence`] and select a [`PromptIntent`], and the
//! compiler returns a [`CompiledPrompt`] with the final text and provenance.

use perspt_core::types::{CompiledPrompt, PromptEvidence, PromptIntent, PromptProvenance};

/// Verifier analysis preamble used in the two-stage correction flow.
pub(crate) const VERIFIER_ANALYSIS_PREAMBLE: &str = "\
You are a Verifier agent. Analyze the following correction request and produce \
concise, structured guidance for the code fixer. Identify:\n\
1. Root cause of each failure\n\
2. Which specific functions/lines need changes\n\
3. Constraints that must be preserved\n\
Do NOT produce code — only analysis and guidance.\n\n";

/// Compile a prompt from a typed intent and gathered evidence.
///
/// The evidence struct carries all possible inputs; each intent family reads
/// only the fields it needs and ignores the rest.
pub fn compile(intent: PromptIntent, evidence: &PromptEvidence) -> CompiledPrompt {
    let mut sources: Vec<String> = Vec::new();

    let text = match intent {
        PromptIntent::ArchitectExisting => {
            sources.push("architect_existing_template".into());
            if evidence.project_summary.is_some() {
                sources.push("project_summary".into());
            }
            compile_architect(crate::prompts::ARCHITECT_EXISTING, evidence)
        }
        PromptIntent::ArchitectGreenfield => {
            sources.push("architect_greenfield_template".into());
            compile_architect(crate::prompts::ARCHITECT_GREENFIELD, evidence)
        }
        PromptIntent::ActuatorMultiOutput => {
            sources.push("actuator_multi_output".into());
            compile_actuator(evidence, true)
        }
        PromptIntent::ActuatorSingleOutput => {
            sources.push("actuator_single_output".into());
            compile_actuator(evidence, false)
        }
        PromptIntent::VerifierAnalysis => {
            sources.push("verifier_check_template".into());
            compile_verifier(evidence)
        }
        PromptIntent::CorrectionRetry => {
            sources.push("correction_retry".into());
            if evidence.verifier_diagnostics.is_some() {
                sources.push("verifier_diagnostics".into());
            }
            if !evidence.existing_file_contents.is_empty() {
                sources.push("existing_file_contents".into());
            }
            if evidence.plugin_correction_fragment.is_some() {
                sources.push("plugin_correction_fragment".into());
            }
            compile_correction(evidence)
        }
        PromptIntent::BundleRetarget => {
            sources.push("bundle_retarget_template".into());
            compile_bundle_retarget(evidence)
        }
        PromptIntent::SpeculatorBasic => {
            sources.push("speculator_basic".into());
            let goal = evidence.node_goal.as_deref().unwrap_or("");
            crate::prompts::SPECULATOR_BASIC.replace("{goal}", goal)
        }
        PromptIntent::SpeculatorLookahead => {
            sources.push("speculator_lookahead".into());
            compile_speculator_lookahead(evidence)
        }
        PromptIntent::SoloGenerate => {
            sources.push("solo_generate_template".into());
            let task = evidence.user_goal.as_deref().unwrap_or("");
            crate::prompts::SOLO_GENERATE.replace("{task}", task)
        }
        PromptIntent::SoloCorrect => {
            sources.push("solo_correction_template".into());
            if evidence.verifier_diagnostics.is_some() {
                sources.push("verifier_diagnostics".into());
            }
            compile_solo_correction(evidence)
        }
        PromptIntent::ProjectNameSuggest => {
            sources.push("project_name_suggest".into());
            let task = evidence.user_goal.as_deref().unwrap_or("");
            crate::prompts::PROJECT_NAME_SUGGEST.replace("{task}", task)
        }
    };

    let plugin_fragment_source = evidence
        .plugin_correction_fragment
        .as_ref()
        .map(|_| "owner_plugin".to_string());

    CompiledPrompt {
        text,
        provenance: PromptProvenance {
            intent,
            plugin_fragment_source,
            evidence_sources: sources,
            compiled_at: epoch_seconds(),
        },
    }
}

// ---------------------------------------------------------------------------
// Per-family compilation helpers
// ---------------------------------------------------------------------------

fn compile_architect(template: &str, ev: &PromptEvidence) -> String {
    let task = ev.user_goal.as_deref().unwrap_or("");
    let project_context = ev.project_summary.as_deref().unwrap_or("");
    let error_feedback = ev.error_feedback.as_deref().unwrap_or("");
    let evidence_section = ev.evidence_section.as_deref().unwrap_or("");
    let working_dir = ev.working_dir.as_deref().unwrap_or(".");

    render_architect(
        template,
        task,
        std::path::Path::new(working_dir),
        project_context,
        error_feedback,
        evidence_section,
        &ev.active_plugins,
    )
}

fn compile_actuator(ev: &PromptEvidence, is_multi: bool) -> String {
    let goal = ev.node_goal.as_deref().unwrap_or("");
    let target_file = ev
        .output_files
        .first()
        .map(|s| s.as_str())
        .unwrap_or("main.py");
    let allowed_output_paths = format!("{:?}", ev.output_files);
    let context_files = format!("{:?}", ev.context_files);
    let interface = ev.interface_signature.as_deref().unwrap_or("");
    let invariants = ev.invariants.as_deref().unwrap_or("");
    let forbidden = ev.forbidden_patterns.as_deref().unwrap_or("");
    let working_dir = ev.working_dir.as_deref().unwrap_or(".");
    let hints = ev.workspace_import_hints.as_deref().unwrap_or("");

    render_actuator(
        goal,
        interface,
        invariants,
        forbidden,
        working_dir,
        &context_files,
        target_file,
        &allowed_output_paths,
        hints,
        is_multi,
    )
}

fn compile_verifier(ev: &PromptEvidence) -> String {
    let implementation = ev
        .existing_file_contents
        .first()
        .map(|(_, content)| content.as_str())
        .unwrap_or("");
    let interface = ev.interface_signature.as_deref().unwrap_or("");
    let invariants = ev.invariants.as_deref().unwrap_or("");
    let forbidden = ev.forbidden_patterns.as_deref().unwrap_or("");
    let weighted_tests = ev.weighted_tests.as_deref().unwrap_or("");

    render_verifier(
        interface,
        invariants,
        forbidden,
        weighted_tests,
        implementation,
    )
}

fn compile_correction(ev: &PromptEvidence) -> String {
    let goal = ev.node_goal.as_deref().unwrap_or("");
    let diagnostics = ev
        .verifier_diagnostics
        .as_deref()
        .unwrap_or("No specific errors captured.");
    let owner_plugin = ev.owner_plugin.as_deref().unwrap_or("");

    // Detect language from first file extension for code fences
    let lang = ev
        .existing_file_contents
        .first()
        .map(|(p, _)| p.as_str())
        .and_then(|p| std::path::Path::new(p).extension())
        .and_then(|e| e.to_str())
        .map(|ext| match ext {
            "py" => "python",
            "rs" => "rust",
            "ts" | "tsx" => "typescript",
            "js" | "jsx" => "javascript",
            "go" => "go",
            "java" => "java",
            "rb" => "ruby",
            "c" | "h" => "c",
            "cpp" | "cc" | "cxx" | "hpp" => "cpp",
            "cs" => "csharp",
            other => other,
        })
        .unwrap_or("text");

    let mut prompt = format!(
        "## Code Correction Required\n\n\
         The code you generated has errors detected by the language toolchain.\n\
         Your task is to fix ALL errors and return the complete corrected file(s).\n\n\
         ### Original Goal\n{}\n\n\
         ### Current Code (with errors)\n",
        goal,
    );

    // Include all affected files
    for (path, content) in &ev.existing_file_contents {
        prompt.push_str(&format!(
            "File: {}\n```{}\n{}\n```\n\n",
            path, lang, content
        ));
    }

    // Diagnostics section (pre-formatted by caller with fix directions if available)
    if let Some(v_syn) = ev.energy_v_syn {
        prompt.push_str(&format!(
            "### Detected Errors (V_syn = {:.2})\n{}\n",
            v_syn, diagnostics
        ));
    } else {
        prompt.push_str(&format!("### Detected Errors\n{}\n", diagnostics));
    }

    if let Some(ref fragment) = ev.plugin_correction_fragment {
        prompt.push_str(&format!("\n### Plugin Guidance\n{}\n", fragment));
    }

    let attempt_count = if !ev.previous_attempts.is_empty() {
        ev.previous_attempts.len()
    } else {
        ev.previous_attempt_count
    };
    if attempt_count > 0 {
        prompt.push_str(&format!(
            "\n### Previous Correction Attempts: {}\n\
             The previous attempts did not fully resolve the errors. \
             Please try a different approach.\n",
            attempt_count
        ));
    }

    // Restriction map context for structural dependencies
    if let Some(ref ctx) = ev.restriction_map_context {
        if !ctx.is_empty() {
            prompt.push_str(&format!("\n### Restriction Map Context\n\n{}\n", ctx));
        }
    }

    // Project file tree for path awareness
    if let Some(ref tree) = ev.project_file_tree {
        if !tree.is_empty() {
            prompt.push_str(&format!(
                "\n### Current Project Tree\n\n```\n{}\n```\n",
                tree
            ));
        }
    }

    // Build/test output from plugin verification
    if let Some(ref output) = ev.build_test_output {
        if !output.is_empty() {
            prompt.push_str(&format!(
                "\n### Build / Test Output\nThe following is the raw output from the build toolchain (e.g. `cargo check` / `cargo build`). \
                 Use this to identify missing dependencies, unresolved imports, or type errors:\n```\n{}\n```\n",
                output
            ));
        }
    }

    let target_paths = ev
        .existing_file_contents
        .iter()
        .map(|(path, _)| path.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    // Generate language-specific dependency command examples
    let commands_example = match owner_plugin {
        "rust" => "cargo add thiserror\ncargo add clap --features derive",
        "python" => "uv add httpx\nuv add --dev pytest",
        "javascript" => "npm install express\nnpm install --save-dev jest",
        _ => "cargo add thiserror\nuv add httpx",
    };

    prompt.push_str(&format!(
        r#"
### Fix Requirements
1. Fix ALL errors listed above - do not leave any unfixed
2. Maintain the original functionality and goal
3. Follow {} language conventions and idioms
4. Import any missing modules or dependencies
5. Return a JSON artifact bundle targeting these exact path(s): {}
6. If errors mention missing crates/packages (e.g. "can't find crate", "unresolved import" for an external dependency, "ModuleNotFoundError", "No module named"), list the required install commands

### Output Format
Return only this JSON object shape. Do not wrap it in markdown unless the provider requires a fenced json block.

```json
{{
    "artifacts": [
        {{
            "operation": "write",
            "path": "path/from/list/above",
            "content": "complete corrected file contents"
        }}
    ],
    "commands": [
        "optional dependency command, for example: {}"
    ]
}}
```

Use an empty commands array when no dependency command is needed.
"#,
                lang,
                if target_paths.is_empty() { "the original file(s)" } else { &target_paths },
                commands_example.lines().next().unwrap_or("")
    ));

    prompt
}

fn compile_bundle_retarget(ev: &PromptEvidence) -> String {
    let expected = ev.output_files.join(", ");
    let dropped = ev.rejected_bundle_summary.as_deref().unwrap_or("(unknown)");
    let original_prompt = ev.node_goal.as_deref().unwrap_or("");

    render_bundle_retarget(&expected, dropped, original_prompt)
}

fn compile_speculator_lookahead(ev: &PromptEvidence) -> String {
    let node_id = ev
        .context_files
        .first()
        .map(|s| s.as_str())
        .unwrap_or("current");
    let goal = ev.node_goal.as_deref().unwrap_or("");
    let downstream = ev
        .output_files
        .iter()
        .map(|s| format!("- {}", s))
        .collect::<Vec<_>>()
        .join("\n");

    render_speculator_lookahead(node_id, goal, &downstream)
}

fn compile_solo_correction(ev: &PromptEvidence) -> String {
    let task = ev.user_goal.as_deref().unwrap_or("");
    let filename = ev.solo_file_path.as_deref().unwrap_or("script.py");
    let current_code = ev
        .existing_file_contents
        .first()
        .map(|(_, c)| c.as_str())
        .unwrap_or("");
    let error_list = ev
        .verifier_diagnostics
        .as_deref()
        .unwrap_or("No specific errors captured, but energy is still too high.");

    render_solo_correction(
        task,
        filename,
        current_code,
        "0.00", // v_syn — caller provides via diagnostics text
        "0.00", // v_log
        "0.00", // v_boot
        error_list,
    )
}

// ---------------------------------------------------------------------------
// Render helpers (moved from prompts.rs — private to the compiler)
// ---------------------------------------------------------------------------

/// JSON brace escapes for templates that contain `{OPEN_BRACE}` / `{CLOSE_BRACE}`.
const OPEN_BRACE: &str = "{";
const CLOSE_BRACE: &str = "}";

fn render_architect(
    template: &str,
    task: &str,
    working_dir: &std::path::Path,
    project_context: &str,
    error_feedback: &str,
    evidence_section: &str,
    active_plugins: &[String],
) -> String {
    let plugin_section = if active_plugins.is_empty() {
        String::new()
    } else {
        format!(
            "\n## Detected Toolchain\nActive language plugins: {}\nPlan verification-aware nodes that align with these plugins' build/test capabilities.\n",
            active_plugins.join(", ")
        )
    };
    let enriched_context = if plugin_section.is_empty() {
        project_context.to_string()
    } else {
        format!("{}{}", project_context, plugin_section)
    };
    template
        .replace("{task}", task)
        .replace("{working_dir}", &working_dir.display().to_string())
        .replace("{project_context}", &enriched_context)
        .replace("{error_feedback}", error_feedback)
        .replace("{evidence_section}", evidence_section)
        .replace("{OPEN_BRACE}", OPEN_BRACE)
        .replace("{CLOSE_BRACE}", CLOSE_BRACE)
}

#[allow(clippy::too_many_arguments)]
fn render_actuator(
    goal: &str,
    interface: &str,
    invariants: &str,
    forbidden: &str,
    working_dir: &str,
    context_files: &str,
    target_file: &str,
    allowed_output_paths: &str,
    workspace_import_hints: &str,
    is_multi_output: bool,
) -> String {
    let output_format = if is_multi_output {
        crate::prompts::ACTUATOR_MULTI_OUTPUT
            .replace("{target_file}", target_file)
            .replace("{OPEN_BRACE}", OPEN_BRACE)
            .replace("{CLOSE_BRACE}", CLOSE_BRACE)
    } else {
        crate::prompts::ACTUATOR_SINGLE_OUTPUT.replace("{target_file}", target_file)
    };

    crate::prompts::ACTUATOR_CODING
        .replace("{goal}", goal)
        .replace("{interface}", interface)
        .replace("{invariants}", invariants)
        .replace("{forbidden}", forbidden)
        .replace("{working_dir}", working_dir)
        .replace("{context_files}", context_files)
        .replace("{target_file}", target_file)
        .replace("{allowed_output_paths}", allowed_output_paths)
        .replace("{workspace_import_hints}", workspace_import_hints)
        .replace("{output_format}", &output_format)
}

fn render_verifier(
    interface: &str,
    invariants: &str,
    forbidden: &str,
    weighted_tests: &str,
    implementation: &str,
) -> String {
    crate::prompts::VERIFIER_CHECK
        .replace("{interface}", interface)
        .replace("{invariants}", invariants)
        .replace("{forbidden}", forbidden)
        .replace("{weighted_tests}", weighted_tests)
        .replace("{implementation}", implementation)
}

fn render_speculator_lookahead(node_id: &str, goal: &str, downstream: &str) -> String {
    crate::prompts::SPECULATOR_LOOKAHEAD
        .replace("{node_id}", node_id)
        .replace("{goal}", goal)
        .replace("{downstream}", downstream)
}

fn render_solo_correction(
    task: &str,
    filename: &str,
    current_code: &str,
    v_syn: &str,
    v_log: &str,
    v_boot: &str,
    error_list: &str,
) -> String {
    crate::prompts::SOLO_CORRECTION
        .replace("{task}", task)
        .replace("{filename}", filename)
        .replace("{current_code}", current_code)
        .replace("{v_syn}", v_syn)
        .replace("{v_log}", v_log)
        .replace("{v_boot}", v_boot)
        .replace("{error_list}", error_list)
}

fn render_bundle_retarget(
    expected_files: &str,
    dropped_files: &str,
    original_prompt: &str,
) -> String {
    crate::prompts::BUNDLE_RETARGET
        .replace("{expected_files}", expected_files)
        .replace("{dropped_files}", dropped_files)
        .replace("{original_prompt}", original_prompt)
}

fn epoch_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_solo_generate() {
        let ev = PromptEvidence {
            user_goal: Some("Calculate fibonacci numbers".into()),
            ..Default::default()
        };
        let compiled = compile(PromptIntent::SoloGenerate, &ev);
        assert!(compiled.text.contains("fibonacci"));
        assert_eq!(compiled.provenance.intent, PromptIntent::SoloGenerate);
        assert!(compiled
            .provenance
            .evidence_sources
            .contains(&"solo_generate_template".to_string()));
    }

    #[test]
    fn test_compile_project_name_suggest() {
        let ev = PromptEvidence {
            user_goal: Some("Build a REST API for user management".into()),
            ..Default::default()
        };
        let compiled = compile(PromptIntent::ProjectNameSuggest, &ev);
        assert!(compiled.text.contains("REST API for user management"));
        assert_eq!(compiled.provenance.intent, PromptIntent::ProjectNameSuggest);
    }

    #[test]
    fn test_compile_correction_with_files() {
        let ev = PromptEvidence {
            node_goal: Some("Implement calculator".into()),
            verifier_diagnostics: Some("error[E0308]: mismatched types".into()),
            existing_file_contents: vec![(
                "src/calc.rs".into(),
                "fn add(a: i32) -> i32 { a }".into(),
            )],
            plugin_correction_fragment: Some("Use cargo check for Rust".into()),
            ..Default::default()
        };
        let compiled = compile(PromptIntent::CorrectionRetry, &ev);
        assert!(compiled.text.contains("calculator"));
        assert!(compiled.text.contains("mismatched types"));
        assert!(compiled.text.contains("src/calc.rs"));
        assert!(compiled.text.contains("Plugin Guidance"));
        assert!(compiled
            .provenance
            .evidence_sources
            .contains(&"verifier_diagnostics".to_string()));
        assert!(compiled
            .provenance
            .evidence_sources
            .contains(&"plugin_correction_fragment".to_string()));
    }

    #[test]
    fn test_compile_bundle_retarget() {
        let ev = PromptEvidence {
            node_goal: Some("Build HTTP server".into()),
            output_files: vec!["src/server.rs".into(), "src/main.rs".into()],
            rejected_bundle_summary: Some("config.json, README.md".into()),
            ..Default::default()
        };
        let compiled = compile(PromptIntent::BundleRetarget, &ev);
        assert!(compiled.text.contains("src/server.rs"));
        assert!(compiled.text.contains("config.json"));
        assert!(compiled.text.contains("REJECTED"));
    }

    #[test]
    fn test_compile_speculator_basic() {
        let ev = PromptEvidence {
            node_goal: Some("Parse JSON config".into()),
            ..Default::default()
        };
        let compiled = compile(PromptIntent::SpeculatorBasic, &ev);
        assert!(compiled.text.contains("Parse JSON config"));
    }

    #[test]
    fn test_compile_architect_existing() {
        let ev = PromptEvidence {
            user_goal: Some("Add logging module".into()),
            project_summary: Some("Rust workspace with 3 crates".into()),
            working_dir: Some("/tmp/project".into()),
            active_plugins: vec!["rust".into()],
            ..Default::default()
        };
        let compiled = compile(PromptIntent::ArchitectExisting, &ev);
        assert!(compiled.text.contains("logging module"));
        assert!(compiled
            .provenance
            .evidence_sources
            .contains(&"project_summary".to_string()));
    }

    #[test]
    fn test_compile_actuator_multi() {
        let ev = PromptEvidence {
            node_goal: Some("Implement auth module".into()),
            output_files: vec!["src/auth.rs".into(), "tests/test_auth.rs".into()],
            ..Default::default()
        };
        let compiled = compile(PromptIntent::ActuatorMultiOutput, &ev);
        assert!(compiled.text.contains("auth module"));
        assert!(compiled.text.contains("Multi-Artifact Bundle"));
    }

    #[test]
    fn test_compile_actuator_single() {
        let ev = PromptEvidence {
            node_goal: Some("Implement utils".into()),
            output_files: vec!["src/utils.py".into()],
            ..Default::default()
        };
        let compiled = compile(PromptIntent::ActuatorSingleOutput, &ev);
        assert!(compiled.text.contains("utils"));
        assert!(!compiled.text.contains("Multi-Artifact Bundle"));
    }

    #[test]
    fn test_compile_solo_correction() {
        let ev = PromptEvidence {
            user_goal: Some("Sort a list".into()),
            solo_file_path: Some("sort_list.py".into()),
            existing_file_contents: vec![("sort_list.py".into(), "def sort(l): pass".into())],
            verifier_diagnostics: Some("NameError: name 'x' is not defined".into()),
            ..Default::default()
        };
        let compiled = compile(PromptIntent::SoloCorrect, &ev);
        assert!(compiled.text.contains("sort_list.py"));
        assert!(compiled.text.contains("NameError"));
    }

    #[test]
    fn test_provenance_records_timestamp() {
        let ev = PromptEvidence::default();
        let compiled = compile(PromptIntent::SpeculatorBasic, &ev);
        assert!(compiled.provenance.compiled_at > 0);
    }

    #[test]
    fn test_correction_with_previous_attempts() {
        let ev = PromptEvidence {
            node_goal: Some("Fix parser".into()),
            previous_attempts: vec![perspt_core::types::CorrectionAttemptRecord {
                attempt: 1,
                parse_state: perspt_core::types::ParseResultState::NoStructuredPayload,
                retry_classification: None,
                response_fingerprint: "abc123".into(),
                response_length: 100,
                energy_after: None,
                accepted: false,
                rejection_reason: Some("Failed to parse".into()),
                created_at: 0,
            }],
            ..Default::default()
        };
        let compiled = compile(PromptIntent::CorrectionRetry, &ev);
        assert!(compiled.text.contains("Previous Correction Attempts: 1"));
    }
}
