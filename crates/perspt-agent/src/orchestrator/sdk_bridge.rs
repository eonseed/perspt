//! Bridge from the live orchestrator to the PSP-8 `perspt-sdk` control plane.
//!
//! This wires the SDK into the running agent without replacing the existing
//! `StabilityMonitor`: on each convergence step it translates the concrete
//! [`VerificationResult`] into SDK [`ResidualEvent`]s, scores them with the
//! `perspt-coding` domain energy model (the canonical `V = sum_e w_e r_e^2`),
//! and evaluates the measured acceptance gate against the best accepted energy.
//! The result is surfaced as telemetry so a real coding run exercises — and
//! shows — the SDK energy, gate decision, and finite-decision bound.

use std::collections::{BTreeSet, HashMap};

use perspt_coding::lang::adapter_for;
use perspt_coding::{defined_symbols, expected_symbols, CodingDomain, CodingLanguage};
use perspt_core::types::{SensorStatus, VerificationResult};
use perspt_sdk::{
    self as sdk, goal_presence_residual, score_candidate, AgentDomainPackage, DomainScope,
    EnergyModel, GateDecision, GoalSpec, IndependenceRoute, ResidualClass, ResidualEvent,
    ResidualSeverity, SensorRef,
};

/// Per-node accepted-energy tracking plus the shared coding energy model.
#[derive(Debug)]
pub struct SdkGateState {
    model: EnergyModel,
    /// node_id -> best accepted energy V(x_best).
    best_accepted: HashMap<String, f64>,
    /// node_id -> baseline energy V_0 (first observation), for the bound.
    baseline: HashMap<String, f64>,
}

/// A single SDK gate evaluation, surfaced as telemetry.
#[derive(Debug, Clone)]
pub struct SdkGateReport {
    pub energy: f64,
    pub v_syn: f64,
    pub v_str: f64,
    pub v_log: f64,
    pub v_boot: f64,
    pub v_sheaf: f64,
    pub hard_pass: bool,
    pub decision: GateDecision,
    pub rho_gate: f64,
    pub decision_bound: u64,
    pub residual_count: usize,
}

impl SdkGateReport {
    /// A compact, human-readable telemetry line.
    pub fn summary(&self) -> String {
        let decision = match &self.decision {
            GateDecision::HardPass => "hard-pass".to_string(),
            GateDecision::AcceptedByDescent { delta_v } => format!("descent (Δ={delta_v:.3})"),
            GateDecision::RejectedNonDescending { delta_v } => {
                format!("rejected (Δ={delta_v:.3})")
            }
            GateDecision::StoppedAtDeclaredFloor => "stopped-at-floor".to_string(),
            GateDecision::ExhaustedWithCertificate { .. } => "exhausted".to_string(),
        };
        format!(
            "SDK V=Σwₑrₑ²={:.3} [syn {:.2}|str {:.2}|log {:.2}|boot {:.2}|sheaf {:.2}] gate={} ρ={} bound≤{} ({} residuals)",
            self.energy,
            self.v_syn,
            self.v_str,
            self.v_log,
            self.v_boot,
            self.v_sheaf,
            decision,
            self.rho_gate,
            self.decision_bound,
            self.residual_count,
        )
    }
}

/// Outcome of the PSP-8 goal-presence sensor: which required symbols are absent
/// from the node's delivered work, and the blocking residual that records it.
#[derive(Debug, Clone)]
pub struct GoalPresenceReport {
    /// Symbol names the node was required to define but did not.
    pub missing: Vec<String>,
    /// The blocking `SymbolMismatch` residual for the absence.
    pub residual: ResidualEvent,
}

impl GoalPresenceReport {
    /// A compact, human-readable telemetry line.
    pub fn summary(&self) -> String {
        format!(
            "goal-presence FAIL: {} required symbol(s) absent: {}",
            self.missing.len(),
            self.missing.join(", ")
        )
    }
}

