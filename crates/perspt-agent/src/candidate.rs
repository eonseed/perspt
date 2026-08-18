//! Reversible coding candidate overlay and compiler-backed measurement.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use perspt_coding::{CodingAdapterRegistry, CodingDomain, LanguageId};
use perspt_sdk::{
    score_candidate, AgentDomainPackage, CandidateStateWitness, CorrectionDirection, DomainScope,
    IndependenceRoute, ResidualClass, ResidualEvent, ResidualSeverity, SensorRef, ToolEntry,
};
use tempfile::TempDir;

use crate::realize::snapshot_workspace;
use crate::toolloop::{
    CandidateCheckpoint, CandidateMeasurer, EffectExecutor, EffectOutcome, Measured,
};
use crate::tools::AgentTools;
use crate::verifier::{run_governed_verifier, VerifierExecution, VerifierJob};

/// A node-local candidate filesystem. Model mutations never target the source
/// workspace directly; only [`promote`](Self::promote) copies touched paths.
pub struct CandidateWorkspace {
    source_root: PathBuf,
    overlay_root: PathBuf,
    _temp: TempDir,
    overlay_tools: AgentTools,
    source_tools: AgentTools,
    /// Checkpoint scope: every path a proposal has *named*, including pure
    /// reads. Used for state-witness hashing, never for promotion.
    tracked: Mutex<BTreeSet<String>>,
    /// Paths an admitted effect actually mutated. Promotion, diffs, and the
    /// approval file list read this set — a merely-read file must never be
    /// copied back into the source workspace.
    mutated_paths: Mutex<BTreeSet<String>>,
    /// Workspace pre-image at the first mutation of each path. Promotion
    /// compares against it to avoid overwriting concurrent user edits.
    source_preimages: Mutex<BTreeMap<String, Option<Vec<u8>>>>,
    /// Pre-images of every mutation, in order. Restoring a checkpoint replays
    /// the suffix recorded after it in reverse, so files first touched after
    /// the checkpoint are restored (or removed) exactly.
    journal: Mutex<Vec<JournalEntry>>,
    snapshots: Mutex<HashMap<String, CandidateSnapshot>>,
    lsp_sessions: crate::tools::handlers::LspSessions,
    mutations: AtomicU32,
    node_id: String,
    generation: u32,
    graph_revision: String,
    allow_unisolated_verifiers: bool,
    /// The open execution plane: exact-name handlers this candidate
    /// dispatches admitted calls through.
    handlers: Arc<crate::tools::handlers::CandidateHandlerRegistry>,
}

