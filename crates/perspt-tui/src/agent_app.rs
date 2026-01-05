//! Agent App - Main TUI Application
//!
//! Coordinates all TUI components for the Agent mode with full keyboard navigation.
//! Now with async event-driven architecture support.

use crate::app_event::{AgentStateUpdate, AppEvent};
use crate::dashboard::Dashboard;
use crate::diff_viewer::DiffViewer;
use crate::review_modal::{ReviewDecision, ReviewModal, StabilityMetrics};
use crate::task_tree::{TaskStatus, TaskTree};
use crate::telemetry::EnergyComponents;
use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyEventKind};
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
    /// Current tab
    pub active_tab: ActiveTab,
    /// Dashboard component
    pub dashboard: Dashboard,
    /// Task tree component
    pub task_tree: TaskTree,
    /// Diff viewer component
    pub diff_viewer: DiffViewer,
    /// Review modal
    pub review_modal: ReviewModal,
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
                    // Toggle diff view mode
                    KeyCode::Char('v') if self.active_tab == ActiveTab::Diff => {
                        self.diff_viewer.toggle_view_mode();
                    }
                    // Navigation within task tree
                    KeyCode::Left | KeyCode::Char('h') => self.handle_left(),
                    KeyCode::Right | KeyCode::Char('l') => self.handle_right(),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Handle an AppEvent from the async event loop
    ///
    /// Returns `true` to continue running, `false` to quit.
    pub fn handle_app_event(&mut self, event: AppEvent) -> bool {
        match event {
            AppEvent::Terminal(crossterm_event) => self.handle_terminal_event(crossterm_event),
            AppEvent::AgentUpdate(update) => {
                self.handle_agent_update(update);
                true
            }
            AppEvent::Tick => {
                // Update animations if needed
                true
            }
            AppEvent::Quit => false,
            AppEvent::Error(e) => {
                self.dashboard.log(format!("Error: {}", e));
                true
            }
            // Stream events not used in agent mode
            AppEvent::StreamChunk(_) | AppEvent::StreamComplete => true,
            // Handle core events from new event system
            AppEvent::CoreEvent(core_event) => {
                self.handle_core_event(core_event);
                true
            }
        }
    }

    /// Handle a terminal event
    fn handle_terminal_event(&mut self, event: CrosstermEvent) -> bool {
        match event {
            CrosstermEvent::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    return true;
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
                    return true;
                }

                match key.code {
                    // Quit
                    KeyCode::Char('q') => return false,
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
                    // Toggle diff view mode
                    KeyCode::Char('v') if self.active_tab == ActiveTab::Diff => {
                        self.diff_viewer.toggle_view_mode();
                    }
                    // Navigation within task tree
                    KeyCode::Left | KeyCode::Char('h') => self.handle_left(),
                    KeyCode::Right | KeyCode::Char('l') => self.handle_right(),
                    _ => {}
                }
            }
            CrosstermEvent::Resize(_, _) => {
                // Terminal resize - render will handle it
            }
            _ => {}
        }
        true
    }

    /// Handle agent state updates
    fn handle_agent_update(&mut self, update: AgentStateUpdate) {
        match update {
            AgentStateUpdate::TaskStatusChanged { task_id, status } => {
                self.task_tree.update_status(&task_id, status);
            }
            AgentStateUpdate::EnergyUpdated(energy) => {
                self.dashboard.update_energy(energy);
            }
            AgentStateUpdate::Log(msg) => {
                self.dashboard.log(msg);
            }
            AgentStateUpdate::NodeCompleted(node_id) => {
                self.dashboard.completed_nodes += 1;
                self.dashboard.log(format!("✓ Node {} completed", node_id));
            }
            AgentStateUpdate::Complete => {
                self.dashboard.log("🎉 Orchestration complete!".to_string());
            }
        }
    }

    /// Handle core events from perspt_core::AgentEvent
    fn handle_core_event(&mut self, event: perspt_core::AgentEvent) {
        use perspt_core::AgentEvent;
        match event {
            AgentEvent::PlanGenerated(plan) => {
                let count = plan.len();
                self.task_tree.populate_from_plan(plan);
                self.dashboard.total_nodes = count;
                self.dashboard
                    .log(format!("📋 Loaded plan with {} tasks", count));
            }
            AgentEvent::TaskStatusChanged { node_id, status } => {
                // Convert NodeStatus to TaskStatus
                let task_status = match status {
                    perspt_core::NodeStatus::Pending => TaskStatus::Pending,
                    perspt_core::NodeStatus::Running => TaskStatus::Running,
                    perspt_core::NodeStatus::Verifying => TaskStatus::Verifying,
                    perspt_core::NodeStatus::Completed => TaskStatus::Completed,
                    perspt_core::NodeStatus::Failed => TaskStatus::Failed,
                    perspt_core::NodeStatus::Escalated => TaskStatus::Escalated,
                };
                self.task_tree.update_status(&node_id, task_status);
            }
            AgentEvent::EnergyUpdated { energy, .. } => {
                self.dashboard.update_energy(energy);
            }
            AgentEvent::Log(msg) => {
                self.dashboard.log(msg);
            }
            AgentEvent::NodeCompleted { node_id, goal } => {
                self.dashboard.completed_nodes += 1;
                self.dashboard.log(format!("✓ {} - {}", node_id, goal));
            }
            AgentEvent::ApprovalRequest {
                description, diff, ..
            } => {
                // Show approval modal with stability metrics
                let metrics = StabilityMetrics {
                    energy: EnergyComponents::default(),
                    is_stable: false,
                    threshold: 0.1,
                    attempts: 1,
                    max_attempts: 3,
                };
                self.review_modal.show_with_stability(
                    "Approval Required".to_string(),
                    description,
                    diff.map(|d| vec![d]).unwrap_or_default(),
                    metrics,
                );
            }
            AgentEvent::Complete { success, message } => {
                let emoji = if success { "🎉" } else { "❌" };
                self.dashboard.log(format!("{} {}", emoji, message));
            }
            AgentEvent::Error(e) => {
                self.dashboard.log(format!("⚠️ Error: {}", e));
            }
        }
    }

    fn handle_review_decision(&mut self, decision: ReviewDecision) {
        match decision {
            ReviewDecision::Approve => {
                self.dashboard.log("✓ Changes approved".to_string());
            }
            ReviewDecision::Reject => {
                self.dashboard.log("✗ Changes rejected".to_string());
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

    fn show_approval_modal(&mut self) {
        // Get current task info if available
        let (title, description, files) = if let Some(task) = self.task_tree.selected_task() {
            (
                format!("Approve: {}", task.goal),
                format!(
                    "Task '{}' has produced changes. Review and approve?",
                    task.id
                ),
                vec!["src/main.rs".to_string(), "src/lib.rs".to_string()],
            )
        } else {
            (
                "Approve Changes?".to_string(),
                "Review the changes below and approve to continue.".to_string(),
                vec!["file1.rs".to_string(), "file2.rs".to_string()],
            )
        };

        // Create stability metrics from dashboard
        let stability = StabilityMetrics {
            energy: EnergyComponents {
                v_syn: 0.02,
                v_str: 0.01,
                v_log: 0.03,
                total: self.dashboard.energy,
            },
            is_stable: self.dashboard.stable,
            threshold: 0.1,
            attempts: 1,
            max_attempts: 3,
        };

        self.review_modal
            .show_with_stability(title, description, files, stability);
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
            self.diff_viewer.page_up(10);
        }
    }

    fn handle_page_down(&mut self) {
        if self.active_tab == ActiveTab::Diff {
            self.diff_viewer.page_down(10);
        }
    }

    fn handle_select(&mut self) {
        if self.active_tab == ActiveTab::Tasks {
            self.task_tree.toggle_collapse();
        }
    }

    fn handle_left(&mut self) {
        if self.active_tab == ActiveTab::Diff {
            self.diff_viewer.prev_hunk();
        }
    }

    fn handle_right(&mut self) {
        if self.active_tab == ActiveTab::Diff {
            self.diff_viewer.next_hunk();
        }
    }

    /// Render the app
    fn render(&mut self, frame: &mut Frame) {
        let size = frame.area();

        // Main layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Tabs
                Constraint::Min(10),   // Content
            ])
            .split(size);

        // Tabs with keyboard hints
        let titles = vec!["Dashboard [1]", "Tasks [2]", "Diff [3]"];
        let pause_indicator = if self.paused { " ⏸ PAUSED" } else { "" };
        let tabs = Tabs::new(titles)
            .block(
                Block::default()
                    .title(format!("🚀 SRBN Agent{}", pause_indicator))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Rgb(96, 125, 139))),
            )
            .select(self.active_tab.index())
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .fg(Color::Rgb(129, 212, 250))
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(tabs, chunks[0]);

        // Content based on active tab
        match self.active_tab {
            ActiveTab::Dashboard => self.dashboard.render(frame, chunks[1]),
            ActiveTab::Tasks => self.task_tree.render(frame, chunks[1]),
            ActiveTab::Diff => self.diff_viewer.render(frame, chunks[1]),
        }

        // Modal overlay
        self.review_modal.render(frame, size);
    }
}

