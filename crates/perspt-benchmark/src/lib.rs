//! Optional PSP-10 evaluation tooling, independent of runtime mechanism tests.
//! Live runs require configured routes and credentials and never run in CI.
//!
//! Arms are a cumulative ladder over matched tasks (spec: Test and
//! Acceptance Plan). Every arm — the ungoverned direct harness included —
//! gets the same configured actuator, task, turn budget, per-call deadline,
//! and hidden test suite; governed arms may additionally use the configured
//! planner, speculator, verifier, and adjudicator. Every ablation is real:
//! paging can be disabled and adaptive search opens branches. Each task keeps
//! its paired per-arm results; the report
//! publishes paired differences with a seeded 10,000-resample percentile
//! bootstrap plus per-cell wall time, time-to-first-mutation,
//! time-to-first-test, denied/repeated calls, search counters, and every
//! infrastructure failure. The primary outcome is hidden-test hard pass.
//!
//! Tasks live in `corpus/<id>/`: a `task.json` (goal, hidden
//! check argv, tags, expectation), a `fixture/` tree the agent works in
//! (weak smoke tests at most), a `hidden/` tree of **withheld** oracle files,
//! and a `solution/` overlay used only by offline corpus validation. The
//! validator proves that every original fixture fails its hidden oracle and
//! that the reference overlay passes it. The hidden suite runs in a fresh
//! copy of the post-run fixture with `hidden/` overlaid on top — overwriting
//! any visible test the agent may have weakened — so the oracle and solution
//! are genuinely unseen and untouchable during evaluation.
//!
//! Adaptive search becomes the default only if the paired CI lower bound
//! for `p_adaptive - p_single` clears the predeclared margin on >= 30
//! tasks (Gate AE), repeated identical failures drop, and no hard-gate
//! regression escapes — the flip itself is a separate reviewed commit.
//!
//! The feature-gated CLI surface is `perspt benchmark`. It reads every role
//! route from the ordinary Perspt configuration; model names are never
//! benchmark arguments.

use std::path::{Path, PathBuf};
use std::process::Command;

use perspt_agent::transport::GenAiTransport;
use perspt_agent::{Psp9AgentRuntime, Psp9RunConfig};
use perspt_sdk::{Conversation, ModelId, ToolChoicePolicy, ToolSpec, TurnOutput};

mod config;
use config::{configured_model_id, configured_topology, load_config, ModelTopology};
mod corpus;
use corpus::materialize_corpus;
#[cfg(test)]
use corpus::source_corpus_root;

const MAX_TURNS: u32 = 12;
const DEADLINE_SECS: u64 = 120;
const CELL_DEADLINE_SECS: u64 = 600;
const TASK_ORDER_SEED: u64 = 0x5053_5031_3045_5641;
const NONINFERIORITY_MARGIN: f64 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkSuite {
    /// One configured production-topology arm over a small task prefix.
    Smoke,
    /// The paging/adaptive pair used by the default-activation decision.
    Adaptive,
    /// The complete seven-arm diagnostic ladder.
    Full,
}

