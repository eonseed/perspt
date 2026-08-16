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
    tracked: Mutex<BTreeSet<String>>,
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

    pub fn touched_paths(&self) -> Vec<String> {
        self.tracked.lock().unwrap().iter().cloned().collect()
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

    /// Promote only paths named by governed proposals.
    pub fn promote(&self) -> Result<Vec<String>> {
        let paths: Vec<String> = self.tracked.lock().unwrap().iter().cloned().collect();
        let mut source_before = BTreeMap::new();
        for rel in &paths {
            let rel = validate_relative_path(rel)?;
            source_before.insert(rel.clone(), std::fs::read(self.source_root.join(rel)).ok());
        }

        let result = (|| {
            for rel in &paths {
                let rel = validate_relative_path(rel)?;
                reject_symlink_ancestor(&self.overlay_root, &rel)?;
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

        let mut contents = BTreeMap::new();
        for rel in &scope {
            let path = self.overlay_root.join(rel);
            let value = match std::fs::read(&path) {
                Ok(bytes) => Some(bytes),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e).with_context(|| format!("checkpointing {rel}")),
            };
            contents.insert(rel.clone(), value);
        }
        let state = snapshot_workspace(&self.overlay_root, &scope)?;
        let id = uuid::Uuid::new_v4().to_string();
        self.snapshots.lock().unwrap().insert(
            id.clone(),
            CandidateSnapshot {
                contents,
                mutations: self.mutations.load(Ordering::SeqCst),
            },
        );
        Ok(CandidateCheckpoint {
            id,
            witness: CandidateStateWitness {
                state_root: state.root_hash(),
                graph_revision: self.graph_revision.clone(),
                node_id: self.node_id.clone(),
                node_generation: self.generation,
                canonical_scope: scope,
                barrier_channels: BTreeMap::new(),
            },
        })
    }

    fn current_witness(&self) -> Result<CandidateStateWitness> {
        let scope: Vec<String> = self.tracked.lock().unwrap().iter().cloned().collect();
        let state = snapshot_workspace(&self.overlay_root, &scope)?;
        Ok(CandidateStateWitness {
            state_root: state.root_hash(),
            graph_revision: self.graph_revision.clone(),
            node_id: self.node_id.clone(),
            node_generation: self.generation,
            canonical_scope: scope,
            barrier_channels: BTreeMap::new(),
        })
    }

    fn restore_snapshot(&self, checkpoint: &CandidateCheckpoint) -> Result<()> {
        let snapshots = self.snapshots.lock().unwrap();
        let snapshot = snapshots
            .get(&checkpoint.id)
            .with_context(|| format!("unknown candidate checkpoint {}", checkpoint.id))?;
        for (rel, content) in &snapshot.contents {
            let path = self.overlay_root.join(rel);
            match content {
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
        )
        .await
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
        for rel in ["path", "to", "from"]
            .iter()
            .filter_map(|field| call.arguments.get(*field).and_then(|v| v.as_str()))
        {
            validate_relative_path(rel)?;
            if !entry.effect.is_read_only() {
                reject_symlink_ancestor(&self.overlay_root, rel)?;
            }
        }

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
            return Ok(EffectOutcome {
                output: if execution.success() {
                    output
                } else {
                    format!("tool failed (exit {:?}): {output}", execution.exit_code)
                },
                mutated: false,
            });
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
        let mutates_source = result.success
            && matches!(
                entry.effect,
                perspt_sdk::EffectKind::WriteArtifact
                    | perspt_sdk::EffectKind::ApplyPatch
                    | perspt_sdk::EffectKind::MoveFile
                    | perspt_sdk::EffectKind::DeleteFile
                    | perspt_sdk::EffectKind::MutateDependencies
            );
        if mutates_source {
            self.mutations.fetch_add(1, Ordering::SeqCst);
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
}

impl CandidateWorkspace {
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
        let output = if kind == "diagnostics" {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            serde_json::to_string(&session.client.get_diagnostics(&relative).await)?
        } else {
            let symbol = call
                .arguments
                .get("symbol")
                .and_then(serde_json::Value::as_str)
                .context("definition, references, and hover queries require symbol")?;
            let (line, character) = symbol_position(&content, symbol)
                .with_context(|| format!("symbol {symbol:?} not found in {relative}"))?;
            match kind {
                "definition" => serde_json::to_string(
                    &session.client.goto_definition(&path, line, character).await,
                )?,
                "references" => serde_json::to_string(
                    &session
                        .client
                        .find_references(&path, line, character, true)
                        .await,
                )?,
                "hover" => {
                    serde_json::to_string(&session.client.hover(&path, line, character).await)?
                }
                other => anyhow::bail!("unknown lsp_query kind {other:?}"),
            }
        };
        Ok(EffectOutcome {
            output,
            mutated: false,
        })
    }
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
}

#[async_trait::async_trait]
impl CandidateMeasurer for CodingCandidateMeasurer<'_> {
    async fn measure(&self) -> Result<Measured> {
        let registry = perspt_core::PluginRegistry::new();
        let plugins = registry.detect_all(self.candidate.overlay_root());
        let mut residuals = Vec::new();
        let mut all_passed = !plugins.is_empty();
        let mut jobs = Vec::new();

        for plugin in plugins {
            let adapter_id = match plugin.name().to_ascii_lowercase().as_str() {
                "js" | "javascript" | "typescript" => LanguageId::new("typescript"),
                other => LanguageId::new(other),
            };
            for capability in plugin.verifier_profile().capabilities {
                let Some(command) = capability.effective_command() else {
                    residuals.push(sensor_unavailable(
                        &self.node_id,
                        self.generation,
                        &format!("{}:{}", plugin.name(), capability.stage),
                    )?);
                    all_passed = false;
                    continue;
                };
                jobs.push(VerifierJob {
                    plugin: plugin.name().to_string(),
                    adapter_id: adapter_id.clone(),
                    stage: capability.stage,
                    command: command.to_string(),
                });
            }
        }

        let ran = jobs.len();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.max_parallel));
        let mut workers = tokio::task::JoinSet::new();
        for (ordinal, job) in jobs.into_iter().enumerate() {
            let semaphore = semaphore.clone();
            let root = self.candidate.overlay_root().to_path_buf();
            let allow_unisolated = self.candidate.allow_unisolated_verifiers;
            workers.spawn(async move {
                let _permit = semaphore.acquire_owned().await.expect("semaphore closed");
                let execution = run_governed_verifier(
                    root,
                    job.command.clone(),
                    allow_unisolated,
                    format!("{}-{ordinal}", job.stage),
                )
                .await;
                (job, execution)
            });
        }

        while let Some(result) = workers.join_next().await {
            let (job, execution) = result.context("verifier worker panicked")?;
            let execution = match execution {
                Ok(execution) => execution,
                Err(error) => {
                    residuals.push(sensor_unavailable(
                        &self.node_id,
                        self.generation,
                        &format!("{}:{} ({error})", job.plugin, job.stage),
                    )?);
                    all_passed = false;
                    continue;
                }
            };
            let combined = execution.output;
            if let Some(adapter) = self.adapters.get(&job.adapter_id) {
                residuals.extend(adapter.parse_diagnostics(
                    &self.node_id,
                    self.generation,
                    &combined,
                ));
            }
            if !execution.success {
                all_passed = false;
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
                        &format!("{} failed: {}", job.stage, concise(&combined)),
                    )?);
                }
            }
        }

        if ran == 0 {
            all_passed = false;
        }
        let scope = DomainScope {
            label: self.node_id.clone(),
            paths: Vec::new(),
        };
        let score = score_candidate(&self.domain.energy_model(&scope), &residuals)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let correction: Option<CorrectionDirection> = self
            .domain
            .correction_directions(&residuals)
            .into_iter()
            .next();
        Ok(Measured {
            hard_pass: self.candidate.has_mutated() && all_passed && residuals.is_empty(),
            energy: score.total,
            residuals,
            correction,
        })
    }
}