impl std::fmt::Debug for CandidateWorkspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CandidateWorkspace")
            .field("source_root", &self.source_root)
            .field("overlay_root", &self.overlay_root)
            .field("node_id", &self.node_id)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl CandidateWorkspace {
    pub fn create(
        source_root: &Path,
        node_id: impl Into<String>,
        generation: u32,
        graph_revision: impl Into<String>,
    ) -> Result<Self> {
        Self::create_with_policy(source_root, node_id, generation, graph_revision, false)
    }

    pub fn create_with_policy(
        source_root: &Path,
        node_id: impl Into<String>,
        generation: u32,
        graph_revision: impl Into<String>,
        allow_unisolated_verifiers: bool,
    ) -> Result<Self> {
        let source_root = source_root
            .canonicalize()
            .with_context(|| format!("canonicalizing {}", source_root.display()))?;
        let temp = tempfile::Builder::new()
            .prefix("perspt-candidate-")
            .tempdir()?;
        let overlay_root = temp.path().join("workspace");
        std::fs::create_dir_all(&overlay_root)?;
        let overlay_root = overlay_root.canonicalize()?;
        copy_workspace(&source_root, &overlay_root)?;
        Ok(Self {
            overlay_tools: AgentTools::new(overlay_root.clone()),
            source_tools: AgentTools::new(source_root.clone()),
            source_root,
            overlay_root,
            _temp: temp,
            tracked: Mutex::new(BTreeSet::new()),
            mutated_paths: Mutex::new(BTreeSet::new()),
            source_preimages: Mutex::new(BTreeMap::new()),
            journal: Mutex::new(Vec::new()),
            snapshots: Mutex::new(HashMap::new()),
            lsp_sessions: tokio::sync::Mutex::new(HashMap::new()),
            mutations: AtomicU32::new(0),
            node_id: node_id.into(),
            generation,
            graph_revision: graph_revision.into(),
            allow_unisolated_verifiers,
            handlers: Arc::new(crate::tools::handlers::CandidateHandlerRegistry::with_builtins()),
        })
    }

    /// Replace the execution plane. The composition root uses this to add
    /// registered tool families beyond the builtins.
    pub fn set_tool_handlers(
        &mut self,
        handlers: Arc<crate::tools::handlers::CandidateHandlerRegistry>,
    ) {
        self.handlers = handlers;
    }

    pub fn overlay_root(&self) -> &Path {
        &self.overlay_root
    }

    pub(crate) fn overlay_tools(&self) -> &AgentTools {
        &self.overlay_tools
    }

    pub(crate) fn source_tools(&self) -> &AgentTools {
        &self.source_tools
    }

    pub(crate) fn lsp_sessions(&self) -> &crate::tools::handlers::LspSessions {
        &self.lsp_sessions
    }

    pub(crate) fn unisolated_verifiers_allowed(&self) -> bool {
        self.allow_unisolated_verifiers
    }

    /// Validate a workspace-relative path exactly as admission does.
    pub(crate) fn validate_relative(&self, path: &str) -> Result<String> {
        validate_relative_path(path)
    }

    pub fn has_mutated(&self) -> bool {
        self.mutations.load(Ordering::SeqCst) > 0
    }

    /// The node generation this candidate was created for.
    pub fn node_generation(&self) -> u32 {
        self.generation
    }

    /// Paths mutated by admitted effects — the only promotable set.
    pub fn touched_paths(&self) -> Vec<String> {
        self.mutated_paths.lock().unwrap().iter().cloned().collect()
    }

    /// Diff the realized candidate against the source workspace. This is an
    /// observation for advisory/conjunctive validators; gating still measures
    /// the candidate filesystem directly.
    pub fn realized_diff(&self) -> Result<String> {
        let mut output = String::new();
        for relative in self.touched_paths() {
            let before =
                std::fs::read_to_string(self.source_root.join(&relative)).unwrap_or_default();
            let after =
                std::fs::read_to_string(self.overlay_root.join(&relative)).unwrap_or_default();
            output.push_str(&format!("diff --perspt {relative}\n"));
            output.push_str(
                &diffy::PatchFormatter::new()
                    .fmt_patch(&diffy::create_patch(&before, &after))
                    .to_string(),
            );
        }
        Ok(output)
    }

    /// Promote only paths mutated by admitted governed proposals. Checks and
    /// writes share one directory descriptor per target (`crate::promote`),
    /// so a path swapped underneath the workspace between validation and
    /// rename is refused rather than followed.
    pub fn promote(&self) -> Result<Vec<String>> {
        let paths: Vec<String> = self.mutated_paths.lock().unwrap().iter().cloned().collect();
        let source = crate::promote::WorkspaceRoot::open(&self.source_root)?;
        let overlay = crate::promote::WorkspaceRoot::open(&self.overlay_root)?;
        let mut staged = Vec::new();
        for rel in &paths {
            let rel = validate_relative_path(rel)?;
            let target = source.target_dir(&rel, true)?;
            let current = target.read_optional()?;
            let expected = self
                .source_preimages
                .lock()
                .unwrap()
                .get(&rel)
                .cloned()
                .context("mutated path has no source pre-image")?;
            anyhow::ensure!(
                current == expected,
                "workspace path changed since the candidate was created: {rel}"
            );
            let after = overlay.read_if_present(&rel)?;
            staged.push((rel, target, current, after));
        }

        for (promoted, (rel, target, _before, after)) in staged.iter().enumerate() {
            if let Err(error) = target.apply(after.as_deref()) {
                let error = error.context(format!("promoting {rel}"));
                return Err(rollback_promotion(&staged[..promoted], error));
            }
        }
        Ok(paths)
    }

    fn scope_snapshot(&self, additional: &[String]) -> Result<CandidateCheckpoint> {
        let mut tracked = self.tracked.lock().unwrap();
        for rel in additional {
            tracked.insert(validate_relative_path(rel)?);
        }
        let scope: Vec<String> = tracked.iter().cloned().collect();
        drop(tracked);

        let witness = self.witness_for(scope)?;
        let id = uuid::Uuid::new_v4().to_string();
        self.snapshots.lock().unwrap().insert(
            id.clone(),
            CandidateSnapshot {
                journal_len: self.journal.lock().unwrap().len(),
                mutations: self.mutations.load(Ordering::SeqCst),
                mutated: self.mutated_paths.lock().unwrap().clone(),
            },
        );
        Ok(CandidateCheckpoint { id, witness })
    }

    fn current_witness(&self) -> Result<CandidateStateWitness> {
        let scope: Vec<String> = self.tracked.lock().unwrap().iter().cloned().collect();
        self.witness_for(scope)
    }

    /// Build the state witness for a scope from the realized overlay: the
    /// content hash re-reads disk, and the barrier channels are measured over
    /// the actually-mutated paths, so the barrier clause observes the
    /// materialized candidate rather than only the declared proposal.
    fn witness_for(&self, scope: Vec<String>) -> Result<CandidateStateWitness> {
        let state = snapshot_workspace(&self.overlay_root, &scope)?;
        let mutated = self.mutated_paths.lock().unwrap();
        let barrier_channels = perspt_coding::OperationalSafetyBarrier::default()
            .measure_channels(mutated.iter().map(String::as_str));
        Ok(CandidateStateWitness {
            state_root: state.root_hash(),
            graph_revision: self.graph_revision.clone(),
            node_id: self.node_id.clone(),
            node_generation: self.generation,
            canonical_scope: scope,
            barrier_channels,
        })
    }

    /// Validate every proposal-named path and, for a mutating effect, record
    /// pre-images first: restore must be exact even for paths first touched
    /// after the accepted checkpoint.
    fn admit_named_paths(
        &self,
        call: &perspt_sdk::ProviderToolCall,
        entry: &ToolEntry,
        mutating: bool,
    ) -> Result<Vec<String>> {
        let named_paths: Vec<String> = ["path", "to", "from"]
            .iter()
            .filter_map(|field| call.arguments.get(*field).and_then(|v| v.as_str()))
            .map(str::to_string)
            .collect();
        for rel in &named_paths {
            validate_relative_path(rel)?;
            if !entry.effect.is_read_only() {
                reject_symlink_ancestor(&self.overlay_root, rel)?;
            }
        }
        if mutating {
            for rel in &named_paths {
                self.journal_pre_image(rel)?;
            }
        }
        Ok(named_paths)
    }

    /// Admit paths a durable external command (e.g. a dependency mutation)
    /// may touch: validate, journal pre-images, and track them, exactly as
    /// argument-named paths are admitted before a mutating effect.
    pub(crate) fn admit_external_paths(&self, paths: &[String]) -> Result<()> {
        for rel in paths {
            let rel = validate_relative_path(rel)?;
            reject_symlink_ancestor(&self.overlay_root, &rel)?;
            self.journal_pre_image(&rel)?;
            self.tracked.lock().unwrap().insert(rel);
        }
        Ok(())
    }

    /// Mark externally mutated paths promotable. Callers pass only paths
    /// whose content actually changed.
    pub(crate) fn note_mutated_paths(&self, paths: &[String]) -> Result<()> {
        let mut mutated = self.mutated_paths.lock().unwrap();
        for rel in paths {
            mutated.insert(validate_relative_path(rel)?);
        }
        Ok(())
    }

    /// Read a path from the overlay; `None` when absent.
    pub(crate) fn overlay_bytes(&self, rel: &str) -> Result<Option<Vec<u8>>> {
        let rel = validate_relative_path(rel)?;
        match std::fs::read(self.overlay_root.join(&rel)) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).with_context(|| format!("reading {rel}")),
        }
    }

    /// Rebuild an accepted candidate state from a durable checkpoint's
    /// exported files (mid-loop resume): each file is journaled and written
    /// into the overlay, and re-enters the mutated (promotable) set.
    pub fn restore_exported(&self, files: &[crate::toolloop::SeedFile]) -> Result<()> {
        for seed in files {
            let rel = validate_relative_path(&seed.path)?;
            reject_symlink_ancestor(&self.overlay_root, &rel)?;
            self.journal_pre_image(&rel)?;
            let path = self.overlay_root.join(&rel);
            match &seed.content {
                Some(bytes) => {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&path, bytes)?;
                }
                None if path.is_file() => std::fs::remove_file(&path)?,
                None => {}
            }
            self.tracked.lock().unwrap().insert(rel.clone());
            self.source_preimages
                .lock()
                .unwrap()
                .insert(rel.clone(), seed.source_preimage.clone());
            self.mutated_paths.lock().unwrap().insert(rel);
            self.mutations.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }

    /// Record the pre-image of a path before a mutating effect touches it.
    fn journal_pre_image(&self, rel: &str) -> Result<()> {
        let path = self.overlay_root.join(rel);
        let prior = match std::fs::read(&path) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e).with_context(|| format!("journaling {rel}")),
        };
        self.source_preimages
            .lock()
            .unwrap()
            .entry(rel.to_string())
            .or_insert_with(|| prior.clone());
        self.journal.lock().unwrap().push(JournalEntry {
            path: rel.to_string(),
            prior,
        });
        Ok(())
    }

    /// Replay the journal suffix recorded after the checkpoint, in reverse.
    /// A file first created after the checkpoint has a `None` pre-image and
    /// is removed; a file modified after it is rewritten to its exact bytes.
    fn restore_snapshot(&self, checkpoint: &CandidateCheckpoint) -> Result<()> {
        let snapshots = self.snapshots.lock().unwrap();
        let snapshot = snapshots
            .get(&checkpoint.id)
            .with_context(|| format!("unknown candidate checkpoint {}", checkpoint.id))?;
        let mut journal = self.journal.lock().unwrap();
        while journal.len() > snapshot.journal_len {
            let entry = journal.pop().expect("journal length checked");
            let path = self.overlay_root.join(&entry.path);
            match entry.prior {
                Some(bytes) => {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&path, bytes)?;
                }
                None if path.is_file() => std::fs::remove_file(&path)?,
                None => {}
            }
        }
        self.mutations.store(snapshot.mutations, Ordering::SeqCst);
        *self.mutated_paths.lock().unwrap() = snapshot.mutated.clone();
        Ok(())
    }

    pub(crate) fn command_for(&self, tool: &str) -> Result<String> {
        let registry = perspt_core::PluginRegistry::new();
        let plugins = registry.detect_all(&self.overlay_root);
        let plugin = plugins.first().context("no language plugin detected")?;
        match tool {
            "run_test" => Ok(plugin.test_command()),
            "run_build" => plugin
                .build_command()
                .or_else(|| plugin.syntax_check_command())
                .context("active language has no build or syntax command"),
            "run_formatter" => plugin
                .format_command()
                .context("active language has no governed formatter command"),
            _ => bail!("not a verifier tool: {tool}"),
        }
    }

    pub(crate) async fn run_governed_verifier(&self, command: &str) -> Result<VerifierExecution> {
        run_governed_verifier(
            self.overlay_root.clone(),
            command.to_string(),
            self.allow_unisolated_verifiers,
            "tool".into(),
            self.verifier_env(),
        )
        .await
    }

    /// Extra environment for governed verifiers. A copied virtualenv is not
    /// valid at the overlay path, so `uv run --no-sync` is pointed at the
    /// *source* project's synced environment instead — read-only use, which
    /// the sandbox enforces (writes outside the overlay are denied).
    pub(crate) fn verifier_env(&self) -> Vec<(String, String)> {
        let venv = self.source_root.join(".venv");
        if venv.is_dir() {
            vec![("UV_PROJECT_ENVIRONMENT".into(), venv.display().to_string())]
        } else {
            Vec::new()
        }
    }

    /// Build a second candidate whose implementation matches the current
    /// candidate but whose pre-existing test files are restored to their
    /// source pre-images. This prevents a candidate from certifying itself by
    /// weakening the test oracle it is measured against.
    fn immutable_test_oracle(&self) -> Result<Option<ImmutableTestOracle>> {
        let registry = perspt_core::PluginRegistry::new();
        let plugins = registry.detect_all(&self.overlay_root);
        let preimages = self.source_preimages.lock().unwrap();
        let touched_tests: Vec<_> = self
            .mutated_paths
            .lock()
            .unwrap()
            .iter()
            .filter(|path| plugins.iter().any(|plugin| plugin.is_test_file(path)))
            .filter_map(|path| {
                preimages
                    .get(path)
                    .cloned()
                    .map(|content| (path.clone(), content))
            })
            .collect();
        if !touched_tests
            .iter()
            .any(|(_, source_preimage)| source_preimage.is_some())
        {
            return Ok(None);
        }

        let temp = tempfile::Builder::new()
            .prefix("perspt-test-oracle-")
            .tempdir()?;
        let root = temp.path().join("workspace");
        std::fs::create_dir_all(&root)?;
        copy_workspace(&self.overlay_root, &root)?;
        for (relative, source_preimage) in touched_tests {
            let path = root.join(&relative);
            match source_preimage {
                Some(bytes) => {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(path, bytes)?;
                }
                None if path.is_file() => std::fs::remove_file(path)?,
                None => {}
            }
        }
        Ok(Some(ImmutableTestOracle { _temp: temp, root }))
    }
}

