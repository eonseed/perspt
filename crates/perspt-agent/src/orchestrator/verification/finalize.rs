use super::*;

impl SRBNOrchestrator {
    /// PSP-5 Phase 9: Finalize verification result — mark degraded, emit events, build summary.
    pub(crate) fn finalize_verification_result(
        &mut self,
        result: &mut perspt_core::types::VerificationResult,
        plugin_name: &str,
    ) {
        if result.has_degraded_stages() {
            result.degraded = true;
            let reasons = result.degraded_stage_reasons();
            result.degraded_reason = Some(reasons.join("; "));

            // Emit per-stage SensorFallback events
            for outcome in &result.stage_outcomes {
                if let perspt_core::types::SensorStatus::Fallback { actual, reason } =
                    &outcome.sensor_status
                {
                    self.emit_event(perspt_core::AgentEvent::SensorFallback {
                        node_id: plugin_name.to_string(),
                        stage: outcome.stage.clone(),
                        primary: reason.clone(),
                        actual: actual.clone(),
                        reason: reason.clone(),
                    });
                }
            }
        }

        // Store result for convergence-time degraded check
        self.last_verification_result = Some(result.clone());

        // Build summary
        result.summary = format!(
            "{}: syntax={}, build={}, tests={}, lint={}{}",
            plugin_name,
            if result.syntax_ok { "✅" } else { "❌" },
            if result.build_ok { "✅" } else { "❌" },
            if result.tests_ok { "✅" } else { "❌" },
            if result.lint_ok { "✅" } else { "⏭️" },
            if result.degraded { " (degraded)" } else { "" },
        );
    }
}

/// Convert diagnostic severity to string
/// Whether a path is a code source file the goal-presence symbol sensor
/// understands (Rust / Python / TypeScript / JavaScript). Manifests, configs,
/// data, and docs are excluded so a scaffold/manifest node carries no
/// code-symbol obligation.
pub(crate) fn is_code_source_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("rs" | "py" | "pyi" | "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs")
    )
}

pub(crate) fn severity_to_str(severity: Option<lsp_types::DiagnosticSeverity>) -> &'static str {
    match severity {
        Some(lsp_types::DiagnosticSeverity::ERROR) => "ERROR",
        Some(lsp_types::DiagnosticSeverity::WARNING) => "WARNING",
        Some(lsp_types::DiagnosticSeverity::INFORMATION) => "INFO",
        Some(lsp_types::DiagnosticSeverity::HINT) => "HINT",
        Some(_) => "OTHER",
        None => "UNKNOWN",
    }
}

/// PSP-5 Phase 9: Determine which verification stages to run based on NodeClass.
///
/// - **Interface**: SyntaxCheck only (signatures/schemas)
/// - **Implementation**: SyntaxCheck + Build (+ Test if weighted_tests non-empty
///   OR output targets include test files)
/// - **Integration**: Full pipeline (SyntaxCheck + Build + Test + Lint)
pub(crate) fn verification_stages_for_node(
    node: &SRBNNode,
) -> Vec<perspt_core::plugin::VerifierStage> {
    use perspt_core::plugin::VerifierStage;
    match node.node_class {
        perspt_core::types::NodeClass::Interface => {
            vec![VerifierStage::SyntaxCheck]
        }
        perspt_core::types::NodeClass::Implementation => {
            let mut stages = vec![VerifierStage::SyntaxCheck, VerifierStage::Build];
            // Include Test stage if the node has weighted tests OR if the
            // node's output targets include test files.  Without this, nodes
            // that produce test files (tests/*.rs, test_*.py, *.test.ts, etc.)
            // only get SyntaxCheck+Build which don't compile/run test targets.
            let has_test_outputs = node.output_targets.iter().any(|p| {
                let s = p.to_string_lossy();
                // Check the filename (last component) for test patterns rather
                // than the full path, to avoid false positives from directory
                // names like "test_seismic/" matching "/test_".
                let filename = p
                    .file_name()
                    .map(|f| f.to_string_lossy())
                    .unwrap_or_default();
                s.contains("/tests/")
                    || filename.starts_with("test_")
                    || filename.contains(".test.")
                    || filename.contains(".spec.")
                    || filename.ends_with("_test.rs")
                    || filename.ends_with("_test.py")
                    || filename.ends_with("_tests.rs")
                    || filename.ends_with("_tests.py")
            });
            if !node.contract.weighted_tests.is_empty() || has_test_outputs {
                stages.push(VerifierStage::Test);
            }
            stages
        }
        perspt_core::types::NodeClass::Integration => {
            vec![
                VerifierStage::SyntaxCheck,
                VerifierStage::Build,
                VerifierStage::Test,
                VerifierStage::Lint,
            ]
        }
    }
}
