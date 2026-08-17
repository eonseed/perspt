//! Larger live parity benchmark: direct whole-file generation versus PSP-9.
//!
//! The two arms receive the same task, starting repository, model route, and
//! immutable test oracle. The direct arm gets one request and may replace only
//! the implementation file. The governed arm gets the ordinary bounded PSP-9
//! tool loop. Run directories and ledgers are retained for inspection.
//!
//! Usage:
//!   cargo run -p perspt-agent --example larger_parity_bench -- config.local.toml
//!   cargo run -p perspt-agent --example larger_parity_bench -- \
//!     config.local.toml vertex::gemini-3.7-flash
//!
//! Requires live credentials and, for the Python case, `uv` plus network access
//! on the first run so pytest can be installed before either timed arm starts.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use perspt_sdk::{Conversation, ModelId, ModelTransport, ToolChoicePolicy, TurnOutput};

#[derive(Clone, Copy)]
enum Language {
    Rust,
    Python,
}

struct SourceFile {
    path: &'static str,
    content: &'static str,
}

struct Fixture {
    id: &'static str,
    language: Language,
    task: &'static str,
    target: &'static str,
    files: &'static [SourceFile],
}

#[derive(Debug)]
struct ArmResult {
    passed: bool,
    seconds: f64,
    requests_or_turns: u32,
    detail: String,
    directory: PathBuf,
    session_id: Option<String>,
    ledger_events: usize,
    tool_calls: usize,
    measurements: usize,
    gate_decisions: usize,
    denials: usize,
}