impl BenchmarkSuite {
    fn label(self) -> &'static str {
        match self {
            Self::Smoke => "smoke",
            Self::Adaptive => "adaptive",
            Self::Full => "full",
        }
    }

    fn arms(self) -> &'static [EvalArm] {
        match self {
            Self::Smoke => &ARMS[6..7],
            Self::Adaptive => &ARMS[3..5],
            Self::Full => &ARMS,
        }
    }

    fn default_task_limit(self) -> usize {
        match self {
            Self::Smoke => 8,
            Self::Adaptive | Self::Full => usize::MAX,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BenchmarkRunOptions {
    pub config_path: Option<PathBuf>,
    pub suite: BenchmarkSuite,
    pub task_limit: Option<usize>,
    pub output: Option<PathBuf>,
}

/// One evaluation arm: a named, cumulative capability toggle set.
#[derive(Debug, Clone, Copy)]
struct EvalArm {
    name: &'static str,
    /// Arm 1 bypasses the governed runtime entirely.
    direct: bool,
    /// Arm 3 ablation switch (packets off below it).
    correction_packets: bool,
    /// Arm 4 ablation switch (paging off below it).
    context_paging: bool,
    /// Arms 5+: `[exploration] max_branches`.
    max_branches: u8,
    /// Arm 6: prefer a distinct family on expansion.
    distinct_family: bool,
    /// Arm 7: multi-node graphs with the integration gate.
    max_parallel_nodes: usize,
}

const ARMS: [EvalArm; 7] = [
    EvalArm {
        name: "direct",
        direct: true,
        correction_packets: false,
        context_paging: false,
        max_branches: 1,
        distinct_family: false,
        max_parallel_nodes: 1,
    },
    EvalArm {
        name: "governed",
        direct: false,
        correction_packets: false,
        context_paging: false,
        max_branches: 1,
        distinct_family: false,
        max_parallel_nodes: 1,
    },
    EvalArm {
        name: "packets",
        direct: false,
        correction_packets: true,
        context_paging: false,
        max_branches: 1,
        distinct_family: false,
        max_parallel_nodes: 1,
    },
    EvalArm {
        name: "paging",
        direct: false,
        correction_packets: true,
        context_paging: true,
        max_branches: 1,
        distinct_family: false,
        max_parallel_nodes: 1,
    },
    EvalArm {
        name: "adaptive",
        direct: false,
        correction_packets: true,
        context_paging: true,
        max_branches: 3,
        distinct_family: false,
        max_parallel_nodes: 1,
    },
    EvalArm {
        name: "multi-family",
        direct: false,
        correction_packets: true,
        context_paging: true,
        max_branches: 3,
        distinct_family: true,
        max_parallel_nodes: 1,
    },
    EvalArm {
        name: "integration",
        direct: false,
        correction_packets: true,
        context_paging: true,
        max_branches: 3,
        distinct_family: true,
        max_parallel_nodes: 2,
    },
];

/// One matched task loaded from `corpus/<id>/`.
#[derive(serde::Deserialize, serde::Serialize)]
struct TaskSpec {
    goal: String,
    /// The hidden suite's argv, run identically for every arm. Python
    /// oracles always run through `uv run --no-sync` — never bare python.
    hidden_check: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
    /// "pass" (a straightforward task) or "recovery" (deliberately
    /// misleading; exercises search and no-good non-suppression).
    #[serde(default = "default_expect")]
    expect: String,
}

fn default_expect() -> String {
    "pass".into()
}

struct Task {
    id: String,
    spec: TaskSpec,
    fixture_dir: PathBuf,
    hidden_dir: PathBuf,
    solution_dir: PathBuf,
}

/// Load every task directory (sorted by id for determinism).
fn load_tasks(root: &Path) -> anyhow::Result<Vec<Task>> {
    let mut tasks = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let dir = entry.path();
        let spec: TaskSpec =
            serde_json::from_str(&std::fs::read_to_string(dir.join("task.json"))?)?;
        anyhow::ensure!(
            !spec.hidden_check.is_empty(),
            "{}: hidden_check must not be empty",
            dir.display()
        );
        tasks.push(Task {
            id: entry.file_name().to_string_lossy().to_string(),
            spec,
            fixture_dir: dir.join("fixture"),
            hidden_dir: dir.join("hidden"),
            solution_dir: dir.join("solution"),
        });
    }
    tasks.sort_by(|a, b| a.id.cmp(&b.id));
    anyhow::ensure!(!tasks.is_empty(), "no tasks under {}", root.display());
    Ok(tasks)
}

fn shuffle_tasks(tasks: &mut [Task], seed: u64) {
    let mut rng = SplitMix64(seed);
    for upper in (1..tasks.len()).rev() {
        let selected = (rng.next() as usize) % (upper + 1);
        tasks.swap(upper, selected);
    }
}

#[derive(Debug, Clone, Default, serde::Serialize)]
struct CorpusCoverage {
    rust: usize,
    python: usize,
    mixed: usize,
    small: usize,
    medium: usize,
    large: usize,
    workspace: usize,
    multi_file: usize,
    graph: usize,
    recovery: usize,
    paging: usize,
    api_contract: usize,
    algorithm: usize,
    state_machine: usize,
    concurrency: usize,
    serialization: usize,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
struct CorpusValidation {
    ready: bool,
    coverage: CorpusCoverage,
    violations: Vec<String>,
    checked_solutions: usize,
    corpus_digest: String,
}

fn has_tag(task: &Task, tag: &str) -> bool {
    task.spec.tags.iter().any(|candidate| candidate == tag)
}

fn count_coverage(task: &Task, coverage: &mut CorpusCoverage) {
    for (tag, count) in [
        ("rust", &mut coverage.rust),
        ("python", &mut coverage.python),
        ("mixed", &mut coverage.mixed),
        ("small", &mut coverage.small),
        ("medium", &mut coverage.medium),
        ("large", &mut coverage.large),
        ("workspace", &mut coverage.workspace),
        ("multi_file", &mut coverage.multi_file),
        ("graph", &mut coverage.graph),
        ("recovery", &mut coverage.recovery),
        ("paging", &mut coverage.paging),
        ("api_contract", &mut coverage.api_contract),
        ("algorithm", &mut coverage.algorithm),
        ("state_machine", &mut coverage.state_machine),
        ("concurrency", &mut coverage.concurrency),
        ("serialization", &mut coverage.serialization),
    ] {
        if has_tag(task, tag) {
            *count += 1;
        }
    }
}

fn validate_task_shape(task: &Task, violations: &mut Vec<String>) {
    let tag_count = |choices: &[&str]| choices.iter().filter(|tag| has_tag(task, tag)).count();
    if tag_count(&["rust", "python", "mixed"]) != 1 {
        violations.push(format!(
            "{}: exactly one language tag (rust/python/mixed) is required",
            task.id
        ));
    }
    if tag_count(&["small", "medium", "large"]) != 1 {
        violations.push(format!(
            "{}: exactly one scale tag (small/medium/large) is required",
            task.id
        ));
    }
    if !matches!(task.spec.expect.as_str(), "pass" | "recovery") {
        violations.push(format!("{}: expect must be pass or recovery", task.id));
    }
    if task.spec.expect == "recovery" && !has_tag(task, "recovery") {
        violations.push(format!(
            "{}: recovery expectation requires the recovery tag",
            task.id
        ));
    }
    for (name, directory) in [
        ("fixture", &task.fixture_dir),
        ("hidden", &task.hidden_dir),
        ("solution", &task.solution_dir),
    ] {
        if !directory.is_dir() {
            violations.push(format!("{}: missing {name}/ directory", task.id));
        }
    }
    let command = task.spec.hidden_check.first().map(String::as_str);
    if has_tag(task, "rust") && command != Some("cargo") {
        violations.push(format!("{}: Rust oracle must run through cargo", task.id));
    }
    if (has_tag(task, "python") || has_tag(task, "mixed")) && command != Some("uv") {
        violations.push(format!(
            "{}: Python/mixed oracle must run through uv",
            task.id
        ));
    }
}

fn validate_coverage(tasks: &[Task], c: &CorpusCoverage, violations: &mut Vec<String>) {
    let floors = [
        ("tasks", tasks.len(), 30),
        ("Rust tasks", c.rust, 12),
        ("Python tasks", c.python, 10),
        ("mixed Rust/Python tasks", c.mixed, 4),
        ("medium-or-large tasks", c.medium + c.large, 16),
        ("large tasks", c.large, 4),
        ("workspace/package tasks", c.workspace, 6),
        ("multi-file tasks", c.multi_file, 16),
        ("graph-planning tasks", c.graph, 6),
        ("recovery tasks", c.recovery, 5),
        ("paging/long-context tasks", c.paging, 6),
        ("API-contract tasks", c.api_contract, 6),
        ("algorithm tasks", c.algorithm, 8),
        ("state-machine tasks", c.state_machine, 4),
        ("concurrency tasks", c.concurrency, 3),
        ("serialization tasks", c.serialization, 3),
    ];
    for (label, actual, minimum) in floors {
        if actual < minimum {
            violations.push(format!(
                "{label}: found {actual}, require at least {minimum}"
            ));
        }
    }
}

/// Gate AE is deliberately a composition gate, not a directory-count gate.
/// These floors prevent a nominal 30-task corpus from being thirty variants
/// of the same one-file bug fix.
fn validate_corpus_shape(tasks: &[Task]) -> CorpusValidation {
    let mut validation = CorpusValidation::default();
    for task in tasks {
        count_coverage(task, &mut validation.coverage);
        validate_task_shape(task, &mut validation.violations);
    }
    validate_coverage(tasks, &validation.coverage, &mut validation.violations);
    match corpus_digest(tasks) {
        Ok(digest) => validation.corpus_digest = digest,
        Err(error) => validation
            .violations
            .push(format!("could not digest corpus: {error}")),
    }
    validation.ready = validation.violations.is_empty();
    validation
}

fn corpus_digest(tasks: &[Task]) -> anyhow::Result<String> {
    fn visit(
        root: &Path,
        directory: &Path,
        rows: &mut Vec<(String, String)>,
    ) -> anyhow::Result<()> {
        let mut entries = std::fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            if entry.file_type()?.is_dir() {
                visit(root, &entry.path(), rows)?;
            } else {
                let relative = entry.path().strip_prefix(root)?.display().to_string();
                let digest = perspt_sdk::content_hash(&std::fs::read(entry.path())?);
                rows.push((relative, digest));
            }
        }
        Ok(())
    }

    let mut manifest = Vec::new();
    for task in tasks {
        manifest.push((
            format!("{}/task.json", task.id),
            perspt_sdk::content_hash(&serde_json::to_vec(&task.spec)?),
        ));
        for (name, directory) in [
            ("fixture", &task.fixture_dir),
            ("hidden", &task.hidden_dir),
            ("solution", &task.solution_dir),
        ] {
            let mut rows = Vec::new();
            visit(directory, directory, &mut rows)?;
            manifest.extend(
                rows.into_iter()
                    .map(|(path, digest)| (format!("{}/{name}/{path}", task.id), digest)),
            );
        }
    }
    manifest.sort();
    Ok(perspt_sdk::content_hash(&serde_json::to_vec(&manifest)?))
}

/// Deterministic SplitMix64 for the seeded bootstrap.
struct SplitMix64(u64);
impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// Paired percentile bootstrap over per-task differences.
fn bootstrap_ci(differences: &[f64], seed: u64, resamples: usize) -> (f64, f64) {
    if differences.is_empty() {
        return (0.0, 0.0);
    }
    let mut rng = SplitMix64(seed);
    let mut means = Vec::with_capacity(resamples);
    for _ in 0..resamples {
        let mut total = 0.0;
        for _ in 0..differences.len() {
            total += differences[(rng.next() as usize) % differences.len()];
        }
        means.push(total / differences.len() as f64);
    }
    means.sort_by(f64::total_cmp);
    (
        means[(resamples as f64 * 0.025) as usize],
        means[(resamples as f64 * 0.975) as usize],
    )
}

/// One cell's recorded outcome and process metrics.
#[derive(Default, serde::Serialize)]
struct CellResult {
    task: String,
    arm: String,
    hard_pass: bool,
    wall_secs: f64,
    model_turns: u64,
    turns_to_first_mutation: Option<u64>,
    turns_to_first_test: Option<u64>,
    denied_calls: u64,
    repeated_calls: u64,
    /// Search and paging counters (PSP-10 Phase 11 exit evidence).
    branches_opened: u64,
    no_goods_recorded: u64,
    suppressed_duplicates: u64,
    branches_abandoned: u64,
    pages_selected: u64,
    failure: Option<String>,
}

/// Directories never copied into the hidden-verification tree.
const SKIP_DIRS: &[&str] = &[
    "target",
    ".venv",
    ".perspt",
    ".perspt-target",
    ".perspt-tmp",
    ".perspt-home",
    ".pytest_cache",
    ".ruff_cache",
    "__pycache__",
    "node_modules",
];

fn copy_tree(source: &Path, destination: &Path) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if SKIP_DIRS.contains(&name.as_str()) || name == "eval-ledger.db" {
            continue;
        }
        let target = destination.join(&name);
        if entry.file_type()?.is_dir() {
            std::fs::create_dir_all(&target)?;
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// The hidden suite: a fresh copy of the post-run fixture with the
/// withheld `hidden/` tree overlaid on top (overwriting any weakened
/// visible test), run identically for every arm. The fixture's synced
/// `.venv` is reused read-only for Python oracles.
fn hidden_pass(fixture: &Path, task: &Task) -> bool {
    let Ok(verify) = tempfile::tempdir() else {
        return false;
    };
    if copy_tree(fixture, verify.path()).is_err() {
        return false;
    }
    if task.hidden_dir.is_dir() && copy_tree(&task.hidden_dir, verify.path()).is_err() {
        return false;
    }
    let check = &task.spec.hidden_check;
    let mut command = Command::new(&check[0]);
    command
        .args(&check[1..])
        .current_dir(verify.path())
        .env("CARGO_INCREMENTAL", "0");
    let venv = fixture.join(".venv");
    if venv.is_dir() {
        command.env("UV_PROJECT_ENVIRONMENT", venv);
    }
    command
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Prove that the oracle distinguishes the unfinished fixture from a known
/// good implementation. `solution/` is never copied into an evaluation
/// workspace; it exists solely to make corpus authoring falsifiable.
fn validate_task_solution(task: &Task) -> anyhow::Result<()> {
    let unfinished = tempfile::tempdir()?;
    write_fixture(unfinished.path(), task)?;
    anyhow::ensure!(
        !hidden_pass(unfinished.path(), task),
        "{}: unfinished fixture unexpectedly passes the hidden oracle",
        task.id
    );

    let solved = tempfile::tempdir()?;
    write_fixture(solved.path(), task)?;
    copy_tree(&task.solution_dir, solved.path())?;
    anyhow::ensure!(
        hidden_pass(solved.path(), task),
        "{}: reference solution does not pass the hidden oracle",
        task.id
    );
    Ok(())
}

fn validate_corpus(tasks: &[Task]) -> CorpusValidation {
    let mut validation = validate_corpus_shape(tasks);
    if !validation.ready {
        return validation;
    }
    for task in tasks {
        match validate_task_solution(task) {
            Ok(()) => validation.checked_solutions += 1,
            Err(error) => validation.violations.push(error.to_string()),
        }
    }
    validation.ready =
        validation.violations.is_empty() && validation.checked_solutions == tasks.len();
    validation
}

/// Copy the visible fixture into the working directory; synthesize any
/// declared large modules; pre-sync the Python environment.
fn write_fixture(dir: &Path, task: &Task) -> anyhow::Result<()> {
    copy_tree(&task.fixture_dir, dir)?;
    let generate = dir.join("__generate__.json");
    if generate.is_file() {
        let specs: Vec<serde_json::Value> =
            serde_json::from_str(&std::fs::read_to_string(&generate)?)?;
        std::fs::remove_file(&generate)?;
        for spec in specs {
            let path = spec["path"].as_str().unwrap_or_default();
            let entries = spec["entries"].as_u64().unwrap_or(0);
            let kind = spec["kind"].as_str().unwrap_or("rust_table");
            let marked = spec["marked_index"].as_u64();
            let full = dir.join(path);
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent)?;
            }
            // Large deterministic tables exercise paged reads without
            // checking multi-megabyte inert fixtures into the repository.
            let mut module = if kind == "python_table" {
                String::from("# Generated lookup table.\nENTRIES = [\n")
            } else {
                String::from(
                    "/// Generated lookup table.\npub const ENTRIES: &[(&str, u64)] = &[\n",
                )
            };
            for index in 0..entries {
                let key = if marked == Some(index) {
                    "release-channel".to_owned()
                } else {
                    format!("entry-{index:06}")
                };
                module.push_str(&format!("    ({key:?}, {index}),\n"));
            }
            module.push_str("]\n");
            if kind != "python_table" {
                module.pop();
                module.push_str(";\n");
            }
            std::fs::write(full, module)?;
        }
    }
    if dir.join("pyproject.toml").is_file() {
        // The synced environment is part of the fixture, not the agent's
        // job: verification is offline (`uv run --no-sync`), so pytest
        // must already be importable from the project's `.venv`.
        for args in [vec!["venv", "-q"], vec!["pip", "install", "-q", "pytest"]] {
            let status = Command::new("uv").args(&args).current_dir(dir).status()?;
            anyhow::ensure!(status.success(), "fixture env setup failed: uv {args:?}");
        }
    }
    Ok(())
}

/// Arm 1: the ungoverned direct harness — the same model, task, turn
/// budget, deadline, and hidden suite, with bare read/write tools applied
/// straight to the workspace (no kernel, no gate, no ledger).
fn direct_schema(kind: &str) -> serde_json::Value {
    match kind {
        "read" => serde_json::json!({"type":"object","properties":{
            "path":{"type":"string"},"offset":{"type":"integer"},"limit":{"type":"integer"}
        },"required":["path"],"additionalProperties":false}),
        "grep" => serde_json::json!({"type":"object","properties":{
            "query":{"type":"string"},"path":{"type":"string"}
        },"required":["query"],"additionalProperties":false}),
        "edit" => serde_json::json!({"type":"object","properties":{
            "path":{"type":"string"},"old_string":{"type":"string"},
            "new_string":{"type":"string"}
        },"required":["path","old_string","new_string"],"additionalProperties":false}),
        "write" => serde_json::json!({"type":"object","properties":{
            "path":{"type":"string"},"content":{"type":"string"}
        },"required":["path","content"],"additionalProperties":false}),
        _ => serde_json::json!({"type":"object","properties":{},"additionalProperties":false}),
    }
}

fn direct_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "read_file".into(),
            description: "Read a file".into(),
            schema: direct_schema("read"),
            strict: false,
        },
        ToolSpec {
            name: "list_files".into(),
            description: "Recursively list workspace-relative files".into(),
            schema: direct_schema("empty"),
            strict: false,
        },
        ToolSpec {
            name: "grep".into(),
            description: "Search workspace file contents with a regular expression".into(),
            schema: direct_schema("grep"),
            strict: false,
        },
        ToolSpec {
            name: "edit_file".into(),
            description: "Replace one unique exact string in a file".into(),
            schema: direct_schema("edit"),
            strict: false,
        },
        ToolSpec {
            name: "run_test".into(),
            description: "Run the workspace's visible test suites".into(),
            schema: direct_schema("empty"),
            strict: false,
        },
        ToolSpec {
            name: "run_build".into(),
            description: "Run the workspace's build or syntax checks".into(),
            schema: direct_schema("empty"),
            strict: false,
        },
        ToolSpec {
            name: "write_file".into(),
            description: "Create or replace a whole file".into(),
            schema: direct_schema("write"),
            strict: false,
        },
    ]
}