/// Run the PSP-8 goal-presence sensor for a coding node.
///
/// Extracts the symbols the node is required to produce (from its contract's
/// interface signature and its goal text) and the symbols actually defined in
/// the delivered `sources`, then asks the SDK sensor whether any required symbol
/// is missing. Returns `None` when the goal declares no checkable symbols or is
/// satisfied — the sensor never invents an obligation, so a node whose success
/// cannot be expressed as named symbols is left to the other verifiers.
///
/// This is the verifier that refuses *false stability*: an empty or placeholder
/// file compiles with `V = 0`, but if the requested symbol is absent the sensor
/// emits a blocking residual so the node cannot be accepted.
pub fn goal_presence_check(
    node_id: &str,
    generation: u32,
    interface_signature: &str,
    goal: &str,
    sources: &[String],
) -> sdk::Result<Option<GoalPresenceReport>> {
    let expected = expected_symbols(interface_signature, goal);
    if expected.is_empty() {
        return Ok(None);
    }
    let spec = GoalSpec::new(node_id, expected);

    let mut observed: BTreeSet<String> = BTreeSet::new();
    for source in sources {
        observed.extend(defined_symbols(source));
    }

    match goal_presence_residual(&spec, generation, &observed)? {
        Some(residual) => {
            let missing = residual
                .affected_symbols
                .iter()
                .map(|s| s.name.clone())
                .collect();
            Ok(Some(GoalPresenceReport { missing, residual }))
        }
        None => Ok(None),
    }
}

/// Map an orchestrator plugin name to a `perspt-coding` language adapter.
pub fn coding_language_for(owner_plugin: &str) -> Option<CodingLanguage> {
    match owner_plugin.to_ascii_lowercase().as_str() {
        "rust" => Some(CodingLanguage::Rust),
        "python" => Some(CodingLanguage::Python),
        "typescript" | "javascript" | "ts" | "js" => Some(CodingLanguage::TypeScript),
        _ => None,
    }
}

/// SRBN residual-directed corrections (PSP-8 / Paper II): parse raw verifier
/// output for the node's language into typed residuals and return the dominant,
/// *directed* correction instructions — one per residual class, in first-seen
/// order, capped. Returns an empty vec for unknown languages or when no residual
/// maps to a direction, so callers can treat it as additive enrichment.
pub fn directed_corrections(
    owner_plugin: &str,
    node_id: &str,
    raw_output: &str,
) -> Vec<(ResidualClass, String)> {
    let Some(lang) = coding_language_for(owner_plugin) else {
        return Vec::new();
    };
    if raw_output.trim().is_empty() {
        return Vec::new();
    }
    let adapter = adapter_for(lang);
    let mut residuals = adapter.parse_diagnostics(node_id, 0, raw_output);

    // Runtime crashes (panics/tracebacks) are not compiler/test diagnostics, so
    // parse_diagnostics misses them — detect them here so a runtime smoke failure
    // also yields a directed Runtime fix.
    if let Some(line) = perspt_coding::crash_marker(raw_output) {
        if let Some(r) = perspt_coding::runtime::runtime_residual(node_id, 0, line) {
            residuals.push(r);
        }
    } else if let Some(tok) = perspt_coding::runtime::numeric_anomaly(raw_output) {
        if let Some(r) = perspt_coding::runtime::runtime_residual(
            node_id,
            0,
            format!("numeric anomaly ({tok}) in output"),
        ) {
            residuals.push(r);
        }
    }

    let mut seen: BTreeSet<ResidualClass> = BTreeSet::new();
    let mut out: Vec<(ResidualClass, String)> = Vec::new();
    for residual in &residuals {
        if !seen.insert(residual.class) {
            continue; // one direction per dominant class
        }
        if let Some(direction) = adapter.correction_for(residual) {
            out.push((residual.class, direction.instruction));
        }
        if out.len() >= 5 {
            break;
        }
    }
    out
}

impl Default for SdkGateState {
    fn default() -> Self {
        Self::new()
    }
}

impl SdkGateState {
    pub fn new() -> Self {
        let domain = CodingDomain::new();
        let model = domain.energy_model(&DomainScope::default());
        Self {
            model,
            best_accepted: HashMap::new(),
            baseline: HashMap::new(),
        }
    }

