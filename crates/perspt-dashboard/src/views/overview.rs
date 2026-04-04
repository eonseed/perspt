use super::normalize_state;
use perspt_store::{BudgetEnvelopeRow, NodeStateRecord, SessionRecord};

/// View model for the overview/sessions list page
pub struct OverviewViewModel {
    pub sessions: Vec<SessionSummary>,
    pub global_stats: GlobalStats,
}

/// Aggregate stats across all sessions
pub struct GlobalStats {
    pub total_sessions: usize,
    pub running_sessions: usize,
    pub completed_sessions: usize,
    pub failed_sessions: usize,
    pub total_llm_requests: i64,
    pub tokens_in_display: String,
    pub tokens_out_display: String,
    pub median_latency_display: String,
    pub total_nodes: usize,
    pub total_completed_nodes: usize,
    pub total_failed_nodes: usize,
}

/// Format a token count into a human-readable string.
/// e.g. 1_662_345 -> "1.7M", 45_200 -> "45.2K", 830 -> "830"
fn format_tokens(count: i64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}

/// Format milliseconds into a human-readable minutes + seconds string.
/// e.g. 135_000 -> "2m 15s", 45_000 -> "45s", 0 -> "0s"
fn format_duration_ms(ms: i64) -> String {
    let total_secs = ms / 1000;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    if mins > 0 {
        format!("{}m {}s", mins, secs)
    } else {
        format!("{}s", secs)
    }
}

/// Summary of a single session for the overview list
pub struct SessionSummary {
    pub session_id: String,
    pub task: String,
    pub working_dir: String,
    pub status: String,
    pub node_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub running_count: usize,
    pub budget: Option<BudgetSummary>,
    pub toolchain: String,
}

pub struct BudgetSummary {
    pub steps_used: i32,
    pub max_steps: Option<i32>,
    pub cost_used_usd: f64,
    pub max_cost_usd: Option<f64>,
}

impl OverviewViewModel {
    pub fn from_store(
        sessions: Vec<SessionRecord>,
        nodes_by_session: &[(String, Vec<NodeStateRecord>)],
        budgets: &[(String, Option<BudgetEnvelopeRow>)],
        llm_summary: (i64, i64, i64, i64),
    ) -> Self {
        let total_sessions = sessions.len();
        let running_sessions = sessions
            .iter()
            .filter(|s| normalize_state(&s.status) == "running")
            .count();
        let completed_sessions = sessions
            .iter()
            .filter(|s| normalize_state(&s.status) == "completed")
            .count();
        let failed_sessions = sessions
            .iter()
            .filter(|s| normalize_state(&s.status) == "failed")
            .count();

        let mut total_nodes = 0usize;
        let mut total_completed_nodes = 0usize;
        let mut total_failed_nodes = 0usize;

        let summaries = sessions
            .into_iter()
            .map(|s| {
                let nodes = nodes_by_session
                    .iter()
                    .find(|(id, _)| id == &s.session_id)
                    .map(|(_, n)| n.as_slice())
                    .unwrap_or(&[]);

                let completed_count = nodes
                    .iter()
                    .filter(|n| normalize_state(&n.state) == "completed")
                    .count();
                let failed_count = nodes
                    .iter()
                    .filter(|n| normalize_state(&n.state) == "failed")
                    .count();
                let running_count = nodes
                    .iter()
                    .filter(|n| normalize_state(&n.state) == "running")
                    .count();

                total_nodes += nodes.len();
                total_completed_nodes += completed_count;
                total_failed_nodes += failed_count;

                let budget = budgets
                    .iter()
                    .find(|(id, _)| id == &s.session_id)
                    .and_then(|(_, b)| b.as_ref())
                    .map(|b| BudgetSummary {
                        steps_used: b.steps_used,
                        max_steps: b.max_steps,
                        cost_used_usd: b.cost_used_usd,
                        max_cost_usd: b.max_cost_usd,
                    });

                SessionSummary {
                    session_id: s.session_id,
                    task: s.task,
                    working_dir: s.working_dir,
                    status: normalize_state(&s.status),
                    toolchain: s.detected_toolchain.unwrap_or_default(),
                    node_count: nodes.len(),
                    completed_count,
                    failed_count,
                    running_count,
                    budget,
                }
            })
            .collect();

        Self {
            sessions: summaries,
            global_stats: GlobalStats {
                total_sessions,
                running_sessions,
                completed_sessions,
                failed_sessions,
                total_llm_requests: llm_summary.0,
                tokens_in_display: format_tokens(llm_summary.1),
                tokens_out_display: format_tokens(llm_summary.2),
                median_latency_display: format_duration_ms(llm_summary.3),
                total_nodes,
                total_completed_nodes,
                total_failed_nodes,
            },
        }
    }
}
