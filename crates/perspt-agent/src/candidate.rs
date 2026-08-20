//! Reversible coding candidate overlay and compiler-backed measurement.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use perspt_sdk::{CandidateStateWitness, ToolEntry};
use tempfile::TempDir;

use crate::realize::snapshot_workspace;
use crate::toolloop::{CandidateCheckpoint, EffectExecutor, EffectOutcome};
use crate::tools::AgentTools;
use crate::verifier::{run_governed_verifier, VerifierExecution};

pub use crate::measure::CodingCandidateMeasurer;

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
    /// Per-stage wall-clock limits for governed verifier processes.
    verifier_timeouts: crate::verifier::VerifierTimeouts,
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
            verifier_timeouts: crate::verifier::VerifierTimeouts::default(),
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

    /// Configure per-stage governed verifier timeouts
    /// (`[verification] stage_timeout_secs` + overrides).
    pub fn set_verifier_timeouts(&mut self, timeouts: crate::verifier::VerifierTimeouts) {
        self.verifier_timeouts = timeouts;
    }

    pub(crate) fn verifier_timeouts(&self) -> crate::verifier::VerifierTimeouts {
        self.verifier_timeouts
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

    /// One `(plugin, command, stage)` per plugin selected for an interactive
    /// verifier tool call. Plugins owning a mutated path are preferred, so a
    /// mixed-language repository runs the suite the model is actually
    /// editing; with nothing mutated yet (or only unowned paths) every
    /// detected plugin runs — matching the final gate's coverage. A
    /// `run_test` `filter` argument narrows through the plugin's native
    /// filter form; the acceptance gate never narrows.
    pub(crate) fn commands_for(
        &self,
        tool: &str,
        arguments: &serde_json::Value,
    ) -> Result<Vec<(String, String, perspt_core::plugin::VerifierStage)>> {
        use perspt_core::plugin::VerifierStage;
        let registry = perspt_core::PluginRegistry::new();
        let plugins = registry.detect_all(&self.overlay_root);
        if plugins.is_empty() {
            bail!("no language plugin detected");
        }
        let mutated = self.mutated_paths.lock().unwrap().clone();
        let owned: Vec<usize> = (0..plugins.len())
            .filter(|&i| mutated.iter().any(|path| plugins[i].owns_file(path)))
            .collect();
        let selected: Vec<usize> = if owned.is_empty() {
            (0..plugins.len()).collect()
        } else {
            owned
        };
        let filter = match arguments.get("filter").and_then(serde_json::Value::as_str) {
            Some(raw) => Some(sanitize_test_filter(raw)?),
            None => None,
        };
        let mut commands = Vec::new();
        for index in selected {
            let plugin = &plugins[index];
            let command = match tool {
                "run_test" => Some((
                    plugin.test_command_with_filter(filter.as_deref()),
                    VerifierStage::Test,
                )),
                "run_build" => plugin
                    .build_command()
                    .or_else(|| plugin.syntax_check_command())
                    .map(|command| (command, VerifierStage::Build)),
                "run_formatter" => plugin
                    .format_command()
                    .map(|command| (command, VerifierStage::Format)),
                _ => bail!("not a verifier tool: {tool}"),
            };
            if let Some((command, stage)) = command {
                commands.push((plugin.name().to_string(), command, stage));
            }
        }
        if commands.is_empty() {
            match tool {
                "run_build" => bail!("active language has no build or syntax command"),
                "run_formatter" => bail!("active language has no governed formatter command"),
                other => bail!("active language has no {other} command"),
            }
        }
        Ok(commands)
    }

    pub(crate) async fn run_governed_verifier(
        &self,
        command: &str,
        stage: Option<perspt_core::plugin::VerifierStage>,
    ) -> Result<VerifierExecution> {
        run_governed_verifier(
            self.overlay_root.clone(),
            command.to_string(),
            self.allow_unisolated_verifiers,
            crate::measure::SHARED_TARGET_SUFFIX.into(),
            self.verifier_env(),
            self.verifier_timeouts.for_stage(stage),
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
    pub(crate) fn immutable_test_oracle(&self) -> Result<Option<ImmutableTestOracle>> {
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

pub(crate) struct ImmutableTestOracle {
    _temp: TempDir,
    pub(crate) root: PathBuf,
}

/// Interactive test filters run under `sh -c`, so only a conservative
/// identifier alphabet is accepted — anything else is refused with a
/// model-facing error rather than quoted through.
fn sanitize_test_filter(raw: &str) -> Result<String> {
    let filter = raw.trim();
    if filter.is_empty() {
        bail!("'filter' must not be empty");
    }
    if !filter
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '.' | '-'))
    {
        bail!(
            "'filter' may only contain letters, digits, and _ : . - \
             (got {filter:?})"
        );
    }
    Ok(filter.to_string())
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

    /// Fixture with both a Cargo and a uv project marker.
    fn mixed_language_source() -> tempfile::TempDir {
        let source = tempfile::tempdir().unwrap();
        std::fs::write(
            source.path().join("Cargo.toml"),
            "[package]\nname = \"x\"\nversion = \"0.0.1\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(source.path().join("src")).unwrap();
        std::fs::write(source.path().join("src/lib.rs"), "").unwrap();
        std::fs::write(
            source.path().join("pyproject.toml"),
            "[project]\nname = \"x\"\nversion = \"0.0.1\"\n",
        )
        .unwrap();
        source
    }

    #[tokio::test]
    async fn mixed_repo_interactive_tests_cover_all_plugins_until_a_mutation_selects_one() {
        let source = mixed_language_source();
        let candidate = CandidateWorkspace::create(source.path(), "n1", 0, "r1").unwrap();

        let all = candidate
            .commands_for("run_test", &serde_json::json!({}))
            .unwrap();
        assert!(all.len() >= 2, "both plugins must run before any mutation");

        candidate
            .apply(&write_call("src/lib.rs", "pub fn f() {}"), &write_entry())
            .await
            .unwrap();
        let narrowed = candidate
            .commands_for("run_test", &serde_json::json!({}))
            .unwrap();
        assert_eq!(narrowed.len(), 1);
        assert!(narrowed[0].1.starts_with("cargo test"));
    }

    #[test]
    fn run_test_filter_threads_through_and_rejects_injection() {
        let source = mixed_language_source();
        let candidate = CandidateWorkspace::create(source.path(), "n1", 0, "r1").unwrap();

        let filtered = candidate
            .commands_for("run_test", &serde_json::json!({"filter": "my_test::case"}))
            .unwrap();
        assert!(filtered
            .iter()
            .any(|(_, command, _)| command.contains("my_test::case")));

        let injected =
            candidate.commands_for("run_test", &serde_json::json!({"filter": "x; rm -rf /"}));
        assert!(injected.is_err(), "shell metacharacters must be refused");
    }

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
