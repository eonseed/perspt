//! Plugin-backed candidate measurement: the full verifier suite at gate
//! boundaries and the cheap syntax-only pass at mutation boundaries.
//! Extracted from `candidate.rs` so the reversible overlay and the
//! measurement plane each stay within the file-size rules.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use perspt_coding::{CodingAdapterRegistry, CodingDomain, LanguageId};
use perspt_sdk::{
    score_candidate, AgentDomainPackage, CorrectionDirection, DomainScope, IndependenceRoute,
    ResidualClass, ResidualEvent, ResidualSeverity, SensorRef,
};

use crate::candidate::{CandidateWorkspace, TestEvidenceWorkspace};
use crate::toolloop::{CandidateMeasurer, Measured};
use crate::verifier::{run_governed_verifier, VerifierExecution, VerifierJob};

/// Every verifier process of one candidate shares this target-dir suffix:
/// gate stages, interactive `run_*` tools, and the incremental syntax pass
/// warm one another instead of each paying a cold build.
pub(crate) const SHARED_TARGET_SUFFIX: &str = "shared";

#[derive(Default)]
struct AdditionalTestEvidence<'a> {
    regression_root: Option<&'a Path>,
    external_oracle: Option<(&'a Path, &'a str)>,
}

#[derive(Default)]
struct PreparedTestEvidence {
    regression: Option<TestEvidenceWorkspace>,
    external: Option<(TestEvidenceWorkspace, String)>,
}

impl PreparedTestEvidence {
    fn borrowed(&self) -> AdditionalTestEvidence<'_> {
        AdditionalTestEvidence {
            regression_root: self.regression.as_ref().map(|view| view.root.as_path()),
            external_oracle: self
                .external
                .as_ref()
                .map(|(view, command)| (view.root.as_path(), command.as_str())),
        }
    }
}

/// Execution order within one shared target dir: the cheap check warms the
/// metadata, the build warms the full artifacts, tests/lints reuse them.
fn stage_rank(stage: perspt_core::plugin::VerifierStage) -> u8 {
    use perspt_core::plugin::VerifierStage;
    match stage {
        VerifierStage::SyntaxCheck => 0,
        VerifierStage::Build => 1,
        VerifierStage::Test => 2,
        VerifierStage::Lint => 3,
        VerifierStage::Format => 4,
    }
}

/// Complete plugin-backed candidate measurement for one coding node.
pub struct CodingCandidateMeasurer<'a> {
    candidate: &'a CandidateWorkspace,
    node_id: String,
    generation: u32,
    domain: Arc<dyn AgentDomainPackage>,
    adapters: CodingAdapterRegistry,
    max_parallel: usize,
    require_format: bool,
    test_policy: perspt_core::TestPolicy,
    external_oracle: Option<perspt_core::ExternalOracleConfig>,
    correction_packets: bool,
}

impl<'a> CodingCandidateMeasurer<'a> {
    pub fn new(candidate: &'a CandidateWorkspace, node_id: &str, generation: u32) -> Self {
        Self {
            candidate,
            node_id: node_id.into(),
            generation,
            domain: Arc::new(CodingDomain::new()),
            adapters: CodingAdapterRegistry::with_builtins(),
            max_parallel: 4,
            require_format: false,
            test_policy: perspt_core::TestPolicy::Evolving,
            external_oracle: None,
            correction_packets: true,
        }
    }

    /// Evaluation ablation: disable typed correction packets (legacy
    /// first-direction text only).
    pub fn with_correction_packets(mut self, enabled: bool) -> Self {
        self.correction_packets = enabled;
        self
    }

    /// Enable the declared `format` acceptance stage
    /// (`[verification] require_format`).
    pub fn with_require_format(mut self, require_format: bool) -> Self {
        self.require_format = require_format;
        self
    }

