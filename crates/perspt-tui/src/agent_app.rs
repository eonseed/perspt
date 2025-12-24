//! Agent App - Main TUI Application
//!
//! Coordinates all TUI components for the Agent mode with full keyboard navigation.

use crate::dashboard::Dashboard;
use crate::diff_viewer::DiffViewer;
use crate::review_modal::{ReviewDecision, ReviewModal, StabilityMetrics};
use crate::task_tree::{TaskStatus, TaskTree};
use crate::telemetry::EnergyComponents;
use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
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
        match self.active_tab {
            ActiveTab::Diff => self.diff_viewer.page_up(10),
            _ => {}
        }
    }

    fn handle_page_down(&mut self) {
        match self.active_tab {
            ActiveTab::Diff => self.diff_viewer.page_down(10),
            _ => {}
        }
    }

    fn handle_select(&mut self) {
        match self.active_tab {
            ActiveTab::Tasks => self.task_tree.toggle_collapse(),
            _ => {}
        }
    }

    fn handle_left(&mut self) {
        match self.active_tab {
            ActiveTab::Diff => self.diff_viewer.prev_hunk(),
            _ => {}
        }
    }

    fn handle_right(&mut self) {
        match self.active_tab {
            ActiveTab::Diff => self.diff_viewer.next_hunk(),
            _ => {}
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

/// Run the agent TUI with demo data
pub fn run_agent_tui() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let mut app = AgentApp::new();

    // Demo task tree with parent relationships
    app.task_tree.add_task_with_parent(
        "root".to_string(),
        "Implement authentication system".to_string(),
        None,
        0,
    );
    app.task_tree.add_task_with_parent(
        "auth-1".to_string(),
        "Create JWT module".to_string(),
        Some("root".to_string()),
        1,
    );
    app.task_tree.add_task_with_parent(
        "auth-2".to_string(),
        "Add password hashing with bcrypt".to_string(),
        Some("root".to_string()),
        1,
    );
    app.task_tree.add_task_with_parent(
        "auth-3".to_string(),
        "Implement session management".to_string(),
        Some("root".to_string()),
        1,
    );
    app.task_tree.add_task_with_parent(
        "jwt-1".to_string(),
        "Define token structure".to_string(),
        Some("auth-1".to_string()),
        2,
    );
    app.task_tree.add_task_with_parent(
        "jwt-2".to_string(),
        "Add token validation".to_string(),
        Some("auth-1".to_string()),
        2,
    );

    // Update statuses and energy
    app.task_tree.update_status("root", TaskStatus::Running);
    app.task_tree.update_status("auth-1", TaskStatus::Completed);
    app.task_tree.update_status("jwt-1", TaskStatus::Completed);
    app.task_tree.update_status("jwt-2", TaskStatus::Completed);
    app.task_tree.update_status("auth-2", TaskStatus::Running);
    app.task_tree.update_energy("auth-2", 0.35);

    // Dashboard data
    app.dashboard.total_nodes = 6;
    app.dashboard.completed_nodes = 3;
    app.dashboard.current_node = Some("auth-2".to_string());
    app.dashboard.update_energy(0.35);
    app.dashboard
        .log("Started task: Implement authentication".to_string());
    app.dashboard.log("OK: JWT module completed".to_string());
    app.dashboard.log("OK: Token structure defined".to_string());
    app.dashboard.log("OK: Token validation added".to_string());
    app.dashboard
        .log("Running: Password hashing...".to_string());

    // Demo diff
    app.diff_viewer.compute_diff(
        "src/auth.rs",
        "use std::collections::HashMap;\n\nfn main() {\n    println!(\"Hello\");\n}\n",
        "use std::collections::HashMap;\nuse bcrypt::{hash, verify};\n\nfn main() {\n    let password = \"secret\";\n    let hashed = hash(password, 10).unwrap();\n    println!(\"Hash: {}\", hashed);\n}\n",
    );

    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}