async fn run_direct(
    transport: &GenAiTransport,
    model: &ModelId,
    dir: &Path,
    task: &Task,
) -> anyhow::Result<CellResult> {
    let specs = direct_specs();
    let mut conversation = Conversation::with_system(
        "You are a coding agent. Read and write files with the provided tools \
         until the task is complete, then reply DONE.",
    );
    conversation.push_user(task.spec.goal.clone());
    let mut cell = CellResult {
        task: task.id.clone(),
        arm: "direct".into(),
        ..Default::default()
    };
    for turn in 1..=MAX_TURNS {
        let output = perspt_agent::turn::chat_turn_with_deadline(
            transport,
            model,
            &conversation,
            &specs,
            ToolChoicePolicy::Auto,
            DEADLINE_SECS,
        )
        .await;
        cell.model_turns = u64::from(turn);
        match output {
            Ok(TurnOutput::Text(_)) => break,
            Ok(TurnOutput::ToolCalls(calls)) => {
                conversation.push_tool_calls(calls.clone());
                for call in calls {
                    let response = apply_direct_call(dir, &call, turn, &mut cell);
                    conversation.push_tool_response(&call.call_id, response);
                }
            }
            Err(error) => {
                cell.failure = Some(error.to_string());
                break;
            }
        }
    }
    Ok(cell)
}