/// Run the agent TUI with demo data (Legacy - Unused)
pub fn run_agent_tui() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let mut app = AgentApp::new();

    app.dashboard
        .log("⚠️ Legacy TUI mode - Please use 'perspt agent <task>'".to_string());

    // Minimal event loop
    loop {
        terminal.draw(|frame| app.render(frame))?;

        if crossterm::event::poll(std::time::Duration::from_millis(100))? {
            if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                if key.kind == crossterm::event::KeyEventKind::Press {
                    if key.code == crossterm::event::KeyCode::Char('q') {
                        break;
                    }
                }
            }
        }
    }
    ratatui::restore();
    Ok(())
}

/// Run the agent TUI with a real SRBNOrchestrator
///
/// This function connects the TUI to the orchestrator via channels,
/// allowing real-time updates and interactive control.
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

    // Spawn orchestrator in background task
    let orchestrator_handle = tokio::spawn(async move { orchestrator.run(task).await });

    // Initialize terminal
    let mut terminal = ratatui::init();
    let mut app = AgentApp::new();

    // Store action sender for TUI to use
    let _action_sender = action_sender;

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
                            if !app.handle_terminal_event(crossterm::event::Event::Key(key)) {
                                break;
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
            app.dashboard.log("🏁 Orchestrator finished".to_string());
            // Allow user to review results before exit
        }
    }

    ratatui::restore();

    // Wait for orchestrator to complete or abort
    orchestrator_handle.abort();

    Ok(())
}