const RUST_FILES: &[SourceFile] = &[
    SourceFile {
        path: "Cargo.toml",
        content: r#"[package]
name = "workflow-scheduler"
version = "0.1.0"
edition = "2021"

[lints.clippy]
all = "deny"
pedantic = "deny"
"#,
    },
    SourceFile {
        path: "src/lib.rs",
        content: r#"use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSpec {
    pub id: String,
    pub duration: u32,
    pub dependencies: Vec<String>,
}

impl TaskSpec {
    #[must_use]
    pub fn new(id: &str, duration: u32, dependencies: &[&str]) -> Self {
        Self {
            id: id.to_string(),
            duration,
            dependencies: dependencies.iter().map(|value| (*value).to_string()).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
    pub batches: Vec<Vec<String>>,
    pub makespan: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleError {
    ZeroParallelism,
    EmptyId,
    DuplicateId(String),
    DuplicateDependency { task: String, dependency: String },
    MissingDependency { task: String, dependency: String },
    SelfDependency(String),
    Cycle(Vec<String>),
    DurationOverflow,
}

/// Build deterministic execution batches for a dependency graph.
///
/// This implementation is intentionally incomplete. See the integration tests
/// and task statement for the required scheduling and validation contract.
pub fn schedule(tasks: &[TaskSpec], max_parallel: usize) -> Result<Schedule, ScheduleError> {
    if max_parallel == 0 {
        return Err(ScheduleError::ZeroParallelism);
    }
    let by_id: BTreeMap<_, _> = tasks.iter().map(|task| (task.id.clone(), task)).collect();
    let mut ready: Vec<_> = by_id.keys().cloned().collect();
    ready.sort();
    Ok(Schedule {
        batches: ready.chunks(max_parallel).map(<[_]>::to_vec).collect(),
        makespan: 0,
    })
}
"#,
    },
    SourceFile {
        path: "tests/scheduler.rs",
        content: r#"use workflow_scheduler::{schedule, Schedule, ScheduleError, TaskSpec};

fn task(id: &str, duration: u32, dependencies: &[&str]) -> TaskSpec {
    TaskSpec::new(id, duration, dependencies)
}

#[test]
fn schedules_a_branching_graph_deterministically() {
    let input = vec![
        task("package", 2, &["compile", "docs"]),
        task("docs", 4, &["fetch"]),
        task("fetch", 3, &[]),
        task("compile", 5, &["fetch"]),
        task("publish", 1, &["package"]),
        task("audit", 2, &["compile"]),
    ];
    assert_eq!(
        schedule(&input, 2),
        Ok(Schedule {
            batches: vec![
                vec!["fetch".into()],
                vec!["compile".into(), "docs".into()],
                vec!["audit".into(), "package".into()],
                vec!["publish".into()],
            ],
            makespan: 15,
        })
    );
}

#[test]
fn ready_tasks_are_ordered_by_duration_then_id() {
    let input = vec![
        task("zebra", 1, &[]),
        task("alpha", 3, &[]),
        task("beta", 1, &[]),
        task("omega", 2, &[]),
    ];
    assert_eq!(
        schedule(&input, 2).unwrap(),
        Schedule {
            batches: vec![
                vec!["beta".into(), "zebra".into()],
                vec!["omega".into(), "alpha".into()],
            ],
            makespan: 4,
        }
    );
}

#[test]
fn validates_identifiers_and_edges() {
    assert_eq!(schedule(&[], 0), Err(ScheduleError::ZeroParallelism));
    assert_eq!(schedule(&[task("", 1, &[])], 1), Err(ScheduleError::EmptyId));
    assert_eq!(
        schedule(&[task("a", 1, &[]), task("a", 2, &[])], 1),
        Err(ScheduleError::DuplicateId("a".into()))
    );
    assert_eq!(
        schedule(&[task("a", 1, &["missing"])], 1),
        Err(ScheduleError::MissingDependency {
            task: "a".into(),
            dependency: "missing".into(),
        })
    );
    assert_eq!(
        schedule(&[task("a", 1, &["a"])], 1),
        Err(ScheduleError::SelfDependency("a".into()))
    );
    assert_eq!(
        schedule(&[task("a", 1, &["b", "b"]), task("b", 1, &[])], 1),
        Err(ScheduleError::DuplicateDependency {
            task: "a".into(),
            dependency: "b".into(),
        })
    );
}

#[test]
fn reports_all_cycle_members_in_sorted_order() {
    let input = vec![
        task("leaf", 1, &["root"]),
        task("root", 1, &["middle"]),
        task("middle", 1, &["root"]),
    ];
    assert_eq!(
        schedule(&input, 2),
        Err(ScheduleError::Cycle(vec!["middle".into(), "root".into()]))
    );
}

#[test]
fn catches_makespan_overflow() {
    let input = vec![task("a", u32::MAX, &[]), task("b", 1, &["a"])];
    assert_eq!(schedule(&input, 1), Err(ScheduleError::DurationOverflow));
}

#[test]
fn input_order_does_not_change_the_schedule() {
    let mut input = vec![task("b", 2, &[]), task("a", 2, &[]), task("c", 1, &["a", "b"])];
    let forward = schedule(&input, 2).unwrap();
    input.reverse();
    assert_eq!(schedule(&input, 2).unwrap(), forward);
}
"#,
    },
];

const PYTHON_FILES: &[SourceFile] = &[
    SourceFile {
        path: "pyproject.toml",
        content: r#"[project]
name = "eventfold"
version = "0.1.0"
requires-python = ">=3.11"

[dependency-groups]
dev = ["pytest>=8.0"]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.pytest.ini_options]
addopts = "-q"
testpaths = ["tests"]
"#,
    },
    SourceFile {
        path: "src/eventfold/__init__.py",
        content: r#"from .fold import (
    AcceptedEvent,
    ConflictingEvent,
    GapError,
    InvalidEvent,
    NegativeBalance,
    Reconciliation,
    reconcile,
)

__all__ = [
    "AcceptedEvent",
    "ConflictingEvent",
    "GapError",
    "InvalidEvent",
    "NegativeBalance",
    "Reconciliation",
    "reconcile",
]
"#,
    },
    SourceFile {
        path: "src/eventfold/fold.py",
        content: r#"from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
from typing import Any, Iterable, Mapping


class ReconcileError(ValueError):
    """Base class for deterministic reconciliation failures."""


class InvalidEvent(ReconcileError):
    pass


class ConflictingEvent(ReconcileError):
    def __init__(self, source: str, offset: int) -> None:
        super().__init__(f"conflicting event at {source}:{offset}")
        self.source = source
        self.offset = offset


class GapError(ReconcileError):
    def __init__(self, source: str, expected: int, actual: int) -> None:
        super().__init__(f"gap for {source}: expected {expected}, got {actual}")
        self.source = source
        self.expected = expected
        self.actual = actual


class NegativeBalance(ReconcileError):
    def __init__(self, account: str, balance: int) -> None:
        super().__init__(f"negative balance for {account}: {balance}")
        self.account = account
        self.balance = balance


@dataclass(frozen=True)
class AcceptedEvent:
    source: str
    offset: int
    timestamp: datetime
    account: str
    kind: str
    amount: int


@dataclass(frozen=True)
class Reconciliation:
    events: tuple[AcceptedEvent, ...]
    balances: Mapping[str, int]
    duplicates_removed: int


def reconcile(events: Iterable[Mapping[str, Any]]) -> Reconciliation:
    """Reconcile source streams. This placeholder ignores the required contract."""
    accepted = []
    balances: dict[str, int] = {}
    for raw in events:
        amount = int(raw.get("amount", 0))
        account = str(raw.get("account", ""))
        balances[account] = balances.get(account, 0) + amount
        accepted.append(
            AcceptedEvent(
                source=str(raw.get("source", "")),
                offset=int(raw.get("offset", 0)),
                timestamp=datetime.fromisoformat(str(raw.get("timestamp"))),
                account=account,
                kind=str(raw.get("kind", "credit")),
                amount=amount,
            )
        )
    return Reconciliation(tuple(accepted), balances, 0)
"#,
    },
    SourceFile {
        path: "tests/test_fold.py",
        content: r#"from __future__ import annotations

from copy import deepcopy
from datetime import timezone
from types import MappingProxyType

import pytest

from eventfold import ConflictingEvent, GapError, InvalidEvent, NegativeBalance, reconcile


def event(source, offset, timestamp, account, kind, amount):
    return {
        "source": source,
        "offset": offset,
        "timestamp": timestamp,
        "account": account,
        "kind": kind,
        "amount": amount,
    }


def test_reconciles_deterministically_and_removes_exact_duplicates():
    rows = [
        event("west", 0, "2026-01-01T00:00:02Z", "alice", "debit", 3),
        event("east", 1, "2026-01-01T00:00:03+00:00", "alice", "debit", 2),
        event("east", 0, "2026-01-01T00:00:01Z", "alice", "credit", 10),
        event("west", 0, "2026-01-01T00:00:02Z", "alice", "debit", 3),
        event("west", 1, "2026-01-01T00:00:03Z", "bob", "credit", 7),
    ]
    before = deepcopy(rows)
    result = reconcile(rows)
    assert rows == before
    assert [(item.source, item.offset) for item in result.events] == [
        ("east", 0), ("west", 0), ("east", 1), ("west", 1)
    ]
    assert all(item.timestamp.tzinfo == timezone.utc for item in result.events)
    assert dict(result.balances) == {"alice": 5, "bob": 7}
    assert isinstance(result.balances, MappingProxyType)
    assert result.duplicates_removed == 1


def test_ties_are_sorted_by_source_then_offset():
    rows = [
        event("z", 0, "2026-01-01T00:00:00Z", "a", "credit", 1),
        event("a", 0, "2026-01-01T00:00:00Z", "a", "credit", 2),
    ]
    assert [(item.source, item.offset) for item in reconcile(rows).events] == [("a", 0), ("z", 0)]


def test_conflicting_duplicate_is_rejected():
    rows = [
        event("a", 0, "2026-01-01T00:00:00Z", "x", "credit", 1),
        event("a", 0, "2026-01-01T00:00:00Z", "x", "credit", 2),
    ]
    with pytest.raises(ConflictingEvent) as caught:
        reconcile(rows)
    assert (caught.value.source, caught.value.offset) == ("a", 0)


def test_source_offsets_must_start_at_zero_and_be_contiguous():
    with pytest.raises(GapError) as caught:
        reconcile([event("a", 1, "2026-01-01T00:00:00Z", "x", "credit", 1)])
    assert (caught.value.source, caught.value.expected, caught.value.actual) == ("a", 0, 1)

    rows = [
        event("a", 0, "2026-01-01T00:00:00Z", "x", "credit", 1),
        event("a", 2, "2026-01-01T00:00:01Z", "x", "credit", 1),
    ]
    with pytest.raises(GapError) as caught:
        reconcile(rows)
    assert (caught.value.source, caught.value.expected, caught.value.actual) == ("a", 1, 2)


@pytest.mark.parametrize(
    "patch",
    [
        {"source": ""},
        {"offset": -1},
        {"offset": True},
        {"timestamp": "not-a-time"},
        {"timestamp": "2026-01-01T00:00:00"},
        {"account": ""},
        {"kind": "refund"},
        {"amount": 0},
        {"amount": True},
        {"amount": 1.5},
    ],
)
def test_invalid_fields_are_rejected(patch):
    row = event("a", 0, "2026-01-01T00:00:00Z", "x", "credit", 1)
    row.update(patch)
    with pytest.raises(InvalidEvent):
        reconcile([row])


def test_debits_are_applied_in_canonical_order_and_cannot_go_negative():
    rows = [
        event("b", 0, "2026-01-01T00:00:02Z", "x", "credit", 5),
        event("a", 0, "2026-01-01T00:00:01Z", "x", "debit", 1),
    ]
    with pytest.raises(NegativeBalance) as caught:
        reconcile(rows)
    assert (caught.value.account, caught.value.balance) == ("x", -1)


def test_empty_input_has_immutable_empty_balances():
    result = reconcile([])
    assert result.events == ()
    assert dict(result.balances) == {}
    assert isinstance(result.balances, MappingProxyType)
"#,
    },
];

