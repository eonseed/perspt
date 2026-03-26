//! Agent App - Main TUI Application
//!
//! Coordinates all TUI components for the Agent mode with full keyboard navigation.
//! Now with async event-driven architecture support.

use crate::app_event::{AgentStateUpdate, AppEvent};
use crate::dashboard::Dashboard;
use crate::diff_viewer::DiffViewer;
use crate::review_modal::{ReviewDecision, ReviewModal};
use crate::task_tree::{TaskStatus, TaskTree};
use crossterm::event::{KeyCode, KeyEventKind};
use perspt_core::AgentEvent;
use ratatui::{
    crossterm::event::{self, Event},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Tabs},
    DefaultTerminal, Frame,
};
use std::io;

/// Active tab
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Dashboard,
    Tasks,
    Diff,
}

impl ActiveTab {
    fn index(&self) -> usize {
        match self {
            ActiveTab::Dashboard => 0,
            ActiveTab::Tasks => 1,
            ActiveTab::Diff => 2,
        }
    }

    #[allow(dead_code)]
    fn from_index(i: usize) -> Self {
        match i {
            0 => ActiveTab::Dashboard,
            1 => ActiveTab::Tasks,
            _ => ActiveTab::Diff,
        }
    }
}

/// PSP-5 Phase 7: Aggregated review state for the active approval boundary.
///
/// Populated incrementally as VerificationComplete, BundleApplied, and
/// ApprovalRequest events arrive. Consumed by the review modal and diff viewer.
#[derive(Debug, Clone, Default)]
pub struct NodeReviewState {
    /// Node currently under review
    pub node_id: Option<String>,
    /// Node class (Interface, Implementation, Integration)
    pub node_class: Option<String>,
    /// Files created by the bundle
    pub files_created: Vec<String>,
    /// Files modified by the bundle
    pub files_modified: Vec<String>,
    /// Write operation count
    pub writes_count: usize,
    /// Diff operation count
    pub diffs_count: usize,
    /// Latest verification result fields
    pub syntax_ok: Option<bool>,
    pub build_ok: Option<bool>,
    pub tests_ok: Option<bool>,
    pub lint_ok: Option<bool>,
    pub diagnostics_count: Option<usize>,
    pub tests_passed: Option<usize>,
    pub tests_failed: Option<usize>,
    pub energy: Option<f32>,
    /// Full energy component breakdown
    pub energy_components: Option<perspt_core::EnergyComponents>,
    /// Per-stage outcomes with sensor status
    pub stage_outcomes: Vec<perspt_core::StageOutcome>,
    /// Whether verification ran degraded
    pub degraded: bool,
    pub degraded_reasons: Vec<String>,
    /// Verification summary line
    pub summary: Option<String>,
    /// Diff text for the viewer
    pub diff: Option<String>,
    /// Approval request description
    pub description: Option<String>,
}

/// Agent app state
pub struct AgentApp {
    /// Dashboard component
    pub dashboard: Dashboard,
    /// Task tree component
    pub task_tree: TaskTree,
    /// Diff viewer component
    pub diff_viewer: DiffViewer,
    /// Review modal component
    pub review_modal: ReviewModal,
    /// Sender for action feedback to orchestrator
    pub action_sender: Option<perspt_core::events::channel::ActionSender>,
    /// Active tab
    pub active_tab: ActiveTab,
    /// Pending approval request ID
    pub pending_request_id: Option<String>,
    /// PSP-5 Phase 7: Aggregated review state for the active approval
    pub review_state: NodeReviewState,
    /// Should quit
    pub should_quit: bool,
    /// Is paused
    pub paused: bool,
}

impl Default for AgentApp {
    fn default() -> Self {
        Self {
            active_tab: ActiveTab::Dashboard,
            dashboard: Dashboard::new(),
            task_tree: TaskTree::new(),
            diff_viewer: DiffViewer::new(),
            review_modal: ReviewModal::new(),
            action_sender: None,
            pending_request_id: None,
            review_state: NodeReviewState::default(),
            should_quit: false,
            paused: false,
        }
    }
}