    /// Record a blocking goal-presence residual on the SDK gate channel.
    ///
    /// The orchestrator's `V_str` penalty is what enforces non-convergence; this
    /// surfaces the canonical PSP-8 residual (class, score, component, affected
    /// symbols) so the measured-gate telemetry reflects exactly why the node is
    /// not stable.
    pub fn record_goal_residual(&self, residual: ResidualEvent) {
        log::info!(
            target: "perspt::sdk_gate",
            "SDK goal-presence residual: node={} class={:?} component={:?} score={} symbols=[{}]",
            residual.node_id,
            residual.class,
            residual.component,
            residual.score,
            residual
                .affected_symbols
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        );
    }

    /// Translate a concrete verification result into SDK residual events.
    pub fn residuals_from(
        node_id: &str,
        generation: u32,
        vr: &VerificationResult,
    ) -> Vec<ResidualEvent> {
        let mut residuals = Vec::new();

        let push = |residuals: &mut Vec<ResidualEvent>,
                    class: ResidualClass,
                    severity: ResidualSeverity,
                    score: f64,
                    sensor: SensorRef| {
            if let Ok(r) = ResidualEvent::new(node_id, generation, class, severity, score, sensor) {
                residuals.push(r);
            }
        };

        // Compiler / LSP diagnostics -> Type residual, magnitude = diagnostic count.
        if vr.diagnostics_count > 0 {
            push(
                &mut residuals,
                ResidualClass::Type,
                ResidualSeverity::Error,
                vr.diagnostics_count as f64,
                SensorRef::new("lsp", IndependenceRoute::Lsp),
            );
        }
        // Build failure -> Build residual.
        if !vr.build_ok {
            push(
                &mut residuals,
                ResidualClass::Build,
                ResidualSeverity::Blocking,
                1.0,
                SensorRef::new("build", IndependenceRoute::Compiler),
            );
        }
        // Failing tests -> TestFailure residual, magnitude = failed count.
        if vr.tests_failed > 0 {
            push(
                &mut residuals,
                ResidualClass::TestFailure,
                ResidualSeverity::Error,
                vr.tests_failed as f64,
                SensorRef::new("test", IndependenceRoute::TestOracle),
            );
        }
        // Lint failure -> Lint residual.
        if !vr.lint_ok {
            push(
                &mut residuals,
                ResidualClass::Lint,
                ResidualSeverity::Warning,
                1.0,
                SensorRef::new("lint", IndependenceRoute::DeterministicTool),
            );
        }
        // Degraded / unavailable sensors -> Boot residuals (energy unknown, not zero).
        for stage in &vr.stage_outcomes {
            if !matches!(stage.sensor_status, SensorStatus::Available) {
                push(
                    &mut residuals,
                    ResidualClass::SensorUnavailable,
                    ResidualSeverity::Blocking,
                    1.0,
                    SensorRef::new(
                        format!("stage:{}", stage.stage),
                        IndependenceRoute::DeterministicTool,
                    ),
                );
            }
        }

        residuals
    }

    /// Evaluate the SDK measured gate for a convergence step.
    ///
    /// When a plugin [`VerificationResult`] is available it is mapped into SDK
    /// residuals and scored with the canonical quadratic energy. Otherwise the
    /// gate is driven directly from the orchestrator's own energy components,
    /// which are always available at the convergence step — so the SDK measured
    /// acceptance gate runs on every live correction step regardless of the
    /// verification configuration.
    pub fn evaluate(
        &mut self,
        node_id: &str,
        generation: u32,
        vr: Option<&VerificationResult>,
        fallback_components: &perspt_core::types::EnergyComponents,
        fallback_total: f64,
        fallback_hard_pass: bool,
    ) -> sdk::Result<SdkGateReport> {
        if let Some(vr) = vr {
            let residuals = Self::residuals_from(node_id, generation, vr);
            let score = score_candidate(&self.model, &residuals)?;
            return self.gate(
                node_id,
                score.total,
                vr.all_passed(),
                [
                    score.components.v_syn,
                    score.components.v_str,
                    score.components.v_log,
                    score.components.v_boot,
                    score.components.v_sheaf,
                ],
                residuals.len(),
            );
        }

        // Fallback: drive the SDK measured gate on the orchestrator's energy.
        let c = fallback_components;
        let comps = [
            c.v_syn as f64,
            c.v_str as f64,
            c.v_log as f64,
            c.v_boot as f64,
            c.v_sheaf as f64,
        ];
        let nonzero = comps.iter().filter(|x| **x > 0.0).count();
        self.gate(node_id, fallback_total, fallback_hard_pass, comps, nonzero)
    }