fn fixtures() -> Vec<Fixture> {
    vec![
        Fixture {
            id: "rust-dependency-scheduler",
            language: Language::Rust,
            target: "src/lib.rs",
            task: concat!(
                "Implement schedule() in src/lib.rs. Preserve the public API and tests. ",
                "Validate zero parallelism, empty and duplicate task IDs, duplicate/missing/self ",
                "dependencies, and cycles. A cycle error must contain only cycle members in ",
                "sorted order, not downstream blocked tasks. Repeatedly choose currently ready ",
                "tasks ordered by (duration, id), take at most max_parallel as the next batch, ",
                "and define batch duration as its maximum task duration. The makespan is the ",
                "checked sum of batch durations; return DurationOverflow on overflow. Results ",
                "must be independent of input order. Do not modify tests."
            ),
            files: RUST_FILES,
        },
        Fixture {
            id: "python-event-reconciler",
            language: Language::Python,
            target: "src/eventfold/fold.py",
            task: concat!(
                "Implement reconcile() in src/eventfold/fold.py without changing its public API ",
                "or tests. Validate every field strictly: source/account are non-empty strings; ",
                "offset is a non-negative int but not bool; timestamp is a timezone-aware ",
                "RFC3339/ISO timestamp normalized to UTC; kind is credit or debit; amount is a ",
                "positive int but not bool. Deduplicate exact same (source, offset) events, count ",
                "removals, and reject different content at the same key. Each source must start ",
                "at offset 0 and be contiguous. Canonically merge by (UTC timestamp, source, ",
                "offset), fold credits/debits in that order, and raise NegativeBalance at the ",
                "first negative balance. Do not mutate inputs. Return tuple events and a read-only ",
                "MappingProxyType balance mapping. Do not modify tests or dependencies."
            ),
            files: PYTHON_FILES,
        },
    ]
}

