use perspt_store::LlmRequestRecord;

/// View model for the LLM telemetry page
pub struct LlmViewModel {
    pub session_id: String,
    pub requests: Vec<LlmRow>,
    pub total_tokens_in: i64,
    pub total_tokens_out: i64,
    pub total_latency_ms: i64,
    pub request_count: usize,
}

pub struct LlmRow {
    pub model: String,
    pub node_id: String,
    pub tokens_in: i32,
    pub tokens_out: i32,
    pub latency_ms: i32,
    pub prompt_preview: String,
    pub response_preview: String,
}

impl LlmViewModel {
    pub fn from_records(session_id: String, records: Vec<LlmRequestRecord>) -> Self {
        let total_tokens_in: i64 = records.iter().map(|r| r.tokens_in as i64).sum();
        let total_tokens_out: i64 = records.iter().map(|r| r.tokens_out as i64).sum();
        let total_latency_ms: i64 = records.iter().map(|r| r.latency_ms as i64).sum();
        let request_count = records.len();

        let rows = records
            .into_iter()
            .map(|r| {
                let prompt_preview = truncate_str(&r.prompt, 120);
                let response_preview = truncate_str(&r.response, 120);
                LlmRow {
                    model: r.model,
                    node_id: r.node_id.unwrap_or_default(),
                    tokens_in: r.tokens_in,
                    tokens_out: r.tokens_out,
                    latency_ms: r.latency_ms,
                    prompt_preview,
                    response_preview,
                }
            })
            .collect();

        Self {
            session_id,
            requests: rows,
            total_tokens_in,
            total_tokens_out,
            total_latency_ms,
            request_count,
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