    /// Select how test evidence participates in the hard gate. The default is
    /// `Evolving`: project tests and their configuration may change together
    /// with the implementation. Stronger policies are explicit.
    pub fn with_test_policy(
        mut self,
        policy: perspt_core::TestPolicy,
        external_oracle: Option<perspt_core::ExternalOracleConfig>,
    ) -> Self {
        self.test_policy = policy;
        self.external_oracle = external_oracle;
        self
    }

    pub fn with_max_parallel(mut self, max_parallel: usize) -> Self {
        self.max_parallel = max_parallel.max(1);
        self
    }

    /// Measure under the selected domain's energy model instead of the
    /// default coding domain.
    pub fn with_domain(mut self, domain: Arc<dyn AgentDomainPackage>) -> Self {
        self.domain = domain;
        self
    }

    fn adapter_for(plugin_name: &str) -> LanguageId {
        match plugin_name.to_ascii_lowercase().as_str() {
            "js" | "javascript" | "typescript" => LanguageId::new("typescript"),
            other => LanguageId::new(other),
        }
    }

    /// Enumerate every declared verifier capability as a runnable job; a
    /// capability with no effective command is a blocking sensor residual.
    /// A declared no-op stage *satisfies* its required-stage obligation —
    /// the plugin states there is nothing to run, which is a covered
    /// sensor, not a missing one (Python's build stage would otherwise
    /// make hard pass unreachable and loop the model forever).
    fn collect_jobs(
        &self,
        test_evidence: AdditionalTestEvidence<'_>,
        residuals: &mut Vec<ResidualEvent>,
        advisory: &mut Vec<ResidualEvent>,
        satisfied_stages: &mut BTreeSet<&'static str>,
        all_passed: &mut bool,
        required: &BTreeSet<String>,
    ) -> Result<Vec<VerifierJob>> {
        let registry = perspt_core::PluginRegistry::new();
        let mut jobs = Vec::new();
        let plugins = registry.detect_all(self.candidate.overlay_root());
        *all_passed = !plugins.is_empty();
        for plugin in plugins {
            let adapter_id = Self::adapter_for(plugin.name());
            for capability in plugin.verifier_profile().capabilities {
                // Formatting gates acceptance only when the run declares it
                // (`[verification] require_format`); otherwise the formatter
                // stays a governed tool, never a silent gate change.
                if capability.stage == perspt_core::plugin::VerifierStage::Format
                    && !self.require_format
                {
                    continue;
                }
                let Some(command) = capability.effective_command() else {
                    // A stage the plugin declares as a no-op — no primary or
                    // fallback command form at all, marked available — has
                    // nothing to run and is not a missing sensor (e.g.
                    // Python's build stage). Only a stage that *has* a
                    // command form with no runnable binary blocks hard pass.
                    if capability.available
                        && capability.command.is_none()
                        && capability.fallback_command.is_none()
                    {
                        satisfied_stages.insert(capability.stage.policy_name());
                        continue;
                    }
                    fold_infra_failure(
                        &self.node_id,
                        self.generation,
                        capability.stage,
                        &format!("{}:{}", plugin.name(), capability.stage),
                        residuals,
                        advisory,
                        all_passed,
                        required,
                    )?;
                    continue;
                };
                jobs.push(VerifierJob {
                    plugin: plugin.name().to_string(),
                    adapter_id: adapter_id.clone(),
                    stage: capability.stage,
                    command: command.to_string(),
                    root: self.candidate.overlay_root().to_path_buf(),
                });
            }
            if let Some(root) = test_evidence.regression_root {
                jobs.push(VerifierJob {
                    plugin: format!("{}:backward-compatible-tests", plugin.name()),
                    adapter_id,
                    stage: perspt_core::plugin::VerifierStage::Test,
                    command: plugin.test_command(),
                    root: root.to_path_buf(),
                });
            }
        }
        if let Some((root, command)) = test_evidence.external_oracle {
            jobs.push(VerifierJob {
                plugin: "external-oracle".into(),
                adapter_id: LanguageId::new("external-oracle"),
                stage: perspt_core::plugin::VerifierStage::Test,
                command: command.to_string(),
                root: root.to_path_buf(),
            });
        }
        Ok(jobs)
    }