fn apply_direct_call(
    dir: &Path,
    call: &perspt_sdk::ProviderToolCall,
    turn: u32,
    cell: &mut CellResult,
) -> String {
    let path = call
        .arguments
        .get("path")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let full = dir.join(path);
    if path.contains("..") || path.starts_with('/') {
        cell.denied_calls += 1;
        return "denied: path escapes the workspace".into();
    }
    match call.name.as_str() {
        "read_file" => direct_read_file(&full, call),
        "write_file" => direct_write_file(&full, call, turn, cell),
        "edit_file" => direct_edit_file(&full, call, turn, cell),
        "list_files" => direct_list_files(dir),
        "grep" => direct_grep(dir, call),
        "run_test" => {
            cell.turns_to_first_test.get_or_insert(u64::from(turn));
            direct_verify(dir, true)
        }
        "run_build" => {
            cell.turns_to_first_test.get_or_insert(u64::from(turn));
            direct_verify(dir, false)
        }
        other => {
            cell.denied_calls += 1;
            format!("unknown tool {other}")
        }
    }
}

fn direct_write_file(
    path: &Path,
    call: &perspt_sdk::ProviderToolCall,
    turn: u32,
    cell: &mut CellResult,
) -> String {
    let content = call
        .arguments
        .get("content")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    cell.turns_to_first_mutation.get_or_insert(u64::from(turn));
    std::fs::write(path, content)
        .map(|_| "ok".into())
        .unwrap_or_else(|error| format!("error: {error}"))
}

