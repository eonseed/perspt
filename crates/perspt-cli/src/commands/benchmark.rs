//! Optional, credentialed benchmark commands.

use std::path::PathBuf;

use anyhow::Result;
use perspt_benchmark::{BenchmarkRunOptions, BenchmarkSuite};

pub fn validate() -> Result<()> {
    let report = perspt_benchmark::validate_corpus_command()?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    anyhow::ensure!(
        report.get("ready").and_then(serde_json::Value::as_bool) == Some(true),
        "evaluation corpus is not ready"
    );
    Ok(())
}

pub async fn run(
    config_path: Option<PathBuf>,
    suite: BenchmarkSuite,
    tasks: Option<usize>,
    output: Option<PathBuf>,
) -> Result<()> {
    let report = perspt_benchmark::run_benchmark(BenchmarkRunOptions {
        config_path,
        suite,
        task_limit: tasks,
        output,
    })
    .await?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

pub fn aggregate(reports: &[PathBuf]) -> Result<()> {
    let report = perspt_benchmark::aggregate_reports(reports)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
