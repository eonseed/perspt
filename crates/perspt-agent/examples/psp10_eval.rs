//! The PSP-10 seven-arm evaluation (Phase 11). LIVE CREDENTIALS REQUIRED;
//! never run in CI.
//!
//! Arms are a cumulative ladder over matched tasks (spec: Test and
//! Acceptance Plan). Every arm — the ungoverned direct harness included —
//! gets the same model, task, turn budget, per-call deadline, and hidden
//! test suite; every ablation toggle is real (packets and paging are
//! actually disabled in their arms, adaptive search actually opens
//! branches). Each task keeps its paired per-arm results; the report
//! publishes paired differences with a seeded 10,000-resample percentile
//! bootstrap plus per-cell wall time, time-to-first-mutation,
//! time-to-first-test, denied and repeated calls, and every
//! infrastructure failure. The primary outcome is hidden-test hard pass.
//! Adaptive search becomes the default only if the paired CI lower bound
//! for `p_adaptive - p_single` clears the predeclared margin, repeated
//! identical failures drop, and no hard-gate regression escapes — the
//! flip itself is a separate reviewed commit, never automatic.
//!
//! Usage: `cargo run --example psp10_eval -- <provider::model> [task-limit]`
//! with routes configured in `config.local.toml`.

use std::path::{Path, PathBuf};
use std::process::Command;

use perspt_agent::transport::GenAiTransport;
use perspt_agent::{Psp9AgentRuntime, Psp9RunConfig};
use perspt_sdk::{Conversation, ModelId, ToolChoicePolicy, ToolSpec, TurnOutput};

const MAX_TURNS: u32 = 12;
const DEADLINE_SECS: u64 = 120;

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

/// One matched task: a fixture layout, a goal, and a hidden verification
/// command the arms never see in their goal text.
struct Task {
    id: &'static str,
    goal: &'static str,
    files: &'static [(&'static str, &'static str)],
    /// The hidden suite, run identically for every arm.
    hidden_check: &'static [&'static str],
}

const RUST_MANIFEST: &str = "[package]\nname='t'\nversion='0.1.0'\nedition='2021'\n";

