//! Workspace file operations, dispatched per catalog name against the
//! candidate's overlay (or the read-only source workspace for `git_read`).

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};

use super::{CandidateHandlerRegistry, CandidateToolHandler};
use crate::candidate::CandidateWorkspace;
use crate::toolloop::EffectOutcome;
use crate::tools::{AgentTools, ToolCall};

/// Which workspace an operation reads or mutates.
#[derive(Clone, Copy)]
enum OpRoot {
    /// The reversible candidate overlay — every mutation lands here.
    Overlay,
    /// The immutable source workspace — read-only git state.
    Source,
}

/// One registered file operation: the catalog name it answers to, the
/// `AgentTools` operation it invokes, and the workspace it runs against.
struct WorkspaceOp {
    operation: fn(&AgentTools, &ToolCall) -> crate::tools::ToolResult,
    invoked_name: &'static str,
    root: OpRoot,
}

#[async_trait::async_trait]
impl CandidateToolHandler for WorkspaceOp {
    async fn apply(
        &self,
        workspace: &CandidateWorkspace,
        call: &perspt_sdk::ProviderToolCall,
        _entry: &perspt_sdk::ToolEntry,
    ) -> Result<EffectOutcome> {
        let tools = match self.root {
            OpRoot::Overlay => workspace.overlay_tools(),
            OpRoot::Source => workspace.source_tools(),
        };
        let result = (self.operation)(
            tools,
            &ToolCall {
                name: self.invoked_name.to_string(),
                arguments: json_arguments(&call.arguments)?,
            },
        );
        Ok(EffectOutcome {
            output: if result.success {
                result.output
            } else {
                format!("tool failed: {}", result.error.unwrap_or_default())
            },
            mutated: result.success,
            completed: true,
        })
    }
}

/// Render provider JSON arguments as the string map `AgentTools` consumes.
pub(crate) fn json_arguments(value: &serde_json::Value) -> Result<HashMap<String, String>> {
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

type Operation = fn(&AgentTools, &ToolCall) -> crate::tools::ToolResult;

/// Catalog name, invoked `AgentTools` operation and name, and workspace root.
const WORKSPACE_OPS: &[(&str, Operation, &str, OpRoot)] = &[
    (
        "read_file",
        AgentTools::read_file,
        "read_file",
        OpRoot::Overlay,
    ),
    (
        "list_files",
        AgentTools::list_files,
        "list_files",
        OpRoot::Overlay,
    ),
    ("glob", AgentTools::glob, "glob", OpRoot::Overlay),
    (
        "grep",
        AgentTools::search_code,
        "search_code",
        OpRoot::Overlay,
    ),
    ("git_read", AgentTools::git_read, "git_read", OpRoot::Source),
    (
        "write_file",
        AgentTools::write_file,
        "write_file",
        OpRoot::Overlay,
    ),
    (
        "edit_file",
        AgentTools::edit_file,
        "edit_file",
        OpRoot::Overlay,
    ),
    (
        "apply_diff",
        AgentTools::apply_diff,
        "apply_diff",
        OpRoot::Overlay,
    ),
    (
        "move_file",
        AgentTools::move_file,
        "move_file",
        OpRoot::Overlay,
    ),
    (
        "delete_file",
        AgentTools::delete_file,
        "delete_file",
        OpRoot::Overlay,
    ),
];

pub(super) fn register_workspace_ops(registry: &mut CandidateHandlerRegistry) {
    for (name, operation, invoked_name, root) in WORKSPACE_OPS {
        registry
            .register(
                *name,
                Arc::new(WorkspaceOp {
                    operation: *operation,
                    invoked_name,
                    root: *root,
                }),
            )
            .expect("builtin workspace ops are registered once");
    }
}
