//! Agent App - Main TUI Application
//!
//! Coordinates all TUI components for the Agent mode.

use crate::dashboard::Dashboard;
use crate::diff_viewer::DiffViewer;
use crate::review_modal::ReviewModal;
use crate::task_tree::{TaskStatus, TaskTree};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
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
                        KeyCode::Enter => {
                            let _decision = self.review_modal.get_decision();
                            self.review_modal.hide();
                            // TODO: Apply decision
                        }
                        KeyCode::Esc => self.review_modal.hide(),
                        _ => {}
                    }
                    return Ok(());
                }

                match key.code {
                    KeyCode::Char('q') => self.should_quit = true,
                    KeyCode::Char('p') => self.paused = true,
                    KeyCode::Char('r') => self.paused = false,
                    KeyCode::Tab => self.next_tab(),
                    KeyCode::BackTab => self.prev_tab(),
                    // Tab-specific keys
                    KeyCode::Up | KeyCode::Char('k') => self.handle_up(),
                    KeyCode::Down | KeyCode::Char('j') => self.handle_down(),
                    KeyCode::Char('a') => {
                        // Approve current - show modal
                        self.review_modal.show(
                            "Approve Changes?".to_string(),
                            "Review the changes below and approve to continue.".to_string(),
                            vec!["file1.rs".to_string(), "file2.rs".to_string()],
                        );
                    }
                    _ => {}
                }
            }
        }
        Ok(())
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

        // Tabs
        let titles = vec!["Dashboard [1]", "Tasks [2]", "Diff [3]"];
        let tabs = Tabs::new(titles)
            .block(Block::default().title("SRBN Agent").borders(Borders::ALL))
            .select(self.active_tab.index())
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Cyan));
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

/// Run the agent TUI
pub fn run_agent_tui() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let mut app = AgentApp::new();

    // Demo data
    app.task_tree.add_task(
        "root".to_string(),
        "Implement authentication".to_string(),
        0,
    );
    app.task_tree
        .add_task("auth-1".to_string(), "Create JWT module".to_string(), 1);
    app.task_tree
        .add_task("auth-2".to_string(), "Add password hashing".to_string(), 1);
    app.task_tree.update_status("root", TaskStatus::Running);
    app.task_tree.update_status("auth-1", TaskStatus::Completed);

    app.dashboard.total_nodes = 3;
    app.dashboard.completed_nodes = 1;
    app.dashboard.current_node = Some("auth-2".to_string());
    app.dashboard.update_energy(0.5);
    app.dashboard
        .log("Started task: Implement authentication".to_string());
    app.dashboard.log("OK: JWT module completed".to_string());

    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}