    /// Fold one verifier run. A residual from a stage the domain's
    /// `HardGatePolicy` does not require (e.g. lint) lands in `advisory` —
    /// still scored, still steering corrections, never blocking the gate —
    /// so a pre-existing repository warning cannot make hard pass
    /// unreachable. Required-stage residuals stay blocking.
    fn fold_execution(
        &self,
        job: &VerifierJob,
        execution: VerifierExecution,
        residuals: &mut Vec<ResidualEvent>,
        advisory: &mut Vec<ResidualEvent>,
        all_passed: &mut bool,
        required: &BTreeSet<String>,
    ) -> Result<()> {
        let blocking = required.contains(job.stage.policy_name());
        let sink: &mut Vec<ResidualEvent> = if blocking { residuals } else { advisory };
        let mut output = execution.output.clone();
        // A stage that declared a JUnit report file (pytest `--junitxml`)
        // produced structured evidence on disk: fold it into the raw sensor
        // text so the structured parser sees it, then drop the file.
        if let Some(report) = junit_report_path(&job.command) {
            let path = job.root.join(report);
            if let Ok(xml) = std::fs::read_to_string(&path) {
                output.push('\n');
                output.push_str(&xml);
                let _ = std::fs::remove_file(&path);
            }
        }
        let execution = VerifierExecution {
            output,
            ..execution
        };
        if let Some(adapter) = self.adapters.get(&job.adapter_id) {
            sink.extend(adapter.parse_diagnostics(
                &self.node_id,
                self.generation,
                &execution.output,
            ));
        }
        if !execution.success {
            if blocking {
                *all_passed = false;
            }
            let class = match job.stage {
                perspt_core::plugin::VerifierStage::Test => ResidualClass::TestFailure,
                perspt_core::plugin::VerifierStage::Lint => ResidualClass::Lint,
                perspt_core::plugin::VerifierStage::Format => ResidualClass::Format,
                _ => ResidualClass::Build,
            };
            if !sink.iter().any(|r| r.class == class) {
                sink.push(tool_residual(
                    &self.node_id,
                    self.generation,
                    class,
                    &format!("{} failed: {}", job.stage, concise(&execution.output)),
                )?);
            }
        }
        Ok(())
    }