fn write_fixture(root: &Path, fixture: &Fixture) -> Result<()> {
    for file in fixture.files {
        let path = root.join(file.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, file.content)?;
    }
    if matches!(fixture.language, Language::Python) {
        let status = Command::new("uv")
            .args(["sync", "--quiet"])
            .current_dir(root)
            .status()
            .context("running uv sync for the Python fixture")?;
        anyhow::ensure!(status.success(), "uv sync failed for Python fixture");
    }
    Ok(())
}

fn verify(root: &Path, language: Language) -> Result<bool> {
    let run = |program: &str, arguments: &[&str]| -> Result<bool> {
        Ok(Command::new(program)
            .args(arguments)
            .current_dir(root)
            .status()?
            .success())
    };
    match language {
        Language::Rust => Ok(run("cargo", &["test", "--quiet"])?
            && run("cargo", &["clippy", "--quiet", "--", "-D", "warnings"])?),
        Language::Python => Ok(run(
            "uv",
            &[
                "run",
                "--no-sync",
                "python",
                "-m",
                "compileall",
                "-q",
                "src",
            ],
        )? && run("uv", &["run", "--no-sync", "pytest", "-q"])?),
    }
}

fn extract_file(text: &str) -> String {
    if let Some(open) = text.find("```") {
        let after = &text[open + 3..];
        let body_start = after.find('\n').map_or(0, |index| index + 1);
        if let Some(close) = after[body_start..].find("```") {
            return after[body_start..body_start + close].to_string();
        }
    }
    text.to_string()
}