    /// Run the measured acceptance gate against the per-node best accepted energy
    /// and build the telemetry report.
    fn gate(
        &mut self,
        node_id: &str,
        total: f64,
        hard_pass: bool,
        comps: [f64; 5],
        residual_count: usize,
    ) -> sdk::Result<SdkGateReport> {
        let baseline = *self.baseline.entry(node_id.to_string()).or_insert(total);
        let best = *self.best_accepted.entry(node_id.to_string()).or_insert(total);

        let decision = sdk::evaluate_gate(hard_pass, total, best, self.model.rho_gate)?;
        if decision.is_accepted() && total < best {
            self.best_accepted.insert(node_id.to_string(), total);
        }

        let decision_bound =
            sdk::finite_decision_bound(baseline, self.model.rho_gate, self.model.correction_budget)?;

        Ok(SdkGateReport {
            energy: total,
            v_syn: comps[0],
            v_str: comps[1],
            v_log: comps[2],
            v_boot: comps[3],
            v_sheaf: comps[4],
            hard_pass,
            decision,
            rho_gate: self.model.rho_gate,
            decision_bound,
            residual_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perspt_core::types::VerificationResult;

    fn clean() -> VerificationResult {
        VerificationResult {
            syntax_ok: true,
            build_ok: true,
            tests_ok: true,
            lint_ok: true,
            diagnostics_count: 0,
            tests_passed: 3,
            tests_failed: 0,
            summary: String::new(),
            raw_output: None,
            degraded: false,
            degraded_reason: None,
            stage_outcomes: vec![],
        }
    }

    fn comps() -> perspt_core::types::EnergyComponents {
        perspt_core::types::EnergyComponents::default()
    }

    #[test]
    fn clean_result_is_hard_pass_zero_energy() {
        let mut state = SdkGateState::new();
        let report = state.evaluate("n1", 0, Some(&clean()), &comps(), 0.0, true).unwrap();
        assert_eq!(report.energy, 0.0);
        assert!(report.hard_pass);
        assert!(matches!(report.decision, GateDecision::HardPass));
    }

    #[test]
    fn fallback_energy_path_runs_without_verification_result() {
        // No VerificationResult -> the gate is driven from energy components.
        let mut state = SdkGateState::new();
        let mut c = comps();
        c.v_syn = 9.0;
        let report = state.evaluate("nX", 0, None, &c, 9.0, false).unwrap();
        assert_eq!(report.energy, 9.0);
        assert_eq!(report.v_syn, 9.0);
        assert!(!report.hard_pass);
    }

    #[test]
    fn diagnostics_and_test_failures_produce_energy() {
        let mut vr = clean();
        vr.syntax_ok = false;
        vr.diagnostics_count = 2;
        vr.tests_ok = false;
        vr.tests_failed = 1;
        let residuals = SdkGateState::residuals_from("n1", 0, &vr);
        // One Type residual (2 diagnostics) + one TestFailure residual (1).
        assert_eq!(residuals.len(), 2);
        let mut state = SdkGateState::new();
        let report = state.evaluate("n1", 0, Some(&vr), &comps(), 0.0, false).unwrap();
        // Type weight 3.0 * 2^2 = 12 (V_syn) + TestFailure 2.0 * 1^2 = 2 (V_log) = 14.
        assert_eq!(report.v_syn, 12.0);
        assert_eq!(report.v_log, 2.0);
        assert_eq!(report.energy, 14.0);
        assert!(!report.hard_pass);
    }

    #[test]
    fn descent_is_detected_across_steps() {
        let mut state = SdkGateState::new();
        let mut vr = clean();
        vr.syntax_ok = false;
        vr.diagnostics_count = 3; // V_syn = 3*9 = 27
        let first = state.evaluate("n1", 0, Some(&vr), &comps(), 0.0, false).unwrap();
        assert!(matches!(first.decision, GateDecision::RejectedNonDescending { .. }));
        // Fewer diagnostics next attempt -> energy descends -> accepted.
        vr.diagnostics_count = 1; // V_syn = 3*1 = 3
        let second = state.evaluate("n1", 1, Some(&vr), &comps(), 0.0, false).unwrap();
        assert!(matches!(second.decision, GateDecision::AcceptedByDescent { .. }));
    }

    #[test]
    fn degraded_sensor_is_not_zero_energy() {
        let mut vr = clean();
        vr.stage_outcomes = vec![perspt_core::types::StageOutcome {
            stage: "tests".to_string(),
            passed: false,
            sensor_status: SensorStatus::Unavailable { reason: "pytest missing".into() },
            output: None,
        }];
        let residuals = SdkGateState::residuals_from("n1", 0, &vr);
        assert!(residuals.iter().any(|r| r.class == ResidualClass::SensorUnavailable));
    }

    #[test]
    fn goal_presence_flags_unwritten_function() {
        // Placeholder file, goal asks for `is_even` — the no-op false-stability case.
        let report = goal_presence_check(
            "n1",
            0,
            "pub fn is_even(n: i32) -> bool",
            "Add is_even(n) returning true for even n.",
            &["// implement here\n".to_string()],
        )
        .unwrap()
        .expect("missing symbol must be flagged");
        assert_eq!(report.missing, vec!["is_even"]);
        assert_eq!(report.residual.class, ResidualClass::SymbolMismatch);
        assert_eq!(report.residual.severity, ResidualSeverity::Blocking);
    }

    #[test]
    fn goal_presence_passes_when_symbol_defined() {
        let report = goal_presence_check(
            "n1",
            0,
            "pub fn multiply(a: i32, b: i32) -> i32",
            "Add `multiply`.",
            &["pub fn multiply(a: i32, b: i32) -> i32 { a * b }\n".to_string()],
        )
        .unwrap();
        assert!(report.is_none());
    }

    #[test]
    fn directed_corrections_rust_import_and_symbol() {
        let raw = "error[E0432]: unresolved import `crate::foo::Bar`\n\
                   error[E0425]: cannot find value `baz` in this scope";
        let dirs = directed_corrections("rust", "n1", raw);
        assert!(!dirs.is_empty());
        assert!(dirs.iter().any(|(c, _)| *c == ResidualClass::ImportGraph));
        assert!(dirs.iter().any(|(c, _)| *c == ResidualClass::SymbolMismatch));
        // The directions carry specific, actionable instructions.
        assert!(dirs.iter().any(|(_, i)| i.contains("use") || i.contains("import")));
    }

    #[test]
    fn directed_corrections_python_failure() {
        let raw = "test_x.py::test_add FAILED\nE   ModuleNotFoundError: No module named 'requests'";
        let dirs = directed_corrections("python", "n1", raw);
        assert!(!dirs.is_empty());
    }

    #[test]
    fn directed_corrections_detects_runtime_crash() {
        // A panic in the (runtime-smoke) output must yield a Runtime fix even
        // though it is not a compiler/test diagnostic.
        let raw = "Running `target/debug/cli predict`\nthread 'main' panicked at src/main.rs:42:9:\nInput tensor size does not match model weights";
        let dirs = directed_corrections("rust", "n1", raw);
        assert!(dirs.iter().any(|(c, _)| *c == ResidualClass::Runtime));
        assert!(dirs.iter().any(|(_, i)| i.contains("runtime")));
    }

    #[test]
    fn directed_corrections_unknown_language_is_empty() {
        assert!(directed_corrections("haskell", "n1", "some error").is_empty());
        // Empty input → no directions even for a known language.
        assert!(directed_corrections("rust", "n1", "   ").is_empty());
    }

    #[test]
    fn goal_presence_silent_without_declared_symbols() {
        // A prose-only goal with no contract signature declares no obligation.
        let report =
            goal_presence_check("n1", 0, "", "Improve the documentation.", &["".to_string()])
                .unwrap();
        assert!(report.is_none());
    }
}
