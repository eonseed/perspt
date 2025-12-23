//! Task Tree Component
//!
//! Displays the SRBN Task DAG as a tree view.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

/// Node status for display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Escalated,
}

impl TaskStatus {
    pub fn icon(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "○",
            TaskStatus::Running => "◐",
            TaskStatus::Completed => "●",
            TaskStatus::Failed => "✗",
            TaskStatus::Escalated => "⚠",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            TaskStatus::Pending => Color::DarkGray,
            TaskStatus::Running => Color::Yellow,
            TaskStatus::Completed => Color::Green,
            TaskStatus::Failed => Color::Red,
            TaskStatus::Escalated => Color::Magenta,
        }
    }
}

/// A task node for the tree view
#[derive(Debug, Clone)]
pub struct TaskNode {
    pub id: String,
    pub goal: String,
    pub status: TaskStatus,
    pub depth: usize,
}

/// Task tree viewer state
#[derive(Default)]
pub struct TaskTree {
    /// Flattened list of tasks for display
    pub tasks: Vec<TaskNode>,
    /// Selection state
    pub state: ListState,
}

impl TaskTree {
    /// Create a new task tree
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a task to the tree
    pub fn add_task(&mut self, id: String, goal: String, depth: usize) {
        self.tasks.push(TaskNode {
            id,
            goal,
            status: TaskStatus::Pending,
            depth,
        });
    }

    /// Update task status
    pub fn update_status(&mut self, id: &str, status: TaskStatus) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.status = status;
        }
    }

    /// Select next task
    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.tasks.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    /// Select previous task
    pub fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.tasks.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    /// Render the task tree
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .tasks
            .iter()
            .map(|task| {
                let indent = "  ".repeat(task.depth);
                let icon = task.status.icon();
                let color = task.status.color();
                let goal = truncate(&task.goal, 40);

                // Use format! to create owned strings
                let display = format!("{}{} {}: {}", indent, icon, task.id, goal);

                ListItem::new(Span::styled(display, Style::default().fg(color)))
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().title("🌳 Task DAG").borders(Borders::ALL))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list, area, &mut self.state);
    }
}

/// Truncate a string to max length with ellipsis
fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..max.saturating_sub(3)])
    } else {
        s.to_string()
    }
}