fn direct_edit_file(
    path: &Path,
    call: &perspt_sdk::ProviderToolCall,
    turn: u32,
    cell: &mut CellResult,
) -> String {
    let argument = |name| {
        call.arguments
            .get(name)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
    };
    let old = argument("old_string");
    let new = argument("new_string");
    match std::fs::read_to_string(path) {
        Ok(content) if content.matches(old).count() == 1 => {
            cell.turns_to_first_mutation.get_or_insert(u64::from(turn));
            std::fs::write(path, content.replacen(old, new, 1))
                .map(|_| "ok".into())
                .unwrap_or_else(|error| format!("error: {error}"))
        }
        Ok(content) => format!(
            "error: old_string matched {} locations",
            content.matches(old).count()
        ),
        Err(error) => format!("error: {error}"),
    }
}

fn direct_read_file(path: &Path, call: &perspt_sdk::ProviderToolCall) -> String {
    use std::io::BufRead;
    let offset = call
        .arguments
        .get("offset")
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .max(1) as usize;
    let limit = call
        .arguments
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(300)
        .clamp(1, 2000) as usize;
    let Ok(file) = std::fs::File::open(path) else {
        return format!("error: could not open {}", path.display());
    };
    let mut selected = Vec::new();
    let mut total = 0usize;
    for line in std::io::BufReader::new(file).lines() {
        total += 1;
        if total >= offset && selected.len() < limit {
            match line {
                Ok(line) => selected.push(line.chars().take(2000).collect::<String>()),
                Err(error) => return format!("error: {error}"),
            }
        }
    }
    let end = offset.saturating_sub(1) + selected.len();
    let mut output = format!("lines {offset}-{end} of {total}\n{}", selected.join("\n"));
    if end < total {
        output.push_str(&format!(
            "\n[{} more lines; continue with offset={}]",
            total - end,
            end + 1
        ));
    }
    output
}

fn direct_list_files(root: &Path) -> String {
    fn walk(root: &Path, directory: &Path, files: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            if SKIP_DIRS.contains(&entry.file_name().to_string_lossy().as_ref()) {
                continue;
            }
            if entry.path().is_dir() {
                walk(root, &entry.path(), files);
            } else if let Ok(path) = entry.path().strip_prefix(root) {
                files.push(path.display().to_string());
            }
            if files.len() >= 10_000 {
                return;
            }
        }
    }
    let mut files = Vec::new();
    walk(root, root, &mut files);
    files.sort();
    files.join("\n")
}