const TASKS: &[Task] = &[
    Task {
        id: "rust-off-by-one",
        goal: "fix the answer function in src/lib.rs so the test suite passes",
        files: &[
            ("Cargo.toml", RUST_MANIFEST),
            (
                "src/lib.rs",
                "pub fn answer() -> u32 { 1 }\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    \
                 fn answer_is_two() { assert_eq!(super::answer(), 2); }\n}\n",
            ),
        ],
        hidden_check: &["cargo", "test", "--quiet"],
    },
    Task {
        id: "rust-dedup-sorted",
        goal: "implement dedup_sorted in src/lib.rs: remove consecutive duplicates from a \
               sorted Vec<i64> in place, preserving order; make the tests pass",
        files: &[
            ("Cargo.toml", RUST_MANIFEST),
            (
                "src/lib.rs",
                "pub fn dedup_sorted(values: &mut Vec<i64>) { let _ = values; todo!() }\n\n\
                 #[cfg(test)]\nmod tests {\n    #[test]\n    fn removes_consecutive() {\n        \
                 let mut v = vec![1, 1, 2, 3, 3, 3];\n        super::dedup_sorted(&mut v);\n        \
                 assert_eq!(v, vec![1, 2, 3]);\n    }\n    #[test]\n    fn empty_ok() {\n        \
                 let mut v: Vec<i64> = vec![];\n        super::dedup_sorted(&mut v);\n        \
                 assert!(v.is_empty());\n    }\n}\n",
            ),
        ],
        hidden_check: &["cargo", "test", "--quiet"],
    },
    Task {
        id: "rust-dependency-order",
        goal: "implement topo_order in src/graph.rs: deterministic topological order of the \
               dependency edges (lexicographic among ready nodes), returning None on a cycle; \
               make the tests pass",
        files: &[
            ("Cargo.toml", RUST_MANIFEST),
            ("src/lib.rs", "pub mod graph;\n"),
            (
                "src/graph.rs",
                "use std::collections::BTreeMap;\n\npub fn topo_order(edges: &[(String, String)]) \
                 -> Option<Vec<String>> {\n    let _ = edges;\n    let _unused: BTreeMap<String, \
                 u32> = BTreeMap::new();\n    todo!()\n}\n\n#[cfg(test)]\nmod tests {\n    use \
                 super::*;\n    fn edge(a: &str, b: &str) -> (String, String) { (a.into(), \
                 b.into()) }\n    #[test]\n    fn orders_dependencies_first() {\n        let \
                 order = topo_order(&[edge(\"a\", \"b\"), edge(\"b\", \"c\")]).unwrap();\n        \
                 assert_eq!(order, vec![\"a\", \"b\", \"c\"]);\n    }\n    #[test]\n    fn \
                 cycles_return_none() {\n        assert!(topo_order(&[edge(\"a\", \"b\"), \
                 edge(\"b\", \"a\")]).is_none());\n    }\n}\n",
            ),
        ],
        hidden_check: &["cargo", "test", "--quiet"],
    },
    Task {
        id: "py-off-by-one",
        goal: "fix answer() in src/t/lib.py so the test suite passes",
        files: &[
            (
                "pyproject.toml",
                "[project]\nname='t'\nversion='0.1.0'\nrequires-python='>=3.10'\n",
            ),
            ("src/t/__init__.py", ""),
            ("src/t/lib.py", "def answer() -> int:\n    return 1\n"),
            (
                "tests/test_lib.py",
                "import sys, pathlib\nsys.path.insert(0, str(pathlib.Path(__file__).\
                 resolve().parents[1] / 'src'))\nfrom t.lib import answer\n\n\ndef \
                 test_answer():\n    assert answer() == 2\n",
            ),
        ],
        hidden_check: &["python3", "-m", "pytest", "-q", "tests"],
    },
    Task {
        id: "py-dependency-order",
        goal: "implement topo_order(edges) in src/t/graph.py: deterministic topological order \
               (lexicographic among ready nodes), returning None on a cycle; make the tests pass",
        files: &[
            (
                "pyproject.toml",
                "[project]\nname='t'\nversion='0.1.0'\nrequires-python='>=3.10'\n",
            ),
            ("src/t/__init__.py", ""),
            (
                "src/t/graph.py",
                "def topo_order(edges):\n    raise NotImplementedError\n",
            ),
            (
                "tests/test_graph.py",
                "import sys, pathlib\nsys.path.insert(0, str(pathlib.Path(__file__).\
                 resolve().parents[1] / 'src'))\nfrom t.graph import topo_order\n\n\ndef \
                 test_orders_dependencies_first():\n    assert topo_order([('a', 'b'), ('b', \
                 'c')]) == ['a', 'b', 'c']\n\n\ndef test_cycles_return_none():\n    assert \
                 topo_order([('a', 'b'), ('b', 'a')]) is None\n",
            ),
        ],
        hidden_check: &["python3", "-m", "pytest", "-q", "tests"],
    },
];

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
    failure: Option<String>,
}

