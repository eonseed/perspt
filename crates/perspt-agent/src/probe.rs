//! Behavioral provider probes (PSP-9 Gate U).
//!
//! A declared capability matrix is a vendor claim; a probe is an observation.
//! Each probe runs a scripted two-tool round trip through the provider-neutral
//! transport and records what the route actually did: whether it issued tool
//! calls at all, whether it selected both distinct tools, whether it batched
//! calls in one turn, whether its arguments satisfied the declared schema,
//! and whether it consumed the results into a final answer. Evidence from a
//! probe is labelled `behavioral`, never merged silently into the declared
//! matrix.

use std::time::Instant;

use anyhow::Result;
use perspt_sdk::{Conversation, ModelId, ModelTransport, ToolChoicePolicy, ToolSpec, TurnOutput};
use serde::{Deserialize, Serialize};

/// What one live probe observed about a route.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeReport {
    pub model: String,
    /// The model issued at least one tool call and consumed its result.
    pub tool_call_round_trip: bool,
    /// The model selected both distinct probe tools across the run.
    pub multi_tool_selection: bool,
    /// The model batched more than one call into a single turn.
    pub parallel_tool_calls: bool,
    /// Every issued call carried arguments satisfying the declared schema.
    pub schema_arguments_valid: bool,
    pub turns: u32,
    pub tool_calls: u32,
    pub total_seconds: f64,
    pub error: Option<String>,
}

fn probe_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
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
        },
        ToolSpec {
            name: "list_files".into(),
            description: "List files in a workspace directory".into(),
            schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Directory to list"}
                },
                "required": ["path"],
            }),
            strict: false,
        },
    ]
}

fn scripted_result(name: &str) -> &'static str {
    match name {
        "read_file" => "pub fn add(a: i32, b: i32) -> i32 { a + b }",
        "list_files" => "src/lib.rs\nsrc/main.rs\nCargo.toml",
        _ => "unknown tool",
    }
}

/// Run one behavioral probe against a route. Never mutates anything: tool
/// results are scripted, so only the transport contract is under test.
pub async fn probe_route(transport: &dyn ModelTransport, model: &ModelId) -> ProbeReport {
    let start = Instant::now();
    let mut report = ProbeReport {
        model: model.to_string(),
        tool_call_round_trip: false,
        multi_tool_selection: false,
        parallel_tool_calls: false,
        schema_arguments_valid: true,
        turns: 0,
        tool_calls: 0,
        total_seconds: 0.0,
        error: None,
    };
    if let Err(error) = drive_probe(transport, model, &mut report).await {
        report.error = Some(format!("{error:#}"));
    }
    report.total_seconds = start.elapsed().as_secs_f64();
    report
}

async fn drive_probe(
    transport: &dyn ModelTransport,
    model: &ModelId,
    report: &mut ProbeReport,
) -> Result<()> {
    let specs = probe_specs();
    let mut conversation = Conversation::with_system(
        "You are verifying tool plumbing. First read src/lib.rs with read_file \
         and list the src directory with list_files (both calls may share one \
         turn), then summarize what you saw in one sentence.",
    );
    conversation.push_user("Inspect the project as instructed, then summarize.");

    let mut names_seen = std::collections::BTreeSet::new();
    let mut saw_text_after_tools = false;
    for _ in 0..5 {
        report.turns += 1;
        // The probe runs pre-session (no recorder), but shares the actor
        // turn discipline (PSP-10 system 27).
        let mut runner = crate::turn::ActorTurnRunner {
            transport,
            model: model.clone(),
            fallbacks: Vec::new(),
            recorder: None,
            actor: crate::turn::ActorKind::CapabilityProbe,
            turn: report.turns,
        };
        let output = runner
            .run_turn(&conversation, &specs, ToolChoicePolicy::Auto)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        match output {
            TurnOutput::ToolCalls(calls) => {
                if calls.len() > 1 {
                    report.parallel_tool_calls = true;
                }
                report.tool_calls += calls.len() as u32;
                conversation.push_tool_calls(calls.clone());
                for call in &calls {
                    names_seen.insert(call.name.clone());
                    let path_ok = call
                        .arguments
                        .get("path")
                        .map(serde_json::Value::is_string)
                        .unwrap_or(false);
                    if !path_ok {
                        report.schema_arguments_valid = false;
                    }
                    conversation
                        .push_tool_response(call.call_id.clone(), scripted_result(&call.name));
                }
            }
            TurnOutput::Text(_) => {
                saw_text_after_tools = report.tool_calls > 0;
                break;
            }
        }
    }
    report.tool_call_round_trip = saw_text_after_tools;
    report.multi_tool_selection =
        names_seen.contains("read_file") && names_seen.contains("list_files");
    Ok(())
}
