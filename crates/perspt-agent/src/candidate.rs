//! Reversible coding candidate overlay and compiler-backed measurement.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use perspt_coding::{CodingAdapterRegistry, CodingDomain, LanguageId};
use perspt_sandbox::{ProcessPolicy, ProcessSandbox, SandboxedCommand};
use perspt_sdk::{
    score_candidate, AgentDomainPackage, CandidateStateWitness, CorrectionDirection, DomainScope,
    IndependenceRoute, ResidualClass, ResidualEvent, ResidualSeverity, SensorRef, ToolEntry,
};
use tempfile::TempDir;
use tokio::process::Command;

use crate::realize::snapshot_workspace;
use crate::toolloop::{
    CandidateCheckpoint, CandidateMeasurer, EffectExecutor, EffectOutcome, Measured,
};
use crate::tools::{AgentTools, ToolCall};

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
    lsp_sessions: tokio::sync::Mutex<HashMap<String, LspSession>>,
    mutations: AtomicU32,
    node_id: String,
    generation: u32,
    graph_revision: String,
    allow_unisolated_verifiers: bool,
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
            overlay_tools: AgentTools::new(overlay_root.clone(), false),
            source_tools: AgentTools::new(source_root.clone(), false),
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
        })
    }

    pub fn overlay_root(&self) -> &Path {
        &self.overlay_root
    }

    pub fn has_mutated(&self) -> bool {
        self.mutations.load(Ordering::SeqCst) > 0
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

    /// Promote only paths mutated by admitted governed proposals.
    pub fn promote(&self) -> Result<Vec<String>> {
        let paths: Vec<String> = self.mutated_paths.lock().unwrap().iter().cloned().collect();
        let mut source_before = BTreeMap::new();
        for rel in &paths {
            let rel = validate_relative_path(rel)?;
            reject_symlink_ancestor(&self.source_root, &rel)?;
            let current = read_optional_file(&self.source_root.join(&rel))?;
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
            source_before.insert(rel.clone(), current);
        }

        let result = (|| {
            for rel in &paths {
                let rel = validate_relative_path(rel)?;
                reject_symlink_ancestor(&self.overlay_root, &rel)?;
                reject_symlink_ancestor(&self.source_root, &rel)?;
                let from = self.overlay_root.join(&rel);
                let to = self.source_root.join(&rel);
                if from.is_file() {
                    let parent = to.parent().context("promotion target has no parent")?;
                    std::fs::create_dir_all(parent)?;
                    let staged = parent.join(format!(".perspt-promote-{}", uuid::Uuid::new_v4()));
                    std::fs::copy(&from, &staged).with_context(|| {
                        format!("staging {} for {}", from.display(), to.display())
                    })?;
                    std::fs::rename(&staged, &to).with_context(|| {
                        format!("promoting {} to {}", from.display(), to.display())
                    })?;
                } else if to.is_file() {
                    std::fs::remove_file(&to)
                        .with_context(|| format!("promoting deletion of {}", to.display()))?;
                }
            }
            Ok::<(), anyhow::Error>(())
        })();
        if let Err(error) = result {
            for (rel, content) in source_before {
                let path = self.source_root.join(rel);
                match content {
                    Some(bytes) => {
                        if let Some(parent) = path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        let _ = std::fs::write(path, bytes);
                    }
                    None if path.is_file() => {
                        let _ = std::fs::remove_file(path);
                    }
                    None => {}
                }
            }
            return Err(error);
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

    fn command_for(&self, tool: &str) -> Result<String> {
        let registry = perspt_core::PluginRegistry::new();
        let plugins = registry.detect_all(&self.overlay_root);
        let plugin = plugins.first().context("no language plugin detected")?;
        match tool {
            "run_test" => Ok(plugin.test_command()),
            "run_build" => plugin
                .build_command()
                .or_else(|| plugin.syntax_check_command())
                .context("active language has no build or syntax command"),
            "run_formatter" => bail!("active language has no governed formatter command"),
            _ => bail!("not a verifier tool: {tool}"),
        }
    }

    async fn run_governed_verifier(&self, command: &str) -> Result<VerifierExecution> {
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

        if matches!(
            call.name.as_str(),
            "run_test" | "run_build" | "run_formatter"
        ) {
            let command = self.command_for(&call.name)?;
            let execution = self.run_governed_verifier(&command).await?;
            return Ok(EffectOutcome {
                output: if execution.success {
                    execution.output
                } else {
                    format!("tool failed: {}", execution.output)
                },
                mutated: false,
            });
        }

        if call.name == "exec" {
            return self.run_inspection_exec(call).await;
        }

        if call.name == "lsp_query" {
            return self.run_lsp_query(call).await;
        }

        let mut name = call.name.as_str();
        let arguments = json_arguments(&call.arguments)?;
        let tools = match name {
            "grep" => {
                name = "search_code";
                &self.overlay_tools
            }
            "git_read" => &self.source_tools,
            _ => &self.overlay_tools,
        };
        let result = tools
            .execute(&ToolCall {
                name: name.to_string(),
                arguments,
            })
            .await;
        let output = if result.success {
            result.output
        } else {
            format!("tool failed: {}", result.error.unwrap_or_default())
        };
        let mutates_source = result.success && mutating;
        if mutates_source {
            self.mutations.fetch_add(1, Ordering::SeqCst);
            let mut mutated = self.mutated_paths.lock().unwrap();
            for rel in &named_paths {
                mutated.insert(rel.clone());
            }
        }
        Ok(EffectOutcome {
            output,
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

impl CandidateWorkspace {
    /// Governed direct-program execution for read-only inspection tools.
    async fn run_inspection_exec(
        &self,
        call: &perspt_sdk::ProviderToolCall,
    ) -> Result<EffectOutcome> {
        let raw = call
            .arguments
            .get("command")
            .and_then(serde_json::Value::as_str)
            .context("exec requires a command string")?;
        let invocation = perspt_sdk::canonicalize(raw, ".");
        anyhow::ensure!(
            perspt_sdk::classify_tier(&invocation) == perspt_sdk::CommandTier::Inspection,
            "exec only admits commands classified as inspection"
        );
        let perspt_sdk::CommandInvocation::Program { program, args, .. } = invocation else {
            anyhow::bail!("exec does not admit shell composition");
        };
        validate_inspection_args(&args)?;
        let policy = if self.allow_unisolated_verifiers {
            ProcessPolicy::inspection(&self.overlay_root).best_effort()
        } else {
            ProcessPolicy::inspection(&self.overlay_root)
        };
        let sandbox = ProcessSandbox::new(program, args, policy)?;
        let execution = tokio::task::spawn_blocking(move || sandbox.execute())
            .await
            .context("inspection process worker panicked")??;
        let output = format!("{}{}", execution.stdout, execution.stderr);
        Ok(EffectOutcome {
            output: if execution.success() {
                output
            } else {
                format!("tool failed (exit {:?}): {output}", execution.exit_code)
            },
            mutated: false,
        })
    }

    async fn run_lsp_query(&self, call: &perspt_sdk::ProviderToolCall) -> Result<EffectOutcome> {
        let relative = call
            .arguments
            .get("path")
            .and_then(serde_json::Value::as_str)
            .context("lsp_query requires path")?;
        let relative = validate_relative_path(relative)?;
        let path = self.overlay_root.join(&relative);
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("reading LSP document {relative}"))?;
        let registry = perspt_core::PluginRegistry::new();
        let plugin = registry
            .detect_all(&self.overlay_root)
            .into_iter()
            .find(|plugin| plugin.owns_file(&relative))
            .context("no language plugin owns the LSP document")?;
        let config = plugin.get_lsp_config();
        let mut sessions = self.lsp_sessions.lock().await;
        if !sessions.contains_key(plugin.name()) {
            let mut client = crate::lsp::LspClient::from_config(&config);
            client
                .start_with_config(&config, &self.overlay_root)
                .await?;
            sessions.insert(
                plugin.name().to_string(),
                LspSession {
                    client,
                    versions: HashMap::new(),
                },
            );
        }
        let session = sessions
            .get_mut(plugin.name())
            .context("LSP session disappeared after insertion")?;
        if let Some(version) = session.versions.get_mut(&relative) {
            *version += 1;
            session.client.did_change(&path, &content, *version).await?;
        } else {
            session.versions.insert(relative.clone(), 1);
            session.client.did_open(&path, &content).await?;
        }
        let kind = call
            .arguments
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .context("lsp_query requires kind")?;
        let output = dispatch_lsp_query(session, kind, call, &path, &relative, &content).await?;
        Ok(EffectOutcome {
            output,
            mutated: false,
        })
    }
}

async fn dispatch_lsp_query(
    session: &mut LspSession,
    kind: &str,
    call: &perspt_sdk::ProviderToolCall,
    path: &Path,
    relative: &str,
    content: &str,
) -> Result<String> {
    if kind == "diagnostics" {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        return Ok(serde_json::to_string(
            &session.client.get_diagnostics(relative).await,
        )?);
    }
    let symbol = call
        .arguments
        .get("symbol")
        .and_then(serde_json::Value::as_str)
        .context("definition, references, and hover queries require symbol")?;
    let (line, character) = symbol_position(content, symbol)
        .with_context(|| format!("symbol {symbol:?} not found in {relative}"))?;
    Ok(match kind {
        "definition" => {
            serde_json::to_string(&session.client.goto_definition(path, line, character).await)?
        }
        "references" => serde_json::to_string(
            &session
                .client
                .find_references(path, line, character, true)
                .await,
        )?,
        "hover" => serde_json::to_string(&session.client.hover(path, line, character).await)?,
        other => anyhow::bail!("unknown lsp_query kind {other:?}"),
    })
}

struct LspSession {
    client: crate::lsp::LspClient,
    versions: HashMap<String, i32>,
}

fn symbol_position(content: &str, symbol: &str) -> Option<(u32, u32)> {
    for (line, text) in content.lines().enumerate() {
        if let Some(column) = text.find(symbol) {
            let utf16_column = text[..column].encode_utf16().count();
            return Some((u32::try_from(line).ok()?, u32::try_from(utf16_column).ok()?));
        }
    }
    None
}

fn validate_inspection_args(args: &[String]) -> Result<()> {
    const PROCESS_SPAWNING_OR_WRITING_FLAGS: &[&str] = &[
        "-exec",
        "-execdir",
        "-ok",
        "-okdir",
        "-delete",
        "-fls",
        "-fprint",
        "-fprint0",
        "-fprintf",
        "--pre",
        "--hostname-bin",
        "--ext-diff",
        "--textconv",
    ];
    for argument in args {
        let lower = argument.to_ascii_lowercase();
        if PROCESS_SPAWNING_OR_WRITING_FLAGS
            .iter()
            .any(|flag| lower == *flag || lower.starts_with(&format!("{flag}=")))
        {
            anyhow::bail!("inspection argument is not read-only: {argument:?}");
        }
        if argument.starts_with('-') || argument == "." {
            continue;
        }
        let path = Path::new(argument);
        if path.is_absolute() || path.components().any(|part| part == Component::ParentDir) {
            anyhow::bail!("inspection argument escapes the workspace: {argument:?}");
        }
    }
    Ok(())
}

/// Complete plugin-backed candidate measurement for one coding node.
pub struct CodingCandidateMeasurer<'a> {
    candidate: &'a CandidateWorkspace,
    node_id: String,
    generation: u32,
    domain: CodingDomain,
    adapters: CodingAdapterRegistry,
    max_parallel: usize,
}

impl<'a> CodingCandidateMeasurer<'a> {
    pub fn new(candidate: &'a CandidateWorkspace, node_id: &str, generation: u32) -> Self {
        Self {
            candidate,
            node_id: node_id.into(),
            generation,
            domain: CodingDomain::new(),
            adapters: CodingAdapterRegistry::with_builtins(),
            max_parallel: 4,
        }
    }

    pub fn with_max_parallel(mut self, max_parallel: usize) -> Self {
        self.max_parallel = max_parallel.max(1);
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

    fn score(&self, residuals: &[ResidualEvent]) -> Result<(f64, Option<CorrectionDirection>)> {
        let scope = DomainScope {
            label: self.node_id.clone(),
            paths: Vec::new(),
        };
        let score = score_candidate(&self.domain.energy_model(&scope), residuals)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let correction = self
            .domain
            .correction_directions(residuals)
            .into_iter()
            .next();
        Ok((score.total, correction))
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
        let (energy, correction) = self.score(&residuals)?;
        Ok(Measured {
            hard_pass: self.candidate.has_mutated() && all_passed && residuals.is_empty(),
            energy,
            residuals,
            correction,
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
        let (energy, correction) = self.score(&residuals)?;
        Ok(Measured {
            hard_pass: false,
            energy,
            residuals,
            correction,
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

struct VerifierExecution {
    success: bool,
    output: String,
}

struct VerifierJob {
    plugin: String,
    adapter_id: LanguageId,
    stage: perspt_core::plugin::VerifierStage,
    command: String,
    root: PathBuf,
}

struct ImmutableTestOracle {
    _temp: TempDir,
    root: PathBuf,
}

async fn run_governed_verifier(
    root: PathBuf,
    command: String,
    allow_unisolated: bool,
    target_suffix: String,
    extra_env: Vec<(String, String)>,
) -> Result<VerifierExecution> {
    let tmp = root.join(".perspt-tmp").join(&target_suffix);
    let target = root.join(".perspt-target").join(&target_suffix);
    std::fs::create_dir_all(&tmp)?;
    std::fs::create_dir_all(&target)?;
    let read_roots = verifier_read_roots(&root, &extra_env);
    let mut process = if allow_unisolated {
        let mut process = Command::new("/bin/sh");
        process.arg("-c").arg(&command).current_dir(&root);
        process
    } else {
        isolated_command(&root, &command, &read_roots)?
    };
    // Toolchain caches must live inside the writable overlay: the sandbox
    // denies network and writes outside the candidate, so `uv` gets a
    // per-run cache dir (mirroring CARGO_TARGET_DIR) and stays offline —
    // verifiers run against the project's already-synced environment.
    let uv_cache = root.join(".perspt-tmp").join("uv-cache");
    let isolated_home = root.join(".perspt-home");
    std::fs::create_dir_all(&uv_cache)?;
    std::fs::create_dir_all(&isolated_home)?;
    let original_home = std::env::var_os("HOME").map(PathBuf::from);
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| original_home.as_ref().map(|home| home.join(".cargo")));
    let rustup_home = std::env::var_os("RUSTUP_HOME")
        .map(PathBuf::from)
        .or_else(|| original_home.as_ref().map(|home| home.join(".rustup")));
    process
        .env("HOME", &isolated_home)
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_TARGET_DIR", target)
        .env("UV_CACHE_DIR", uv_cache)
        .env("UV_OFFLINE", "1")
        .env("TMPDIR", tmp)
        .envs(extra_env)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(path) = cargo_home.filter(|path| path.is_dir()) {
        process.env("CARGO_HOME", path);
    }
    if let Some(path) = rustup_home.filter(|path| path.is_dir()) {
        process.env("RUSTUP_HOME", path);
    }
    let output = tokio::time::timeout(std::time::Duration::from_secs(180), process.output())
        .await
        .context("governed verifier exceeded 180 second limit")??;
    Ok(VerifierExecution {
        success: output.status.success(),
        output: format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    })
}

#[cfg(target_os = "macos")]
fn isolated_command(root: &Path, command: &str, read_roots: &[PathBuf]) -> Result<Command> {
    let profile = macos_sandbox_profile(root, read_roots);
    let mut process = Command::new("/usr/bin/sandbox-exec");
    process
        .arg("-p")
        .arg(profile)
        .arg("/bin/sh")
        .arg("-c")
        .arg(command)
        .current_dir(root);
    Ok(process)
}

#[cfg(target_os = "macos")]
fn macos_sandbox_profile(root: &Path, read_roots: &[PathBuf]) -> String {
    let escaped = root.to_string_lossy().replace('"', "\\\"");
    let mut readable = read_roots.to_vec();
    readable.push(root.to_path_buf());
    let users_deny = deny_data_except("/Users", &readable);
    let home_deny = deny_data_except("/home", &readable);
    let root_home_deny = deny_data_except("/root", &readable);
    let private_tmp_deny = deny_data_except("/private/tmp", &readable);
    let user_tmp_deny = deny_data_except("/private/var/folders", &readable);
    let volumes_deny = deny_data_except("/Volumes", &readable);
    let network_deny = deny_data_except("/Network", &readable);
    let extra_reads = read_roots
        .iter()
        .map(|path| {
            format!(
                "(allow file-read* (subpath \"{}\"))",
                path.to_string_lossy()
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "(version 1)\n\
             (deny default)\n\
             (allow process*)\n\
             (allow sysctl-read)\n\
             (allow file-read*)\n\
             {users_deny}\n\
             {home_deny}\n\
             {root_home_deny}\n\
             {private_tmp_deny}\n\
             {user_tmp_deny}\n\
             {volumes_deny}\n\
             {network_deny}\n\
             (allow file-read* (subpath \"{escaped}\"))\n\
             {extra_reads}\n\
             (allow file-write* (literal \"/dev/null\"))\n\
             (allow file-write* (subpath \"{escaped}\"))\n\
             (deny network*)"
    )
}

#[cfg(target_os = "macos")]
fn deny_data_except(parent: &str, allowed: &[PathBuf]) -> String {
    let exceptions = allowed
        .iter()
        .filter(|path| path.starts_with(parent))
        .map(|path| {
            let escaped = path
                .to_string_lossy()
                .replace('\\', "\\\\")
                .replace('"', "\\\"");
            format!("(require-not (subpath \"{escaped}\"))")
        })
        .collect::<Vec<_>>();
    if exceptions.is_empty() {
        format!("(deny file-read-data (subpath \"{parent}\"))")
    } else {
        format!(
            "(deny file-read-data (require-all (subpath \"{parent}\") {}))",
            exceptions.join(" ")
        )
    }
}

#[cfg(target_os = "linux")]
fn isolated_command(root: &Path, command: &str, read_roots: &[PathBuf]) -> Result<Command> {
    let bwrap = ["/usr/bin/bwrap", "/bin/bwrap"]
        .into_iter()
        .find(|path| Path::new(path).is_file())
        .context("bubblewrap is required for governed verifier execution")?;
    let mut process = Command::new(bwrap);
    process.args(["--die-with-parent", "--unshare-net", "--ro-bind", "/", "/"]);
    for sensitive in ["/home", "/root", "/Users"] {
        if Path::new(sensitive).exists() {
            process.arg("--tmpfs").arg(sensitive);
        }
    }
    process.arg("--tmpfs").arg("/tmp");
    for read_root in read_roots {
        if !read_root.starts_with(root) {
            prepare_bwrap_destination(&mut process, read_root);
            process.arg("--ro-bind").arg(read_root).arg(read_root);
        }
    }
    prepare_bwrap_destination(&mut process, root);
    process
        .arg("--bind")
        .arg(root)
        .arg(root)
        .arg("--chdir")
        .arg(root)
        .arg("/bin/sh")
        .arg("-c")
        .arg(command);
    Ok(process)
}

#[cfg(target_os = "linux")]
fn prepare_bwrap_destination(process: &mut Command, destination: &Path) {
    for directory in bwrap_destination_parents(destination) {
        process.arg("--dir").arg(directory);
    }
}

#[cfg(any(test, target_os = "linux"))]
fn bwrap_destination_parents(destination: &Path) -> Vec<PathBuf> {
    let Some(parent) = destination.parent() else {
        return Vec::new();
    };
    let masked_roots = [
        Path::new("/home"),
        Path::new("/root"),
        Path::new("/Users"),
        Path::new("/tmp"),
    ];
    let Some(masked) = masked_roots
        .into_iter()
        .find(|masked| parent.starts_with(masked))
    else {
        return Vec::new();
    };
    let mut missing_parents: Vec<_> = parent
        .ancestors()
        .take_while(|ancestor| *ancestor != masked)
        .map(Path::to_path_buf)
        .collect();
    missing_parents.reverse();
    missing_parents
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn isolated_command(_root: &Path, _command: &str, _read_roots: &[PathBuf]) -> Result<Command> {
    anyhow::bail!("this platform has no registered governed process sandbox")
}

fn verifier_read_roots(root: &Path, extra_env: &[(String, String)]) -> Vec<PathBuf> {
    let mut roots = BTreeSet::new();
    for (_, value) in extra_env {
        let path = PathBuf::from(value);
        if path.exists() {
            roots.insert(path.canonicalize().unwrap_or_else(|_| path.clone()));
            for interpreter in [
                path.join("bin/python"),
                path.join("bin/python3"),
                path.join("Scripts/python.exe"),
            ] {
                if let Ok(target) = interpreter.canonicalize() {
                    if let Some(installation) = target.parent().and_then(Path::parent) {
                        roots.insert(installation.to_path_buf());
                    }
                }
            }
        }
    }
    if let Ok(value) = std::env::var("CARGO_HOME") {
        extend_cargo_read_roots(&mut roots, &PathBuf::from(value));
    }
    if let Ok(value) = std::env::var("RUSTUP_HOME") {
        let path = PathBuf::from(value);
        if path.exists() {
            roots.insert(path.canonicalize().unwrap_or(path));
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        extend_cargo_read_roots(&mut roots, &home.join(".cargo"));
        for relative in [".rustup", ".local/bin"] {
            let path = home.join(relative);
            if path.exists() {
                roots.insert(path.canonicalize().unwrap_or(path));
            }
        }
    }
    if let Ok(path) = std::env::var("PATH") {
        for directory in std::env::split_paths(&path) {
            if directory.exists()
                && !directory.starts_with("/usr")
                && !directory.starts_with("/bin")
            {
                roots.insert(directory.canonicalize().unwrap_or(directory));
            }
        }
    }
    roots.remove(root);
    roots.into_iter().collect()
}

fn extend_cargo_read_roots(roots: &mut BTreeSet<PathBuf>, cargo_home: &Path) {
    // Cargo credentials are intentionally absent. Offline verification needs
    // only executables, configuration, and already-fetched source/index data.
    for relative in ["bin", "config", "config.toml", "git", "registry"] {
        let path = cargo_home.join(relative);
        if path.exists() {
            roots.insert(path.canonicalize().unwrap_or(path));
        }
    }
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

pub(crate) fn read_optional_file(path: &Path) -> Result<Option<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
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

fn json_arguments(value: &serde_json::Value) -> Result<HashMap<String, String>> {
    let object = value
        .as_object()
        .context("tool arguments must be an object")?;
    object
        .iter()
        .map(|(key, value)| {
            let rendered = match value {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            Ok((key.clone(), rendered))
        })
        .collect()
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

    #[test]
    fn inspection_arguments_cannot_escape_the_candidate() {
        assert!(validate_inspection_args(&["src".into(), "--hidden".into()]).is_ok());
        assert!(validate_inspection_args(&["../secret".into()]).is_err());
        assert!(validate_inspection_args(&["/etc/passwd".into()]).is_err());
        assert!(validate_inspection_args(&[".".into(), "-exec".into(), "sh".into()]).is_err());
        assert!(validate_inspection_args(&["--pre=sh".into()]).is_err());
    }

    #[test]
    fn lsp_symbol_columns_use_utf16_units() {
        assert_eq!(symbol_position("let cafe = 1;", "cafe"), Some((0, 4)));
        assert_eq!(symbol_position("let s = \"😀\"; y", "y"), Some((0, 14)));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_profile_only_allows_candidate_and_null_device_writes() {
        let profile = macos_sandbox_profile(
            Path::new("/private/tmp/candidate"),
            &[PathBuf::from("/Users/test/.rustup")],
        );
        assert!(profile.contains("(allow file-write* (literal \"/dev/null\"))"));
        assert!(profile.contains("(allow file-write* (subpath \"/private/tmp/candidate\"))"));
        assert_eq!(profile.matches("allow file-write*").count(), 2);
        assert!(profile
            .lines()
            .any(|line| line.trim() == "(allow file-read*)"));
        assert!(profile.contains("(deny file-read-data (require-all (subpath \"/Users\")"));
        assert!(profile.contains("(deny file-read-data (require-all (subpath \"/private/tmp\")"));
        assert!(profile.contains("/Users/test/.rustup"));
        assert!(profile.contains("(deny network*)"));
    }

    #[cfg(unix)]
    #[test]
    fn verifier_roots_follow_virtualenv_interpreter_symlinks() {
        let root = tempfile::tempdir().unwrap();
        let install = root.path().join("uv-python");
        let venv = root.path().join("project/.venv");
        std::fs::create_dir_all(install.join("bin")).unwrap();
        std::fs::create_dir_all(venv.join("bin")).unwrap();
        std::fs::write(install.join("bin/python3"), "binary").unwrap();
        std::os::unix::fs::symlink(install.join("bin/python3"), venv.join("bin/python3")).unwrap();

        let roots = verifier_read_roots(
            root.path(),
            &[("UV_PROJECT_ENVIRONMENT".into(), venv.display().to_string())],
        );
        assert!(roots.contains(&install.canonicalize().unwrap()));
    }

    #[test]
    fn cargo_verifier_roots_exclude_credentials() {
        let root = tempfile::tempdir().unwrap();
        let cargo_home = root.path().join("cargo-home");
        std::fs::create_dir_all(cargo_home.join("bin")).unwrap();
        std::fs::create_dir_all(cargo_home.join("registry")).unwrap();
        std::fs::write(cargo_home.join("credentials.toml"), "[registry]").unwrap();

        let mut roots = BTreeSet::new();
        extend_cargo_read_roots(&mut roots, &cargo_home);
        assert!(roots.contains(&cargo_home.join("bin").canonicalize().unwrap()));
        assert!(roots.contains(&cargo_home.join("registry").canonicalize().unwrap()));
        assert!(!roots.contains(&cargo_home));
        assert!(!roots.contains(&cargo_home.join("credentials.toml")));
    }

    #[test]
    fn bubblewrap_rebinds_recreate_only_masked_destination_parents() {
        assert_eq!(
            bwrap_destination_parents(Path::new("/home/user/.cargo/registry")),
            [
                PathBuf::from("/home/user"),
                PathBuf::from("/home/user/.cargo")
            ]
        );
        assert!(bwrap_destination_parents(Path::new("/usr/bin/cargo")).is_empty());
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
