//! Parity and ablation benchmark (PSP-9 phase 14).
//!
//! Two arms per fixture task, failures preserved and reported:
//!
//! * `tool-loop` — the authoritative governed PSP-9 runtime.
//! * `whole-file` — the offline ablation baseline: one un-governed
//!   whole-file generation per task, measuring whether a typed whole-file
//!   proposal tool would be justified. It is a measurement arm only; it is
//!   not a production path and never becomes one.
//!
//! Usage:
//!   cargo run -p perspt-agent --example parity_bench -- config.local.toml
//!
//! Requires live credentials; never run in CI.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use perspt_sdk::{
    BenchmarkOutcome, BenchmarkReport, BenchmarkResult, Conversation, ModelId, ModelTransport,
    NodeTerminalOutcome, ToolChoicePolicy, TurnOutput,
};

struct Fixture {
    case_id: &'static str,
    task: &'static str,
    lib_rs: &'static str,
}

fn fixtures() -> Vec<Fixture> {
    vec![
        Fixture {
            case_id: "fix-failing-test",
            task: "Make the existing test pass by fixing answer() in src/lib.rs.",
            lib_rs: "pub fn answer() -> u32 { 1 }\n\n#[cfg(test)]\nmod tests {\n    \
                     #[test]\n    fn answer_is_two() { assert_eq!(super::answer(), 2); }\n}\n",
        },
        Fixture {
            case_id: "implement-missing-fn",
            task: "Implement the missing double() function in src/lib.rs so the \
                   crate compiles and its test passes.",
            lib_rs: "#[cfg(test)]\nmod tests {\n    #[test]\n    \
                     fn doubles() { assert_eq!(crate::double(21), 42); }\n}\n",
        },
    ]
}

fn write_project(root: &Path, lib_rs: &str) -> anyhow::Result<()> {
    std::fs::create_dir_all(root.join("src"))?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"parity-fixture\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
    )?;
    std::fs::write(root.join("src/lib.rs"), lib_rs)?;
    Ok(())
}