#[async_trait::async_trait]
impl EffectExecutor for CandidateWorkspace {
    async fn checkpoint(&self, scope: &[String]) -> Result<CandidateCheckpoint> {
        self.scope_snapshot(scope)
    }

    async fn apply(
        &self,
        call: &perspt_sdk::ProviderToolCall,
        entry: &ToolEntry,
    ) -> Result<EffectOutcome> {
        let mutating = crate::toolloop::candidate_mutating_effect(entry.effect);
        let named_paths = self.admit_named_paths(call, entry, mutating)?;

        let Some(handler) = self.handlers.resolve(&call.name).cloned() else {
            return Ok(EffectOutcome {
                output: format!("tool failed: no executor registered for {}", call.name),
                mutated: false,
            });
        };
        let outcome = handler.apply(self, call, entry).await?;

        // Mutation bookkeeping is centralized: only a successful handler run
        // of a mutating effect marks its admitted named paths promotable.
        let mutates_source = outcome.mutated && mutating;
        if mutates_source {
            self.mutations.fetch_add(1, Ordering::SeqCst);
            let mut mutated = self.mutated_paths.lock().unwrap();
            for rel in &named_paths {
                mutated.insert(rel.clone());
            }
        }
        Ok(EffectOutcome {
            output: outcome.output,
            mutated: mutates_source,
        })
    }

