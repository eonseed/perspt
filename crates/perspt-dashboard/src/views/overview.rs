use perspt_store::{BudgetEnvelopeRow, NodeStateRecord, SessionRecord};

/// View model for the overview/sessions list page
pub struct OverviewViewModel {
    pub sessions: Vec<SessionSummary>,
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
    pub budget: Option<BudgetSummary>,
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
    ) -> Self {
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
                    .filter(|n| n.state == "committed" || n.state == "verified")
                    .count();
                let failed_count = nodes.iter().filter(|n| n.state == "failed").count();

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
                    status: s.status,
                    node_count: nodes.len(),
                    completed_count,
                    failed_count,
                    budget,
                }
            })
            .collect();

        Self {
            sessions: summaries,
        }
    }
}