fn tests_pass(root: &Path) -> bool {
    std::process::Command::new("cargo")
        .args(["test", "--quiet"])
        .current_dir(root)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

struct ArmRun {
    outcome: BenchmarkOutcome,
    seconds: f64,
    detail: String,
}

async fn tool_loop_arm(config: &perspt_core::Config, fixture: &Fixture) -> anyhow::Result<ArmRun> {
    let dir = tempfile::tempdir()?;
    write_project(dir.path(), fixture.lib_rs)?;
    let database = dir.path().join("bench.db");
    let runtime = perspt_agent::Psp9AgentRuntime::from_config(
        dir.path().to_path_buf(),
        config,
        perspt_agent::Psp9ModelRoutes::default(),
        perspt_agent::Psp9RunConfig {
            approval_policy: perspt_sdk::ApprovalPolicy::Auto,
            max_turns: 8,
            allow_unisolated_verifiers: false,
            ..perspt_agent::Psp9RunConfig::default()
        },
    )?
    .with_database_path(database);
    let start = Instant::now();
    let result = runtime.run(fixture.task.to_string()).await;
    let seconds = start.elapsed().as_secs_f64();
    let (outcome, detail) = match result {
        Ok(summary) if matches!(summary.outcome, NodeTerminalOutcome::HardPass) => {
            // Accepted-state correctness: re-verify the *promoted* workspace.
            if tests_pass(dir.path()) {
                (
                    BenchmarkOutcome::HardPass,
                    format!("{} turn(s)", summary.turns_used),
                )
            } else {
                (
                    BenchmarkOutcome::FalseStability,
                    "hard pass claimed but promoted workspace fails".into(),
                )
            }
        }
        Ok(summary) => (
            BenchmarkOutcome::ResidualCertified,
            format!("{:?} after {} turn(s)", summary.outcome, summary.turns_used),
        ),
        Err(error) => (
            BenchmarkOutcome::ResidualCertified,
            format!("error: {error:#}"),
        ),
    };
    Ok(ArmRun {
        outcome,
        seconds,
        detail,
    })
}

fn extract_file(text: &str) -> String {
    // Take the first fenced code block if present, else the raw text.
    if let Some(open) = text.find("```") {
        let after = &text[open + 3..];
        let body_start = after.find('\n').map(|i| i + 1).unwrap_or(0);
        if let Some(close) = after[body_start..].find("```") {
            return after[body_start..body_start + close].to_string();
        }
    }
    text.to_string()
}

async fn whole_file_arm(
    transport: &perspt_agent::GenAiTransport,
    model: &ModelId,
    fixture: &Fixture,
) -> anyhow::Result<ArmRun> {
    let dir = tempfile::tempdir()?;
    write_project(dir.path(), fixture.lib_rs)?;
    let mut conversation = Conversation::with_system(
        "You are a coding assistant. Reply with the complete new contents of \
         src/lib.rs in a single fenced code block. No commentary.",
    );
    conversation.push_user(format!(
        "Task: {}\n\nCurrent src/lib.rs:\n```rust\n{}\n```",
        fixture.task, fixture.lib_rs
    ));
    let start = Instant::now();
    let output = transport
        .chat_turn(model, &conversation, &[], ToolChoicePolicy::None)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let TurnOutput::Text(text) = output else {
        anyhow::bail!("whole-file arm received tool calls with no tools declared");
    };
    std::fs::write(dir.path().join("src/lib.rs"), extract_file(&text))?;
    let passed = tests_pass(dir.path());
    let seconds = start.elapsed().as_secs_f64();
    Ok(ArmRun {
        outcome: if passed {
            BenchmarkOutcome::HardPass
        } else {
            BenchmarkOutcome::ResidualCertified
        },
        seconds,
        detail: "1 request, ungoverned baseline".into(),
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.local.toml".into());
    let config = perspt_core::Config::load_from_path(Path::new(&config_path))?;
    let models = config.models.clone().unwrap_or_default();
    let actuator: ModelId = models
        .actuator
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("[models].actuator required"))?
        .parse()
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let portfolio = Arc::new(perspt_core::ModelPortfolio::from_config(&config)?);
    let transport = perspt_agent::GenAiTransport::new(portfolio);

    let mut tool_loop_report = BenchmarkReport::new();
    let mut whole_file_report = BenchmarkReport::new();
    println!("Parity benchmark: governed tool loop vs whole-file ablation baseline\n");
    println!(
        "  {:<24} {:<12} {:>9} {:>9}  detail",
        "case", "arm", "outcome", "seconds",
    );
    for fixture in fixtures() {
        let governed = tool_loop_arm(&config, &fixture).await?;
        println!(
            "  {:<24} {:<12} {:>9} {:>8.2}s  {}",
            fixture.case_id,
            "tool-loop",
            format!("{:?}", governed.outcome),
            governed.seconds,
            governed.detail
        );
        tool_loop_report.add(BenchmarkResult::new(fixture.case_id, governed.outcome));

        let baseline = whole_file_arm(&transport, &actuator, &fixture).await?;
        println!(
            "  {:<24} {:<12} {:>9} {:>8.2}s  {}",
            fixture.case_id,
            "whole-file",
            format!("{:?}", baseline.outcome),
            baseline.seconds,
            baseline.detail
        );
        whole_file_report.add(BenchmarkResult::new(fixture.case_id, baseline.outcome));
    }

    println!();
    println!(
        "tool-loop:  hard-pass {:.0}%, residual-certified {:.0}%, false-stability {:.0}% (must be 0)",
        tool_loop_report.hard_pass_rate() * 100.0,
        tool_loop_report.residual_certified_rate() * 100.0,
        tool_loop_report.false_stability_rate() * 100.0,
    );
    println!(
        "whole-file: hard-pass {:.0}%, residual-certified {:.0}%  (ungoverned ablation baseline)",
        whole_file_report.hard_pass_rate() * 100.0,
        whole_file_report.residual_certified_rate() * 100.0,
    );
    anyhow::ensure!(
        tool_loop_report.is_correctness_conformant(),
        "false stability observed: the parity gate FAILS"
    );
    println!();
    println!("Failures preserved; no run was omitted (System 13).");
    Ok(())
}