struct CandidateSnapshot {
    contents: BTreeMap<String, Option<Vec<u8>>>,
    mutations: u32,
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
}

async fn run_governed_verifier(
    root: PathBuf,
    command: String,
    allow_unisolated: bool,
    target_suffix: String,
) -> Result<VerifierExecution> {
    let tmp = root.join(".perspt-tmp").join(&target_suffix);
    let target = root.join(".perspt-target").join(&target_suffix);
    std::fs::create_dir_all(&tmp)?;
    std::fs::create_dir_all(&target)?;
    let mut process = if allow_unisolated {
        let mut process = Command::new("/bin/sh");
        process.arg("-c").arg(&command).current_dir(&root);
        process
    } else {
        isolated_command(&root, &command)?
    };
    process
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_TARGET_DIR", target)
        .env("TMPDIR", tmp)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
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
fn isolated_command(root: &Path, command: &str) -> Result<Command> {
    let profile = macos_sandbox_profile(root);
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
fn macos_sandbox_profile(root: &Path) -> String {
    let escaped = root.to_string_lossy().replace('"', "\\\"");
    format!(
        "(version 1)\n\
             (deny default)\n\
             (allow process*)\n\
             (allow sysctl-read)\n\
             (allow file-read*)\n\
             (allow file-write* (literal \"/dev/null\"))\n\
             (allow file-write* (subpath \"{escaped}\"))\n\
             (deny network*)"
    )
}

#[cfg(target_os = "linux")]
fn isolated_command(root: &Path, command: &str) -> Result<Command> {
    let bwrap = ["/usr/bin/bwrap", "/bin/bwrap"]
        .into_iter()
        .find(|path| Path::new(path).is_file())
        .context("bubblewrap is required for governed verifier execution")?;
    let mut process = Command::new(bwrap);
    process
        .args(["--die-with-parent", "--unshare-net", "--ro-bind", "/", "/"])
        .arg("--bind")
        .arg(root)
        .arg(root)
        .arg("--tmpfs")
        .arg("/tmp")
        .arg("--chdir")
        .arg(root)
        .arg("/bin/sh")
        .arg("-c")
        .arg(command);
    Ok(process)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn isolated_command(_root: &Path, _command: &str) -> Result<Command> {
    anyhow::bail!("this platform has no registered governed process sandbox")
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

fn reject_symlink_ancestor(root: &Path, relative: &str) -> Result<()> {
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
        if matches!(name.to_str(), Some(".git" | ".perspt" | "target")) {
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
        let profile = macos_sandbox_profile(Path::new("/private/tmp/candidate"));
        assert!(profile.contains("(allow file-write* (literal \"/dev/null\"))"));
        assert!(profile.contains("(allow file-write* (subpath \"/private/tmp/candidate\"))"));
        assert_eq!(profile.matches("allow file-write*").count(), 2);
        assert!(profile.contains("(deny network*)"));
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

    #[tokio::test]
    async fn checkpoint_restore_and_promotion_use_touched_paths() {
        let source = tempfile::tempdir().unwrap();
        std::fs::write(source.path().join("a.txt"), "old").unwrap();
        let candidate = CandidateWorkspace::create(source.path(), "n1", 0, "r1").unwrap();
        let before = candidate.checkpoint(&["a.txt".into()]).await.unwrap();
        std::fs::write(candidate.overlay_root().join("a.txt"), "new").unwrap();
        candidate.restore(&before).await.unwrap();
        assert_eq!(
            std::fs::read_to_string(candidate.overlay_root().join("a.txt")).unwrap(),
            "old"
        );
        std::fs::write(candidate.overlay_root().join("a.txt"), "accepted").unwrap();
        candidate.promote().unwrap();
        assert_eq!(
            std::fs::read_to_string(source.path().join("a.txt")).unwrap(),
            "accepted"
        );
    }
}