    /// One task per `(root, plugin)` group, its stages run **sequentially**
    /// in stage order against one shared `CARGO_TARGET_DIR` (suffix
    /// `shared`, the same warm pool the interactive tools and incremental
    /// syntax pass use), so a gate pays one cold build plus incrementals
    /// instead of a cold tree per stage. Groups still run in parallel under
    /// the `max_parallel` semaphore; any additional regression or protected
    /// acceptance root is its own group and stays isolated from candidate
    /// build artifacts.
    fn spawn_verifier_groups(
        &self,
        jobs: Vec<VerifierJob>,
    ) -> tokio::task::JoinSet<Vec<(VerifierJob, Result<VerifierExecution>)>> {
        let mut groups: std::collections::BTreeMap<(std::path::PathBuf, String), Vec<VerifierJob>> =
            std::collections::BTreeMap::new();
        for job in jobs {
            groups
                .entry((job.root.clone(), job.plugin.clone()))
                .or_default()
                .push(job);
        }
        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.max_parallel));
        let mut workers = tokio::task::JoinSet::new();
        for (_, mut group) in groups {
            group.sort_by_key(|job| stage_rank(job.stage));
            let semaphore = semaphore.clone();
            let allow_unisolated = self.candidate.unisolated_verifiers_allowed();
            let extra_env = self.candidate.verifier_env();
            let timeouts = self.candidate.verifier_timeouts();
            workers.spawn(async move {
                let _permit = semaphore.acquire_owned().await.expect("semaphore closed");
                let mut results = Vec::with_capacity(group.len());
                for job in group {
                    let execution = run_governed_verifier(
                        job.root.clone(),
                        job.command.clone(),
                        allow_unisolated,
                        SHARED_TARGET_SUFFIX.into(),
                        extra_env.clone(),
                        timeouts.for_stage(Some(job.stage)),
                    )
                    .await;
                    results.push((job, execution));
                }
                results
            });
        }
        workers
    }

    /// The stages whose residuals block hard pass: the domain's
    /// `HardGatePolicy`, plus `format` when the run declares it
    /// (`[verification] require_format`).
    fn required_stages(&self) -> BTreeSet<String> {
        let scope = DomainScope {
            label: self.node_id.clone(),
            paths: Vec::new(),
        };
        let mut required: BTreeSet<String> = self
            .domain
            .hard_gate_policy(&scope)
            .required_stages
            .into_iter()
            .collect();
        if self.require_format {
            required.insert("format".into());
        }
        required
    }

    fn prepare_test_evidence(&self) -> Result<PreparedTestEvidence> {
        anyhow::ensure!(
            self.test_policy == perspt_core::TestPolicy::ExternalOracle
                || self.external_oracle.is_none(),
            "protected external test configuration requires the external-oracle test policy"
        );
        let regression = match self.test_policy {
            perspt_core::TestPolicy::BackwardCompatible => {
                self.candidate.backward_compatibility_tests()?
            }
            _ => None,
        };
        let external = match self.test_policy {
            perspt_core::TestPolicy::ExternalOracle => {
                let configured = self
                    .external_oracle
                    .as_ref()
                    .context("external-oracle test policy has no protected oracle configuration")?;
                Some((
                    self.candidate.external_oracle_tests(&configured.path)?,
                    configured.command.clone(),
                ))
            }
            _ => None,
        };
        Ok(PreparedTestEvidence {
            regression,
            external,
        })
    }

    /// Gate X / PSP-10 Phase 1: `HardGatePolicy::required_stages` is
    /// authoritative. A declared stage that no plugin ran is a missing
    /// sensor: it records `SensorUnavailable` and forces `hard_pass = false`.
    /// The check only adds a necessary condition; it can never loosen the
    /// plugin-derived verdict.
    fn enforce_required_stages(
        &self,
        ran_stages: &BTreeSet<&'static str>,
        residuals: &mut Vec<ResidualEvent>,
        all_passed: &mut bool,
    ) -> Result<()> {
        let scope = DomainScope {
            label: self.node_id.clone(),
            paths: Vec::new(),
        };
        for stage in self.domain.hard_gate_policy(&scope).required_stages {
            if !ran_stages.contains(stage.as_str()) {
                residuals.push(sensor_unavailable(
                    &self.node_id,
                    self.generation,
                    &format!("required-stage:{stage}"),
                )?);
                *all_passed = false;
            }
        }
        Ok(())
    }

    #[allow(clippy::type_complexity)]
    fn score(
        &self,
        residuals: &[ResidualEvent],
    ) -> Result<(
        f64,
        Option<CorrectionDirection>,
        Option<perspt_sdk::CorrectionPacket>,
    )> {
        let scope = DomainScope {
            label: self.node_id.clone(),
            paths: Vec::new(),
        };
        let score = score_candidate(&self.domain.energy_model(&scope), residuals)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        // PSP-10 system 26: the typed packet folds every direction with
        // full paths/symbols/rationale; its rendering through the
        // `branch_correct` section program becomes the steering message.
        // Domains without packets keep the legacy first-direction path.
        let packet = self
            .correction_packets
            .then(|| self.domain.correction_packet(residuals))
            .flatten();
        let correction = match &packet {
            Some(packet) if !packet.is_empty() => {
                let rendered = perspt_coding::prompts::render_correction(packet)
                    .map_err(|e| anyhow::anyhow!("rendering correction packet: {e}"))?;
                Some(
                    CorrectionDirection::new(packet.dominant_cluster.class, rendered)
                        .with_paths(packet.affected.paths.clone()),
                )
            }
            _ => self
                .domain
                .correction_directions(residuals)
                .into_iter()
                .next(),
        };
        Ok((score.total, correction, packet))
    }
}

