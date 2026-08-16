//! Deterministic Starlark composition for bounded tool proposals.
//!
//! A program cannot touch files, environment, network, time, or the kernel.
//! It returns JSON describing nested calls; the parent agent loop submits each
//! call to the ordinary five-clause admissibility path.

use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use starlark::environment::{Globals, Module};
use starlark::eval::Evaluator;
use starlark::syntax::{AstModule, Dialect};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolProgramCall {
    pub tool: String,
    #[serde(default)]
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolProgramLimits {
    pub max_source_bytes: usize,
    pub max_calls: usize,
    pub max_result_bytes: usize,
    pub max_heap_bytes: usize,
    pub max_ticks: u64,
    pub max_callstack: usize,
}

impl Default for ToolProgramLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 64 * 1024,
            max_calls: 16,
            max_result_bytes: 1024 * 1024,
            max_heap_bytes: 64 * 1024 * 1024,
            max_ticks: 100_000,
            max_callstack: 64,
        }
    }
}

/// Evaluate a pure Starlark expression that returns a JSON string.
///
/// The source's final expression must be a zero-argument function. Calling it
/// must return a JSON array of `{ "tool": ..., "arguments": {...} }` values.
/// Starlark is deliberately given only its standard pure globals and no loader.
pub fn evaluate_tool_program(
    source: &str,
    limits: ToolProgramLimits,
) -> Result<Vec<ToolProgramCall>> {
    ensure!(
        source.len() <= limits.max_source_bytes,
        "tool program exceeds source limit"
    );
    let ast = AstModule::parse("tool_program.star", source.to_owned(), &Dialect::Standard)
        .map_err(|error| anyhow::anyhow!("tool program parse error: {error}"))?;
    Module::with_temp_heap(|module| {
        let mut evaluator = Evaluator::new(&module);
        evaluator.set_max_callstack_size(limits.max_callstack)?;
        evaluator.set_max_heap_size(limits.max_heap_bytes)?;
        evaluator.set_max_tick_count(limits.max_ticks)?;
        let main = evaluator
            .eval_module(ast, &Globals::standard())
            .map_err(|error| anyhow::anyhow!("tool program evaluation error: {error}"))?;
        let result = evaluator
            .eval_function(main, &[], &[])
            .map_err(|error| anyhow::anyhow!("tool program main() failed: {error}"))?;
        let json = result
            .unpack_str()
            .context("tool program main() must return a JSON string")?;
        ensure!(
            json.len() <= limits.max_result_bytes,
            "tool program result exceeds output limit"
        );
        let calls: Vec<ToolProgramCall> =
            serde_json::from_str(json).context("tool program returned invalid call JSON")?;
        ensure!(
            calls.len() <= limits.max_calls,
            "tool program exceeds call limit"
        );
        for call in &calls {
            ensure!(
                !call.tool.is_empty(),
                "tool program contains an empty tool name"
            );
            ensure!(
                call.tool != "tool_program",
                "nested tool_program is forbidden"
            );
            ensure!(
                call.arguments.is_object(),
                "tool program arguments must be JSON objects"
            );
        }
        Ok(calls)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_program_produces_bounded_calls() {
        let calls = evaluate_tool_program(
            r#"
def main():
    return '[{"tool":"read_file","arguments":{"path":"Cargo.toml"}}]'
main
"#,
            ToolProgramLimits::default(),
        )
        .unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "read_file");
    }

    #[test]
    fn recursion_is_stopped_by_the_evaluator_budget() {
        assert!(evaluate_tool_program(
            "def main():\n    return main()\nmain",
            ToolProgramLimits::default(),
        )
        .is_err());
    }

    #[test]
    fn nested_programs_are_rejected() {
        assert!(evaluate_tool_program(
            r#"def main():
    return '[{"tool":"tool_program","arguments":{}}]'
main"#,
            ToolProgramLimits::default(),
        )
        .is_err());
    }
}
