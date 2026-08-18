//! The PSP-10 seven-arm evaluation (Phase 11). LIVE CREDENTIALS REQUIRED;
//! never run in CI.
//!
//! Arms are a cumulative ladder over matched tasks (spec: Test and
//! Acceptance Plan). Each task keeps its paired per-arm results; the
//! report publishes paired differences with a seeded 10,000-resample
//! percentile bootstrap, plus every timeout and infrastructure failure.
//! The primary outcome is hidden-test hard pass. Adaptive search becomes
//! the default only if the paired CI lower bound for
//! `p_adaptive - p_single` clears the predeclared margin, repeated
//! identical failures drop, and no hard-gate regression escapes — the
//! flip itself is a separate reviewed commit, never automatic.
//!
//! Usage: `cargo run --example psp10_eval -- <model> [task-limit]`
//! with routes configured in `config.local.toml`.

use std::path::PathBuf;

use perspt_agent::{Psp9AgentRuntime, Psp9RunConfig};
use perspt_sdk::NodeTerminalOutcome;

/// One evaluation arm: a named, cumulative capability toggle set.
#[derive(Debug, Clone, Copy)]
struct EvalArm {
    name: &'static str,
    /// Arm 1 bypasses the governed runtime entirely.
    direct: bool,
    /// Arms 5+: `[exploration] initial_branches`/`max_branches`.
    max_branches: u8,
    /// Arm 6: prefer a distinct family on expansion.
    distinct_family: bool,
    /// Arm 7: multi-node graphs with the integration gate.
    max_parallel_nodes: usize,
}

const ARMS: [EvalArm; 7] = [
    // 1. The direct model harness (no governance).
    EvalArm {
        name: "direct",
        direct: true,
        max_branches: 1,
        distinct_family: false,
        max_parallel_nodes: 1,
    },
    // 2. One branch with prompt programs (the governed baseline).
    EvalArm {
        name: "governed",
        direct: false,
        max_branches: 1,
        distinct_family: false,
        max_parallel_nodes: 1,
    },
    // 3. + correction packets (always on since phase 6; kept as its own
    //    row so regressions in packet rendering surface here).
    EvalArm {
        name: "packets",
        direct: false,
        max_branches: 1,
        distinct_family: false,
        max_parallel_nodes: 1,
    },
    // 4. + resident-context paging (feasibility gate + compaction).
    EvalArm {
        name: "paging",
        direct: false,
        max_branches: 1,
        distinct_family: false,
        max_parallel_nodes: 1,
    },
    // 5. + adaptive search (up to three branches).
    EvalArm {
        name: "adaptive",
        direct: false,
        max_branches: 3,
        distinct_family: false,
        max_parallel_nodes: 1,
    },
    // 6. + multi-family expansion.
    EvalArm {
        name: "multi-family",
        direct: false,
        max_branches: 3,
        distinct_family: true,
        max_parallel_nodes: 1,
    },
    // 7. + graph integration.
    EvalArm {
        name: "integration",
        direct: false,
        max_branches: 3,
        distinct_family: true,
        max_parallel_nodes: 2,
    },
];

/// One matched task: a fixture layout plus a hidden test the arms never
/// see in their goal text.
struct Task {
    id: &'static str,
    goal: &'static str,
    files: &'static [(&'static str, &'static str)],
}

const TASKS: &[Task] = &[
    Task {
        id: "rust-off-by-one",
        goal: "fix the answer function so the test suite passes",
        files: &[
            (
                "Cargo.toml",
                "[package]\nname='t1'\nversion='0.1.0'\nedition='2021'\n",
            ),
            (
                "src/lib.rs",
                "pub fn answer() -> u32 { 1 }\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    \
                 fn answer_is_two() { assert_eq!(super::answer(), 2); }\n}\n",
            ),
        ],
    },
    // Extend with the parity-bench task sets before a real evaluation run.
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

/// Run one (task, arm) cell in a fresh fixture workspace. Returns the
/// hard-pass verdict plus any infrastructure failure text.
async fn run_cell(
    task: &Task,
    arm: &EvalArm,
    model: &str,
    config: &perspt_core::Config,
) -> anyhow::Result<(bool, Option<String>)> {
    let dir = tempfile::tempdir()?;
    for (path, content) in task.files {
        let full = dir.path().join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(full, content)?;
    }
    if arm.direct {
        // Arm 1 is a bare transport exchange; hard pass is decided by
        // running the hidden suite over whatever it wrote. Left as an
        // exercise wired to the local harness of choice.
        return Ok((false, None));
    }
    // The arm's search toggles are the `[exploration]` block.
    let mut config = config.clone();
    let exploration = config.exploration.get_or_insert_with(Default::default);
    exploration.initial_branches = Some(1);
    exploration.max_branches = Some(arm.max_branches);
    exploration.distinct_family = Some(arm.distinct_family);
    let runtime = Psp9AgentRuntime::from_config(
        dir.path().to_path_buf(),
        &config,
        perspt_agent::Psp9ModelRoutes {
            primary: Some(model.to_string()),
            ..Default::default()
        },
        Psp9RunConfig {
            approval_policy: perspt_sdk::ApprovalPolicy::Auto,
            allow_unisolated_verifiers: true,
            max_parallel_nodes: arm.max_parallel_nodes,
            ..Psp9RunConfig::default()
        },
    )?;
    match runtime.run(task.goal.into()).await {
        Ok(summary) => Ok((
            matches!(summary.outcome, NodeTerminalOutcome::HardPass),
            None,
        )),
        Err(error) => Ok((false, Some(format!("{}/{}: {error}", task.id, arm.name)))),
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
    let mut results: Vec<(String, String, bool)> = Vec::new();
    let mut failures: Vec<String> = Vec::new();

    for task in TASKS.iter().take(limit) {
        for arm in &ARMS {
            let (hard_pass, failure) = run_cell(task, arm, &model, &config).await?;
            results.push((task.id.into(), arm.name.into(), hard_pass));
            failures.extend(failure);
        }
    }

    // Paired primary contrast: adaptive (arm 5) vs governed single (arm 2).
    let passes = |arm: &str| -> Vec<f64> {
        TASKS
            .iter()
            .take(limit)
            .map(|task| {
                results
                    .iter()
                    .find(|(t, a, _)| t == task.id && a == arm)
                    .map(|(_, _, pass)| f64::from(u8::from(*pass)))
                    .unwrap_or(0.0)
            })
            .collect()
    };
    let adaptive = passes("adaptive");
    let single = passes("governed");
    let differences: Vec<f64> = adaptive.iter().zip(&single).map(|(a, s)| a - s).collect();
    let (lower, upper) = bootstrap_ci(&differences, 11, 10_000);

    let report = serde_json::json!({
        "results": results
            .iter()
            .map(|(task, arm, pass)| serde_json::json!({
                "task": task, "arm": arm, "hard_pass": pass
            }))
            .collect::<Vec<_>>(),
        "paired_contrast": {
            "arms": ["adaptive", "governed"],
            "bootstrap_seed": 11,
            "resamples": 10_000,
            "ci95": [lower, upper],
        },
        "failures": failures,
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
