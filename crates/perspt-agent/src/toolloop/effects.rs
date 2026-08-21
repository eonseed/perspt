//! Execution helpers shared by sequential and concurrent effect paths.

use super::*;

impl ToolLoop<'_> {
    /// Execute an already-certified non-mutating call and close its durable
    /// bracket only when the executor observed definitive completion.
    pub(super) async fn execute_non_mutating(
        &mut self,
        call: &ProviderToolCall,
        entry: &ToolEntry,
        recall: Option<&resident::RecallIndex>,
        log: &mut EventLog,
    ) -> Result<String> {
        let bracket_key = self.open_effect_bracket(call, entry)?;
        let outcome = self.apply_non_mutating(call, entry, recall).await?;
        if outcome.completed {
            if let (Some(recorder), Some(key)) = (self.recorder, bracket_key.as_deref()) {
                recorder.external_result(key, &serde_json::json!({"mutated": outcome.mutated}))?;
            }
        }
        let output = bounded_model_output(self.recorder, outcome.output)?;
        emit(
            self.recorder,
            log,
            LoopEvent::EffectApplied {
                call_id: call.call_id.clone(),
                mutated: false,
                output: output.clone(),
            },
        )?;
        Ok(output)
    }

    /// Host-side read surfaces bypass the candidate executor; registered
    /// reads, including MCP tools, use the ordinary executor port.
    async fn apply_non_mutating(
        &self,
        call: &ProviderToolCall,
        entry: &ToolEntry,
        recall: Option<&resident::RecallIndex>,
    ) -> Result<EffectOutcome> {
        let complete = |output| EffectOutcome {
            output,
            mutated: false,
            completed: true,
        };
        if call.name == "context_recall" {
            return Ok(complete(match recall {
                Some(index) => index.lookup(&call.arguments),
                None => "miss: no context pages exist yet in this session".to_string(),
            }));
        }
        if call.name == "read_artifact" {
            return Ok(complete(artifact::read_artifact_window(
                self.recorder,
                &call.arguments,
            )?));
        }
        if call.name == "tool_search" {
            let query = call
                .arguments
                .get("query")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let limit = call
                .arguments
                .get("limit")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or(8);
            let matches = self
                .catalog
                .search_specs(&self.capabilities, query, limit, false);
            return Ok(complete(serde_json::to_string(&matches)?));
        }
        if call.name == "tool_program" {
            let source = call
                .arguments
                .get("source")
                .and_then(serde_json::Value::as_str)
                .context("tool_program requires string source")?;
            let calls = perspt_policy::evaluate_tool_program(
                source,
                perspt_policy::ToolProgramLimits::default(),
            )?;
            return Ok(complete(serde_json::to_string(&calls)?));
        }
        self.executor.apply(call, entry).await
    }

    /// R5 bracketing: a durable effect's intent is ledgered before it runs,
    /// so an interruption leaves a visible open bracket.
    pub(super) fn open_effect_bracket(
        &self,
        call: &ProviderToolCall,
        entry: &ToolEntry,
    ) -> Result<Option<String>> {
        let Some(key) = entry
            .durable
            .then(|| format!("tool:{}:{}", call.name, call.call_id))
        else {
            return Ok(None);
        };
        if let Some(recorder) = self.recorder {
            recorder.external_intent(
                &key,
                &serde_json::json!({
                    "tool": call.name,
                    "call_id": call.call_id,
                    "arguments": call.arguments,
                    "node_id": self.node_id,
                    "generation": self.generation,
                }),
            )?;
        }
        Ok(Some(key))
    }
}