#[async_trait::async_trait]
impl CandidateMeasurer for CodingCandidateMeasurer<'_> {
    async fn measure(&self) -> Result<Measured> {
        let mut residuals = Vec::new();
        let mut advisory = Vec::new();
        let required = self.required_stages();
        let mut all_passed = false;
        let mut ran_stages: BTreeSet<&'static str> = BTreeSet::new();
        let test_evidence = self.prepare_test_evidence()?;
        let jobs = self.collect_jobs(
            test_evidence.borrowed(),
            &mut residuals,
            &mut advisory,
            &mut ran_stages,
            &mut all_passed,
            &required,
        )?;

        let ran = jobs.len();
        ran_stages.extend(jobs.iter().map(|job| job.stage.policy_name()));
        self.enforce_required_stages(&ran_stages, &mut residuals, &mut all_passed)?;
        let mut workers = self.spawn_verifier_groups(jobs);

        while let Some(result) = workers.join_next().await {
            for (job, execution) in result.context("verifier worker panicked")? {
                match execution {
                    Ok(execution) => self.fold_execution(
                        &job,
                        execution,
                        &mut residuals,
                        &mut advisory,
                        &mut all_passed,
                        &required,
                    )?,
                    Err(error) => fold_infra_failure(
                        &self.node_id,
                        self.generation,
                        job.stage,
                        &format!("{}:{} ({error})", job.plugin, job.stage),
                        &mut residuals,
                        &mut advisory,
                        &mut all_passed,
                        &required,
                    )?,
                }
            }
        }

        if ran == 0 {
            all_passed = false;
        }
        // Only required-stage residuals gate; advisory ones (lint outside
        // the hard-gate policy) still enter the energy and corrections.
        let hard_pass = self.candidate.has_mutated() && all_passed && residuals.is_empty();
        residuals.extend(advisory);
        let (energy, correction, packet) = self.score(&residuals)?;
        Ok(Measured {
            hard_pass,
            energy,
            residuals,
            correction,
            packet,
        })
    }

    /// Per-mutation boundary: a syntax-only pass. The complete suite — every
    /// build and test, each with its own timeout — belongs to the gate
    /// boundary (`measure`); running it after every single admitted mutation
    /// multiplies a turn's cost by its edit count for no additional gate
    /// evidence. `hard_pass` is never claimed from this cheap pass.
    async fn measure_incremental(&self) -> Result<Measured> {
        let registry = perspt_core::PluginRegistry::new();
        let plugins = registry.detect_all(self.candidate.overlay_root());
        let mut residuals = Vec::new();
        for plugin in plugins {
            let Some(command) = plugin.syntax_check_command() else {
                continue;
            };
            let adapter_id = Self::adapter_for(plugin.name());
            let execution = run_governed_verifier(
                self.candidate.overlay_root().to_path_buf(),
                command,
                self.candidate.unisolated_verifiers_allowed(),
                SHARED_TARGET_SUFFIX.into(),
                self.candidate.verifier_env(),
                self.candidate
                    .verifier_timeouts()
                    .for_stage(Some(perspt_core::plugin::VerifierStage::SyntaxCheck)),
            )
            .await?;
            if let Some(adapter) = self.adapters.get(&adapter_id) {
                residuals.extend(adapter.parse_diagnostics(
                    &self.node_id,
                    self.generation,
                    &execution.output,
                ));
            }
            if !execution.success && !residuals.iter().any(|r| r.class == ResidualClass::Build) {
                residuals.push(tool_residual(
                    &self.node_id,
                    self.generation,
                    ResidualClass::Build,
                    &format!("syntax check failed: {}", concise(&execution.output)),
                )?);
            }
        }
        let (energy, correction, packet) = self.score(&residuals)?;
        Ok(Measured {
            hard_pass: false,
            energy,
            residuals,
            correction,
            packet,
        })
    }
}

