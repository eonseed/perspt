//! PSP-7 §5: Typed prompt compiler.
//!
//! Maps `(PromptIntent, PromptEvidence) → CompiledPrompt` with full provenance
//! tracking. Each prompt family delegates to the template constants in
//! `crate::prompts` but wraps the result in a `CompiledPrompt` that records
//! which intent, plugin fragments, and evidence sources contributed.

use perspt_core::types::{CompiledPrompt, PromptEvidence, PromptIntent, PromptProvenance};

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
    let error_feedback = ev.verifier_diagnostics.as_deref().unwrap_or("");

    // Evidence section is empty for greenfield, populated for existing.
    let evidence_section = ev
        .existing_file_contents
        .iter()
        .map(|(path, _)| path.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    let active_plugins: Vec<String> = ev.legal_support_files.clone();

    crate::prompts::render_architect(
        template,
        task,
        std::path::Path::new(ev.context_files.first().map(|s| s.as_str()).unwrap_or(".")),
        project_context,
        error_feedback,
        &evidence_section,
        &active_plugins,
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

    crate::prompts::render_actuator(
        goal,
        "",  // interface — caller should set via contract
        "",  // invariants
        "",  // forbidden
        ".", // working_dir
        &context_files,
        target_file,
        &allowed_output_paths,
        "", // workspace_import_hints
        is_multi,
    )
}

fn compile_verifier(ev: &PromptEvidence) -> String {
    let implementation = ev
        .existing_file_contents
        .first()
        .map(|(_, content)| content.as_str())
        .unwrap_or("");

    crate::prompts::render_verifier("", "", "", "", implementation)
}

fn compile_correction(ev: &PromptEvidence) -> String {
    let goal = ev.node_goal.as_deref().unwrap_or("");
    let diagnostics = ev
        .verifier_diagnostics
        .as_deref()
        .unwrap_or("No specific errors captured.");

    let mut prompt = format!(
        "## Code Correction Required\n\n\
         Your task is to fix ALL errors and return the complete corrected file(s).\n\n\
         ### Original Goal\n{}\n\n\
         ### Current Code (with errors)\n",
        goal,
    );

    for (path, content) in &ev.existing_file_contents {
        let lang = std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|ext| match ext {
                "py" => "python",
                "rs" => "rust",
                "ts" | "tsx" => "typescript",
                "js" | "jsx" => "javascript",
                other => other,
            })
            .unwrap_or("text");
        prompt.push_str(&format!(
            "File: {}\n```{}\n{}\n```\n\n",
            path, lang, content
        ));
    }

    prompt.push_str(&format!("### Detected Errors\n{}\n", diagnostics));

    if let Some(ref fragment) = ev.plugin_correction_fragment {
        prompt.push_str(&format!("\n### Plugin Guidance\n{}\n", fragment));
    }

    if !ev.previous_attempts.is_empty() {
        prompt.push_str(&format!(
            "\n### Previous Correction Attempts: {}\n\
             The previous attempts did not fully resolve the errors. \
             Please try a different approach.\n",
            ev.previous_attempts.len()
        ));
    }

    prompt
}

fn compile_bundle_retarget(ev: &PromptEvidence) -> String {
    let expected = ev.output_files.join(", ");
    let dropped = ev.rejected_bundle_summary.as_deref().unwrap_or("(unknown)");
    let original_prompt = ev.node_goal.as_deref().unwrap_or("");

    crate::prompts::render_bundle_retarget(&expected, dropped, original_prompt)
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

    crate::prompts::render_speculator_lookahead(node_id, goal, &downstream)
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

    crate::prompts::render_solo_correction(
        task,
        filename,
        current_code,
        "0.00", // v_syn — caller provides via diagnostics text
        "0.00", // v_log
        "0.00", // v_boot
        error_list,
    )
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
            context_files: vec!["/tmp/project".into()],
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
