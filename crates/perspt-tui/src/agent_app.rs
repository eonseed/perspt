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
                diff: _,
            } => {
                self.pending_request_id = Some(request_id);
                // Map ActionType to a set of files or something similar for the modal
                let files = match action_type {
                    perspt_core::ActionType::FileWrite { path } => vec![path],
                    _ => vec![],
                };
                self.review_modal
                    .show(format!("Approval: {}", node_id), description, files);
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
            _ => {}
        }
    }

    fn handle_review_decision(&mut self, decision: ReviewDecision) {
        let request_id = self.pending_request_id.take();

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