fn repository_prompt(fixture: &Fixture) -> String {
    let mut prompt = format!("Task: {}\n\nRepository files:\n", fixture.task);
    for file in fixture.files {
        prompt.push_str(&format!(
            "\n--- {} ---\n```\n{}\n```\n",
            file.path, file.content
        ));
    }
    prompt
}

async fn direct_arm(
    root: &Path,
    transport: &perspt_agent::GenAiTransport,
    model: &ModelId,
    fixture: &Fixture,
) -> Result<ArmResult> {
    write_fixture(root, fixture)?;
    let mut conversation = Conversation::with_system(format!(
        concat!(
            "You are a coding assistant in a controlled benchmark. Reply with the complete new ",
            "contents of {} in exactly one fenced code block and no commentary. You may not ",
            "change any other file."
        ),
        fixture.target
    ));
    conversation.push_user(repository_prompt(fixture));
    let start = Instant::now();
    let output = match transport
        .chat_turn(model, &conversation, &[], ToolChoicePolicy::None)
        .await
    {
        Ok(output) => output,
        Err(error) => {
            return Ok(ArmResult {
                passed: false,
                seconds: start.elapsed().as_secs_f64(),
                requests_or_turns: 1,
                detail: format!("transport failure preserved: {error}"),
                directory: root.to_path_buf(),
                session_id: None,
                ledger_events: 0,
                tool_calls: 0,
                measurements: 0,
                gate_decisions: 0,
                denials: 0,
            });
        }
    };
    let TurnOutput::Text(text) = output else {
        return Ok(ArmResult {
            passed: false,
            seconds: start.elapsed().as_secs_f64(),
            requests_or_turns: 1,
            detail: "protocol failure preserved: model returned undeclared tool calls".into(),
            directory: root.to_path_buf(),
            session_id: None,
            ledger_events: 0,
            tool_calls: 0,
            measurements: 0,
            gate_decisions: 0,
            denials: 0,
        });
    };
    std::fs::write(root.join(fixture.target), extract_file(&text))?;
    let passed = verify(root, fixture.language)?;
    Ok(ArmResult {
        passed,
        seconds: start.elapsed().as_secs_f64(),
        requests_or_turns: 1,
        detail: "one ungoverned whole-file request".into(),
        directory: root.to_path_buf(),
        session_id: None,
        ledger_events: 0,
        tool_calls: 0,
        measurements: 0,
        gate_decisions: 0,
        denials: 0,
    })
}

async fn governed_arm(
    root: &Path,
    config: &perspt_core::Config,
    model: &ModelId,
    fixture: &Fixture,
) -> Result<ArmResult> {
    write_fixture(root, fixture)?;
    let database = root.join("perspt-eval.db");
    let runtime = governed_runtime(root, config, model, database.clone())?;

    let start = Instant::now();
    let result = runtime.run(fixture.task.to_string()).await;
    let seconds = start.elapsed().as_secs_f64();
    let (session_id, turns, outcome_detail) = match result {
        Ok(summary) => (
            summary.session_id,
            summary.turns_used,
            format!("{:?}", summary.outcome),
        ),
        Err(error) => {
            let session_id = perspt_store::SessionStore::open(&database)?
                .list_recent_sessions(1)?
                .into_iter()
                .next()
                .map(|session| session.session_id)
                .unwrap_or_default();
            (session_id, 0, format!("runtime error: {error:#}"))
        }
    };
    summarize_governed(
        root,
        fixture,
        &database,
        session_id,
        turns,
        seconds,
        outcome_detail,
    )
}

