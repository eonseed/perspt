//! Conjunctive adjudication: a tool-free validator reviews only the
//! realized diff and records an uncalibrated verdict (system 8).

use super::*;

impl Psp9AgentRuntime {
    pub(super) async fn adjudicate_candidate(
        &self,
        recorder: &Psp9Recorder,
        candidate: &CandidateWorkspace,
        task: &str,
        stratum: &str,
    ) -> Result<bool> {
        let Some(model) = &self.adjudicator_model else {
            return Ok(true);
        };
        let diff = candidate.realized_diff()?;
        let diff_handle = recorder.record_artifact(diff.as_bytes(), "text/x-diff")?;
        let mut boundary = diff.len().min(100_000);
        while !diff.is_char_boundary(boundary) {
            boundary -= 1;
        }
        let mut conversation = Conversation::with_system(
            "You are a conjunctive coding validator with no tools or authority. \
             Review only the realized diff. Return strict JSON: \
             {\"pass\":bool,\"reason\":string}. Reject uncertainty; do not \
             propose edits.",
        );
        conversation.push_user(format!(
            "Task: {task}\nDiff artifact: {diff_handle}\nRealized diff:\n{}",
            &diff[..boundary]
        ));
        recorder.record_custom(
            "adjudication_requested",
            serde_json::json!({"model": model, "diff_artifact": diff_handle}),
        )?;
        let output = self
            .transport
            .chat_turn(model, &conversation, &[], ToolChoicePolicy::None)
            .await?;
        let TurnOutput::Text(text) = output else {
            anyhow::bail!("adjudicator returned tool calls despite having no tools");
        };
        #[derive(serde::Deserialize)]
        struct Verdict {
            pass: bool,
            reason: String,
        }
        let verdict: Verdict = serde_json::from_str(text.trim())
            .context("adjudicator did not return strict verdict JSON")?;
        let evidence_hash = recorder.record_artifact(text.as_bytes(), "application/json")?;
        let candidate_id = candidate.checkpoint(&[]).await?.witness.state_root;
        // The verdict shares the epoch's serialized stratum so verdicts and
        // calibration samples can be joined during delayed-label ingestion.
        recorder.store.record_psp9_verdict(&Psp9VerdictRow {
            session_id: recorder.session_id.clone(),
            candidate_id,
            validator_id: model.to_string(),
            stratum: stratum.to_string(),
            missed: !verdict.pass,
            unsafe_label: None,
            evidence_hash,
        })?;
        recorder.record_custom(
            "adjudication_verdict",
            serde_json::json!({
                "model": model,
                "pass": verdict.pass,
                "reason": verdict.reason,
                // Certified only when every pair met the matched-label
                // floor; otherwise absent, never a fabricated number.
                "certified_risk": certified_pairwise_risk(&recorder.store),
            }),
        )?;
        Ok(verdict.pass)
    }
}