fn direct_grep(root: &Path, call: &perspt_sdk::ProviderToolCall) -> String {
    let query = call
        .arguments
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let path = call
        .arguments
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or(".");
    if path.contains("..") || path.starts_with('/') {
        return "denied: path escapes the workspace".into();
    }
    let output = Command::new("rg")
        .args([
            "-n",
            "--no-heading",
            "--color",
            "never",
            "--max-count",
            "50",
            "--max-columns",
            "2000",
            "--",
            query,
            path,
        ])
        .current_dir(root)
        .output();
    match output {
        Ok(output) if output.status.success() || output.status.code() == Some(1) => {
            String::from_utf8_lossy(&output.stdout[..output.stdout.len().min(2 * 1024 * 1024)])
                .into_owned()
        }
        Ok(output) => format!("error: {}", String::from_utf8_lossy(&output.stderr)),
        Err(error) => format!("error: {error}"),
    }
}

fn direct_verify(root: &Path, test: bool) -> String {
    let mut commands: Vec<(&str, Vec<&str>)> = Vec::new();
    if root.join("Cargo.toml").is_file() {
        commands.push((
            "cargo",
            vec![if test { "test" } else { "check" }, "--quiet"],
        ));
    }
    if root.join("pyproject.toml").is_file() {
        commands.push(if test {
            ("uv", vec!["run", "--no-sync", "pytest", "-q"])
        } else {
            (
                "uv",
                vec!["run", "--no-sync", "python", "-m", "compileall", "-q", "."],
            )
        });
    }
    let mut rendered = String::new();
    for (program, args) in commands {
        match Command::new(program).args(args).current_dir(root).output() {
            Ok(output) => {
                rendered.push_str(&String::from_utf8_lossy(&output.stdout));
                rendered.push_str(&String::from_utf8_lossy(&output.stderr));
                if !output.status.success() {
                    rendered.push_str("\nverification failed");
                    return rendered;
                }
            }
            Err(error) => return format!("error: {error}"),
        }
    }
    if rendered.is_empty() {
        "ok".into()
    } else {
        rendered
    }
}

/// A governed arm: the full runtime with the arm's real toggles.
async fn run_governed(
    arm: &EvalArm,
    topology: &ModelTopology,
    config: &perspt_core::Config,
    dir: &Path,
    task: &Task,
) -> anyhow::Result<CellResult> {
    let mut config = config.clone();
    let exploration = config.exploration.get_or_insert_with(Default::default);
    exploration.initial_branches = Some(1);
    exploration.max_branches = Some(arm.max_branches);
    exploration.distinct_family = Some(arm.distinct_family);
    let database = dir.join("eval-ledger.db");
    let runtime = Psp9AgentRuntime::from_config(
        dir.to_path_buf(),
        &config,
        perspt_agent::Psp9ModelRoutes {
            primary: Some(topology.actuator.clone()),
            actuator: Some(topology.actuator.clone()),
            explorer: topology.speculator.clone(),
            adjudicator: topology.adjudicator.clone(),
            ..Default::default()
        },
        Psp9RunConfig {
            approval_policy: perspt_sdk::ApprovalPolicy::Auto,
            allow_unisolated_verifiers: true,
            max_turns: MAX_TURNS,
            turn_deadline_secs: DEADLINE_SECS,
            max_parallel_nodes: arm.max_parallel_nodes,
            ablate_correction_packets: !arm.correction_packets,
            ablate_context_paging: !arm.context_paging,
            ..Psp9RunConfig::default()
        },
    )?
    .with_database_path(database.clone());
    let mut cell = CellResult {
        task: task.id.clone(),
        arm: arm.name.into(),
        ..Default::default()
    };
    match runtime.run(task.spec.goal.clone()).await {
        Ok(summary) => {
            cell.model_turns = u64::from(summary.turns_used);
            fold_ledger_metrics(&database, &summary.session_id, &mut cell);
        }
        Err(error) => cell.failure = Some(error.to_string()),
    }
    Ok(cell)
}

/// Ledger-derived process and search metrics for a governed cell.
fn fold_ledger_metrics(database: &Path, session_id: &str, cell: &mut CellResult) {
    let Ok(store) = perspt_store::SessionStore::open(&database.to_path_buf()) else {
        return;
    };
    let Ok(rows) = store.get_psp9_events(session_id) else {
        return;
    };
    let mut turn = 0u64;
    let mut seen_calls: std::collections::BTreeMap<String, u64> = Default::default();
    for row in &rows {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&row.event_json) else {
            continue;
        };
        if value.get("kind").and_then(|kind| kind.as_str()) != Some("tool_loop") {
            continue;
        }
        let payload = value.get("payload").cloned().unwrap_or_default();
        let body = payload.get("body").cloned().unwrap_or(payload);
        match body.get("event").and_then(|event| event.as_str()) {
            Some("turn_observed") => {
                turn = body.get("turn").and_then(|t| t.as_u64()).unwrap_or(turn);
            }
            Some("tool_call_observed") => {
                let key = body
                    .get("call")
                    .map(|call| call.to_string())
                    .unwrap_or_default();
                let count = seen_calls.entry(key).or_insert(0);
                *count += 1;
                if *count > 1 {
                    cell.repeated_calls += 1;
                }
                let name = body
                    .pointer("/call/name")
                    .and_then(|name| name.as_str())
                    .unwrap_or_default();
                if matches!(name, "run_test" | "run_build") {
                    cell.turns_to_first_test.get_or_insert(turn);
                }
            }
            Some("effect_applied") => {
                if body.get("mutated").and_then(|m| m.as_bool()) == Some(true) {
                    cell.turns_to_first_mutation.get_or_insert(turn);
                }
            }
            Some("effect_denied") => cell.denied_calls += 1,
            Some("branch_forked") => cell.branches_opened += 1,
            Some("no_good_recorded") => cell.no_goods_recorded += 1,
            Some("branch_abandoned") => cell.branches_abandoned += 1,
            Some("branch_observation") => {
                let observation = body
                    .get("observation")
                    .and_then(|o| o.as_str())
                    .unwrap_or_default();
                if observation.contains("suppressed") {
                    cell.suppressed_duplicates += 1;
                }
            }
            Some("context_pages_selected") => cell.pages_selected += 1,
            _ => {}
        }
    }
}