impl AgentApp {
    /// Create a new agent app
    pub fn new() -> Self {
        Self::default()
    }

    /// Set action sender
    pub fn set_action_sender(&mut self, sender: perspt_core::events::channel::ActionSender) {
        self.action_sender = Some(sender);
    }

    /// PSP-5 Phase 8: Prepopulate task tree from persisted node states.
    ///
    /// Called before resuming so the TUI shows completed nodes immediately
    /// instead of waiting for orchestrator events (which skip terminal nodes).
    pub fn prepopulate_from_store(&mut self, session_id: &str) {
        let Ok(store) = perspt_store::SessionStore::new() else {
            return;
        };

        let nodes = store.get_latest_node_states(session_id).unwrap_or_default();

        for ns in &nodes {
            let status = match ns.state.as_str() {
                "Completed" | "COMPLETED" | "STABLE" => TaskStatus::Completed,
                "Failed" | "FAILED" => TaskStatus::Failed,
                "Escalated" | "ESCALATED" => TaskStatus::Escalated,
                "Coding" => TaskStatus::Coding,
                "Verifying" => TaskStatus::Verifying,
                "Committing" => TaskStatus::Committing,
                _ => TaskStatus::Pending,
            };

            // Add the node to the task tree if not already present
            let goal = ns.goal.clone().unwrap_or_else(|| ns.node_id.clone());
            self.task_tree
                .add_or_update_node(&ns.node_id, &goal, status);
        }

        self.dashboard.log(format!(
            "📦 Restored {} nodes from session {}",
            nodes.len(),
            &session_id[..8.min(session_id.len())]
        ));
    }

    /// Run the app main loop
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| self.render(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    /// Handle input events
    fn handle_events(&mut self) -> io::Result<()> {
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    return Ok(());
                }

                // Handle modal first if visible
                if self.review_modal.visible {
                    match key.code {
                        KeyCode::Left => self.review_modal.select_left(),
                        KeyCode::Right => self.review_modal.select_right(),
                        KeyCode::Char(c) => {
                            if let Some(decision) = self.review_modal.handle_key(c) {
                                self.handle_review_decision(decision);
                                self.review_modal.hide();
                            }
                        }
                        KeyCode::Enter => {
                            let decision = self.review_modal.get_decision();
                            self.handle_review_decision(decision);
                            self.review_modal.hide();
                        }
                        KeyCode::Esc => self.review_modal.hide(),
                        _ => {}
                    }
                    return Ok(());
                }

                match key.code {
                    // Quit
                    KeyCode::Char('q') => self.should_quit = true,
                    // Pause/Resume
                    KeyCode::Char('p') => self.paused = !self.paused,
                    // Tab navigation
                    KeyCode::Tab => self.next_tab(),
                    KeyCode::BackTab => self.prev_tab(),
                    KeyCode::Char('1') => self.active_tab = ActiveTab::Dashboard,
                    KeyCode::Char('2') => self.active_tab = ActiveTab::Tasks,
                    KeyCode::Char('3') => self.active_tab = ActiveTab::Diff,
                    // Vertical navigation (vim-style)
                    KeyCode::Up | KeyCode::Char('k') => self.handle_up(),
                    KeyCode::Down | KeyCode::Char('j') => self.handle_down(),
                    // Page navigation
                    KeyCode::PageUp => self.handle_page_up(),
                    KeyCode::PageDown => self.handle_page_down(),
                    // Task tree specific
                    KeyCode::Char(' ') | KeyCode::Enter => self.handle_select(),
                    // Approve current
                    KeyCode::Char('a') => self.show_approval_modal(),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Handle logical app events
    pub fn handle_app_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::CoreEvent(core_event) => self.handle_core_event(core_event),
            AppEvent::AgentUpdate(update) => self.handle_agent_update(update),
            _ => {}
        }
    }