/// The hidden suite, identical for every arm.
fn hidden_pass(dir: &Path, check: &[&str]) -> bool {
    Command::new(check[0])
        .args(&check[1..])
        .current_dir(dir)
        .env("CARGO_INCREMENTAL", "0")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn write_fixture(dir: &Path, task: &Task) -> anyhow::Result<()> {
    for (path, content) in task.files {
        let full = dir.join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(full, content)?;
    }
    Ok(())
}

/// Arm 1: the ungoverned direct harness — the same model, task, turn
/// budget, deadline, and hidden suite, with bare read/write tools applied
/// straight to the workspace (no kernel, no gate, no ledger).
async fn run_direct(
    transport: &GenAiTransport,
    model: &ModelId,
    dir: &Path,
    task: &Task,
) -> anyhow::Result<CellResult> {
    let specs = vec![
        ToolSpec {
            name: "read_file".into(),
            description: "Read a file".into(),
            schema: serde_json::json!({"type": "object", "properties": {
                "path": {"type": "string", "description": "Relative path"}},
                "required": ["path"], "additionalProperties": false}),
            strict: false,
        },
        ToolSpec {
            name: "write_file".into(),
            description: "Create or replace a whole file".into(),
            schema: serde_json::json!({"type": "object", "properties": {
                "path": {"type": "string", "description": "Relative path"},
                "content": {"type": "string", "description": "Full file content"}},
                "required": ["path", "content"], "additionalProperties": false}),
            strict: false,
        },
    ];
    let mut conversation = Conversation::with_system(
        "You are a coding agent. Read and write files with the provided tools \
         until the task is complete, then reply DONE.",
    );
    conversation.push_user(task.goal.to_string());
    let mut cell = CellResult {
        task: task.id.into(),
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
        "read_file" => std::fs::read_to_string(&full).unwrap_or_else(|e| format!("error: {e}")),
        "write_file" => {
            let content = call
                .arguments
                .get("content")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            if let Some(parent) = full.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            cell.turns_to_first_mutation.get_or_insert(u64::from(turn));
            match std::fs::write(&full, content) {
                Ok(()) => "ok".into(),
                Err(error) => format!("error: {error}"),
            }
        }
        other => {
            cell.denied_calls += 1;
            format!("unknown tool {other}")
        }
    }
}

/// A governed arm: the full runtime with the arm's real toggles.
async fn run_governed(
    arm: &EvalArm,
    model: &str,
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
            primary: Some(model.to_string()),
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
        task: task.id.into(),
        arm: arm.name.into(),
        ..Default::default()
    };
    match runtime.run(task.goal.into()).await {
        Ok(summary) => {
            cell.model_turns = u64::from(summary.turns_used);
            fold_ledger_metrics(&database, &summary.session_id, &mut cell);
        }
        Err(error) => cell.failure = Some(error.to_string()),
    }
    Ok(cell)
}

/// Ledger-derived process metrics for a governed cell.
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
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let model = std::env::args()
        .nth(1)
        .expect("usage: psp10_eval <provider::model> [task-limit]");
    let limit: usize = std::env::args()
        .nth(2)
        .and_then(|value| value.parse().ok())
        .unwrap_or(TASKS.len());
    let config = perspt_core::Config::load_from_path(&PathBuf::from("config.local.toml"))?;
    let portfolio = std::sync::Arc::new(perspt_core::ModelPortfolio::from_config(&config)?);
    let direct_transport = GenAiTransport::new(portfolio);
    let (provider, bare_model) = model.split_once("::").unwrap_or(("openai", model.as_str()));
    let direct_model = ModelId::new(provider, bare_model);

    let mut results: Vec<CellResult> = Vec::new();
    for task in TASKS.iter().take(limit) {
        for arm in &ARMS {
            let dir = tempfile::tempdir()?;
            write_fixture(dir.path(), task)?;
            let started = std::time::Instant::now();
            let mut cell = if arm.direct {
                run_direct(&direct_transport, &direct_model, dir.path(), task).await?
            } else {
                run_governed(arm, &model, &config, dir.path(), task).await?
            };
            cell.wall_secs = started.elapsed().as_secs_f64();
            // The hidden suite decides hard pass for every arm identically.
            cell.hard_pass = hidden_pass(dir.path(), task.hidden_check);
            eprintln!(
                "{}/{}: hard_pass={} wall={:.1}s turns={}",
                task.id, arm.name, cell.hard_pass, cell.wall_secs, cell.model_turns
            );
            results.push(cell);
        }
    }

    // Paired primary contrast: adaptive (arm 5) vs governed single (arm 4:
    // the full non-search stack, so the contrast isolates search).
    let passes = |arm: &str| -> Vec<f64> {
        TASKS
            .iter()
            .take(limit)
            .map(|task| {
                results
                    .iter()
                    .find(|cell| cell.task == task.id && cell.arm == arm)
                    .map(|cell| f64::from(u8::from(cell.hard_pass)))
                    .unwrap_or(0.0)
            })
            .collect()
    };
    let adaptive = passes("adaptive");
    let single = passes("paging");
    let differences: Vec<f64> = adaptive.iter().zip(&single).map(|(a, s)| a - s).collect();
    let (lower, upper) = bootstrap_ci(&differences, 11, 10_000);

    let report = serde_json::json!({
        "results": results,
        "paired_contrast": {
            "arms": ["adaptive", "paging"],
            "bootstrap_seed": 11,
            "resamples": 10_000,
            "ci95": [lower, upper],
        },
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
