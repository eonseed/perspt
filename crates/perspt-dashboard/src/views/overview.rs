//! Overview view model: session list with node counts projected from each
//! session's latest PSP-9 graph revision.

use super::psp9::LedgerProjection;
use super::{friendly_name, normalize_status};
use perspt_sdk::workgraph::WorkNodeState;
use perspt_store::SessionRecord;

/// View model for the overview/sessions list page.
pub struct OverviewViewModel {
    pub sessions: Vec<SessionSummary>,
    pub global_stats: GlobalStats,
}

/// Aggregate stats across the listed sessions.
pub struct GlobalStats {
    pub total_sessions: usize,
    pub running_sessions: usize,
    pub completed_sessions: usize,
    pub failed_sessions: usize,
    pub total_nodes: usize,
    pub total_stable_nodes: usize,
    pub total_events: usize,
}

/// Summary of a single session for the overview list.
pub struct SessionSummary {
    pub session_id: String,
    pub display_name: String,
    pub task: String,
    pub working_dir: String,
    pub status: String,
    pub node_count: usize,
    pub stable_count: usize,
    pub running_count: usize,
    pub event_count: usize,
    pub toolchain: String,
}

impl OverviewViewModel {
    pub fn from_store(
        sessions: Vec<SessionRecord>,
        projections: &[(String, LedgerProjection)],
    ) -> Self {
        let total_sessions = sessions.len();
        let status_count = |wanted: &str| {
            sessions
                .iter()
                .filter(|s| normalize_status(&s.status) == wanted)
                .count()
        };
        let running_sessions = status_count("running");
        let completed_sessions = status_count("completed");
        let failed_sessions = status_count("failed");

        let mut total_nodes = 0usize;
        let mut total_stable_nodes = 0usize;
        let mut total_events = 0usize;

        let summaries = sessions
            .into_iter()
            .map(|s| {
                let projection = projections
                    .iter()
                    .find(|(id, _)| id == &s.session_id)
                    .map(|(_, p)| p);
                let summary = session_summary(s, projection);
                total_nodes += summary.node_count;
                total_stable_nodes += summary.stable_count;
                total_events += summary.event_count;
                summary
            })
            .collect();

        Self {
            sessions: summaries,
            global_stats: GlobalStats {
                total_sessions,
                running_sessions,
                completed_sessions,
                failed_sessions,
                total_nodes,
                total_stable_nodes,
                total_events,
            },
        }
    }
}

fn session_summary(s: SessionRecord, projection: Option<&LedgerProjection>) -> SessionSummary {
    let nodes: &[perspt_sdk::WorkNode] = projection
        .and_then(|p| p.latest_revision())
        .map(|r| r.nodes.as_slice())
        .unwrap_or(&[]);
    let stable_count = nodes
        .iter()
        .filter(|n| matches!(n.state, WorkNodeState::Stable))
        .count();
    let running_count = nodes
        .iter()
        .filter(|n| matches!(n.state, WorkNodeState::Running))
        .count();
    SessionSummary {
        display_name: friendly_name(&s.session_id),
        session_id: s.session_id,
        task: s.task,
        working_dir: s.working_dir,
        status: normalize_status(&s.status),
        toolchain: s.detected_toolchain.unwrap_or_default(),
        node_count: nodes.len(),
        stable_count,
        running_count,
        event_count: projection.map(|p| p.event_count).unwrap_or(0),
    }
}