    /// Handle events from the SRBN Orchestrator
    fn handle_core_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::PlanGenerated(plan) => {
                self.dashboard
                    .log(format!("Plan generated with {} tasks", plan.tasks.len()));
                self.task_tree.populate_from_plan(plan.clone());
            }
            AgentEvent::TaskStatusChanged { node_id, status } => {
                self.task_tree.update_status(&node_id, status.into());
                // Update verifier stage indicator
                let status_label: crate::task_tree::TaskStatus = status.into();
                if matches!(
                    status_label,
                    crate::task_tree::TaskStatus::Verifying
                        | crate::task_tree::TaskStatus::SheafCheck
                        | crate::task_tree::TaskStatus::Coding
                        | crate::task_tree::TaskStatus::Committing
                ) {
                    self.dashboard.verifier_stage = Some(format!("{:?}", status_label));
                }
                self.dashboard
                    .log(format!("🔄 Task {} -> {:?}", node_id, status));
            }
            AgentEvent::Log(message) => {
                self.dashboard.log(message);
            }
            AgentEvent::NodeCompleted { node_id, goal } => {
                self.task_tree
                    .update_status(&node_id, TaskStatus::Completed);
                self.dashboard.log(format!("✓ {} - {}", node_id, goal));
            }
            AgentEvent::ApprovalRequest {
                request_id,
                node_id,
                action_type,
                description,
                diff,
            } => {
                self.pending_request_id = Some(request_id);
                // Populate review state node context
                self.review_state.description = Some(description.clone());
                self.review_state.diff = diff.clone();
                if self.review_state.node_id.is_none() {
                    self.review_state.node_id = Some(node_id.clone());
                }
                // Collect affected files from action type
                let files = match &action_type {
                    perspt_core::ActionType::FileWrite { path } => vec![path.clone()],
                    perspt_core::ActionType::BundleWrite { files, .. } => files.clone(),
                    _ => self
                        .review_state
                        .files_created
                        .iter()
                        .chain(self.review_state.files_modified.iter())
                        .cloned()
                        .collect(),
                };

                // PSP-5 Phase 7: Populate diff viewer bundle summary
                self.diff_viewer.bundle_summary = Some(crate::diff_viewer::BundleSummary {
                    node_id: node_id.clone(),
                    node_class: self.review_state.node_class.clone().unwrap_or_default(),
                    files_created: self.review_state.files_created.len(),
                    files_modified: self.review_state.files_modified.len(),
                    writes_count: self.review_state.writes_count,
                    diffs_count: self.review_state.diffs_count,
                });
                if let Some(ref diff_text) = diff {
                    self.diff_viewer.parse_diff(diff_text);
                    // Tag hunks with operation labels from review state
                    for hunk in &mut self.diff_viewer.hunks {
                        if self.review_state.files_created.contains(&hunk.file_path) {
                            hunk.operation = Some("created".to_string());
                        } else if self.review_state.files_modified.contains(&hunk.file_path) {
                            hunk.operation = Some("modified".to_string());
                        }
                    }
                }

                // PSP-5 Phase 7: Build stability metrics with verification context
                use crate::review_modal::StabilityMetrics;
                let stability = if self.review_state.energy.is_some()
                    || self.review_state.syntax_ok.is_some()
                {
                    let energy = self.review_state.energy.unwrap_or(0.0);
                    Some(StabilityMetrics {
                        energy: crate::telemetry::EnergyComponents {
                            v_syn: self
                                .review_state
                                .energy_components
                                .as_ref()
                                .map(|e| e.v_syn)
                                .unwrap_or(0.0),
                            v_str: self
                                .review_state
                                .energy_components
                                .as_ref()
                                .map(|e| e.v_str)
                                .unwrap_or(0.0),
                            v_log: self
                                .review_state
                                .energy_components
                                .as_ref()
                                .map(|e| e.v_log)
                                .unwrap_or(0.0),
                            v_boot: self
                                .review_state
                                .energy_components
                                .as_ref()
                                .map(|e| e.v_boot)
                                .unwrap_or(0.0),
                            v_sheaf: self
                                .review_state
                                .energy_components
                                .as_ref()
                                .map(|e| e.v_sheaf)
                                .unwrap_or(0.0),
                            total: energy,
                        },
                        is_stable: energy < 0.1,
                        threshold: 0.1,
                        attempts: 0,
                        max_attempts: 0,
                        syntax_ok: self.review_state.syntax_ok,
                        build_ok: self.review_state.build_ok,
                        tests_ok: self.review_state.tests_ok,
                        lint_ok: self.review_state.lint_ok,
                        tests_passed: self.review_state.tests_passed,
                        tests_failed: self.review_state.tests_failed,
                        degraded: self.review_state.degraded,
                        degraded_reasons: self.review_state.degraded_reasons.clone(),
                        node_class: self.review_state.node_class.clone(),
                    })
                } else {
                    None
                };

                if let Some(stability) = stability {
                    self.review_modal.show_with_stability(
                        format!("Approval: {}", node_id),
                        description,
                        files,
                        stability,
                    );
                } else {
                    self.review_modal
                        .show(format!("Approval: {}", node_id), description, files);
                }
            }
            AgentEvent::Complete { success, message } => {
                let emoji = if success { "🎉" } else { "❌" };
                self.dashboard
                    .log(format!("{} Session Complete: {}", emoji, message));
            }
            AgentEvent::EscalationClassified {
                node_id,
                category,
                action,
            } => {
                self.dashboard.escalation_count += 1;
                self.dashboard.log(format!(
                    "⚠️ Escalation: {} → {} (action: {})",
                    node_id, category, action
                ));
            }
            AgentEvent::SheafValidationComplete {
                node_id,
                validators_run,
                failures,
                v_sheaf,
            } => {
                if failures > 0 {
                    self.dashboard.log(format!(
                        "🔍 Sheaf: {} — {}/{} failed (V_sheaf={:.3})",
                        node_id, failures, validators_run, v_sheaf
                    ));
                } else {
                    self.dashboard.log(format!(
                        "✓ Sheaf: {} — {}/{} passed",
                        node_id, validators_run, validators_run
                    ));
                }
            }
            AgentEvent::GraphRewriteApplied {
                trigger_node,
                action,
                nodes_affected,
            } => {
                self.dashboard.log(format!(
                    "🔧 Rewrite: {} via {} ({} nodes)",
                    trigger_node, action, nodes_affected
                ));
            }
            // PSP-5 Phase 6: Provisional branch lifecycle events
            AgentEvent::BranchCreated {
                branch_id,
                node_id,
                parent_node_id,
            } => {
                self.dashboard.active_branches += 1;
                self.dashboard.log(format!(
                    "🌿 Branch: {} for {} (parent: {})",
                    &branch_id[..branch_id.len().min(16)],
                    node_id,
                    parent_node_id
                ));
            }
            AgentEvent::InterfaceSealed {
                node_id,
                sealed_paths,
                seal_hash,
            } => {
                self.dashboard.log(format!(
                    "🔒 Sealed: {} ({} artifact{}) [{}]",
                    node_id,
                    sealed_paths.len(),
                    if sealed_paths.len() == 1 { "" } else { "s" },
                    &seal_hash[..seal_hash.len().min(12)]
                ));
            }
            AgentEvent::BranchFlushed {
                parent_node_id,
                flushed_branch_ids,
                reason,
            } => {
                self.dashboard.active_branches = self
                    .dashboard
                    .active_branches
                    .saturating_sub(flushed_branch_ids.len());
                self.dashboard.log(format!(
                    "🗑️  Flushed: {} branch(es) from {} — {}",
                    flushed_branch_ids.len(),
                    parent_node_id,
                    reason
                ));
            }
            AgentEvent::DependentUnblocked {
                child_node_id,
                parent_node_id,
            } => {
                self.dashboard.log(format!(
                    "🔓 Unblocked: {} (parent {} sealed)",
                    child_node_id, parent_node_id
                ));
            }
            AgentEvent::BranchMerged { branch_id, node_id } => {
                self.dashboard.active_branches = self.dashboard.active_branches.saturating_sub(1);
                self.dashboard.log(format!(
                    "✅ Merged: branch {} for {}",
                    &branch_id[..branch_id.len().min(16)],
                    node_id
                ));
            }
            AgentEvent::ContextDegraded {
                node_id,
                budget_exceeded,
                missing_owned_files,
                included_file_count,
                total_bytes: _,
                reason,
            } => {
                let detail = if budget_exceeded {
                    format!("{} files included (budget exceeded)", included_file_count)
                } else {
                    format!("{} owned file(s) missing", missing_owned_files.len())
                };
                self.dashboard.log(format!(
                    "⚠️ Context degraded: {} — {} ({})",
                    node_id, reason, detail
                ));
            }
            AgentEvent::ProvenanceDrift {
                node_id,
                missing_files,
                reason: _,
            } => {
                self.dashboard.log(format!(
                    "⚠️ Provenance drift: {} — {} file(s) missing since last run",
                    node_id,
                    missing_files.len()
                ));
            }
            AgentEvent::ToolReadiness {
                plugins,
                strictness,
            } => {
                self.dashboard
                    .log(format!("🔧 Verifier strictness: {}", strictness));
                for pr in &plugins {
                    if pr.degraded_stages.is_empty() {
                        self.dashboard
                            .log(format!("🔌 {} — all stages available", pr.plugin_name));
                    } else {
                        self.dashboard.log(format!(
                            "🔌 {} — degraded: {}",
                            pr.plugin_name,
                            pr.degraded_stages.join(", ")
                        ));
                    }
                }
            }
            // PSP-5 Phase 7: Populate review state from verification and bundle events
            AgentEvent::VerificationComplete {
                node_id,
                syntax_ok,
                build_ok,
                tests_ok,
                lint_ok,
                diagnostics_count,
                tests_passed,
                tests_failed,
                energy,
                energy_components,
                stage_outcomes,
                degraded,
                degraded_reasons,
                summary,
                node_class,
            } => {
                self.review_state.node_id = Some(node_id.clone());
                self.review_state.node_class = Some(node_class);
                self.review_state.syntax_ok = Some(syntax_ok);
                self.review_state.build_ok = Some(build_ok);
                self.review_state.tests_ok = Some(tests_ok);
                self.review_state.lint_ok = Some(lint_ok);
                self.review_state.diagnostics_count = Some(diagnostics_count);
                self.review_state.tests_passed = Some(tests_passed);
                self.review_state.tests_failed = Some(tests_failed);
                self.review_state.energy = Some(energy);
                self.review_state.energy_components = Some(energy_components.clone());
                self.review_state.stage_outcomes = stage_outcomes;
                self.review_state.degraded = degraded;
                self.review_state.degraded_reasons = degraded_reasons;
                self.review_state.summary = Some(summary.clone());

                self.dashboard.update_energy(energy);
                self.dashboard.energy_components = Some(energy_components);
                self.dashboard.verifier_stage = Some(if degraded {
                    "Degraded".to_string()
                } else {
                    "Complete".to_string()
                });
                self.dashboard
                    .log(format!("🔍 Verified: {} — {}", node_id, summary));
            }
            AgentEvent::BundleApplied {
                node_id,
                files_created,
                files_modified,
                writes_count,
                diffs_count,
                node_class,
            } => {
                self.review_state.node_id = Some(node_id.clone());
                self.review_state.node_class = Some(node_class);
                self.review_state.files_created = files_created.clone();
                self.review_state.files_modified = files_modified.clone();
                self.review_state.writes_count = writes_count;
                self.review_state.diffs_count = diffs_count;

                self.dashboard.log(format!(
                    "📦 Bundle: {} ({} writes, {} diffs)",
                    node_id, writes_count, diffs_count
                ));
            }
            _ => {}
        }
    }

    fn handle_review_decision(&mut self, decision: ReviewDecision) {
        let request_id = self.pending_request_id.take();
        // PSP-5 Phase 7: Reset review state after decision
        self.review_state = NodeReviewState::default();

        match decision {
            ReviewDecision::Approve => {
                self.dashboard.log("✓ Changes approved".to_string());
                if let (Some(sender), Some(rid)) = (&self.action_sender, request_id) {
                    let _ = sender.send(perspt_core::AgentAction::Approve { request_id: rid });
                }
            }
            ReviewDecision::Reject => {
                self.dashboard.log("✗ Changes rejected".to_string());
                if let (Some(sender), Some(rid)) = (&self.action_sender, request_id) {
                    let _ = sender.send(perspt_core::AgentAction::Reject {
                        request_id: rid,
                        reason: Some("User rejected in TUI".to_string()),
                    });
                }
            }
            ReviewDecision::Edit => {
                self.dashboard.log("📝 Opening in editor...".to_string());
            }
            ReviewDecision::ViewDiff => {
                self.active_tab = ActiveTab::Diff;
            }
            ReviewDecision::RequestCorrection => {
                self.dashboard.log("🔄 Correction requested".to_string());
                if let (Some(sender), Some(rid)) = (&self.action_sender, request_id) {
                    let _ = sender.send(perspt_core::AgentAction::RequestCorrection {
                        request_id: rid,
                        feedback: "User requested correction via TUI review".to_string(),
                    });
                }
            }
            ReviewDecision::Skip => {
                self.dashboard.log("⏭ Skipped review".to_string());
            }
        }
    }

    fn handle_agent_update(&mut self, update: AgentStateUpdate) {
        match update {
            AgentStateUpdate::Energy { node_id, energy } => {
                self.dashboard.update_energy(energy);
                self.dashboard.current_node = Some(node_id.clone());
                self.task_tree.update_energy(&node_id, energy);
            }
            AgentStateUpdate::Status { node_id, status } => {
                self.task_tree.update_status(&node_id, status);
            }
            AgentStateUpdate::Log(msg) => {
                self.dashboard.log(msg);
            }
            AgentStateUpdate::NodeCompleted(node_id) => {
                self.dashboard.log(format!("Node {} completed", node_id));
            }
            AgentStateUpdate::Complete => {
                self.dashboard.log("Orchestration complete".to_string());
                self.dashboard.status = "Complete".to_string();
            }
        }
    }

    fn next_tab(&mut self) {
        self.active_tab = match self.active_tab {
            ActiveTab::Dashboard => ActiveTab::Tasks,
            ActiveTab::Tasks => ActiveTab::Diff,
            ActiveTab::Diff => ActiveTab::Dashboard,
        };
    }

    fn prev_tab(&mut self) {
        self.active_tab = match self.active_tab {
            ActiveTab::Dashboard => ActiveTab::Diff,
            ActiveTab::Tasks => ActiveTab::Dashboard,
            ActiveTab::Diff => ActiveTab::Tasks,
        };
    }

    fn handle_up(&mut self) {
        match self.active_tab {
            ActiveTab::Tasks => self.task_tree.previous(),
            ActiveTab::Diff => self.diff_viewer.scroll_up(),
            _ => {}
        }
    }

    fn handle_down(&mut self) {
        match self.active_tab {
            ActiveTab::Tasks => self.task_tree.next(),
            ActiveTab::Diff => self.diff_viewer.scroll_down(),
            _ => {}
        }
    }

    fn handle_page_up(&mut self) {
        if self.active_tab == ActiveTab::Diff {
            self.diff_viewer.page_up(20);
        }
    }

    fn handle_page_down(&mut self) {
        if self.active_tab == ActiveTab::Diff {
            self.diff_viewer.page_down(20);
        }
    }

    fn handle_select(&mut self) {
        if self.active_tab == ActiveTab::Tasks {
            if let Some(node) = self.task_tree.selected_task() {
                self.dashboard.log(format!("Selected: {}", node.id));
            }
        }
    }

    fn show_approval_modal(&mut self) {
        // Placeholder for manual approval trigger if needed
        self.dashboard
            .log("Manual approval modal Not Implemented".to_string());
    }

    pub fn handle_terminal_event(&mut self, event: crossterm::event::Event) -> bool {
        // Legacy bridge for run_agent_tui_with_orchestrator
        if let crossterm::event::Event::Key(key) = event {
            if key.code == KeyCode::Char('q') {
                return false;
            }
        }
        true
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(frame.area());

        // Header with Tabs
        let titles = vec!["[1] Dashboard", "[2] Task Tree", "[3] Diff Viewer"];
        let tabs = Tabs::new(titles)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" perspt Agent mode "),
            )
            .select(self.active_tab.index())
            .style(Style::default().fg(Color::Cyan))
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .bg(Color::Black)
                    .fg(Color::Yellow),
            );
        frame.render_widget(tabs, chunks[0]);

        // Main Content
        match self.active_tab {
            ActiveTab::Dashboard => self.dashboard.render(frame, chunks[1]),
            ActiveTab::Tasks => self.task_tree.render(frame, chunks[1]),
            ActiveTab::Diff => self.diff_viewer.render(frame, chunks[1]),
        }

        // Modals
        if self.review_modal.visible {
            self.review_modal.render(frame, frame.area());
        }
    }
}