    async fn restore(&self, checkpoint: &CandidateCheckpoint) -> Result<()> {
        self.restore_snapshot(checkpoint)
    }

    async fn state_witness(&self) -> Result<CandidateStateWitness> {
        self.current_witness()
    }

    async fn export_accepted(&self) -> Result<Vec<crate::toolloop::SeedFile>> {
        let mutated = self.touched_paths();
        let preimages = self.source_preimages.lock().unwrap();
        let mut exported = Vec::with_capacity(mutated.len());
        for rel in mutated {
            let path = self.overlay_root.join(&rel);
            let bytes = match std::fs::read(&path) {
                Ok(bytes) => Some(bytes),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e).with_context(|| format!("exporting {rel}")),
            };
            let source_preimage = preimages
                .get(&rel)
                .cloned()
                .context("mutated path has no source pre-image")?;
            exported.push(crate::toolloop::SeedFile {
                path: rel,
                content: bytes,
                source_preimage,
            });
        }
        Ok(exported)
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
        }
    }

    /// Enable the declared `format` acceptance stage
    /// (`[verification] require_format`).
    pub fn with_require_format(mut self, require_format: bool) -> Self {
        self.require_format = require_format;
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
    fn collect_jobs(
        &self,
        immutable_oracle_root: Option<&Path>,
        residuals: &mut Vec<ResidualEvent>,
        all_passed: &mut bool,
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
                        continue;
                    }
                    residuals.push(sensor_unavailable(
                        &self.node_id,
                        self.generation,
                        &format!("{}:{}", plugin.name(), capability.stage),
                    )?);
                    *all_passed = false;
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
            if let Some(root) = immutable_oracle_root {
                jobs.push(VerifierJob {
                    plugin: format!("{}:immutable-test-oracle", plugin.name()),
                    adapter_id,
                    stage: perspt_core::plugin::VerifierStage::Test,
                    command: plugin.test_command(),
                    root: root.to_path_buf(),
                });
            }
        }
        Ok(jobs)
    }

    fn fold_execution(
        &self,
        job: &VerifierJob,
        execution: VerifierExecution,
        residuals: &mut Vec<ResidualEvent>,
        all_passed: &mut bool,
    ) -> Result<()> {
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
            residuals.extend(adapter.parse_diagnostics(
                &self.node_id,
                self.generation,
                &execution.output,
            ));
        }
        if !execution.success {
            *all_passed = false;
            let class = match job.stage {
                perspt_core::plugin::VerifierStage::Test => ResidualClass::TestFailure,
                perspt_core::plugin::VerifierStage::Lint => ResidualClass::Lint,
                perspt_core::plugin::VerifierStage::Format => ResidualClass::Format,
                _ => ResidualClass::Build,
            };
            if !residuals.iter().any(|r| r.class == class) {
                residuals.push(tool_residual(
                    &self.node_id,
                    self.generation,
                    class,
                    &format!("{} failed: {}", job.stage, concise(&execution.output)),
                )?);
            }
        }
        Ok(())
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
        let packet = self.domain.correction_packet(residuals);
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
        let mut all_passed = false;
        let immutable_oracle = self.candidate.immutable_test_oracle()?;
        let jobs = self.collect_jobs(
            immutable_oracle
                .as_ref()
                .map(|oracle| oracle.root.as_path()),
            &mut residuals,
            &mut all_passed,
        )?;

        let ran = jobs.len();
        let ran_stages: BTreeSet<&'static str> =
            jobs.iter().map(|job| job.stage.policy_name()).collect();
        self.enforce_required_stages(&ran_stages, &mut residuals, &mut all_passed)?;
        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.max_parallel));
        let mut workers = tokio::task::JoinSet::new();
        for (ordinal, job) in jobs.into_iter().enumerate() {
            let semaphore = semaphore.clone();
            let root = job.root.clone();
            let allow_unisolated = self.candidate.allow_unisolated_verifiers;
            let extra_env = self.candidate.verifier_env();
            workers.spawn(async move {
                let _permit = semaphore.acquire_owned().await.expect("semaphore closed");
                let execution = run_governed_verifier(
                    root,
                    job.command.clone(),
                    allow_unisolated,
                    format!("{}-{ordinal}", job.stage),
                    extra_env,
                )
                .await;
                (job, execution)
            });
        }

        while let Some(result) = workers.join_next().await {
            let (job, execution) = result.context("verifier worker panicked")?;
            match execution {
                Ok(execution) => {
                    self.fold_execution(&job, execution, &mut residuals, &mut all_passed)?
                }
                Err(error) => {
                    residuals.push(sensor_unavailable(
                        &self.node_id,
                        self.generation,
                        &format!("{}:{} ({error})", job.plugin, job.stage),
                    )?);
                    all_passed = false;
                }
            }
        }

        if ran == 0 {
            all_passed = false;
        }
        let (energy, correction, packet) = self.score(&residuals)?;
        Ok(Measured {
            hard_pass: self.candidate.has_mutated() && all_passed && residuals.is_empty(),
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
                self.candidate.allow_unisolated_verifiers,
                "incremental-syntax".into(),
                self.candidate.verifier_env(),
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

struct CandidateSnapshot {
    /// Journal position at snapshot time; restore replays everything after it.
    journal_len: usize,
    mutations: u32,
    mutated: BTreeSet<String>,
}

/// The pre-image of one path recorded immediately before a mutation.
struct JournalEntry {
    path: String,
    prior: Option<Vec<u8>>,
}

struct ImmutableTestOracle {
    _temp: TempDir,
    root: PathBuf,
}

fn validate_relative_path(path: &str) -> Result<String> {
    let value = Path::new(path);
    if value.as_os_str().is_empty()
        || value.is_absolute()
        || value.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        bail!("path must be a non-empty workspace-relative path: {path:?}");
    }
    Ok(value
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/"))
}

/// One staged promotion target: relative path, held parent descriptor,
/// source pre-image, and overlay content (`None` means deletion).
type StagedPromotion = (
    String,
    crate::promote::TargetDir,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
);

/// Restore already-promoted targets to their pre-images after a failed
/// promotion, through the same held descriptors. Restore failures are part
/// of the returned error, never swallowed: a half-promoted workspace must
/// be visible to the caller.
fn rollback_promotion(promoted: &[StagedPromotion], error: anyhow::Error) -> anyhow::Error {
    let mut failures = Vec::new();
    for (rel, target, before, _) in promoted {
        if let Err(restore) = target.apply(before.as_deref()) {
            failures.push(format!("{rel}: {restore:#}"));
        }
    }
    if failures.is_empty() {
        error
    } else {
        error.context(format!(
            "rollback left the workspace inconsistent: {}",
            failures.join("; ")
        ))
    }
}

pub(crate) fn reject_symlink_ancestor(root: &Path, relative: &str) -> Result<()> {
    let mut current = root.to_path_buf();
    for component in Path::new(relative).components() {
        if let Component::Normal(part) = component {
            current.push(part);
            match std::fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    bail!("mutating through a candidate symlink is forbidden: {relative}")
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
    Ok(())
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

fn copy_workspace(source: &Path, destination: &Path) -> Result<()> {
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        if matches!(
            name.to_str(),
            Some(
                ".git"
                    | ".perspt"
                    | ".perspt-target"
                    | ".perspt-tmp"
                    | ".perspt-home"
                    | ".venv"
                    | ".pytest_cache"
                    | ".ruff_cache"
                    | "__pycache__"
                    | "target"
            )
        ) {
            continue;
        }
        let from = entry.path();
        let to = destination.join(&name);
        let kind = entry.file_type()?;
        if kind.is_dir() {
            if name == "node_modules" {
                link_dependency_dir(&from, &to)?;
            } else {
                std::fs::create_dir_all(&to)?;
                copy_workspace(&from, &to)?;
            }
        } else if kind.is_file() {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn link_dependency_dir(source: &Path, destination: &Path) -> Result<()> {
    std::os::unix::fs::symlink(source, destination)?;
    Ok(())
}

#[cfg(not(unix))]
fn link_dependency_dir(source: &Path, destination: &Path) -> Result<()> {
    std::fs::create_dir_all(destination)?;
    copy_workspace(source, destination)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_cannot_escape_the_candidate() {
        assert!(validate_relative_path("src/lib.rs").is_ok());
        assert!(validate_relative_path("../outside").is_err());
        assert!(validate_relative_path("/tmp/outside").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn mutation_cannot_cross_an_overlay_symlink() {
        let overlay = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), overlay.path().join("deps")).unwrap();
        assert!(reject_symlink_ancestor(overlay.path(), "deps/package/file.rs").is_err());
        assert!(reject_symlink_ancestor(overlay.path(), "src/file.rs").is_ok());
    }

    fn write_call(path: &str, content: &str) -> perspt_sdk::ProviderToolCall {
        perspt_sdk::ProviderToolCall {
            call_id: uuid::Uuid::new_v4().to_string(),
            name: "write_file".into(),
            arguments: serde_json::json!({"path": path, "content": content}),
        }
    }

    fn write_entry() -> ToolEntry {
        perspt_sdk::base_entries()
            .into_iter()
            .find(|entry| entry.name == "write_file")
            .expect("base catalog has write_file")
    }

    #[tokio::test]
    async fn checkpoint_restore_and_promotion_use_touched_paths() {
        let source = tempfile::tempdir().unwrap();
        std::fs::write(source.path().join("a.txt"), "old").unwrap();
        let candidate = CandidateWorkspace::create(source.path(), "n1", 0, "r1").unwrap();
        let before = candidate.checkpoint(&["a.txt".into()]).await.unwrap();
        let entry = write_entry();
        candidate
            .apply(&write_call("a.txt", "new"), &entry)
            .await
            .unwrap();
        candidate.restore(&before).await.unwrap();
        assert_eq!(
            std::fs::read_to_string(candidate.overlay_root().join("a.txt")).unwrap(),
            "old"
        );
        assert!(!candidate.has_mutated());
        candidate
            .apply(&write_call("a.txt", "accepted"), &entry)
            .await
            .unwrap();
        candidate.promote().unwrap();
        assert_eq!(
            std::fs::read_to_string(source.path().join("a.txt")).unwrap(),
            "accepted"
        );
    }

    #[tokio::test]
    async fn restore_removes_files_first_created_after_the_checkpoint() {
        let source = tempfile::tempdir().unwrap();
        std::fs::write(source.path().join("a.txt"), "old").unwrap();
        let candidate = CandidateWorkspace::create(source.path(), "n1", 0, "r1").unwrap();
        let accepted = candidate.checkpoint(&[]).await.unwrap();
        let entry = write_entry();
        // A file first touched *after* the accepted checkpoint must be gone
        // after a gate rejection, on disk and in the promotable set.
        candidate
            .apply(&write_call("fresh.rs", "fn injected() {}"), &entry)
            .await
            .unwrap();
        assert!(candidate.overlay_root().join("fresh.rs").is_file());
        assert!(candidate.touched_paths().contains(&"fresh.rs".to_string()));
        candidate.restore(&accepted).await.unwrap();
        assert!(!candidate.overlay_root().join("fresh.rs").exists());
        assert!(candidate.touched_paths().is_empty());
        assert!(!candidate.has_mutated());
    }

    #[tokio::test]
    async fn read_paths_are_never_promoted() {
        let source = tempfile::tempdir().unwrap();
        std::fs::write(source.path().join("read-only.txt"), "user edit kept").unwrap();
        let candidate = CandidateWorkspace::create(source.path(), "n1", 0, "r1").unwrap();
        // The checkpoint scope names the read path; the user then edits the
        // source file while the agent merely read it. Promotion must not
        // rewrite it from the stale overlay copy.
        candidate
            .checkpoint(&["read-only.txt".into()])
            .await
            .unwrap();
        std::fs::write(source.path().join("read-only.txt"), "user edit v2").unwrap();
        assert!(candidate.touched_paths().is_empty());
        candidate.promote().unwrap();
        assert_eq!(
            std::fs::read_to_string(source.path().join("read-only.txt")).unwrap(),
            "user edit v2"
        );
    }

    #[tokio::test]
    async fn immutable_oracle_restores_existing_tests_but_keeps_candidate_code() {
        let source = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(source.path().join("src")).unwrap();
        std::fs::create_dir_all(source.path().join("tests")).unwrap();
        std::fs::write(
            source.path().join("pyproject.toml"),
            "[project]\nname='x'\n",
        )
        .unwrap();
        std::fs::write(source.path().join("src/logic.py"), "VALUE = 'old'\n").unwrap();
        std::fs::write(
            source.path().join("tests/test_logic.py"),
            "def test_value(): assert False\n",
        )
        .unwrap();
        let candidate = CandidateWorkspace::create(source.path(), "n1", 0, "r1").unwrap();
        candidate
            .apply(
                &write_call("src/logic.py", "VALUE = 'candidate'\n"),
                &write_entry(),
            )
            .await
            .unwrap();
        candidate
            .apply(
                &write_call("tests/test_logic.py", "def test_value(): pass\n"),
                &write_entry(),
            )
            .await
            .unwrap();

        let oracle = candidate.immutable_test_oracle().unwrap().unwrap();
        assert_eq!(
            std::fs::read_to_string(oracle.root.join("src/logic.py")).unwrap(),
            "VALUE = 'candidate'\n"
        );
        assert_eq!(
            std::fs::read_to_string(oracle.root.join("tests/test_logic.py")).unwrap(),
            "def test_value(): assert False\n"
        );
    }

    #[tokio::test]
    async fn promotion_refuses_a_concurrent_workspace_edit() {
        let source = tempfile::tempdir().unwrap();
        std::fs::write(source.path().join("a.txt"), "baseline").unwrap();
        let candidate = CandidateWorkspace::create(source.path(), "n1", 0, "r1").unwrap();
        candidate
            .apply(&write_call("a.txt", "agent edit"), &write_entry())
            .await
            .unwrap();
        std::fs::write(source.path().join("a.txt"), "user edit").unwrap();
        assert!(candidate.promote().is_err());
        assert_eq!(
            std::fs::read_to_string(source.path().join("a.txt")).unwrap(),
            "user edit"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn promotion_cannot_cross_a_source_workspace_symlink() {
        let source = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), source.path().join("linked")).unwrap();
        let candidate = CandidateWorkspace::create(source.path(), "n1", 0, "r1").unwrap();
        candidate
            .apply(
                &write_call("linked/escaped.txt", "must stay in overlay"),
                &write_entry(),
            )
            .await
            .unwrap();
        assert!(candidate.promote().is_err());
        assert!(!outside.path().join("escaped.txt").exists());
    }

    /// The TOCTOU case: the ancestor is a real directory at admission time
    /// and becomes a symlink only after the effect was admitted. The
    /// descriptor walk at promotion time must still refuse it.
    #[cfg(unix)]
    #[tokio::test]
    async fn ancestor_swapped_for_symlink_after_admission_is_denied() {
        let source = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir(source.path().join("dir")).unwrap();
        std::fs::write(source.path().join("dir/a.txt"), "baseline").unwrap();
        let candidate = CandidateWorkspace::create(source.path(), "n1", 0, "r1").unwrap();
        candidate
            .apply(&write_call("dir/a.txt", "mutated"), &write_entry())
            .await
            .unwrap();

        std::fs::write(outside.path().join("a.txt"), "baseline").unwrap();
        std::fs::remove_dir_all(source.path().join("dir")).unwrap();
        std::os::unix::fs::symlink(outside.path(), source.path().join("dir")).unwrap();

        let error = candidate.promote().unwrap_err();
        assert!(
            error.to_string().contains("descending into dir"),
            "expected symlink refusal, got: {error:#}"
        );
        assert_eq!(
            std::fs::read_to_string(outside.path().join("a.txt")).unwrap(),
            "baseline"
        );
    }

    #[tokio::test]
    async fn resumed_candidate_keeps_the_original_source_precondition() {
        let source = tempfile::tempdir().unwrap();
        std::fs::write(source.path().join("a.txt"), "baseline").unwrap();
        let first = CandidateWorkspace::create(source.path(), "n1", 0, "r1").unwrap();
        first
            .apply(&write_call("a.txt", "accepted"), &write_entry())
            .await
            .unwrap();
        let seed = first.export_accepted().await.unwrap();

        std::fs::write(source.path().join("a.txt"), "user edit while stopped").unwrap();
        let resumed = CandidateWorkspace::create(source.path(), "n1", 0, "r2").unwrap();
        resumed.restore_exported(&seed).unwrap();
        assert!(resumed.promote().is_err());
        assert_eq!(
            std::fs::read_to_string(source.path().join("a.txt")).unwrap(),
            "user edit while stopped"
        );
    }
}
