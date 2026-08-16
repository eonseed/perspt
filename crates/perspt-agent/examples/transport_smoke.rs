//! Live transport smoke test and micro-benchmark (PSP-9 phase 1 exit
//! criterion): a model completes a two-tool round trip on each configured
//! route from one binary, through the SDK's provider-neutral contract.
//!
//! Usage:
//!   cargo run -p perspt-agent --example transport_smoke -- config.local.toml
//!
//! Requires live credentials; never run in CI.

use std::sync::Arc;
use std::time::Instant;

use perspt_agent::GenAiTransport;
use perspt_sdk::{Conversation, ModelId, ModelTransport, ToolChoicePolicy, ToolSpec, TurnOutput};

fn tool_specs() -> Vec<ToolSpec> {
    vec![ToolSpec {
        name: "read_file".into(),
        description: "Read the contents of a workspace file".into(),
        schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Workspace-relative path"}
            },
            "required": ["path"],
        }),
        strict: false,
    }]
}

/// One two-tool round trip; returns (turns, tool calls observed, latencies).
async fn round_trip(
    transport: &GenAiTransport,
    model: &ModelId,
) -> anyhow::Result<(u32, u32, Vec<f64>)> {
    let mut conversation = Conversation::with_system(
        "You are a coding agent. Use the read_file tool to inspect files before answering. \
         Keep answers to one sentence.",
    );
    conversation
        .push_user("What does src/lib.rs contain? Read it first, then summarize in one sentence.");

    let specs = tool_specs();
    let mut latencies = Vec::new();
    let mut tool_calls_seen = 0u32;
    let mut turns = 0u32;

    for _ in 0..4 {
        turns += 1;
        let start = Instant::now();
        let output = transport
            .chat_turn(model, &conversation, &specs, ToolChoicePolicy::Auto)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        latencies.push(start.elapsed().as_secs_f64());

        match output {
            TurnOutput::ToolCalls(calls) => {
                tool_calls_seen += calls.len() as u32;
                conversation.push_tool_calls(calls.clone());
                for call in &calls {
                    // Scripted tool result: the transport contract is what is
                    // under test, not the executor.
                    conversation.push_tool_response(
                        call.call_id.clone(),
                        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
                    );
                }
            }
            TurnOutput::Text(text) => {
                println!("    final: {}", text.trim().lines().next().unwrap_or(""));
                break;
            }
        }
    }
    Ok((turns, tool_calls_seen, latencies))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.local.toml".into());
    let config = perspt_core::Config::load_from_path(std::path::Path::new(&config_path))?;
    let models = config.models.clone().unwrap_or_default();
    let portfolio = Arc::new(perspt_core::ModelPortfolio::from_config(&config)?);
    let transport = GenAiTransport::new(portfolio);

    let routes: Vec<ModelId> = [models.architect, models.actuator]
        .into_iter()
        .flatten()
        .map(|s| s.parse())
        .collect::<Result<_, _>>()
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let rounds: u32 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    println!(
        "Transport smoke: {} route(s), {rounds} round(s) each, from one binary\n",
        routes.len()
    );
    let mut failures = 0;
    for model in &routes {
        println!("== {model} (family {:?})", transport.family_of(model));
        let mut totals = Vec::new();
        let mut all_calls = 0;
        for round in 0..rounds {
            match round_trip(&transport, model).await {
                Ok((turns, calls, latencies)) => {
                    let total: f64 = latencies.iter().sum();
                    totals.push(total);
                    all_calls += calls;
                    let per_turn: Vec<String> =
                        latencies.iter().map(|l| format!("{l:.2}s")).collect();
                    println!(
                        "    round {}: turns {turns}, tool calls {calls}, {total:.2}s \
                         [{}]",
                        round + 1,
                        per_turn.join(", ")
                    );
                }
                Err(e) => {
                    failures += 1;
                    println!("    round {}: FAIL: {e:#}", round + 1);
                }
            }
        }
        if !totals.is_empty() {
            let mean = totals.iter().sum::<f64>() / totals.len() as f64;
            let min = totals.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = totals.iter().cloned().fold(0.0, f64::max);
            println!(
                "    summary: mean {mean:.2}s, min {min:.2}s, max {max:.2}s over {} \
                 round(s), {all_calls} tool call(s)",
                totals.len()
            );
            if all_calls == 0 {
                println!("    WARN: route returned no tool calls (degradation candidate)");
            }
        }
        println!();
    }
    if failures > 0 {
        anyhow::bail!("{failures} route(s) failed the round trip");
    }
    println!("All routes completed the tool round trip through one contract (Gate S).");
    Ok(())
}