/// Run the agent TUI with a real SRBNOrchestrator
pub async fn run_agent_tui_with_orchestrator(
    mut orchestrator: perspt_agent::SRBNOrchestrator,
    task: String,
) -> anyhow::Result<()> {
    use crate::app_event::AppEvent;
    use perspt_core::events::channel;

    // Create channels for bidirectional communication
    let (event_sender, mut event_receiver) = channel::event_channel();
    let (action_sender, action_receiver) = channel::action_channel();

    // Connect orchestrator to TUI
    orchestrator.connect_tui(event_sender, action_receiver);

    // Initializing terminal
    let mut terminal = ratatui::init();
    let mut app = AgentApp::new();
    app.set_action_sender(action_sender);

    // Spawn orchestrator in background task
    let orchestrator_handle = tokio::spawn(async move { orchestrator.run(task).await });

    // Main event loop
    loop {
        // Render
        terminal.draw(|frame| app.render(frame))?;

        // Handle events with timeout for responsiveness
        tokio::select! {
            // Terminal events
            _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {
                if crossterm::event::poll(std::time::Duration::from_millis(0))? {
                    if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                        if key.kind == crossterm::event::KeyEventKind::Press {
                            // Map Key Events to app state
                            if key.code == KeyCode::Char('q') {
                                app.should_quit = true;
                            }
                            // Pass keys to modal if visible
                            if app.review_modal.visible {
                                match key.code {
                                    KeyCode::Left => app.review_modal.select_left(),
                                    KeyCode::Right => app.review_modal.select_right(),
                                    KeyCode::Char(c) => {
                                        if let Some(decision) = app.review_modal.handle_key(c) {
                                            app.handle_review_decision(decision);
                                            app.review_modal.hide();
                                        }
                                    }
                                    KeyCode::Enter => {
                                        let decision = app.review_modal.get_decision();
                                        app.handle_review_decision(decision);
                                        app.review_modal.hide();
                                    }
                                    KeyCode::Esc => app.review_modal.hide(),
                                    _ => {}
                                }
                            } else {
                                match key.code {
                                    KeyCode::Tab => app.next_tab(),
                                    KeyCode::Char('1') => app.active_tab = ActiveTab::Dashboard,
                                    KeyCode::Char('2') => app.active_tab = ActiveTab::Tasks,
                                    KeyCode::Char('3') => app.active_tab = ActiveTab::Diff,
                                    KeyCode::Up | KeyCode::Char('k') => app.handle_up(),
                                    KeyCode::Down | KeyCode::Char('j') => app.handle_down(),
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
            // Orchestrator events
            Some(event) = event_receiver.recv() => {
                app.handle_app_event(AppEvent::CoreEvent(event));
            }
        }

        if app.should_quit {
            break;
        }

        // Check if orchestrator finished
        if orchestrator_handle.is_finished() {
            // app.dashboard.log("🏁 Orchestrator finished".to_string());
        }
    }

    ratatui::restore();
    orchestrator_handle.abort();
    Ok(())
}
