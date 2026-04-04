use perspt_store::LlmRequestRecord;

/// Rough token estimate: ~4 characters per token for English text.
fn estimate_tokens(text: &str) -> i32 {
    (text.len() as i32 + 3) / 4
}

/// View model for the LLM telemetry page
pub struct LlmViewModel {
    pub session_id: String,
    pub requests: Vec<LlmRow>,
    pub total_tokens_in: i64,
    pub total_tokens_out: i64,
    pub total_latency_secs: f64,
    pub request_count: usize,
    pub avg_latency_secs: f64,
    pub models_used: Vec<String>,
    pub tokens_estimated: bool,
}

pub struct LlmRow {
    pub index: usize,
    pub model: String,
    pub node_id: String,
    pub tokens_in: i32,
    pub tokens_out: i32,
    pub latency_secs: f64,
    pub prompt_preview: String,
    pub response_preview: String,
    pub prompt_full: String,
    pub response_full: String,
}

impl LlmViewModel {
    pub fn from_records(session_id: String, records: Vec<LlmRequestRecord>) -> Self {
        let raw_tokens_in: i64 = records.iter().map(|r| r.tokens_in as i64).sum();
        let raw_tokens_out: i64 = records.iter().map(|r| r.tokens_out as i64).sum();
        let total_latency_ms: i64 = records.iter().map(|r| r.latency_ms as i64).sum();
        let request_count = records.len();

        // If the provider didn't record token counts, estimate from content length
        let tokens_estimated = raw_tokens_in == 0 && raw_tokens_out == 0 && !records.is_empty();

        // Collect unique models
        let mut models: Vec<String> = records.iter().map(|r| r.model.clone()).collect();
        models.sort();
        models.dedup();

        let rows: Vec<LlmRow> = records
            .into_iter()
            .enumerate()
            .map(|(i, r)| {
                let tok_in = if r.tokens_in > 0 {
                    r.tokens_in
                } else {
                    estimate_tokens(&r.prompt)
                };
                let tok_out = if r.tokens_out > 0 {
                    r.tokens_out
                } else {
                    estimate_tokens(&r.response)
                };
                let prompt_preview = truncate_str(&r.prompt, 150);
                let response_preview = truncate_str(&r.response, 150);
                LlmRow {
                    index: i,
                    model: r.model,
                    node_id: r.node_id.unwrap_or_default(),
                    tokens_in: tok_in,
                    tokens_out: tok_out,
                    latency_secs: r.latency_ms as f64 / 1000.0,
                    prompt_preview,
                    response_preview,
                    prompt_full: r.prompt,
                    response_full: r.response,
                }
            })
            .collect();

        let total_tokens_in: i64 = rows.iter().map(|r| r.tokens_in as i64).sum();
        let total_tokens_out: i64 = rows.iter().map(|r| r.tokens_out as i64).sum();
        let total_latency_secs = total_latency_ms as f64 / 1000.0;
        let avg_latency_secs = if request_count > 0 {
            total_latency_secs / request_count as f64
        } else {
            0.0
        };

        Self {
            session_id,
            requests: rows,
            total_tokens_in,
            total_tokens_out,
            total_latency_secs,
            request_count,
            avg_latency_secs,
            models_used: models,
            tokens_estimated,
        }
    }
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}