/// Validate the bundled corpus without contacting a model provider.
pub fn validate_corpus_command() -> anyhow::Result<serde_json::Value> {
    let corpus = materialize_corpus()?;
    let tasks = load_tasks(corpus.path())?;
    let validation = validate_corpus(&tasks);
    Ok(serde_json::to_value(validation)?)
}

/// Aggregate completed reports. This is also credential-free.
pub fn aggregate_reports(paths: &[PathBuf]) -> anyhow::Result<serde_json::Value> {
    anyhow::ensure!(
        paths.len() >= 2,
        "aggregation requires at least two reports"
    );
    let mut reports = Vec::new();
    for path in paths {
        let report: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        reports.push((path, report));
    }
    let report_digests: Vec<&str> = reports
        .iter()
        .filter_map(|(_, report)| report.pointer("/corpus_validation/corpus_digest")?.as_str())
        .collect();
    let digests: std::collections::BTreeSet<&str> = report_digests.iter().copied().collect();
    anyhow::ensure!(
        digests.len() == 1 && report_digests.len() == reports.len(),
        "reports must carry one identical content-addressed corpus digest"
    );
    let families: std::collections::BTreeSet<&str> = reports
        .iter()
        .filter_map(|(_, report)| report.pointer("/topology/actuator_family")?.as_str())
        .collect();
    let route_failures: Vec<serde_json::Value> = reports
        .iter()
        .filter(|(_, report)| {
            report
                .get("adaptive_route_accepted")
                .and_then(|v| v.as_bool())
                != Some(true)
        })
        .map(|(path, report)| {
            serde_json::json!({
                "path": path,
                "suite": report.get("suite"),
                "topology": report.get("topology"),
                "gate_ae_ready": report.get("gate_ae_ready"),
                "paired_contrast": report.get("paired_contrast"),
            })
        })
        .collect();
    let accepted = families.len() >= 2 && route_failures.is_empty();
    Ok(serde_json::json!({
        "adaptive_default_accepted": accepted,
        "corpus_digest": digests.first().copied(),
        "distinct_model_families": families,
        "required_model_families": 2,
        "reports": paths,
        "route_failures": route_failures,
    }))
}

async fn evaluate_cells(
    topology: &ModelTopology,
    tasks: &[Task],
    limit: usize,
    arms: &[EvalArm],
    config: &perspt_core::Config,
    direct_transport: &GenAiTransport,
    direct_model: &ModelId,
) -> anyhow::Result<Vec<CellResult>> {
    let mut results = Vec::new();
    for task in tasks.iter().take(limit) {
        for arm in arms {
            let dir = tempfile::tempdir()?;
            write_fixture(dir.path(), task)?;
            let started = std::time::Instant::now();
            let run = async {
                if arm.direct {
                    run_direct(direct_transport, direct_model, dir.path(), task).await
                } else {
                    run_governed(arm, topology, config, dir.path(), task).await
                }
            };
            let mut cell =
                match tokio::time::timeout(std::time::Duration::from_secs(CELL_DEADLINE_SECS), run)
                    .await
                {
                    Ok(result) => result?,
                    Err(_) => CellResult {
                        task: task.id.clone(),
                        arm: arm.name.into(),
                        failure: Some(format!("cell exceeded {CELL_DEADLINE_SECS}s")),
                        ..Default::default()
                    },
                };
            cell.wall_secs = started.elapsed().as_secs_f64();
            cell.hard_pass = hidden_pass(dir.path(), task);
            eprintln!(
                "{}/{}: hard_pass={} wall={:.1}s turns={} branches={}",
                task.id,
                arm.name,
                cell.hard_pass,
                cell.wall_secs,
                cell.model_turns,
                cell.branches_opened,
            );
            results.push(cell);
        }
    }
    Ok(results)
}

/// Run an optional credentialed benchmark using the configured role topology.
pub async fn run_benchmark(options: BenchmarkRunOptions) -> anyhow::Result<serde_json::Value> {
    let corpus = materialize_corpus()?;
    let mut tasks = load_tasks(corpus.path())?;
    // A live run only needs the fast structural/content-addressed validation.
    // `benchmark validate` performs the intentionally slower fail-before and
    // pass-after oracle executions.
    let corpus_validation = validate_corpus_shape(&tasks);
    anyhow::ensure!(
        corpus_validation.ready,
        "evaluation corpus is not ready; run `perspt benchmark validate` for details"
    );
    let config = load_config(options.config_path.as_deref())?;
    let portfolio = std::sync::Arc::new(perspt_core::ModelPortfolio::from_config(&config)?);
    let topology = configured_topology(&config, &portfolio)?;
    let direct_model = configured_model_id(&topology.actuator, &config, &portfolio)?;
    let direct_transport = GenAiTransport::new(portfolio);
    shuffle_tasks(&mut tasks, TASK_ORDER_SEED);
    let limit = options
        .task_limit
        .unwrap_or_else(|| options.suite.default_task_limit())
        .min(tasks.len());
    anyhow::ensure!(limit > 0, "benchmark task limit must be greater than zero");
    let arms = options.suite.arms();
    let results = evaluate_cells(
        &topology,
        &tasks,
        limit,
        arms,
        &config,
        &direct_transport,
        &direct_model,
    )
    .await?;
    let report = build_report(
        &topology,
        options.suite,
        arms,
        &tasks,
        limit,
        results,
        corpus_validation,
    );
    if let Some(path) = options.output {
        std::fs::write(path, serde_json::to_vec_pretty(&report)?)?;
    }
    Ok(report)
}

