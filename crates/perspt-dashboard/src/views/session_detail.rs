use super::normalize_state;
use crate::views::overview::BudgetSummary;
use perspt_store::{
    BudgetEnvelopeRow, EnergyRecord, LlmRequestRecord, NodeStateRecord, VerificationResultRow,
};

/// View model for the session detail summary page
pub struct SessionDetailViewModel {
    pub session_id: String,
    pub task: String,
    pub working_dir: String,
    pub status: String,
    pub toolchain: String,
    pub total_nodes: usize,
    pub completed_nodes: usize,
    pub failed_nodes: usize,
    pub running_nodes: usize,
    pub llm_request_count: usize,
    pub llm_tokens_in: i64,
    pub llm_tokens_out: i64,
    pub avg_energy: f32,
    pub budget: Option<BudgetSummary>,
    pub nodes: Vec<NodeSummaryRow>,
    pub verifications: Vec<VerifSummaryRow>,
}

pub struct NodeSummaryRow {
    pub node_id: String,
    pub state: String,
    pub node_class: String,
    pub goal: String,
    pub v_total: f32,
    pub attempt_count: i32,
}

pub struct VerifSummaryRow {
    pub node_id: String,
    pub syntax_ok: bool,
    pub build_ok: bool,
    pub tests_ok: bool,
    pub lint_ok: bool,
    pub tests_passed: i32,
    pub tests_failed: i32,
}

impl SessionDetailViewModel {
    #[allow(clippy::too_many_arguments)]
    pub fn from_store(
        session_id: String,
        task: String,
        working_dir: String,
        status: String,
        toolchain: Option<String>,
        nodes: Vec<NodeStateRecord>,
        llm_records: &[LlmRequestRecord],
        energy_records: &[EnergyRecord],
        budget: Option<BudgetEnvelopeRow>,
        verifications: Vec<VerificationResultRow>,
    ) -> Self {
        let total_nodes = nodes.len();
        let completed_nodes = nodes
            .iter()
            .filter(|n| normalize_state(&n.state) == "completed")
            .count();
        let failed_nodes = nodes
            .iter()
            .filter(|n| normalize_state(&n.state) == "failed")
            .count();
        let running_nodes = nodes
            .iter()
            .filter(|n| normalize_state(&n.state) == "running")
            .count();

        let llm_request_count = llm_records.len();
        let llm_tokens_in: i64 = llm_records.iter().map(|r| r.tokens_in as i64).sum();
        let llm_tokens_out: i64 = llm_records.iter().map(|r| r.tokens_out as i64).sum();

        let avg_energy = if energy_records.is_empty() {
            0.0
        } else {
            energy_records.iter().map(|r| r.v_total).sum::<f32>() / energy_records.len() as f32
        };

        let budget_summary = budget.map(|b| BudgetSummary {
            steps_used: b.steps_used,
            max_steps: b.max_steps,
            cost_used_usd: b.cost_used_usd,
            max_cost_usd: b.max_cost_usd,
        });

        let node_rows = nodes
            .into_iter()
            .map(|n| NodeSummaryRow {
                node_id: n.node_id,
                state: normalize_state(&n.state),
                node_class: n.node_class.unwrap_or_default(),
                goal: n.goal.unwrap_or_default(),
                v_total: n.v_total,
                attempt_count: n.attempt_count,
            })
            .collect();

        let verif_rows = verifications
            .into_iter()
            .map(|v| VerifSummaryRow {
                node_id: v.node_id,
                syntax_ok: v.syntax_ok,
                build_ok: v.build_ok,
                tests_ok: v.tests_ok,
                lint_ok: v.lint_ok,
                tests_passed: v.tests_passed,
                tests_failed: v.tests_failed,
            })
            .collect();

        Self {
            session_id,
            task,
            working_dir,
            status: normalize_state(&status),
            toolchain: toolchain.unwrap_or_default(),
            total_nodes,
            completed_nodes,
            failed_nodes,
            running_nodes,
            llm_request_count,
            llm_tokens_in,
            llm_tokens_out,
            avg_energy,
            budget: budget_summary,
            nodes: node_rows,
            verifications: verif_rows,
        }
    }
}