fn governed_runtime(
    root: &Path,
    config: &perspt_core::Config,
    model: &ModelId,
    database: PathBuf,
) -> Result<perspt_agent::Psp9AgentRuntime> {
    let mut handlers = perspt_agent::CandidateHandlerRegistry::with_builtins();
    perspt_agent::tools::families::register_standard_families(&mut handlers)?;
    Ok(perspt_agent::Psp9AgentRuntime::from_config(
        root.to_path_buf(),
        config,
        perspt_agent::Psp9ModelRoutes {
            actuator: Some(model.to_string()),
            ..perspt_agent::Psp9ModelRoutes::default()
        },
        perspt_agent::Psp9RunConfig {
            approval_policy: perspt_sdk::ApprovalPolicy::Auto,
            max_turns: 12,
            max_calls_per_turn: 8,
            rejection_budget: 4,
            allow_unisolated_verifiers: false,
            ..perspt_agent::Psp9RunConfig::default()
        },
    )?
    .with_database_path(database.clone())
    .with_domain(Arc::new(perspt_coding::CodingDomain::new()))
    .with_tool_family(perspt_agent::tools::families::standard_family_entries())
    .with_tool_handlers(handlers))
}

fn summarize_governed(
    root: &Path,
    fixture: &Fixture,
    database: &PathBuf,
    session_id: String,
    turns: u32,
    seconds: f64,
    outcome_detail: String,
) -> Result<ArmResult> {
    let passed = verify(root, fixture.language)?;

    let rows = if session_id.is_empty() {
        Vec::new()
    } else {
        perspt_store::SessionStore::open(database)?.get_psp9_events(&session_id)?
    };
    let count = |needle: &str| {
        rows.iter()
            .filter(|row| row.event_json.contains(needle))
            .count()
    };
    let terminal = if passed {
        "post-run tests pass"
    } else {
        "post-run tests fail"
    };
    Ok(ArmResult {
        passed,
        seconds,
        requests_or_turns: turns,
        detail: format!("{outcome_detail}; {terminal}"),
        directory: root.to_path_buf(),
        session_id: (!session_id.is_empty()).then_some(session_id),
        ledger_events: rows.len(),
        tool_calls: count("tool_call_observed"),
        measurements: count("candidate_measured"),
        gate_decisions: count("gate_decision_recorded"),
        denials: count("effect_denied"),
    })
}

fn print_result(case: &str, arm: &str, result: &ArmResult) {
    println!(
        "{case:<29} {arm:<10} {:<4} {:>7.2}s {:>2} request/turn(s)  {}",
        if result.passed { "PASS" } else { "FAIL" },
        result.seconds,
        result.requests_or_turns,
        result.detail,
    );
    println!("  directory: {}", result.directory.display());
    if let Some(session_id) = &result.session_id {
        println!(
            "  session: {session_id}; ledger={} tool-calls={} measurements={} gates={} denials={}",
            result.ledger_events,
            result.tool_calls,
            result.measurements,
            result.gate_decisions,
            result.denials,
        );
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.local.toml".into());
    let config = perspt_core::Config::load_from_path(Path::new(&config_path))?;
    let configured_actuator: ModelId = config
        .models
        .clone()
        .unwrap_or_default()
        .actuator
        .context("[models].actuator is required")?
        .parse()
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    let actuator: ModelId = match std::env::args().nth(2) {
        Some(value) => value
            .parse()
            .map_err(|error| anyhow::anyhow!("invalid route override: {error}"))?,
        None => configured_actuator,
    };
    let portfolio = Arc::new(perspt_core::ModelPortfolio::from_config(&config)?);
    let transport = perspt_agent::GenAiTransport::new(portfolio);
    let output_root =
        std::env::temp_dir().join(format!("perspt-larger-parity-{}", std::process::id()));
    std::fs::create_dir_all(&output_root)?;

    println!("Larger parity benchmark using {actuator}");
    println!("Artifacts retained under {}\n", output_root.display());
    println!(
        "{:<29} {:<10} {:<4} {:>8} {:>20}  detail",
        "case", "arm", "test", "seconds", "model budget"
    );

    let mut failures = 0usize;
    for fixture in fixtures() {
        let direct_root = output_root.join(fixture.id).join("direct");
        let governed_root = output_root.join(fixture.id).join("governed");
        let direct = direct_arm(&direct_root, &transport, &actuator, &fixture).await?;
        print_result(fixture.id, "direct", &direct);
        failures += usize::from(!direct.passed);

        let governed = governed_arm(&governed_root, &config, &actuator, &fixture).await?;
        print_result(fixture.id, "governed", &governed);
        failures += usize::from(!governed.passed);
    }

    println!("\nAll failures were retained; {failures} of 4 arms failed their unchanged oracle.");
    println!("This is a smoke comparison, not a statistically powered reliability estimate.");
    Ok(())
}