fn tool_residual(
    node: &str,
    generation: u32,
    class: ResidualClass,
    summary: &str,
) -> Result<ResidualEvent> {
    let mut residual = ResidualEvent::new(
        node,
        generation,
        class,
        ResidualSeverity::Error,
        1.0,
        SensorRef::new("governed-verifier", IndependenceRoute::DeterministicTool),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    residual.evidence.summary = summary.into();
    Ok(residual)
}

/// The report path a verifier command declared via `--junitxml <path>` or
/// `--junitxml=<path>`.
fn junit_report_path(command: &str) -> Option<String> {
    let mut words = command.split_whitespace().peekable();
    while let Some(word) = words.next() {
        if word == "--junitxml" {
            return words.peek().map(|path| (*path).to_string());
        }
        if let Some(path) = word.strip_prefix("--junitxml=") {
            return Some(path.to_string());
        }
    }
    None
}

/// Route one verifier *infrastructure* failure (unavailable capability,
/// launch error, timeout) by stage: it blocks hard pass only when the
/// stage is in the domain's required set — a lint timeout or a missing
/// lint binary is as advisory as a lint finding (erratum 11: non-required
/// stages cannot block acceptance), while a build or test failure to run
/// still blocks.
#[allow(clippy::too_many_arguments)]
fn fold_infra_failure(
    node_id: &str,
    generation: u32,
    stage: perspt_core::plugin::VerifierStage,
    detail: &str,
    residuals: &mut Vec<ResidualEvent>,
    advisory: &mut Vec<ResidualEvent>,
    all_passed: &mut bool,
    required: &BTreeSet<String>,
) -> Result<()> {
    let blocking = required.contains(stage.policy_name());
    let sink = if blocking { residuals } else { advisory };
    sink.push(sensor_unavailable(node_id, generation, detail)?);
    if blocking {
        *all_passed = false;
    }
    Ok(())
}

fn sensor_unavailable(node: &str, generation: u32, sensor: &str) -> Result<ResidualEvent> {
    tool_residual(
        node,
        generation,
        ResidualClass::SensorUnavailable,
        &format!("required sensor unavailable: {sensor}"),
    )
}

fn concise(output: &str) -> String {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn required() -> BTreeSet<String> {
        ["syntax", "build", "test"]
            .into_iter()
            .map(String::from)
            .collect()
    }

    /// Erratum 11 covers infrastructure too: a lint-stage timeout or
    /// missing lint binary lands in the advisory sink and leaves
    /// `all_passed` alone, while the same failure on a required stage
    /// still blocks.
    #[test]
    fn infra_failures_block_only_on_required_stages() {
        use perspt_core::plugin::VerifierStage;
        let mut residuals = Vec::new();
        let mut advisory = Vec::new();
        let mut all_passed = true;
        fold_infra_failure(
            "n1",
            0,
            VerifierStage::Lint,
            "rust:lint (deadline elapsed)",
            &mut residuals,
            &mut advisory,
            &mut all_passed,
            &required(),
        )
        .unwrap();
        assert!(all_passed, "a lint infra failure must not clear all_passed");
        assert!(residuals.is_empty(), "nothing enters the blocking sink");
        assert_eq!(advisory.len(), 1, "the failure is still reported");
        assert_eq!(advisory[0].class, ResidualClass::SensorUnavailable);

        fold_infra_failure(
            "n1",
            0,
            VerifierStage::Test,
            "rust:test (deadline elapsed)",
            &mut residuals,
            &mut advisory,
            &mut all_passed,
            &required(),
        )
        .unwrap();
        assert!(!all_passed, "a required-stage infra failure still blocks");
        assert_eq!(residuals.len(), 1);
    }
}