/// Assemble the published report: paired primary contrast (adaptive vs the
/// full non-search stack, isolating search), per-task tags, and the Gate AE
/// readiness flag (>= 30 paired tasks; this corpus is scaffolding until
/// the authoring effort reaches that).
struct ReportAnalysis {
    task_count: usize,
    expected_cells: usize,
    complete_cells: bool,
    lower: f64,
    upper: f64,
    escaped_regressions: usize,
    single_repeated: u64,
    adaptive_repeated: u64,
    has_adaptive_pair: bool,
}

fn analyze_results(
    tasks: &[Task],
    limit: usize,
    arms: &[EvalArm],
    results: &[CellResult],
) -> ReportAnalysis {
    let pass_vector = |arm: &str| {
        tasks
            .iter()
            .take(limit)
            .map(|task| {
                results
                    .iter()
                    .find(|cell| cell.task == task.id && cell.arm == arm)
                    .map(|cell| f64::from(u8::from(cell.hard_pass)))
                    .unwrap_or(0.0)
            })
            .collect::<Vec<_>>()
    };
    let adaptive = pass_vector("adaptive");
    let single = pass_vector("paging");
    let differences: Vec<f64> = adaptive.iter().zip(&single).map(|(a, s)| a - s).collect();
    let (lower, upper) = bootstrap_ci(&differences, 11, 10_000);
    let task_count = tasks.len().min(limit);
    let passed = |task: &Task, arm: &str| {
        results
            .iter()
            .find(|cell| cell.task == task.id && cell.arm == arm)
            .is_some_and(|cell| cell.hard_pass)
    };
    let escaped_regressions = tasks
        .iter()
        .take(limit)
        .filter(|task| passed(task, "paging") && !passed(task, "adaptive"))
        .count();
    let repeated = |arm: &str| {
        results
            .iter()
            .filter(|cell| cell.arm == arm)
            .map(|cell| cell.repeated_calls)
            .sum()
    };
    let expected_cells = task_count * arms.len();
    let has_arm = |name: &str| arms.iter().any(|arm| arm.name == name);
    ReportAnalysis {
        task_count,
        expected_cells,
        complete_cells: results.len() == expected_cells,
        lower,
        upper,
        escaped_regressions,
        single_repeated: repeated("paging"),
        adaptive_repeated: repeated("adaptive"),
        has_adaptive_pair: has_arm("paging") && has_arm("adaptive"),
    }
}

fn build_report(
    topology: &ModelTopology,
    suite: BenchmarkSuite,
    arms: &[EvalArm],
    tasks: &[Task],
    limit: usize,
    results: Vec<CellResult>,
    corpus_validation: CorpusValidation,
) -> serde_json::Value {
    let analysis = analyze_results(tasks, limit, arms, &results);
    let topology_json = serde_json::to_vec(topology).unwrap_or_default();
    let adaptive_evidence_ready = analysis.has_adaptive_pair
        && analysis.task_count >= 30
        && corpus_validation.ready
        && analysis.complete_cells;
    let seven_arm_complete = suite == BenchmarkSuite::Full && analysis.complete_cells;
    let gate_ae_ready = adaptive_evidence_ready && seven_arm_complete;
    let adaptive_route_accepted = gate_ae_ready
        && analysis.lower >= -NONINFERIORITY_MARGIN
        && analysis.adaptive_repeated < analysis.single_repeated
        && analysis.escaped_regressions == 0;
    let paired_contrast = analysis.has_adaptive_pair.then(|| {
        serde_json::json!({
            "arms": ["adaptive", "paging"],
            "bootstrap_seed": 11,
            "resamples": 10_000,
            "ci95": [analysis.lower, analysis.upper],
            "noninferiority_margin": NONINFERIORITY_MARGIN,
            "escaped_hard_gate_regressions": analysis.escaped_regressions,
            "single_repeated_calls": analysis.single_repeated,
            "adaptive_repeated_calls": analysis.adaptive_repeated,
        })
    });
    serde_json::json!({
        "suite": suite.label(),
        "arms": arms.iter().map(|arm| arm.name).collect::<Vec<_>>(),
        "topology": topology,
        "topology_digest": perspt_sdk::content_hash(&topology_json),
        "results": results,
        "tasks": tasks.iter().take(limit).map(|task| serde_json::json!({
            "id": task.id,
            "tags": task.spec.tags,
            "expect": task.spec.expect,
        })).collect::<Vec<_>>(),
        "task_count": analysis.task_count,
        "task_order_seed": TASK_ORDER_SEED,
        "cell_elapsed_limit_secs": CELL_DEADLINE_SECS,
        "expected_cells": analysis.expected_cells,
        "complete_cells": analysis.complete_cells,
        "adaptive_evidence_ready": adaptive_evidence_ready,
        "seven_arm_complete": seven_arm_complete,
        "gate_ae_ready": gate_ae_ready,
        "adaptive_route_accepted": adaptive_route_accepted,
        "adaptive_default_accepted": false,
        "adaptive_default_blocker": concat!(
            "aggregate full accepted reports from at least two distinct ",
            "model families"
        ),
        "corpus_validation": corpus_validation,
        "paired_contrast": paired_contrast,
    })
}

#[cfg(test)]
mod tests;
